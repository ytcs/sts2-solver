# STS2 combat simulator: design

Target: Slay the Spire 2 **v0.111.0 public beta** (Steam build 24724944, commit 41cef1ea). Single-player combat only.
Purpose: an RL environment that replays real fights faithfully (relics and relic state, potions, draw/discard/exhaust order, enemy move patterns, RNG) at 10^4-10^5 parallel fights.
Priorities, in order: fidelity, throughput, memory. Fidelity is verified by differential testing against the real game (Validation).

## Decisions
| Topic | Decision | Why |
|---|---|---|
| Language | Rust core (`crates/sts2sim`), batch env (`crates/sts2env`), PyO3 bindings (`crates/sts2py`) | no GC, predictable layout, rayon; Python/PyTorch on top |
| Source of truth | decompiled `sts2.dll` in `decomp/` (gitignored, not in the checkout; regenerate with ilspycmd), `docs/spec/01-05` | the game logic is regular C#; rules are ported, not guessed |
| State | plain data, fixed capacities, `Clone` = memcpy, no heap on the hot path | search and RL need cheap copies |
| Control flow | explicit step machine, not async: `step(action)` runs until the next decision | async futures cannot be cloned |
| Decisions | everything the player chooses (card/target, potion, end turn, card-select prompts) is an `Action` from `legal_actions()` | uniform RL interface |
| Content | one Rust `listener!` per card/relic/power/potion/monster/enchantment/affliction, class name = C# class name; dispatch by dense id, per-kind hook bitmask | fast dispatch, no `dyn` beyond the listener table |
| Numbers | integer / fixed-point `Dec` mirroring the game's `decimal` pipeline | bit-exact damage |
| RNG | port of `MegaRandom` / `Rng`, golden-tested against the game's own code (`oracle/RngGolden`, `tests/rng_golden.rs`); .NET introsort for reshuffle order (`sort.rs`) | same seeds, same draws |

### Suspension model
The game awaits inside nested hook calls. The simulator mirrors its synchronous semantics with direct recursion and resolves player choices with a phase-resumable effect function: `fn on_play(cx, play, phase) -> Flow`, where `Flow::Suspend(next_phase)` records a pending `Decision` and returns to the step loop; the chosen value is available to `on_play(.., next_phase)`. Cards without choices are single-phase. Choices raised inside hooks use `cx.hook_ctx = Some((me, phase))` plus `Listener::resume_hook`; turn start and end resume through `turn_cont` / `end_turn_resume`. Conventions: `docs/porting-guide.md`.

Prompts raised inside draws that cannot pause (Stratagem's reshuffle pick in a card's draw, a hook's draw, `AutoPlayFromDrawPile`) are **replayed** (`engine/replay.rs`): the step is snapshotted, the agent sees the state at the prompt, then the snapshot is restored and the same action re-runs with the recorded answer (the engine is deterministic). Only combats containing a Stratagem (`Combat::strat_possible`) pay for it (one `Combat` clone per step); a `Combat` cloned at a prompt carries its snapshot. Search must `determinize` before the action, not at a replayed prompt.

### State and API
* `Combat`: RNG streams, turn/side/phase, player, enemies, card arena (`cards[CAP]`, piles are index arrays preserving order), relics, potions, orbs, op stack, pending decision, history ring. `Creature { hp, max_hp, block, powers }` with powers in game list order. `size_of::<Combat>()` is 19,024 B (about 19 KB); `tests/robustness.rs::state_size_budget` fails if it exceeds its budget (19,200 B).
* Rust: `Combat::new(&Scenario)`, `try_new`, `new_with(&Scenario, &ScenarioExtras)` (deck enchantments, saved props, gold, act), `legal_actions`, `action_mask`, `step(Action) -> bool`, `observe`, `determinize`; `reset` / `reset_with` / `reset_validated` re-initialise in place (bit-identical to `new`, tested).
* `sts2env::BatchEnv` (rayon, flat `f32` observation tensor, action masks, `fork_from`, autoreset) and `sts2.VecEnv`, `sts2.Sim` (one steppable, alignable fight), `sts2.provably_unwinnable` in Python. Contract: `docs/env-api.md`.
* Capacities (`MAX_CARDS` 160, `MAX_POWERS` 16, `MAX_CREATURES` 12, `MAX_PICK` 64, `ACTION_SPACE` 252) are in `docs/env-api.md`. A full `ArrayVec` never panics or drops silently: it raises a flag folded into `Combat::overflow` and `BatchEnv` ends the episode with `OUTCOME_OVERFLOW`. The release profile is `panic = "abort"`, so invalid scenarios return `Err(ScenarioError)` instead of panicking. `ACTION_SPACE` depends on `MAX_CREATURES`: changing it invalidates saved policy heads.
* Runaway-work safeguard (`engine/budget.rs`): one `step` (and one look-ahead turn) is bounded by `WORK_LIMIT` work units (hook passes with listeners, card plays, attack hits, monster transitions; 20,000, ~115x the corpus maximum of 174), `HOOK_DEPTH_LIMIT` nested hook passes (64; max seen 12; a looping chain is recursion and would overflow the stack first) and `TURN_LIMIT` player turns started (20; max seen 2). Past a limit the step is cut short (`in_progress = false`, every further pass skipped), returns with `ov::LOOP` in `Combat::overflow` and leaves the combat in `Stage::Over`: only safe to drop, reported as `OUTCOME_OVERFLOW` by `BatchEnv` and the search. Cost ~1% single-threaded.
* `BatchEnv` runs on its own rayon pool with 32 MB worker stacks (the inlined per-env step has a multi-KB frame) and a non-inlined leaf.

### Adding content
1. Stats exist in `content/gen_*.rs`; regenerate after a game update with `scripts/porting/gen_defs.py`, `gen_ids.py`, `gen_relics.py`.
2. Write `listener!(Name { fn hook(...) {...} })` in the matching `content/*/` file; the macro derives the hook mask from the methods overridden. New files register automatically (`build.rs`).
3. Unregistered ids are rejected by `Scenario::validate` and, if met mid-fight, flagged as `Combat::missing`; they never run silently.
4. Port from the decompiled `OnPlay` / hook body, then validate against the oracle. Details: `docs/porting-guide.md`.

## Validation
1. **Golden tests**: RNG, string hash and shuffle against the game's own code.
2. **Differential oracle**: the real game's combat code runs headless (`oracle/`, `docs/oracle.md`); a scenario plus a script produces a full-state trace after every step; `crates/sts2diff` replays it in Rust and compares field by field (all nine RNG streams included).
3. **Template corpus**: 418 templates in `oracle/templates/**` (`verify/diff_sweep.py` per template, `verify/regress.py` for all). `verify/regress_cache.py record` stores the oracle's traces once; `check` replays them in seconds, also through the in-place reset with `STS2DIFF_REUSE=1`.
4. **Frozen regressions**: 89 scenarios (`oracle/regression/` replayed by `cargo test`, `oracle/regression_scripted/` by `tools/fuzz_gen.py regress`).
5. **Randomized fuzzing**: random A10 decks x relics x potions x every encounter (`tools/fuzz_gen*.py`). About 250,000 fights at the last full round, 0 residual mismatches.
6. **Live game**: `python -m agent.fidelity_sweep` plays fights in the real game and counts every divergence between the simulator's prediction and the visible state.

Run the fuzz and regression sweeps after any content or engine change.

## Coverage
Implemented means a `listener!`, `MonsterDef` or encounter spawn exists.
| kind | implemented | total | not ported |
|---|---|---|---|
| cards | 595 | 596 | `DEPRECATED_CARD` |
| powers | 256 | 265 | multiplayer-only (Concoct, Covered, Fade, Guarded, Intercept) and powers nothing applies (Gravity, Leadership, MagicBomb, NoEnergyGain) |
| relics | 300 | 300 | none |
| potions | 64 | 65 | `DEPRECATED_POTION` |
| monsters | 114 | 120 | test/mock/deprecated (BigDummy, OneHp, TenHp, SingleAttack/MultiAttackMove, Deprecated) |
| encounters | 89 | 90 | `DEPRECATED_ENCOUNTER` |

Also implemented: 23 enchantments, 7 afflictions. Characters: Ironclad, Silent, Defect (orbs), Necrobinder (Osty, Doom), Regent (stars, Forge); acts 1-3 (Overgrowth or Underdocks, Hive, Glory) plus events.

## Performance
Measured with `cargo run --release -p sts2env --example bench` (random policy, observation and legal actions each step) and callgrind on the `prof` profile (`examples/prof.rs`: `fights|reset|env N`, prints a checksum that must not change under a pure optimisation).

| metric | value |
|---|---|
| single-thread full fights/s | ~84k (`Combat::new` per fight), ~93k (reset in place) |
| env-steps/s per core | 0.17-0.18M; 1/2/4 threads: 0.17/0.30/0.58M (linear); 0.95M on 14 threads with ~10 foreign cores busy; ~2.4M on an idle 14-core box (estimate) |
| instructions, 1000 greedy starter-vs-Nibbit fights | 108.9 M Ir for the 1000-fight workload (`new` per fight), 86.2 M (reset); env step 52.4 M Ir for 100 random episodes |

Rules that keep it there:
* `ArrayVec` is `MaybeUninit`-backed; hook snapshots are filled in place (`snapshot_into`, `damage_into`, `modify_*_into`), never returned as 3 KB lists. `Combat::listen` is the union of every present model's hook mask, so a hook with no listener is one bit test; keep the test inline and the body out of line (`dispatch_u` / `dispatch_g`).
* Reset in place: one memset and memcpy per fight; `Combat::reset_validated` names every `Combat` field in a destructuring `let`, so an uninitialised new field is a compile error.
* Observation: one memset, empty slots skipped, packed sort keys for pile multisets; `observe_ex` + `legal_actions_ex` evaluate `can_play` once per hand card. `Creature::secondary` caches whether an enemy is secondary, which the ending test reads (call `sync_secondary` after any write to `powers`). `Dec::trunc` on `i64`.
* Memory: `Card` 48 B x 160, `Creature` 416 B x 12, hook `Snapshot` 256 entries (~3 KB stack temporary).
* Not optimised because it would change semantics or needs a new design: `Dec::mul` needs the 128-bit product (`__divti3` ~3% of a fight); observation previews re-run `modify_damage` / `modify_block` per hand card because modifiers are hooks that may depend on the instance; `hooks_enabled` / `is_ending` are not cached across calls (a cache needs invalidation on every hp / power / pending-loss write); about 700 branch mispredictions per fight (dyn hook calls, jump tables) are the remaining floor.

## Known gaps
* **Fixed capacities**: an overflowing fight sets `Combat::overflow` (also `ov::LOOP` for a step that ran away, see State and API); only extreme stall fights reach it (corpus peak of random-policy 200-step fights: 143 of 160 card slots, 13 of 16 player powers, 8 creatures, 39 decision candidates).
* **Information contract not verified against the real UI**: piles are exposed as unordered multisets and pile selection screens in a canonical order (`docs/env-api.md`); "known top card" knowledge after put-on-top effects is not tracked.
* Run-level deck-copy listeners, `GainsBlock` as a card property (approximated by "has a Block var"), non-integer named card vars (Tank) are not modelled.
* The oracle's own game code crashes on a few paths (Inky on non-enemy-targeted cards, Entropy with no eligible card); those cases are untestable and excluded.
* Content the simulator does not port is reported, never simulated silently (`Combat::missing`, `OUTCOME_UNIMPLEMENTED`, fuzz verdict `unimplemented`).
