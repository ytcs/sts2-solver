# STS2 combat simulator — design

Target: Slay the Spire 2 **v0.111.0 public-beta** (Steam build 24724944, commit 41cef1ea). Single-player combat only.
Purpose: an RL environment that replays real fights faithfully (relics + relic state, potions, draw/discard/exhaust
order, enemy move patterns, RNG) at 10⁴–10⁵ parallel fights.

Priorities, in order: **fidelity → throughput → memory**. Fidelity is verified by differential testing against the
real game (see Validation); it is never assumed.

## Decisions

| Topic | Decision | Why |
|---|---|---|
| Language | Rust core (`crates/sts2sim`), PyO3 bindings later | No GC, predictable layout, rayon; Python/PyTorch on top |
| Source of truth | Decompiled `sts2.dll` in `decomp/` (gitignored, regenerate with ilspycmd) | Game logic is regular C#; we port rules, not guess them |
| State | Plain data, fixed capacities, `Clone` = memcpy, zero heap on the hot path | Search/RL need cheap copies; 10⁵ envs ≈ hundreds of MB |
| Control flow | Explicit step machine, **not** async. `step(state, action)` runs until the next decision | Async futures cannot be cloned/snapshotted |
| Decisions | Everything the player chooses (play card/target, potion, end turn, card-select prompts) is an `Action` over a `legal_actions()` list | Uniform RL interface |
| Content | One Rust module per card/relic/power/potion/monster, dispatched by `match` on a dense id; per-kind hook bitmask so iteration skips non-listeners | Fast dispatch, no dyn |
| Numbers | Integer / fixed-point arithmetic mirroring the game's `decimal` pipeline (exact rounding points taken from spec 02) | Bit-exact damage |
| RNG | Port of `MegaRandom`/`Rng` (done; golden-tested against the game's own code) | Same seeds → same draws |

### Suspension model
The game awaits inside nested hook calls. We mirror its synchronous semantics with direct recursion, and
resolve player choices with a **phase-resumable** effect function: `fn on_play(cx, phase) -> Flow`, where
`Flow::Suspend(next_phase)` records a pending decision and returns to the caller loop; the chosen value is
available to `on_play(cx, next_phase)`. Cards with no choices are single-phase. Choices raised from inside
hooks (rare) are handled case-by-case by queueing the decision and a resume op at the front of the op stack.

### State layout (target)
* `Combat { rng streams, turn/side/phase, player: Creature, enemies: [Creature; N], cards: [Card; CAP], piles, relics, potions, op_stack, pending_decision }`
* `Creature { hp, max_hp, block, powers: [(PowerId u16, amount i32, aux i32); P] }` — powers kept in game list order
  when order affects hook order (spec 02 decides).
* Cards live in an arena (`cards[i]`); piles are small index arrays preserving order (draw pile order matters).

### Public API (planned)
* `Combat::new(&Scenario, seed) -> Combat`
* `combat.legal_actions(&mut ActionBuf)`, `combat.step(Action) -> StepOutcome`
* `BatchEnv` (rayon) with a flat `f32` observation tensor and action masks for NumPy zero-copy.

## Validation (fidelity)
1. **Unit golden tests**: RNG/hash/shuffle vs the game's own code (done — `oracle/RngGolden`).
2. **Differential combat oracle**: a mod that runs the *real* game headless, injects a scenario (character, deck with
   upgrades/enchantments, relics with state, potions, encounter, seed) and a scripted action list, and dumps the full
   state after every step. The Rust sim replays the same script and must match field-for-field. Prior art for the
   harness (headless boot, `EnterRoomDebug`, autopilot) is in git history at `a1a341a:mods/DataDumper`.
3. **Random-play fuzzing** against the oracle for every encounter × character; mismatches are triaged per entity.

## Status (engine core + vertical slice)

Built and tested (`cargo test -p sts2sim`):
* `rng`, `sort` — bit-exact RNG (golden-tested against the game's own `Rng`), `.NET` introsort for reshuffle order.
* `dec` — fixed-point `decimal` stand-in for the damage pipeline.
* `engine/` — listener dispatch with the game's snapshot/liveness/guard semantics, creatures, damage pipeline
  (modify → block → HP-loss phases → post-hooks → kill), powers (stacking, Artifact-style received-amount hooks,
  tick-down), piles (draw/shuffle/exhaust/hand-full redirect), resumable card-play pipeline, monster state machine
  (weighted branches, repeat/cooldown rules, spawn HP rule), turn loop (player/enemy turns, innate, flush, win/loss).
* `content/` — generated stat tables for all 596 cards and 265 powers (`tools/gen_defs.py`) and generated id tables
  (`tools/gen_ids.py`); hand-written behaviour only for Strike/Defend/Bash, Strength/Dexterity/Vulnerable/Weak/Frail,
  Burning Blood, Nibbit (`NIBBITS_WEAK`).
* Throughput (release, this machine): ~94k full fights/s/thread, ~660k fights/s on 14 threads (~11M agent-steps/s),
  `size_of::<Combat>() ≈ 14 KB`. See `crates/sts2sim/examples/bench.rs`.

Since the first slice: decisions (click/confirm model, hand/pile/choose-a-card), potions, dense action space +
`legal_actions`/`action_mask`, human-information observation with a hidden-state leak test, card pools +
`GetDistinctForCombat`, 15 more Ironclad cards + Discovery, 8 potions. See `docs/env-api.md`.

### Performance notes
* `ArrayVec` is `MaybeUninit`-backed: temporaries (hook snapshots, damage results) cost nothing to create.
* `Combat::listen` is the union of every present model's hook mask; a hook with no listener is one bit test.
  (First version zero-filled a 3 KB snapshot per dispatch: 11k → 94k fights/s/thread after this change.)
* (Hardening phase, below: snapshots are filled in place, dispatch slow paths are out of line, `Combat` is 17.5 KB, resets are in place.)

### How to add content
1. Stats already exist in `content/gen_*.rs`. Re-run `tools/gen_defs.py` / `tools/gen_ids.py` after a game update.
2. Write a `listener!(Name { fn hook(...) {...} })` in `content/{cards,powers,relics,monsters}.rs` — the macro derives the
   hook mask from the methods you override. Card effects use `on_play`; resumable effects return `Flow::Suspend(phase)`
   after raising a `Decision`.
3. Register it in `content/mod.rs`; unregistered ids are rejected by `Scenario::validate` (never silently simulated).
4. Port from the decompiled `OnPlay`/hook body, following the specs; add a differential trace once the oracle exists.

### Content status (final integration)
Everything on `sim-rebuild` is validated by differential sweeps against the real-game oracle: the template corpus
(`oracle/templates/**`, `python3 tools/regress.py`, 418 templates, all `ok`), 1,254 recorded real-game traces replayed bit-identically
(`tools/regress_cache.py check`, also through the in-place reset), 70+ frozen regression scenarios (`oracle/regression*`), and ~250,000
randomized A10 fuzz fights (random decks × relics × potions × every encounter) with 0 residual mismatches.

Coverage (`python3 tools/coverage.py`; implemented = a `listener!` / `MonsterDef` / encounter spawn exists):

| kind | implemented | total | not ported |
|---|---|---|---|
| cards | 595 | 596 | `DEPRECATED_CARD` |
| powers | 256 | 265 | multiplayer-only (Concoct, Covered, Fade, Guarded, Intercept) and powers nothing applies (Gravity, Leadership, MagicBomb, NoEnergyGain) |
| relics | 300 | 300 | — |
| potions | 64 | 65 | `DEPRECATED_POTION` |
| monsters | 114 | 120 | test/mock/deprecated (BigDummy, OneHp, TenHp, SingleAttack/MultiAttackMove, Deprecated) |
| encounters | 89 | 90 | `DEPRECATED_ENCOUNTER` |

Throughput (release, shared/loaded machine): ~80-95k full fights/s/thread, ~450k+ fights/s on 14 threads; `size_of::<Combat>()` ≈ 18.7 KB.

### Hardening phase (robustness, memory, throughput)
Done on `sim-rebuild` after the content merge; every step was verified bit-identical (unit tests, `tools/regress_cache.py check` over the
1236 cached real-game traces of all 398 templates, once with a fresh `Combat` and once through the in-place reset with `STS2DIFF_REUSE=1`,
and the instruction-count harness checksums). Tools: `examples/prof.rs` (callgrind workload), `examples/sizes.rs`, `[profile.prof]`,
`tools/regress_cache.py` (record the oracle's traces once, replay them in ~15 s).

**Robustness** (details and the capacity table: `docs/env-api.md`). A full `ArrayVec` never panics or drops silently: it raises a
thread-local flag that `Combat::step` folds into `Combat::overflow` (`state::ov::*`); card arena, creature slots, history ring, counters,
uids and the scenario capacities are flagged the same way; `BatchEnv` ends such an episode with `OUTCOME_OVERFLOW` (4, also in `sts2py`),
`sts2diff` reports it as an error. Invalid scenarios are `Err(ScenarioError)` (`Combat::try_new`), buffer-size mistakes are
`Err(EnvError::Buffer)`, never panics (the release profile is `panic = "abort"`: a panic would kill the whole training process).
The history ring only stores the kinds some content queries per turn and flags the overwrite of a still-live entry. Silent truncations were
removed (forced selections cut at 16, `attack_results` cut at 16, `deck_enchant_inc` saturation, wrapping uids). Corpus audit (random-policy
fights of 200 steps): up to 143 of 160 card slots (Test Subject boss), 13 of 16 player powers, 6 enemy powers, 8 creatures, 39 decision
candidates (the observation shows 16 of them, the header carries the true count). Found and fixed on the way: the batch env overflowed
rayon's 2 MB worker stacks about every other run (the inlined per-env step has a multi-KB frame that rayon stacks once per split level) -
it now runs on its own pool with 32 MB stacks and a non-inlined leaf.

**Memory** (`size_of::<Combat>()`, budget test `tests/robustness.rs::state_size_budget`):

| | before | after |
|---|---|---|
| `Combat` | 21,072 B | 17,488 B |
| `Card` x 160 | 60 B | 48 B (`CostMod` 4 -> 2 B, `u8` flags, `u8`-length lists) |
| `Creature` x slots | 416 B x 16 | 416 B x 12 (largest encounter starts with 4 enemies; corpus peak 8) |
| hook `Snapshot` (stack temporary) | 6,152 B | 3,076 B (no 32-byte mask per listener, capacity 128 -> 256) |
| `ACTION_SPACE` | 308 | 252 (`MAX_CREATURES` + 1 target slots per hand card / potion): **invalidates saved policy heads** |

**Throughput** (instruction counts from callgrind on the `prof` profile, 1000 greedy starter-vs-Nibbit fights; wall-clock on this shared
machine fluctuates 30-50% from other jobs, so the A/B runs are interleaved):

| | before (integrated `962cd3e`) | after |
|---|---|---|
| fight, `Combat::new` per fight | 158.7 M Ir | 108.9 M Ir (-31%) |
| fight, one `Combat` reset in place | n/a | 86.2 M Ir (-46% vs `new` before; 82.0 M before the final robustness checks) |
| env step (legal actions + observation + step), 100 random episodes | 91.9 M Ir | 52.4 M Ir (-43%) |
| single-thread fights/s (interleaved release runs, best of 8 at load 8-24) | 48 k | 84 k (`new`), 93 k (reset); 1.4-1.9x per run |
| env-steps/s per core (`sts2env` bench, 1 thread) | 0.11 M | 0.17-0.18 M |
| env-steps/s, 14 threads on a machine with ~10 foreign busy cores | 0.55 M | 0.95 M |

Per-core scaling is linear (1/2/4 threads: 0.17/0.30/0.58 M), so an idle 14-core box gives about 2.4 M env-steps/s (the 2 M target).
What paid off, in order: reset in place and not building / copying 20 KB structs (one memset + memcpy per fight); filling snapshots in place
instead of returning a 3 KB list (the compiler copied it whole on every call, even when empty); keeping the "nobody listens" test inline and the
body out of line (`dispatch_*`: the 3 KB snapshot frame no longer sits in every caller); no 300-byte / 1.3 KB list copies in the damage and block
pipelines; `Creature::secondary` instead of scanning powers in every `is_ending`; observation: one memset, skipping empty slots, packed sort
keys for the draw-pile multiset, `can_play` evaluated once per hand card for mask + observation (`legal_actions_ex` / `observe_ex`);
`Dec::trunc` on `i64`; cheaper `ArrayVec::insert/remove`. Callgrind counts `rep stos` per byte, so memset-heavy code looks worse in Ir than it is.

Not optimized (would change semantics or needs a different design): `Dec::mul` needs the 128-bit product (the `decimal` stand-in must stay
exact; `__divti3` is ~3% of a fight); observation previews re-run `modify_damage` / `modify_block` per hand card because every modifier is a
hook that may depend on the card instance; `hooks_enabled` / `is_ending` are not cached across calls (a cache would need invalidation on every
hp / power / pending-loss write); the profile is now flat, with ~700 branch mispredictions per fight (dyn hook calls, jump tables) as the
remaining floor; per-creature power capacity by role (`Power` is 20 B x 16 per creature; the player reached 13 powers in the corpus, so 16
stays).

### Known gaps (honest list)
* **Stratagem decisions in draws that cannot suspend** (draws started from inside another hook — Centennial Puzzle, Iteration — or
  `AutoPlayFromDrawPile` shuffles — Mayhem, Cascade, Havoc —, and draws that are not an effect's last action — Battle Trance, Acrobatics...):
  the fight is flagged `Combat::missing` and batch envs end the episode with `OUTCOME_UNIMPLEMENTED`. ≈0.1-0.4% of fights *in decks that
  contain Stratagem*; training distributions can simply exclude that one card. Parked oracle traces: `oracle/regression_pending/`.
* **Fixed capacities** (160 cards, 16 powers per creature, 12 creature slots, 64 decision candidates in the action space): an overflowing
  fight raises the sticky `Combat::overflow` flag and envs end it with `OUTCOME_OVERFLOW`; only extreme stall fights reach it.
* **Information-contract assumptions not yet verified against the real UI**: discard/exhaust are exposed in pile order; "known top card"
  knowledge (after put-on-top effects) is not tracked.
* Run-level deck-copy listeners, `GainsBlock` as a card property (approximated by "has a Block var"), non-integer named card vars (Tank).
* The oracle's own game code crashes on a few paths (Inky on non-enemy-targeted cards, Entropy with no eligible card): untestable, excluded.
Fidelity TODOs are marked `TODO(fidelity)` in code.

## Milestones
1. ✅ Specs from the decompiled source (`docs/spec/01–05`)
2. ✅ Engine core + vertical slice
3. ✅ Oracle: the real game's combat code runs headless (`oracle/`, `docs/oracle.md`)
4. ✅ Differential harness (`crates/sts2diff`), corpus regression (`tools/regress.py`), randomized fuzzing (`tools/fuzz_gen*.py`)
5. ✅ Content breadth: all five characters, all four acts + events, relics, potions, enchantments
6. ✅ Batched RL env (`crates/sts2env`) and Python bindings (`crates/sts2py`); hardening (robustness flags, memory, throughput)
7. ⏳ Ongoing: more fuzz rounds after any content/engine change; Stratagem non-suspendable contexts; throughput tuning
