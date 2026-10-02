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
* Next candidates: card arena/piles as u8 handles are already compact; avoid `is_ending()` scans in `hooks_enabled`
  (cache per action), shrink `Combat` (power capacity per creature), SIMD-friendly observation packing.

### How to add content
1. Stats already exist in `content/gen_*.rs`. Re-run `tools/gen_defs.py` / `tools/gen_ids.py` after a game update.
2. Write a `listener!(Name { fn hook(...) {...} })` in `content/{cards,powers,relics,monsters}.rs` — the macro derives the
   hook mask from the methods you override. Card effects use `on_play`; resumable effects return `Flow::Suspend(phase)`
   after raising a `Decision`.
3. Register it in `content/mod.rs`; unregistered ids are rejected by `Scenario::validate` (never silently simulated).
4. Port from the decompiled `OnPlay`/hook body, following the specs; add a differential trace once the oracle exists.

### Integrated content status (after merging the parallel slices)
Merge order on `sim-rebuild`: engine-core, Ironclad M-Z, Silent A-M, Silent N-Z, Overgrowth, Regent, Necrobinder (Ironclad A-L and potions
were merged earlier). Every slice is validated by differential sweeps against the oracle (`oracle/templates/*.json`, run with
`python3 tools/diff_sweep.py oracle/templates/<t>.json --n 20 --jobs 4`; potions via `tools/potion_sweep.py --all`, Necrobinder via
`tools/necro_sweep.py all`): all `ok`; the only non-`ok` runs are `UNIMPLEMENTED` hits on content not yet merged (colorless / token /
status / curse cards such as Debris, Minion Dive Bomb, The Gambit; Underdocks monsters).

Coverage (`python3 tools/coverage.py`, implemented = a `listener!` / `MonsterDef` / encounter spawn exists):

| kind | implemented | total |
|---|---|---|
| cards | 378 | 596 (63%) |
| powers | 152 | 265 (57%) |
| relics | 8 | 300 (2%) |
| potions | 60 | 65 (92%) |
| monsters | 30 | 120 (25%) |
| encounters | 22 | 90 (24%) |
| total | 650 | 1436 |

Throughput after the merge (release, loaded shared machine, `crates/sts2sim/examples/bench.rs` / `crates/sts2env/examples/bench.rs`):
69k full fights/s/thread (was 87k at the Ironclad-A-L+potions baseline, 94k originally), 458k fights/s on 14 threads, 0.73M env-steps/s
(was 0.80M); `size_of::<Combat>()` 20.6 KB (was 14.5 KB: `Card` grew by star/enchant/affliction state, `HistLog` ring 2 KB, nested
play/auto-play stacks). Candidates: shrink `HistLog` entries / ring, move rarely used card state out of `Card`, skip history writes
for kinds nobody queries.

### Known gaps (engine)
Done in the engine-core pass: every `Hook.*` dispatcher, death/kill sequence (preventers, minions, escape, player death), mid-combat
summons, stun / forced moves, nested auto-play + Sly + dupes + transform, replay / result-location hooks, global keywords, X values,
enchantments (23) + affliction framework, extra turns, play history, end-turn requests, scenario extras (see the cheat sheet in
`docs/porting-guide.md`). Added by the content slices: stars / star costs / Forge (Regent), Osty summon / revive / redirect (Necrobinder),
turn-start and turn-end hook decisions (Tools of the Trade, Stampede), mid-combat monster lifecycle (Overgrowth).
Orbs (Defect) are done (`engine/orbs.rs`). Still open: a decision raised by an auto-play started from a *turn-start hook* that is not the first listener of its
pass (Mayhem / Imbued: listeners after the suspended one are skipped), deck-copy (run-level) listeners, encounter-local slot tables,
`GainsBlock` as a card property (approximated by "has a Block var"), non-integer named card vars (Tank 1.5 / 0.5), `CalculatedVar`
cards that need a calculated-var table (cards compute them by hand). Fidelity TODOs are marked `TODO(fidelity)` in code.

## Milestones
1. ✅ Specs from the decompiled source (`docs/spec/01–05`)
2. ✅ Engine core + vertical slice (Ironclad starter vs Nibbit)
3. ⏳ Oracle: run the real game headless from a scenario and dump per-step state (`oracle/`, `docs/oracle.md`)
4. Differential harness: replay oracle traces in Rust, field-by-field compare; random-play fuzzing
5. Content breadth (cards → powers → relics → potions → monsters/encounters), each validated against the oracle
6. PyO3 batch env (`BatchEnv`, flat f32 observations, action masks, rayon) + benchmarks
