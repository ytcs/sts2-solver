# Combat solver (`rl/`, `crates/sts2env/src/search.rs`)

In: deck, relics, potions, HP, encounter (A10). Out: play maximising the fight objective (default linear: win +1 + 0.5 x HP fraction left, loss/stall -1; aborted episode 0).
Metrics: `win`; `hp_lost_all` = mean max-HP fraction lost, loss/stall charged the full start HP.

## Components
| file | role |
|---|---|
| `rl/model.py` | policy/value net (entity encoders, 2 message-passing rounds, pointer head over the dense action space, signed-log features); `Net(obs_version=)` per-checkpoint layout v1/v2 |
| `rl/heads.py` | fight-outcome head: categorical over loss / win x end-HP bin (`NC`, `BIN` must match Rust: asserted) |
| `rl/fastsearch.py` | `FastSearch`: evaluates networks for the Rust state machine; CUDA graphs per padded batch (`GraphFn.plan`), `torch.compile`, bf16 |
| `rl/solver.py` | `Solver`: batch solver; CLI |
| `rl/predictor.py` | fight predictor for macro pricing (outcome distribution) |
| `rl/exit.py`, `rl/ppo.py` | training (expert iteration, PPO) |
| `agent/engine.py` | `Engine`: harness service (live decision, tables) |
| `tools/gen_train.py` | A10 scenario sets (`--seed`: disjoint sets; `--energy-prob P`; `--drop-unwinnable`) |
| `tools/bench_search.py` | search-quality bench vs a Monte Carlo referee (regret per config) |

Networks: `models/current.json` (edited by hand) names `policy` (solver_h128.pt, outcome head, no extra value nets) and `predictor` (predictor_r2.pt). Sets: `data/train/eval.json` (1,500 held-out fights, 5 chars, 3 acts); `data/bench/*.json` (mix, tail, nearmiss, corpus, pairs, eval).

## Search
- At each decision: policy's top-M legal actions x K determinized futures (`VecEnv.fork_from` + `Combat::determinize`: piles' orders and all 9 RNG streams resampled, visible state unchanged). A future plays the policy for `LEAF_TURNS`=2 player turns (`rl/fastsearch.py`), then the value head (or the terminal reward). Root plays the best mean q.
- State machine: each fight is a root with M x K copies; engine runs to the next network request on rayon, one row per request; Python batches rows; `groups=2` alternates CPU sim / GPU eval. Results reproducible per job, independent of pool size and threads (gate checksums).
- On by default: `lead` (an option's in-turn play simulated once; futures branch at the first step touching hidden info), `strat` (futures share one shuffle, rotated: disjoint next hands), `carry` (chosen line's prefix reused next decision; known: rarely fires, speed only), auto-Confirm of a full selection.
- Worth: `run(..., worth=)` / `solve(worth=)` / `decide(worth=)`: per scenario None = linear, or dict(u=[NC class worths], price=[per belt slot]) combined in Rust with the outcome head's class probabilities (needs the head and no extra value nets: `Engine.worth_ok`). Win-only act-boss table: `agent.proposal.win_only_worth`.
- A panic aborts only that fight (stderr, `panics`); overflow -> `OUTCOME_OVERFLOW`.

## Root modes
`SearchCfg::root` / `FastSearch(root=)`. Current: `topm` only (live play, every table, collection). A bigger budget under top-M wins no more (E8-E12: root tries only what the prior ranks high; play-outs follow the same policy). Planned replacement of the candidate list: every distinct legal action (~6 avg; top-5 coverage 91%).

## API
```python
from solver import Solver                     # rl/solver.py
S = Solver()                                  # current.json networks, 3 options x 8 futures
res = S.solve(scenarios, attempts=32)         # per scenario: win, win_se, hp_lost, hp_lost_se, hp_left_on_win, attempts, aborted
```
- `Solver(ckpt, M=3, K=8, max_steps=300, roots=None, groups=2, conf=1.01, roll_ckpt=None, amp=None, threads=None)`; roots default 2048 CUDA / 256 CPU. `solve(scenarios, attempts, search=True, seed=0, groups=None, worth=None)`: `search=False` = greedy net; same `groups` id = same seeds per attempt (common random numbers across deck variants). CLI `python rl/solver.py --scenarios F.json --attempts 32 [--no-search] [--out F]`.
- `FastSearch.run(scenarios, job_scen, job_seed, worth=)` -> rows [scenario, outcome, hp_lost, hp_end, length, finished, end HP abs, potions-kept bits]; `decide(scenario, sim, seed, worth=)` one live decision (`sim` = aligned `sts2.Sim`).
- `Engine(M=5, K=32, ckpt=None)`: `decide(scenario, sim, budget, seed, tol_hp=1.0, keep_potions, worth, rounds)` repeats 5x32 rounds (~10 ms each) until budget or expected regret of the leader < `tol_hp` HP (min 4 rounds; tolerance scaled by the worth table's span); held potions removed via `sim.without_potions`, potion discards never chosen. `solve(scenarios, attempts=64, seed, groups, worth)` for tables (seeded per screen: `table_seed`). `play_on(scenario, starts, seeds, worth)`.
- Precision: 32 attempts/deck ~ +-5.3 win pts. Strength (`eval.json`, 1,500 x 4, 3x8, depth 2, h128): win 74.4-75.0%, hp_lost_all 0.348 (greedy ~65.5%).

## Performance invariants (a perf change must keep all)
- **Bit identity.** `bash tools/gate.sh` must pass unchanged: `searchprof` checksums v1/v2 (search results + moves), `envprof` checksums v1/v2 (obs, masks, rewards, outcomes), cargo tests `sts2diff/regression`, `sts2sim/{observe,rng_golden,sync}`, `sts2env/lookahead_cache`, `tests/rl/test_obs_version.py`. A deliberate behaviour change updates the checksums in `tools/gate.sh` in the same commit, stated as such. Net-side speedups must be bitwise on CPU and CUDA fp32/bf16 and through `torch.compile`; non-identical speedups (rollout forward as one fixed-shape CUDA graph 2.5x, bf16 update 1.13-1.2x) are not adopted.
- **Look-ahead cache key exactness** (`crates/sts2sim/src/engine/monster.rs`). Cache: 8-way set-associative LRU, one per search root (`with_look_cache`). Exact key = digest of every creature (machine state, powers, HP, block, presence) + turn position (`look_key_of`). Relaxed key = the same with enemy HP reduced to alive/dead and no block; valid only for projections that read no enemy starting HP/block (`Creature::pristine`, `look_dep` records reads). Player powers/relics enter the key only if they can reach the rows: a hook in `LOOK_PLAYER_HOOKS` (modify_damage additive / multiplicative / cap), `LOOK_READ_POWERS` (Debilitate), `LOOK_READ_RELICS` (Paper Krane, Paper Phrog, Whispering Earring). Any new read of state inside a projection (new hook, `has_relic`/power-by-id read, starting-HP read) must extend these or record a `look_dep`. Check: `STS2_LOOK_VERIFY=1` with `envprof`/`searchprof` (every hit recomputed fresh, panics on a difference) and `tests/lookahead_cache.rs`.
- **`Combat::clone_from` copies only live state** (`crates/sts2sim/src/state.rs`): `cards[..n_cards]`, live entries of fixed-capacity lists (`ArrayVec::copy_from`), written history-ring entries (`HistLog::copy_from`). Slots beyond are stale: never read past `n_cards`/`len`; a newly allocated slot must be fully written.
- **Hook snapshots** (`dispatch.rs` `snapshot_into`): "nobody listens" test inline, scan out of line; listener order must stay the game's order bit-exactly (do not cache snapshots across state changes; a per-observation memo and per-category masks gave nothing).
- Content registry tables (hook masks, listeners, implemented flags) are compile-time; keep new content in the same `const fn` match.

## Where time goes (current)
| scope | split |
|---|---|
| live 5x32 depth 2, local RTX 4070 Super | GPU-bound: graph replay ~47 of 62 s (2,048 eval fights, roots 1,024), engine hidden behind it; replay cost ~1.26 us/policy row, proportional to padded batch; H2D obs copies ~283 GB/2,048 fights (rows 94% zeros) |
| PPO env step (1 thread, v1, 7.9 us) | look-ahead ~52% (projection ~34k cycles: enemy moves 12k, rolls 7k, intent damage 4k, enemy turn start 4k); hook snapshots ~15% |
| 5x32 search engine | ~57 snapshots per observation row; look-ahead the largest share of an observation; `Combat` clones ~30% of a projection |
| PPO (d128 v1 / d256 v1 / v2) | 30.0k / 20.7k / 23.3k steps/s (1,024 envs x 48) |
Profilers: `crates/sts2env/examples/searchprof.rs` (engine without network, stand-in answers), `envprof.rs` (PPO env step), `sampleprof.rs` (sampling profiler, Windows, no admin); feature `obs_prof` for per-section observation cycles; `FastSearch(profile_gpu=True)`.
Do not retry: int8 quantization (slower, changes decisions); skip search when policy >= 95% (-2 pts); dropping low-prior candidates (-3.7 pts); value net without simulating the enemy turn (-2.8 pts); full-fight play-outs without value head (-5.5 pts, 4x slower); more roots/groups than 2,048/2; per-env look-ahead cache.

## Provably unwinnable fights (`sts2sim::bounds`, `sts2.provably_unwinnable`)
`provably_unwinnable(scenario_json)` -> proof sentence or None. Relaxes the game in the player's favour, proves the enemy still cannot die first:
- Every card **pure** (probed: only spends energy, fixed damage/block (+Vulnerable/Weak), moves cards; nothing else). Covered: 28 Ironclad, 28 Silent cards.
- Relics only from the fight-irrelevant set (144/300); no potions, starting powers, enchantments.
- One enemy that cannot react to damage (no stun/sleep/flee intents, no damage-reactive start powers); its damage bounded below over every move-machine state.
- Per turn any affordable subset of the deck (hand size, draw order ignored), block not carried; DP over turns (HP lost -> damage dealt).
Code `crates/sts2sim/src/bounds.rs` (no test in the gate). Soundness `[sim]`: no policy won a flagged fight (3 seeds x ~530 flagged x 20k episodes). Coverage on random training decks is small; `gen_train.py --drop-unwinnable` filters little.
