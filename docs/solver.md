# Combat solver (`rl/`)

Input: a deck, relics, potions, HP and an encounter (Ascension 10). Output: a play that wins the fight first and loses as little HP as possible.

Metrics:
* `win`: fraction of fights won.
* `hp_lost_all`: mean fraction of max HP lost over all fights; a loss or a stall past `max_steps` is charged all the HP the player started with.

Training reward: +1 for a win (+ `0.5 x HP fraction left`), -1 for a loss or stall, 0 for aborted episodes (unported content, capacity overflow).

## Components
| file | role |
|---|---|
| `tools/gen_train.py` | realistic A10 scenario sets (starter deck + act-scaled picks, relics, potions, every encounter); a different `--seed` gives a disjoint set; `--energy-prob P` makes a share P of scenarios 4-7 energy; `--drop-unwinnable` filters fights the prover (below) rejects |
| `rl/model.py` | policy/value network: entity encoders (player, enemies with intents and expert look-ahead, hand, potions, pending-selection candidates, pile multisets as card-embedding bags), pooled context, 2 message-passing rounds, pointer action head scoring the env's dense action space. Re-clicking a selected card is masked. Features are signed-log scaled (`sign(x) log(1+|x|)`): HP, damage and power amounts are unbounded |
| `rl/ppo.py` | PPO, resumable checkpoints, held-out evaluation every N iterations |
| `rl/fastsearch.py`, `crates/sts2env/src/search.rs` | the search: a Rust state machine plus the Python driver that evaluates networks |
| `rl/solver.py` | `Solver`, the batch solver |
| `rl/bench_fast.py` | throughput / strength of the search on a scenario set (default `data/train/eval.json`) |
| `agent/engine.py` | `Engine`, the harness's solver service (one live decision, or a batch) |
| `scripts/pod_train.sh` | fresh training on a GPU pod |

### Scenario sets
`tools/gen_train.py` seeds: 1 = training, 22 = held-out eval (base mix, 3 energy), 23 = held-out high-energy eval (`--energy-prob 1.0`, 600 scenarios). `python -m agent.improve gate` and `scripts/pod_train.sh` generate them into `target/train/` on first use. `data/train/` holds committed copies: `eval.json` (the seed-22 set, 1,500 fights, 5 characters, 3 acts) and `mid.json` (786 mid-difficulty scenarios, where the greedy network wins 15-85%). Every table below uses `eval.json` unless it says "mid".

## Search
`VecEnv.fork_from` copies a fight and `Combat::determinize` resamples draw / discard / exhaust orders and all nine RNG streams; everything the player can see is unchanged (tested). At each decision the policy's top-M legal actions are each played on K determinized futures. A future follows the policy to the end of the current player turn (the enemy turn included) and is scored by the final reward (fight over) or the value head at the start of the next turn. The root plays the best mean. This is one step of policy improvement over the base policy; it needs no extra training.

Engine mechanics (all on by default):
* **State machine.** Each fight is a root with its own `M x K` copies. The engine runs everything up to the next network request on the rayon pool (forced moves, simulation, forks, scoring, starting the next queued fight) and writes one observation row per request. Python only evaluates the networks on pending rows, so batches stay large whatever the fight lengths. Two engines alternate (`groups=2`) so CPU simulation overlaps GPU evaluation. Results are reproducible per job and independent of pool size and thread count (`crates/sts2env/tests/search.rs`).
* **Selection screens.** A full selection with `Confirm` legal is confirmed automatically (a swap of the last pick could have been the last pick directly). Without it, sampled policies loop between picks and 7% of play-outs hit the 60-step cap.
* **`lead`.** Only 27% of actions touch hidden information (EndTurn 86%, a Strike 3-7%). An option's in-turn play is simulated once on a scratch copy; the K futures branch at the first step that touches hidden information. Policy rows -42%, simulator steps -30%, forks -75%, about 0.3 points of win rate.
* **`strat`** (`Combat::determinize_strat`). The futures of a decision share one uniform shuffle; future i rotates the draw pile by a different fraction, so the next hands are disjoint parts of one shuffle. +0.2-0.3 points at every K at no cost.
* **`carry`.** The chosen option keeps its shared prefix and estimate; at the next decision the candidate equal to the line's next action is not simulated again (15% fewer rows). Valid because the prefix touched no hidden information.
* **Lookahead path cache** (thread-local, keyed by monster state machine, powers and enemy line-up; `tests/lookahead_cache.rs` checks cached == fresh on 40k rows): observation cost -6%.
* **Failure handling.** A panic in any fight is caught: job / scenario / seed go to stderr, only that fight aborts, `panics` counts them. Capacity overflow reports `OUTCOME_OVERFLOW`.
* **GPU path** (`GraphFn`, `compile=True`, `amp=True` on CUDA): policy and value (the 3-network ensemble in one graph) are captured as CUDA graphs per padded batch size (static shapes, no host syncs), with `torch.compile` fusion and bf16 autocast inside the graphs (no change in strength). Compiled graphs are cached on disk after the first run (about 2 minutes). Thread counts follow the cgroup CPU quota (`fastsearch.available_cpus`), not `os.cpu_count()`.

## API
```python
from solver import Solver                    # rl/solver.py
S = Solver()                                  # defaults below
res = S.solve(scenarios, attempts=32)         # one dict per scenario: win, win_se, hp_lost, hp_lost_se, hp_left_on_win, attempts, aborted
```
* `Solver(ckpt, M=3, K=8, pmin=0.0, margin=0.0, max_steps=300, value_ckpts="default", roots=None, groups=2, conf=1.01, roll_ckpt=None, amp=None, threads=None)`. `roots` defaults to 2,048 on CUDA, 256 on CPU. `solve(..., search=False)` plays the network greedily. `conf=1.01` means no decision is skipped for confidence. Attempts of a scenario differ only in random streams. CLI: `python rl/solver.py --scenarios F.json --attempts 32 [--no-search]`.
* Defaults: policy `models/solver_b128.pt`; value = mean of b128, c128, d128 (three 128-wide networks, different seeds, 314M PPO steps each, 65-66% greedy on `eval.json`). `models/current.json`, written by `python -m agent.improve adopt`, overrides both.
* `FastSearch(net, value_nets, M, K, conf, roots, groups, threads, roll_net, use_graphs, amp, compile, lead, merge_dec, carry, strat, force_end_turn, ...)`: `run(scenarios, job_scen, job_seed)` plays many fights; `decide(scenario, sim, seed)` searches one decision of a live fight (`sim` is an aligned `sts2.Sim`).
* `Engine` (`agent/engine.py`): `decide(scenario, sim, budget)` is the harness's micro decision, M=5 options x K=32 futures per round (about 10 ms a round, `roots=1`), repeating rounds with fresh futures until the budget is spent or the expected regret of the leading action falls below 1 HP. `solve(scenarios, attempts=64)` wraps `Solver.solve` for macro evaluation.
* Precision: 100 decks x 32 attempts give +-5.3 points per deck; +-2.5 needs about 150 attempts per deck.

## Results
### Networks (greedy, `eval.json`)
| policy | win | hp_lost_all |
|---|---|---|
| random | 0.145 | 0.686 |
| untrained network | 0.031 | 0.725 |
| scripted heuristic | 0.363 | 0.572 |
| 64-wide, PPO 60M steps (`solver_base.pt`) | 0.630 | 0.443 |
| 64-wide, +66M steps on 30,000 scenarios with potion-hold randomization | 0.638 | 0.436 |
| **128-wide, 314M steps (`solver_b128.pt`)** | **0.658** | **0.423** |

PPO runs at about 50k samples/s on an RTX 4090 (laptop CPU: 3-4k).

### Search on mid (300 paired fights, +-2.4; 5 options x 8 futures)
| network | greedy | search |
|---|---|---|
| 64-wide | 55.9% | 81.3% |
| 128-wide (b128) | 64.0% | 79.3% (0.459 HP lost) |

### Search budget and evaluator (mid, 600 paired fights, +-1.6; b128; s/fight = first, Python search)
| setting | win | HP lost | s / fight |
|---|---|---|---|
| greedy network | 65.3% | 0.546 | - |
| 5 x 8 | 80.8% | 0.453 | 0.67 |
| 5 x 24 | 81.0% | 0.446 | 1.16 |
| 8 x 16 | 81.0% | 0.452 | 1.23 |
| 5 x 8, full-fight play-outs, no value head | 75.3% | 0.487 | 2.79 |
| 3 x 8 | 80.2-81.3% | 0.460 | 0.54 |
| 3 x 4 / 2 x 8 | 78.2% / 77.0% | 0.462 / 0.476 | 0.46 / 0.47 |

Search saturates near 81% on this set. Averaging value heads helps (3 x 8):
| evaluator | win | HP lost | s / fight |
|---|---|---|---|
| b128 alone | 80.2% | 0.456 | 0.54 |
| **b128 policy, value = mean(b128, c128, d128) (default)** | **82.8%** | 0.444 | 0.56 |
| b128 policy, value = mean(b128, a64) | 83.7% | 0.446 | 0.55 |
| ensemble of 3 (policy and value averaged) | 81.5% | 0.442 | 1.22 |
| ensemble of 4 | 82.0% | 0.444 | 1.57 |

### Default solver on `eval.json`
**Since 2026-10-06: `solver_h128` (fight-outcome head, `docs/rl_redesign.md` M1a), no extra value nets, search depth 2: 74.4-75.0% win, 0.348 of max HP lost** (`tools/gate_m1.py`, 1,500 fights x 4 attempts, 3 options x 8 futures; the same run gives b128 + c/d 74.0-74.5% / 0.360). Paired vs b128 + c/d: win +0.005 +- 0.003, HP lost -0.012 +- 0.0015 (`evals/gate_m1_full2.json`).

Before (b128 + c128/d128 values, depth 1): **73.7-73.9% win (+-0.33), 0.362 of max HP lost** (12-18k fights, pod C below). The same network greedy: 65.5% / 0.424.

By character and act, from the Python-search run (72.6% overall, 0.360; the Rust default is about 1 point higher):
| | Ironclad | Regent | Defect | Silent | Necrobinder | act 1 | act 2 | act 3 |
|---|---|---|---|---|---|---|---|---|
| win | 0.757 | 0.746 | 0.735 | 0.697 | 0.692 | 0.886 | 0.745 | 0.569 |
| HP lost | 0.318 | 0.356 | 0.360 | 0.378 | 0.392 | 0.197 | 0.374 | 0.488 |

A large part of act 2-3 losses are fights no policy wins.

## Throughput
Pods: A = RTX 4090, 16 vCPU; B = RTX 4090, 10.2-CPU quota; C = 13.6-CPU quota. Fights/s compare only within a pod. Strength is on `eval.json` (A: 6,000-12,000 fights, +-0.4; B and C: 12-18k fights, +-0.33).

| change | pod | fights/s | win | HP lost |
|---|---|---|---|---|
| Rust state machine instead of Python search (512 roots, eager fp32) | A | 4.4 -> 27.8 | 72.6% -> 73.9% | 0.360 |
| 2,048 roots (at 512 the network is launch-bound: ~3.5 ms per call) | A | 27.8 -> 71 | 73.9% -> 74.3% | - |
| CUDA graphs | A | 71 -> 129 | 74.3% -> 73.9% | - |
| bf16 autocast in the graphs | A | 129 -> 191-196 | 73.9% -> 74.0% | 0.359 |
| `torch.compile` fusion in the graphs | B | 221 -> 348 | 73.8% -> 73.9% | 0.360 |
| `lead` (K=8) | B | 379 -> 536 | 73.9% -> 73.6% | 0.359 -> 0.3627 |
| `strat` | C | 390-465 -> 433 | 73.7% -> 74.0% | 0.3625 -> 0.3615 |
| `carry` (= default) | C | 433 -> 464-545 | 74.0% -> 73.7-73.9% | 0.362 |
| K sweep of the default: K=8 / 6 / 4 / 3 / 2 | C | 464-545 / 584 / 854-874 / 896-1041 / 1153 | 73.7-73.9 / 73.4 / 73.1-73.2 / 72.8 / 72.2% | 0.362 / 0.365 / 0.366 / 0.3685 / 0.372 |
| without `lead` (K=8, with `strat`), for reference | C | 319 | 74.3% | 0.358 |

Other measured settings (pod A, 1,500 x 2 fights, +-0.8, bf16 graphs without `lead`): K=4 173 fights/s, 73.7% / 0.364; the full `Solver.solve` protocol with fp32 graphs 126 fights/s, 74.2% / 0.358; 2 value networks with K=6 278 fights/s, 73.5% / 0.364 (8,000 fights); 100 decks x 64 attempts (6,400 fights of mid) take 43 s.

Where the time goes: the network is memory-bound (pod B: matmuls 16% of a forward; a 64-wide network is barely faster than 128-wide); inductor fusion gives 1.6-2x per forward (max-autotune: +12% for 2 minutes of compile). The CPU engine is the bottleneck (pod B): simulator steps 46% of cycles (turn-ending steps half of that), observation rows 41% (lookahead 37% of an observation, hand cards 28%, piles 13%, intents 11%), forks 5%, legal actions 4%. Profiles: `crates/sts2env/examples/{hiddenprof,obsprof,endturnprof}.rs` (`obsprof` needs feature `obs_prof`). In the turn-ending step (callgrind, pod C) `snapshot_into` is about 9% inclusive; hook listener order must stay bit-exact, so it is not cached. About 1,000 fights/s costs about 1 point of win rate.

## Tried and rejected
* int8 dynamic quantization: slower, changes decisions.
* Forcing extra candidates (end turn, likeliest potion): broad set 65% vs 69%, mid 75.5% vs 75.0%.
* Full-fight play-outs instead of end-of-turn + value head: 75.3% vs 80.8% (mid), 4x slower.
* More options or futures than 3 x 8: saturates near 81%. M=2: 71.8% (1,500 x 2).
* Skipping search where the policy is >= 95% sure: 206 fights/s, 71.9% (vs 74.0%).
* Dropping candidates below 3% / 8% policy probability: -3.7 / -4.5 points.
* Value network on the state where the policy ends its turn, without simulating the enemy turn: +27% speed, -2.8 points.
* Play-outs cut after d policy decisions with a mid-turn value: d=2 -1.4, d=3 -0.6 points, +10% speed.
* Fewer futures for options needing hidden information at their first action: K=4/3/2 -0.5/-1.0/-1.6 points.
* Greedy shared prefix: no gain, loops. Adaptive futures (3-4 first, more only for options within z standard errors of the best): lands on the line between K=4 and K=8.
* More fights in flight (4,096 roots), 3 pipeline groups, separate graphs for rows with a pending selection: no gain.
* 64-wide play-out network: 146 vs 124 fights/s (fp32), 73.6%: not worth it.
* Distilling the search into the network, three variants (mid, 300 fights unless noted): 6,072 confirmed single-state disagreements (64-wide, 3,144 episodes) 60.8% vs 60.1% base, within noise; policy distillation with soft targets over every searched option (217k decisions) 65.7% greedy / 79.0% search vs 64.0% / 79.3% base, and agreement with the search's best option fell 61% -> 50% (option values come from 8 futures; most differences are noise); value-only fit to search-played results 73.7% search vs 79.3% (search compares candidate states that search-play never visits). More PPO steps, bigger networks and search on top work.

Open ideas: sharing the enemy turn between futures (needs a pause point between enemy phase and draw in `Combat::step`), sequential / adaptive K, one value head distilled from the three (GPU -30%).

## Provably unwinnable fights (`sts2sim::bounds`, `sts2.provably_unwinnable`)
Many generated fights cannot be won by anyone (a starter deck against a boss). `provably_unwinnable(scenario_json)` returns a sentence proving it, or `None` (nothing proven). It relaxes the game in the player's favour and checks that the enemy still cannot be killed before the player dies:

* Every card must be **pure**: probing in the simulator (five situations, the same card twice, a Vulnerable enemy) shows it only spends its energy, deals fixed damage / gains fixed block (+ Vulnerable / Weak), moves cards between piles and changes nothing else (no energy, powers, upgrades, cost changes, extra plays, growth). 28 Ironclad and 28 Silent cards qualify; Defect, Necrobinder and Regent are not covered.
* Relics must only hook things that cannot matter in a fight (144 of 300); no potions, starting powers or enchantments.
* One enemy that cannot react to damage (no stun / sleep / flee intents, no damage-reactive start powers), with behaviour bounded from below by the cheapest damage over every state its move machine can be in each turn.
* Per turn the player may play any subset of its deck (hand size and draw order ignored) whose cost fits the energy, i.e. a damage / block frontier; block does not carry over. A DP over turns (state = HP lost, value = damage dealt) shows whether the enemy's HP is reachable.

`crates/sts2sim/tests/bounds.rs` covers purity, the not-proven cases (potions, unknown relics, impure cards) and a coverage report. Soundness was measured once with decks of pure cards against every encounter: no policy (random, scripted, the network sampled and greedy) won a flagged fight (3 seeds x ~530 flagged fights x 20k episodes per policy; that sweep script is not in the repo). Coverage on the random-deck training distribution is small (decks nearly always contain a card, relic or potion the prover cannot vouch for), so `gen_train.py --drop-unwinnable` filters little there; the verdict is a cheap "do not simulate this" for starter-deck-like cases.
