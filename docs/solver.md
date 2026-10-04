# Combat solver (`rl/`)

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
| `rl/search.py` | determinized play-out search on top of a network (below) |
| `rl/exit.py` | expert iteration: collect search targets, distill them into the network |

Observation features are signed-log scaled (`S(x) = sign(x) log(1+|x|)`): HP, damage, counters and power amounts are unbounded in this game
(a first run without it showed value-loss spikes up to 1600 and a frozen policy).

## Search (`rl/search.py`)
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

Search on top (same fights, paired seeds; `rl/bench_search.py`):

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

## The solver API (`rl/solver.py`)
`Solver().solve(scenarios, attempts=32)` plays `attempts` fights of every scenario (deck variants ...) with the network + search, all together in large
batches, and returns win rate (+ standard error), mean HP lost (losses charged in full), HP left on wins. 576 fights (9 deck variants x 64) took 65 s.
`rl/whatif.py` (card removal, split timing), `rl/potion_whatif.py`, `rl/trace.py` (play-by-play page of the best / worst line) build on the same pieces.

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
