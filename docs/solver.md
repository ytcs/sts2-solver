# Combat solver (`rl/`, `crates/sts2env/src/search.rs`)
In: deck, relics, potions, HP, encounter (A10). Out: play maximising the fight objective (default linear: win +1 + 0.5 x HP fraction left, loss/stall -1; aborted 0). Networks: `models/current.json` (`policy`, `predictor`). Sets: `data/train/eval.json` (1,500 held-out fights), `data/bench/*.json`.

| file | role |
|---|---|
| `rl/model.py` | policy/value net (entity encoders, 2 message-passing rounds, pointer head over the dense action space) |
| `rl/heads.py` | outcome head: categorical over loss / win x end-HP bin (`NC`, `BIN` must match Rust: asserted) |
| `rl/fastsearch.py` | `FastSearch`: networks for the Rust state machine; CUDA graphs per padded batch, `torch.compile`, bf16, copy-stream uploads |
| `rl/solver.py` | `Solver`: batch solver + CLI |
| `rl/predictor.py` | fight-start outcome distribution for macro pricing |
| `rl/exit.py`, `rl/ppo.py` | training (expert iteration, PPO) |
| `agent/engine.py` | `Engine`: live decisions and tables |
| `tools/gen_train.py` | A10 scenario sets (`--seed`, `--energy-prob`, `--drop-unwinnable`) |
| `tools/bench_search.py` | search quality vs a Monte Carlo referee |

## Search
- Each decision: candidates x determinized futures (`Combat::determinize`: pile orders and all 9 RNG streams resampled, visible state unchanged). Candidates: top-M by policy, or with `cover` every distinct legal action. A future plays the policy for `LEAF_TURNS` = 2 player turns, then the value head. Root plays the best mean q.
- `exact_turn`: when the searched values are blind, enumerate every line to the end of the turn (`ExactCfg`, `EXACT_TURN`).
- Live `Engine.decide` = cover + exact turn; `Engine.solve` tables and `Solver` = top-M (`cover` optional).
- State machine: each root has its copies; the engine runs to the next network request on rayon; Python batches rows; `groups=2` alternates CPU sim / GPU eval. Results reproducible per job, independent of pool size and threads.
- Built-in: an option's in-turn play simulated once, futures branch at the first hidden-info step; futures share one rotated shuffle (disjoint next hands); the chosen line's prefix is reused next decision (rarely fires); a full selection auto-confirms.
- Worth: `worth=` per scenario: None = linear, or dict(u=[NC class worths]) combined in Rust with the outcome head (`Engine.worth_ok`). Win-only boss table: `agent.proposal.win_only_worth`.
- A panic aborts only that fight; overflow -> `OUTCOME_OVERFLOW`.
- A bigger top-M budget wins no more (E8); cover adds the actions the prior ranks low (E27).

## API
- `Solver(ckpt, M=3, K=8, max_steps=300, roots=None, groups=2, roll_ckpt=None, amp=None, threads=None, cover=False)`; roots default 2048 CUDA / 256 CPU. `solve(scenarios, attempts=32, search=True, seed=0, groups=None, worth=None)` -> per scenario win, win_se, hp_lost, hp_lost_se, hp_left_on_win, attempts, aborted; same `groups` id = common random numbers across variants. CLI `python rl/solver.py --scenarios F.json --attempts 32 [--no-search] [--out F]`.
- `FastSearch(net, M, K, ..., record=False, leaf_turns=None, clairvoyant=False, cover=False, futures=0, exact_turn=None)`: `run(scenarios, job_scen, job_seed, starts=None, worth=None)` -> rows [scenario, outcome, hp_lost, hp_end, length, finished, end HP, potions-kept bits]; `decide(scenario, sim, seed, worth)` one decision on an aligned `sts2.Sim`.
- `Engine(M=5, K=32, ckpt=None, cover=True, futures=0, exact_turn=True)`: `decide(scenario, sim, budget, seed, tol_hp=1.0, keep_potions, worth, rounds)` repeats rounds until budget or the leader's expected regret < `tol_hp` (min 4 rounds); held potions removed via `sim.without_potions`. `solve(...)` for tables (seeded per screen); `play_on(scenario, starts, seeds, worth, record)`.
- Precision: 32 attempts per deck ~ +-5.3 win points.

## Performance invariants (a perf change keeps all)
- **Bit identity.** `bash tools/gate.sh` passes unchanged: `searchprof` and `envprof` checksums, cargo tests `sts2diff/regression`, `sts2sim/{observe,rng_golden,sync}`, `sts2env/lookahead_cache`. A deliberate behaviour change updates the checksum in the same commit. Net-side speedups are bitwise or switchable and adopted after a statistical equivalence check (PPO `--graph-rollout`).
- **Look-ahead cache key exactness** (`crates/sts2sim/src/engine/monster.rs`): 8-way set-associative LRU per search root. Exact key = digest of every creature (machine state, powers, HP, block, presence) + turn position (`look_key_of`); relaxed key (enemy HP alive/dead, no block) only for projections that read no enemy starting HP/block (`Creature::pristine`, `look_dep`). Player state enters the key only via `LOOK_PLAYER_HOOKS`, `LOOK_READ_POWERS`, `LOOK_READ_RELICS`: any new read inside a projection must extend these or record a `look_dep`. Check: `STS2_LOOK_VERIFY=1` with `envprof`/`searchprof`, and `crates/sts2env/tests/lookahead_cache.rs`.
- **`Combat::clone_from` copies only live state** (`state.rs`): never read past `n_cards`/`len`; a new slot must be fully written.
- **Hook snapshots** (`dispatch.rs` `snapshot_into`): listener order stays the game's; never cache snapshots across state changes.
- Content registry tables are compile-time `const fn` matches.

## Where time goes
- Search (RTX 4070 Super): GPU-bound in the full-batch phase; the run's tail (batches < 1,800 rows) ~23%; engine advance hidden behind the GPU.
- PPO env step (7.9 us, 1 thread): look-ahead ~52%, hook snapshots ~15%. 5x32 search: ~57 snapshots per observation row; `Combat` clones ~30% of a projection.
- PPO (1,024 envs x 48): d128 46k / d256 26k steps/s.
- Profilers: `crates/sts2env/examples/searchprof.rs`, `envprof.rs`, `sampleprof.rs` (Windows); feature `obs_prof`.
- Do not retry: int8 quantization; skipping search at policy >= 95%; dropping low-prior candidates; value without simulating the enemy turn; full-fight play-outs without value; > 2,048 roots / 2 groups; per-env look-ahead cache; shape buckets or split decision graphs in search; bf16/TF32, `torch.compile` or one CUDA graph for the PPO update.

## Provably unwinnable fights (`sts2.provably_unwinnable`, `crates/sts2sim/src/bounds.rs`)
Relaxes the game in the player's favour and proves the enemy still cannot die first: every card pure (energy, fixed damage/block, Vulnerable/Weak; 28 Ironclad + 28 Silent cards), fight-irrelevant relics only, no potions/powers/enchantments, one non-reactive enemy, per-turn any affordable subset, DP over turns. Soundness `[sim]`: no policy won a flagged fight (3 seeds x ~530 x 20k). Not in the gate; coverage on random decks is small.
