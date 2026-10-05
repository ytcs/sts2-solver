# Combat solver (`rl/`)

> Note: the harness restructure removed development scripts (fuzzers, sweeps, audits, coverage, generators of training sets, `rl/trace.py`, `rl/winnable.py`, `rl/baselines.py`, `rl/sweep.py`, `rl/bench_net.py`) and the training / analysis data. Scripts named below that are gone are in git history: `git show 22730bd:<path>`.


Goal: given a deck, relics, potions, HP and an encounter (Ascension 10), play the fight to **win first, then lose as little HP as
possible**. The run-level optimizer that will call this as a subroutine is out of scope; the primary metrics are therefore

* `win` — fraction of fights won,
* `hp_lost_all` — mean fraction of max HP lost over *all* fights; a loss (or a stall past `max_steps`) is charged all the HP the player
  started with. (`hp_lost_on_win` is reported too but hides losses.)

Training reward: +1 for a win (+ `0.5 x HP fraction left`), -1 for a loss or a stall, 0 for aborted episodes (unported content /
capacity overflow; never seen in the training distribution so far).

## Pieces
| file | what |
|---|---|
| `tools/gen_train.py` | realistic A10 scenario sets (starter deck + act-scaled picks, relics, potions, every encounter); different `--seed` = disjoint set |
| `rl/model.py` | policy/value network: entity encoders (player, enemies with intents + expert look-ahead, hand cards, potions, pending-selection candidates, pile multisets as card-embedding bags), pooled context, 2 message-passing rounds, **pointer action head** scoring exactly the env's dense action space; clicking an already selected card again is masked out |
| `rl/ppo.py` | PPO (CPU), resumable checkpoints, held-out evaluation every N iterations |
| `rl/baselines.py` | random, untrained-greedy, scripted heuristic; also evaluates any checkpoint (`ckpt:PATH`) |
| `rl/fastsearch.py`, `crates/sts2env/src/search.rs` | determinized play-out search on top of a network: the Rust state machine and its network driver (below) |

Observation features are signed-log scaled (`S(x) = sign(x) log(1+|x|)`): HP, damage, counters and power amounts are unbounded in this game
(a first run without it showed value-loss spikes up to 1600 and a frozen policy).

## Search
(The experiments below up to "Speed" were run with the first, python implementation of the search and with distillation / mining tools; those were removed once the Rust engine replaced them, last commit with them: 443d7e7. The method is the same.)
`VecEnv.fork_from` copies a fight and **determinizes** it (`Combat::determinize`: draw / discard / exhaust orders and all nine RNG streams are
resampled; everything the player can see is unchanged, tested). At each decision the policy's top-M legal actions are each played on K
determinized copies (the same K seeds for every action: a paired comparison); a copy then follows the policy to the end of the
current player turn and is scored by the final reward (fight over) or the value head at the start of the next turn. The root plays the
best mean; where the policy's top action has probability >= `conf` the search is skipped. This is one step of policy improvement over the
base policy and needs no extra training.

## Results
Eval set: `target/train/eval.json` (1500 fights, 5 characters, 3 acts, seed 22), greedy policy, first finished episode of 500 envs x 3.

| policy | win | hp_lost_all |
|---|---|---|
| random | 0.145 | 0.686 |
| untrained net | 0.031 | 0.725 |
| scripted heuristic | 0.363 | 0.572 |
| PPO, 2.5M steps (early) | 0.477 | 0.526 |
| PPO, 60M steps (`models/solver_base.pt`) | 0.630 | 0.443 |

Search on top (same fights, paired seeds):

| set | greedy policy | + search (5 options x 8 futures) |
|---|---|---|
| broad eval, 150 fights | 64.7% / 0.424 | 72.0% / 0.379 |
| mid-difficulty (model wins 15-85%), 200 fights | 52.0% / 0.577 | 75.0% / 0.475 |
| Phrog Parasite deck from the user, 120 attempts | 56% | 83% |

What the search work taught (all measured, see the benchmark scripts):
* Cost is the network: 85% of a search is network inference (before the policy-only / value-only split and the batched value calls; now ~60% rollout
  policy, ~18% value, ~10% simulator). int8 dynamic quantization is slower and changes decisions; the lever is doing less work (skip the network when
  one action is legal, pruning candidates below 3% probability) and **big batches**: 60 roots ~5 s/fight, 150 ~3 s, 400 ~2.3 s. Wall-clock on this laptop
  CPU varies 2-5x between identical runs (thermal throttling): compare work, not single timings.
* Forcing extra candidates (end turn, the likeliest potion action) into the search was neutral to harmful (broad set 65% vs 69%, mid set 75.5% vs
  75.0%); off by default. Scoring options with full-fight play-outs instead of end-of-turn + value head was not better on the broad set (71.3% vs 72.0%)
  and several times slower.
* The first distillation (25k search-labelled states) did not improve the network: targets from 4 noisy futures per action. `rl/mine.py` confirms
  disagreements with 48 futures before keeping them.
* What-if examples (Phrog Parasite deck): the network alone drinks a Duplicator potion on turn 1 whatever it holds (72.5% win); forcing "only with Perfected
  Strike in hand" gives 77.7%. With search the potion goes on Perfected Strike whenever it is in hand (85.7% win), and forcing the rule is not better.

## GPU session (Runpod RTX 4090) results
Throughput: PPO about 50k samples/s per run (laptop CPU: 3-4k), search 4 fights/s on mid-difficulty fights when the CPU is not shared (laptop: 0.3-1).
Networks (held-out eval set, 1500 fights, greedy): 64-wide, 60M steps 63.0% / 0.443 HP lost; 64-wide + 66M more steps on a 30,000-scenario set with potion-hold
randomization 63.8% / 0.436; **128-wide from scratch, 314M steps: 65.8% / 0.423** (`models/solver_b128.pt`, the best network).

Mid-difficulty set (`data/train/mid.json`, 300 paired fights, +- 2.4): greedy / search(5 options x 8 futures)
| network | greedy | search |
|---|---|---|
| 64-wide (a64) | 55.9% | 81.3% |
| 128-wide (b128) | 64.0% | 79.3% (0.459 HP lost) |
| b128 + policy distillation on 217k searched decisions (soft targets) | 65.7% | 79.0% |
| b128 + value-only fine-tune on the search-played results | 64.0% | 73.7% |
| b128 + 6,072 confirmed disagreements (round 1, 64-wide) | 60.8% vs 60.1% base (3144 episodes) | - |

**Distilling the search into the network did not work**, in three variants: single-state confirmed corrections (tiny gain, within noise), soft targets over every
searched option (held-out agreement with the search's best option fell 61% -> 50%: the option values come from 8 futures and most differences are noise), and a
value-only fit to the final results of search-played fights (search got 5.6 points *worse*: the search compares candidate states that search-play never visits).
What does pay off is bigger networks and more PPO steps, and the search on top.

### Search budget and evaluator (600 paired mid-set fights, +- 1.6)
| search setting (128-wide network) | win | HP lost | s / fight |
|---|---|---|---|
| greedy network alone | 65.3% | 0.546 | - |
| 5 options x 8 futures | 80.8% | 0.453 | 0.67 |
| 5 x 24 | 81.0% | 0.446 | 1.16 |
| 8 x 16 | 81.0% | 0.452 | 1.23 |
| 5 x 8 scored by full-fight play-outs (no value head) | 75.3% | 0.487 | 2.79 |
| **3 x 8** (the default) | 80.2-81.3% | 0.460 | 0.54 |
| 3 x 4 / 2 x 8 | 78.2% / 77.0% | 0.462 / 0.476 | 0.46 / 0.47 |

Search saturates around 81%: more options or futures do not help, and scoring with full-fight play-outs is worse (the value head is a better evaluator than
noisy play-outs). **Averaging the value heads of several networks does help**:
| evaluator at 3 x 8 | win | HP lost | s / fight |
|---|---|---|---|
| b128 alone | 80.2% | 0.456 | 0.54 |
| **b128 policy, value = mean(b128, c128, d128)** (default) | **82.8%** | 0.444 | 0.56 |
| b128 policy, value = mean(b128, a64) | 83.7% | 0.446 | 0.55 |
| ensemble of 3 (policy and value averaged) | 81.5% | 0.442 | 1.22 |
| ensemble of 4 | 82.0% | 0.444 | 1.57 |
(b128, c128, d128: three 128-wide networks, different seeds, 314M PPO steps each, 65-66% greedy on the eval set.)

### Final solver (`rl/solver.py` defaults) on the full held-out eval set (1,500 fights x 2 attempts)
**72.6% win (+- 0.8), 0.360 of max HP lost** at 4.4 fights/s (16-vCPU pod). The same network played greedily: 65.5% / 0.424; the scripted heuristic: 36.3% / 0.572;
random: 14.5% / 0.686. By character (win, HP lost): Ironclad 0.757 / 0.318, Regent 0.746 / 0.356, Defect 0.735 / 0.360, Silent 0.697 / 0.378, Necrobinder
0.692 / 0.392; by act: 0.886 / 0.197, 0.745 / 0.374, 0.569 / 0.488 (a large part of act 2-3's losses are fights that no policy wins, see the winnable analysis).

### Throughput (Runpod RTX 4090 pod, 16 vCPUs; `data/analysis/gpu_session/`)
* PPO about 50k samples/s per run (laptop CPU 3-4k); search 4.4 fights/s on the eval mix with 4 worker processes, 2.9 on 100 mid-difficulty decks
  (long fights); one worker process: 1.3-2.9 fights/s depending on the batch (large batches matter). `Solver(procs=4)` is a 2.2x gain: the Python around
  the search is single-threaded. 100 decks x 32 attempts = 3,200 fights gave +-5.3 points per deck: +-2.5 needs about 150 attempts per deck.
* Cost of this whole session on Runpod: about $8.5 of the $50 budget (secure-cloud 4090 at $0.74/h for 11.3 h plus about $0.12 of failed community-cloud attempts).

## Speed: the Rust search engine (about 45x, same strength)
The first, python search did 4.4 fights/s on the pod. The default `Solver` now runs **190+ fights/s** on the same pod (RTX 4090, 16 vCPUs) at the same
or better strength. Where the time went and what changed (counters: `rl/bench_fast.py`; 300 fights of the eval set unless noted):

| | before (python search) | after |
|---|---|---|
| policy rows through the network per fight | 13,950 | 1,550 |
| simulator steps per fight, live / stepped | 15,250 / 432,000 | 2,640 / 2,640 |
| network calls per 300 fights | 18,000 of ~230 rows | 553 of ~1,200 rows (steady state: thousands) |

1. **A policy loop in card-selection screens (about 9x fewer rows).** Picking a card when the selection is already full replaces the latest pick, so a *sampled*
   policy can wander between picks for dozens of steps instead of confirming. 7% of the play-outs hit the 60-step cap that way (they scored no value at all)
   and on some fights they were 90% of the network rows. A full selection with `Confirm` legal is now confirmed automatically (the swap could have been chosen
   as the last pick directly). This also made the estimates better: the same fights went from 67.7% to 72.7% win on a 300-fight check.
2. **A Rust state machine instead of lock-step python** (`crates/sts2env/src/search.rs`, `rl/fastsearch.py`). Each fight is a "root" with its own `M x K` play-out copies;
   the engine runs everything up to the next decision (forced moves, the simulator, forks, scoring, the next fight of the queue) on the rayon pool and writes one observation
   row per request. Python only evaluates the networks on whatever rows are pending, so batches stay large regardless of how long each fight is (lock-step batches thinned out
   to a few fights in the tail), finished or idle slots cost nothing (96% of the old simulator work was frozen slots re-writing their observations), and rows from many fights at different
   phases share one batch. Two engines alternate so the CPU simulates one while the GPU evaluates the other. Results are reproducible per job and independent of the pool size and
   thread count (`crates/sts2env/tests/search.rs`).
3. **A big pool.** At 512 roots the network was launch-bound (about 3.5 ms per call whatever the batch size: 145k rows/s at 512 rows, 1.2M rows/s at 4096); 2,048 roots in flight keeps
   the batches at thousands of rows: 28 -> 71 fights/s.
4. **CUDA graphs** (`GraphFn`): policy and value (the 3-network ensemble in one graph) per padded batch size, rows without / with a pending selection in separate graphs
   (static shapes, no host syncs): 71 -> 129 fights/s. **bf16 autocast inside the graphs**: 129 -> 191-196 fights/s, no change in strength.

Measured on the pod (6,000-12,000 fights of the eval set, `rl/bench_fast.py`, `rl/sweep.py`):
| configuration | fights/s | win | HP lost |
|---|---|---|---|
| python search, 4 processes (before) | 4.4 | 72.6% | 0.360 |
| Rust engine, 512 roots, eager fp32 (1,500 fights) | 27.8 | 73.9% | - |
| + 2,048 roots | 71 | 74.3% | - |
| + CUDA graphs | 129 | 73.9% | - |
| + bf16 (**default**; 12,000 fights, +-0.4) | **191-196** | **74.0%** | 0.359 |
| full eval protocol (1,500 x 2, fp32 graphs, `Solver.solve`) | 126 | 74.2% | 0.358 |

Search-setting sweep at the new speed (1,500 x 2 fights, +-0.8; bf16): K=4 futures 173 f/s 73.7% / 0.364; 2 value networks instead of 3 with K=6 (8,000-fight run: 278 f/s, 73.5% / 0.364);
M=2 options 184 f/s, 71.8% (worse); skipping the search where the policy is >= 95% sure 206 f/s, 71.9% (worse); a 64-wide play-out network 146 f/s vs 124 (fp32) and 73.6%: not worth it.
`Solver(..., conf=, roll_ckpt=, K=, value_ckpts=)` exposes them. 100 decks x 64 attempts (6,400 fights of the mid-difficulty set) take 43 s (150 fights/s; the python search needed about 40 minutes).
Not done: more overlap between the CPU engine (37% of the wall time) and the GPU wait (58%), adaptive attempts per deck, an even cheaper value ensemble (distillation).

### Second round: 4.4 -> 540 fights/s (K=4: 780-880), the 1000 fights/s question
Measured on a second pod (RTX 4090, **10.2 CPUs of quota**; `os.cpu_count()` showed 48 host cores, so thread counts must follow the cgroup quota: `fastsearch.available_cpus`).
Everything below is the default `Solver`; logs in `data/analysis/speed_session2/`.

| step | fights/s | win | HP lost |
|---|---|---|---|
| bf16 CUDA graphs (end of the first round, this pod) | 221 | 73.8% | 0.360 |
| + `torch.compile` (inductor fusion, dynamic batch) inside the graphs | 348 | 73.9% | 0.360 |
| + shared prefix per option (`lead`, default), K=8 (18,000 fights) | **536** | 73.6% (+-0.33) | 0.3627 |
| same, K=4 futures | **781-880** | 73.4% | 0.366 |
| without `lead` (K=8), same run | 379 | 73.9% | 0.359 |

What was learned:
* **The network was memory-bound, not compute-bound.** Kernel profile of one forward: matmuls 16%, embedding gathers / bags / elementwise the rest; a 64-wide network is barely faster than the
  128-wide one. Inductor fusion gives 1.6-2x per forward (policy 4096 rows: 2.24 -> 1.41 ms; value 1.92 -> 0.93 ms; 12% more with max-autotune at 2 minutes of compile). Compiled graphs are cached on disk after the first run.
* **Shared prefix (`lead`).** Of all actions only 27% touch hidden information (RNG streams, draw-pile order; `examples/hiddenprof.rs`): EndTurn 86%, a Strike 3-7%. So an option's in-turn play is simulated
  once on a scratch copy and the K futures (own determinization each) branch at the first step that touches hidden information, usually the turn-ending step. Policy rows -42%, simulator steps -30%, forks
  -75%. Costs about 0.3 points of win rate (one sampled continuation per option instead of K).
* **CPU is now the bottleneck** (engine 70% of the wall time). Per fight, cycles: simulator steps 46% (turn-ending steps alone are half of it), observation rows 41% (lookahead 37% of an observation,
  hand cards 28%, piles 13%, intents 11%), forks 5%, legal actions 4%. `examples/obsprof.rs` (feature `obs_prof`) profiles the observation per section. GPU time is about equal; the pod's CPU quota is what limits.
  With 16 CPUs the same code should clear 1000 fights/s at K=4; K=8 needs about 24.
* **Negative results (removed again):** value network on the state where the policy ends its turn instead of simulating the enemy turn (+27% speed, **-2.8 points**: the value head is worse on pre-turn-end
  states); play-outs stopping after d policy decisions with a mid-turn value (d=2: -1.4, d=3: -0.6 points, +10% speed); fewer futures for options that need hidden information at their first action
  (K=4/3/2: -0.5/-1.0/-1.6 points: those are exactly the high-variance options); a greedy shared prefix (no gain, loops); dropping candidates below 3% / 8% policy probability (-3.7 / -4.5 points: unlikely actions
  matter); more fights in flight (4096 roots) or 3 pipeline groups: no gain; separate graphs for rows with a pending selection: neutral against merging them (merged is the default).
* **A simulator bug found by the engine** (rare, ~1 in 30,000 fights): Uproar auto-playing a random Attack from the draw pile nests plays; with 6+ Uproars in the draw pile the fixed play stack overflowed and indexed
  out of bounds. It now raises the capacity flag (fight reported as aborted, `OUTCOME_OVERFLOW`); the engine also catches a panic in any fight, reports job / scenario / seed on stderr, aborts only that fight
  and continues (`panics` counter), regression test in `tests/defect_cards.rs`.
* Not done: caching the observation's lookahead per enemy state (about -10% CPU), sharing the enemy turn between futures (needs a pause point between the enemy phase and the draw in `Combat::step`),
  carrying a searched line's estimate to the next decision (about -20% futures, biased), sequential / adaptive K, distilling the 3 value heads into one (GPU -30%).

### Third round: algorithmic ideas, no extra hardware (pod with 13.6 CPUs of quota; logs in `data/analysis/speed_session3/`)
Same machine within a session, 12-18k fights per row (+-0.33 points), all with `lead`:

| configuration | fights/s | win | HP lost |
|---|---|---|---|
| `lead`, K=8 (round two default) | 390-465 | 73.7% | 0.3625 |
| + stratified determinization (`strat`) | 433 | 74.0% | 0.3615 |
| + carried lines (`carry`) = **new default** | 464-545 | 73.7-73.9% | 0.362 |
| K=6 | 584 | 73.4% | 0.365 |
| K=4 | 854-874 | 73.1-73.2% | 0.366 |
| K=3 | 896-1041 | 72.8% | 0.3685 |
| K=2 | 1153 | 72.2% | 0.372 |
| no `lead`, K=8 (+strat) | 319 | 74.3% | 0.358 |

* **Stratified determinization** (`Combat::determinize_strat`): all futures of a decision share one uniform shuffle and future i rotates the draw pile by a different fraction, so the next hands the
  futures draw are disjoint parts of one shuffle (each future is still uniform). Free: +0.2-0.3 points at every K, no extra work.
* **Carried lines** (`carry`): the option chosen at a decision remembers its shared prefix and its estimate; at the next decision the candidate equal to the line's next action is not simulated again
  (15% fewer rows, quality unchanged). Only valid because the prefix touched no hidden information.
* **Lookahead path cache** (thread-local, keyed by the monster's machine state, its powers and the enemy line-up; `tests/lookahead_cache.rs` checks cached == fresh on 40k rows): observation -6%.
  The rest of the lookahead is the per-node damage pipeline.
* **Tested, no gain, removed:** adaptive futures (3-4 first, the rest only for options within z standard errors of the best, paired): lands exactly on the line between K=4 and K=8.
* **Where the CPU goes now** (callgrind, `examples/endturnprof.rs`): the turn-ending step is dominated by enemy turns, `snapshot_into` (building the listener list for each hook dispatch, scanning
  every pile when any card listens) is about 9% inclusive; hand-card damage/block in observations about 5% of all CPU. Neither is attractive: listener order must stay bit-exact and the damage
  pipeline reads arbitrary state, so a cache could silently go stale. The quality-for-speed frontier above (K) is the remaining lever: 1000 fights/s costs about 1 point of win rate on this hardware.

## The solver API (`rl/solver.py`)
`Solver().solve(scenarios, attempts=32)` (the Rust engine; `engine="py"` selects the python search, which the analysis tools use) plays `attempts` fights of every scenario (deck variants ...) with the network + search, all together in large
batches, and returns win rate (+ standard error), mean HP lost (losses charged in full), HP left on wins. 576 fights (9 deck variants x 64) took 65 s.
`rl/whatif.py` (card removal / addition), `rl/trace.py` (play-by-play page of the best / worst line) build on the same pieces.

## Provably unwinnable fights (`sts2sim::bounds`, `sts2.provably_unwinnable`)
Many generated fights cannot be won by anyone (a starter deck against a boss). `provably_unwinnable(scenario)` returns a sentence proving it, or `None`
(= nothing proven). It relaxes the game in the player's favour and checks that even then the enemy cannot be killed before the player dies:

* every card of the deck must be **pure**: found by probing it in the simulator (five different situations, the same card played twice, a Vulnerable
  enemy) to spend only its energy, deal a fixed damage / gain a fixed block (+ Vulnerable / Weak), move cards between piles and change nothing else
  (no energy, powers, upgrades, cost changes, extra plays, growth). 28 Ironclad and 28 Silent cards qualify; Defect / Necrobinder / Regent are not covered;
* relics must only hook things that cannot matter in a fight (144 of 300), no potions, no starting powers, no enchantments;
* one enemy that cannot react to damage (no stun / sleep / flee intents, no damage-reactive start powers) and whose behaviour is bounded from below
  by the cheapest damage over every state its move machine can be in each turn (every branch counts as possible);
* per turn the player may play any subset of its deck (hand size and draw order ignored) whose cost fits the energy: a damage / block frontier; block
  does not carry over. A DP over turns (state = HP lost, value = damage dealt) shows whether the enemy's HP is ever reachable.

Soundness is checked empirically, not just argued: `tools/check_bounds.py` generates decks of pure cards against every encounter, and no policy
(random, scripted heuristic, the trained network sampled and greedy) may ever win a fight the prover flags. The first run found a real hole (Terror
Eel's Shriek stuns it when damaged, which lowers its damage: reactive enemies are now refused); since then 3 seeds x ~530 flagged fights x 20k
episodes per policy gave 0 wins. Coverage is small on the random-deck training distribution (decks nearly always contain a card, relic or potion the
prover cannot vouch for), so `gen_train.py --drop-unwinnable` filters little there; it matters for the starter-deck-like cases a run-level optimizer
would ask about, and the verdict is a cheap "do not even simulate this".
