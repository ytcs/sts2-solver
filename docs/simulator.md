# Combat simulator (sts2sim / sts2env / sts2py / sts2diff)

Target: STS2 v0.111.0 public beta (Steam build 24724944), single-player combat, A10 only. Priorities: fidelity > throughput > memory. Truth = the real game's code, replayed by the oracle. Verify every simulator or search change with `bash tools/gate.sh` (exit 0 = pass; checksums = bit identity of search + env, v1 and v2 obs; oracle regression traces, RNG goldens, information contract, sync, look-ahead cache exactness, obs v1 identity). A deliberate behaviour change updates the gate's checksums in the same commit.

## Design (was docs/design.md)
- Crates: `sts2sim` core, `sts2env` batch env + search, `sts2py` bindings (`import sts2`), `sts2diff` differential replayer.
- Truth: decompiled `sts2.dll` in `decomp/` (gitignored; regenerate with ilspycmd). Port, never guess; class name = id.
- State: plain data, fixed capacities, `Clone` = memcpy, no heap on the hot path. Control: explicit step machine, `step(action)` runs to the next decision (no async). Every player choice is an `Action`.
- Content: one `listener!` per entity, dense-id dispatch, per-kind hook bitmask. Numbers: `Dec` mirrors C# `decimal`. RNG: port of `MegaRandom` / `Rng`, golden-tested (`oracle/RngGolden`, `crates/sts2sim/tests/rng_golden.rs`); .NET introsort reshuffle (`sort.rs`).

**Suspension.** Synchronous recursion mirrors the game's nested awaits. Player choices: phase-resumable `fn on_play(cx, play, phase) -> Flow`; `Flow::Suspend(next)` records a `Decision`, the pick is read at `on_play(.., next)`. Choices inside hooks: `cx.hook_ctx = Some((me, phase))` + `Listener::resume_hook`; turn start resumes via `turn_cont`, turn end via `end_turn_resume`. Prompts inside draws that cannot pause (Stratagem reshuffle in a card's / hook's draw, `AutoPlayFromDrawPile`) are replayed (`engine/replay.rs`): snapshot the step, show the prompt state, restore, re-run with the recorded answer. Only combats with a Stratagem (`Combat::strat_possible`) pay (one clone per step). Search must `determinize` before the action, never at a replayed prompt.

**State / API.** `Combat` = RNG streams, turn/side/phase, player, enemies, card arena (`cards[CAP]`, piles = ordered index arrays), relics, potions, orbs, op stack, pending decision, history ring; ~19 KB. `Creature { hp, max_hp, block, powers }`, powers in game list order. Rust: `Combat::new(&Scenario)`, `try_new`, `new_with(&Scenario, &ScenarioExtras)` (deck enchantments, saved props, gold, act), `legal_actions`, `action_mask`, `step(Action) -> bool`, `observe` / `observe_v`, `determinize`, `reset` / `reset_with` / `reset_validated` (in place, bit-identical to `new`). Python: `sts2.VecEnv` (over `sts2env::BatchEnv`: rayon, flat f32 obs, masks, `fork_from`, autoreset), `sts2.Sim` (one steppable, alignable fight), `sts2.provably_unwinnable` (`sts2sim::bounds`). `BatchEnv` runs its own rayon pool with 32 MB worker stacks.

**Runaway-work guard** (`engine/budget.rs`): per step / per look-ahead turn `WORK_LIMIT` 20,000 work units (corpus max 174), `HOOK_DEPTH_LIMIT` 64 (max 12), `TURN_LIMIT` 20 turns started (max 2). Past a limit: step cut short, `ov::LOOP` in `Combat::overflow`, combat `Over` (only safe to drop); env and search score it as a LOSS (`sts2env::looped`; counted by `BatchEnv::loops`, search stats `end_loop` / `fight_loops`): the real game soft-locks there (`docs/game-bugs.md` 1, evidence E6).

**Coverage:** every non-deprecated card, relic, potion, encounter; powers 256/265 (missing: multiplayer-only or never applied); monsters 114/120 (missing: test/mock); 23 enchantments, 7 afflictions; 5 characters; acts 1-3 + event encounters.

**Known gaps.** Not modelled: run-level deck-copy listeners; `GainsBlock` as a card property (approximated by "has a Block var"); non-integer named vars (Tank); "known top card" after put-on-top effects. Oracle crashes on a few paths (Inky on non-enemy-targeted cards, Entropy with no eligible card): untestable, excluded. Unported content is always reported (`Combat::missing`, `OUTCOME_UNIMPLEMENTED`, fuzz verdict `unimplemented`), never simulated silently.

## Env API (was docs/env-api.md)
Contract: expose exactly what a human can do and see. Enforced by `crates/sts2sim/tests/observe.rs` (gate).

**Choice space** (`Action`, `Action::index/from_index`, `Combat::action_mask`; `ACTION_SPACE` = 252 with `MAX_CREATURES` 12: always read the constant; changing `MAX_CREATURES` invalidates policy heads):
| action | legal when |
|---|---|
| `PlayCard{hand_pos, target}` | play phase, `can_play` (cost incl. modifiers, `ShouldPlay` vetoes, card logic); one per living enemy for single-target cards, else `target = NO`. Unplayable cards are not listed |
| `UsePotion{slot, target}` | play phase, `CombatOnly`/`AnyTime` (never `Automatic`); self-targeted take no target |
| `DiscardPotion{slot}` | play phase, slot filled |
| `EndTurn` | play phase |
| `Pick{idx}` / `Confirm` | a decision is pending |

**Decisions** (`Combat::decision`: choose `min..=max` of an ordered candidate list):
- Hand selections in hand order. Every pile selection (draw/discard/exhaust) in a canonical order by visible properties (rarity, id, upgrade, cost, enchantment, affliction; ties by game order) so pile order never leaks. `Pick{idx}` indexes this displayed list (`decision_view`); the engine keeps game order (`Decision::cands`); `sts2diff` clicks in game order (`step_pick_game_order`).
- Forced choices auto-resolve (`|cands| <= min` with `min == max`, or empty).
- `Pick` toggles; at `max` selected the most recent is replaced. Completes at `max` unless `confirm_required` (`min != max`); `Confirm` finishes (or skips when skippable and empty). Result order = click order.
- A skip exists only where the game's `CardSelectCmd` call allows it (`ask_options(.., can_skip)` must match the C#).

**Information contract.** Only the exact random state is hidden. Visible: HP/max/block/energy/stars/powers, relics + counters, potions, hand in order (cost, playability, keywords, damage/block previews), draw/discard/exhaust as unordered multisets, enemies' HP/block/powers, intents (per-hit damage with UI modifiers, hit count), last 4 moves, the move-pattern look-ahead, per-turn play counters, the pending decision. Hidden: pile order, RNG states, realized random branch outcomes, monster AI state beyond the pattern. Tests: `hidden_state_does_not_leak` (permute piles, rewrite RNG, scramble monster logs: obs unchanged, v1 and v2), `big_pile_order_does_not_leak_in_v2`, pile-selection order test.

**Look-ahead** (`Combat::lookahead`, `engine/monster.rs`, `look_turn` in `engine/turn.rs`): per enemy and each of the next `LOOK_H` = 4 turns after the shown intent, P(move node) over `LOOK_NODES` = 16 + expected attack damage, from a projected copy: player passive and inert (player-forced branches not anticipated); enemy turns run fully; each roll branches at the game's odds on the projected combat (the looked-at monster and its `LOOK_JOINT` peers; others take their likeliest move); equal states merge, <= 8 paths per turn. Consumes no RNG, reads no realized outcome (fixed-seed copy). `enemy_moves` section: pending node + 1 (255 = stunned), stored follow-up + 1. Cache-key exactness: `docs/solver.md`.

**Observation layout** (`observe.rs`, sizes are constants, `OBS_SIZE` asserted): 1 global, 2 player, 3 relics, 4 potions, 5 hand (`CARD_F` each), 6 piles as sorted multisets + sizes, 7 enemies, 8 pending decision, 9 Regent star costs, 10 Necrobinder Osty, 11 Defect orbs, 12 look-ahead, 13 enemy moves. Rule: new sections append at the END only.

**Observation versions.** v1 (`OBS_SIZE` 1924, `CARD_F` 12) is frozen bit-identical (`tests/rl/test_obs_version.py`, gate). v2 (`OBS_SIZE_V2` 2997, `observe_v(.., 2)`) adds what v1 hides (evidence E20): calculated card numbers + count, affliction amount, replay count (`CARD_F` 15), displayed number per power, 64 candidates (v1 16), `dec_source` (what asked for the selection), `played` (the card being played). Selection: process-wide `observe::OBS_VERSION` (`sts2.set_obs_version`, default 1) is read by `observe` and by a `BatchEnv` / `SearchEngine` at creation (or `obs_version=`); `replay(_rows)`, `Sim.observe`, `layout`, `obs_size` take an optional version; checkpoints store `args["obs_version"]` and loaders honour it. v2 is for the next from-scratch run.

Observation window limits (simulation unaffected): 8 enemies, 16 powers per creature, 64 cards per pile list, 16/64 candidates (header carries the true count).

**Outcomes** (`sts2env::OUTCOME_*` = `sts2.OUTCOME_*`):
| code | name | reward |
|---|---|---|
| 1 | WIN | `win + hp_bonus * hp / max_hp` |
| -1 | LOSS (also `ov::LOOP`) | `loss` |
| 2 | TRUNCATED (`max_steps`) | step only |
| 3 | UNIMPLEMENTED (`Combat::missing`) | step only |
| 4 | OVERFLOW (`Combat::overflow`, not LOOP) | step only |
Codes 2-4 are truncations: bootstrap from the last state's value, never treat as win/loss.

**Capacities.** A full `ArrayVec` ignores the push and raises `util::raise_overflow`, folded into sticky `Combat::overflow` (`ov::CONTAINER, CARDS, CREATURES, HISTORY, COUNTER, SCENARIO, LOOP`); invalid scenarios return `Err(ScenarioError)` (release is `panic = "abort"`). Caps: card arena `MAX_CARDS` 160 (never recycled), deck at start `MAX_DECK` 80, hand 10 (game rule), creatures `MAX_CREATURES` 12, powers per creature `MAX_POWERS` 16, relics/potions/orbs 24/4/10 (`ScenarioError`), hook snapshot 256, per-attack results 16, candidates `MAX_PICK` 64, selected 16, history ring `HIST_CAP` 160 (`ov::HISTORY` only if a this/last-turn entry is overwritten), combat counters 65535. Corpus peaks: 143/160 cards, 13/16 powers, 8 creatures, 39 candidates.

**Reset in place.** `BatchEnv` resets finished episodes via `reset_validated` with allocation-free `ScenarioSource::pick`; reset == `new` bit for bit (also replayed vs the oracle with `STS2DIFF_REUSE=1`).

## Porting (was docs/porting-guide.md)
Done = differential replay vs the oracle shows 0 mismatches on scenarios exercising it. Always A10 (`ascension: 10`, `max_potion_slots: 2`, Ascender's Bane in the deck; A8/A9 monster values via `asc::val`). Never validate at A0 only.

**Layout.** `crates/sts2sim/src/content/{cards,powers,relics,potions,monsters,encounters,enchantments,afflictions}/*.rs`; new files auto-register (`build.rs`). Generated, never edit: `content/gen_*.rs`, `ids.rs` (regenerate after a game update: `scripts/porting/gen_defs.py`, `gen_ids.py`, `gen_relics.py`). Unregistered ids: rejected by `Scenario::validate`, flagged `Combat::missing` mid-fight.

**Writing content.**
- `listener!(ClassName { fn hook(...) {...} })`, exact C# class name; duplicates = build error; `listener!(Name {});` registers without hooks; `//` comments (not `///`) before the macro. Check: `grep -rho "^listener!(\w*" crates/sts2sim/src/content | sort | uniq -d` prints nothing.
- Hooks (`hooks.rs`, `Listener`): one trait method per C# `override` (incl. direct virtuals), single-variant signature; extra C# args via `cx.dmg_card` / `cx.dmg_result` / `cx.play_serial`; tiers (`*_early` / `*_late`) = separate passes. New hook: trait method + bit in `hookbit::bits!` + dispatch at the game's point (listener order, guarded or not); append at list ends.
- Cards: stats from `gen_cards.rs`; `on_play(cx, play, phase)`; suspend after `cx.ask_hand/ask_pile/ask_options`. Vars: `cx.card_var(card, VarKind::X)`, `cx.card_power_var`, named vars `card_named_var(card, var_name::SHIVS)`. Never hard-code a number present in generated tables or the decomp.
- Powers: stats in `gen_powers.rs`; implement only the C# overrides. Monsters: `pub static <SLUG>_DEF: MonsterDef` (see `content/monsters/nibbit.rs`) + optional listener. Encounters: `pub fn spawn_<slug>(rng, ascension) -> Spawns` (`content/encounters/basic.rs`). Event encounters: `monsters/event_only.rs`, `monsters/mysterious_knight.rs`, `encounters/events.rs`, `cards/event_pool.rs`, `cards/mad_science.rs`.
- `Dec` exactly where C# uses `decimal`. Prefer `engine/cmds.rs` helpers; add generic ones there.
- Hot path: no heap, no `Vec`/`String` in content, `ArrayVec`, no `dyn` beyond the listener table.

**Engine API** (read the helper before writing a new one).
- Death `cx.kill(_ex)`, `escape`, `heal`, max-HP helpers; preventers `should_die(_late)` + `after_preventing_death`. Monsters: `summon_enemy`, `stun`, `set_move_immediate`, `STUN_NODE` (read via `cx.move_view(c)`); enemy slots not recycled while a free one exists.
- Cards: `auto_play(..) -> RunResult`, `auto_play_from_draw_pile`, `auto_play_list`, `transform_cards`, `create_dupe`, `clone_card`, `x_value`, `engine/cost.rs`; per-card state `Card::counter` (`[i16; 2]`); per-instance defs via `cx.card_def(c)`, never `content::card_def(id)`.
- Auto-play: helper returns `RunResult::Suspended` -> `on_play` returns `Flow::Suspend(next)`, phase `next` = finished. Queues are a stack (`autoplay_stack`). Turn-start auto-play decisions resume via `dispatch_resumable` (`turn_cont` 4, 6-8; `susp` stack). Per-play power state: `hist.remember_play` / `take_play`.
- Draws: `cx.draw_cards(n, false)` and continue (Stratagem prompts are replayed).
- Monster-move decisions (Knowledge Demon curse): `ask_options`; on `Ask::Pending` set `cx.hook_ctx = Some((cx.monster_me(me), phase)); cx.stage = Stage::AwaitChoice;` and return; `resume_hook` finishes the move.
- History, never private counters: `cx.plays_this_turn`, `hist_count_this_turn`, `hist_total`, `hist_any_last_player_turn`, `hist_log.*`, `hist.*_finished_this_turn`. A new per-turn query of a counter-only kind must join `HKind::in_ring()`.
- Stars `energy.rs`, `engine/regent.rs`; Osty `engine/pets.rs`, redirect in `damage.rs`. Scenario extras: `Combat::new_with`, filled by `convert::scenario_ex` (sts2diff).
- `FromChooseACardScreen(canSkip: false)`: the oracle patches `canSkip` in (`P_ChooseACardSkip`) so `min` = 1 matches `ask_options(.., can_skip = false)`. Event cards overriding `VisualCardPool` are not colorless for `IsColorless` filters.
- One listener per class repo-wide (shared power files: `powers/artifact_minion.rs`, `vigor.rs`, `silent_a_shared.rs`, `silent_b.rs`, `ironclad_*`).

**Conventions.**
- Capacities never fail silently: push every element (no bare `.take(N)` / `min(CAP)`), or `util::raise_overflow(ov::..)`; fallible allocs (`new_card`, `add_enemy` -> `None`) flag first.
- Hot path: fill lists via out-params (`snapshot_into`, `damage_into`, `modify_*_into`); "nobody listens" test inline, body out of line (`dispatch_u/g`); `observe_ex` + `legal_actions_ex` (`can_play` once per card); whoever mutates `Creature::powers` calls `sync_secondary(c)`; cost mods via `CostMod::new`.
- Reset: `reset_validated` destructures every `Combat` field (a new uninitialised field = compile error). Card slots beyond `n_cards`, ring beyond `n` are never read.
- Performance contract (look-ahead cache keys, `clone_from` stale slots, checksums): `docs/solver.md`.

**Validation loop.**
1. Scenario JSON (oracle schema below), A10, with the ported entities.
2. `oracle/combat/oracle.sh run S.json --random SEED --out T.jsonl` (or scripted), then `cargo build -p sts2diff && target/debug/sts2diff run S.json T.jsonl` (`sts2diff dir DIR` for a directory). `ok` = every step matched (incl. all 9 RNG streams); `mismatch` prints the first differing field (`STS2DIFF_DUMP=N` dumps both snapshots at record N; `STS2DIFF_LENIENT=1` lets unported generated cards pass); exit 3 = unimplemented content hit.
3. Fix until clean over several seeds. The oracle trace beats your reading of the C#.
4. Non-obvious rule: freeze the scenario + trace into `oracle/regression/` (`NAME.scenario.json` + `NAME.jsonl`; replayed by `cargo test -p sts2diff --test regression`).
5. Fuzz (below), then `bash tools/gate.sh`.
6. Live: `python -m agent.fidelity_sweep` counts divergences between simulator prediction and the real game's visible state.

## Oracle (was docs/oracle.md)
`oracle/combat/`: a `dotnet` console app running the game's own combat code (`CombatManager`, hooks, cards, powers, monsters, RNG) on `ModelDb` / `RunState` / `Player`, no Godot runtime, no game file modified. Build/run: `oracle/combat/README.md`. The reference by construction; the TestMode audit argues the stubs change no behaviour.

**Stubs** (`GodotStub.cs`, `Patches.cs`, `Boot.cs`, `Pump.cs`): Godot native calls hit an mmap'd x86-64 no-op stub; Harmony patches for OS/Time/localization/preload/persistence; `TestMode.TurnOnInternal()`; game `Log.Error` = fatal oracle error (whitelist in `Boot.cs`); `LocalContext.NetId = 1`; a single-threaded `SynchronizationContext` pumps continuations FIFO; `ApplyAscensionEffects` only with `apply_ascension_effects: true`.

**TestMode audit** (only gameplay differences): Rolling Boulder damages all enemies in one batch (order among enemies may differ, per-enemy results equal); `FromChooseABundleScreen` takes `bundles[0]`; Slither `TestEnergyCostOverride` unused. No in-combat RNG draw is TestMode-gated.

**Scenario JSON.**
```jsonc
{ "name": "x", "ascension": 10, "encounter": "NIBBITS_WEAK", "character": "IRONCLAD",
  "hp": 80, "max_hp": 80, "max_energy": 3, "gold": 99, "max_potion_slots": 2, "base_orb_slots": 0,  // optional
  "seed": "1", "rng": {"shuffle": {"counter":9,"s0":0,"s1":0,"s2":0,"s3":0}},  // seed STRING; rng = optional full stream override
  "total_floor": 1, "act": 0,
  "deck": ["BASH+", {"id":"X","upgrade":1,"enchantment":{"id":"SHARP","amount":2},"props":{}}],  // ordered
  "relics": [{"id":"X","props":{"counter":3}}], "potions": [{"id":"X","slot":1}],  // relics in hook order; props = [SavedProperty]
  "apply_ascension_effects": false,
  "script": [{"play":{"hand_pos":2,"target":0}}, {"choose":[1]}, {"end_turn":true}, {"use_potion":{"slot":0,"target":0}}],
  "policy": {"kind":"random|playall|stall","seed":7,"endw":1,"atkw":1,"potw":1,"max_steps":400,"max_rounds":60} }
```
Omitted deck/relics/potions = starting inventory. Streams: `shuffle, combat_card_generation, combat_potion_generation, combat_card_selection, combat_energy_costs, combat_targets, monster_ai, niche, combat_orbs`; unlisted streams come from the seed.
- `play`: `hand_pos` in the current hand; `target` = index in dumped `enemies` (dead-but-listed included); `target_ally` = index in `[player, pets...]`. Runs `CardModel.TryManualPlay`; illegal = oracle error.
- `end_turn`: record written at the next play phase or combat end. `use_potion`: via `PotionModel.EnqueueManualUse`.
- `choose`: answers prompts in order, directly after the raising action (leading entries answer setup prompts); indexes the list as the game hands it to the selector (draw-pile prompts sorted by rarity then id). Too few / unused = error.
- Policy keys: `endw` (0 = never end turn while anything else is legal), `atkw` (<1 stalls), `potw`.

**Injection gotchas.** Injection = the game's save-load path (`Player.FromSerializable`, `SavedProperties.Fill`, `RunRngSet.GetRng(t).LoadFromSerializable`, `EnterRoomDebug`); deck order feeds the initial `UnstableShuffle`. The seed string matters even with explicit `rng` (encounter-local Rng = `Seed + TotalFloor + xxhash64(encounter)`). Pre-combat draws (niche HP rolls, initial monster_ai, shuffle n-1) precede record 0: give pre-combat stream states.

**Trace** (JSONL; schema in `Dump.cs`, the only place to add fields): record 0 = after setup and first turn start (`action: null`), then one record per action, at player decision points only: full visible state (player, pets, enemies with intents, hand, piles with draw top-first, orbs, relics with props/counter, potions), the prompts raised (`choices`), the history `log` since the last record (localises enemy-turn mismatches), and all 9 RNG streams `{counter,s0..s3}`.

**Determinism.** Same scenario = byte-identical trace; a random run = replay of its `--record` script; `check-shuffle`, `dump-rng` round-trip. The Windows build reproduces the Linux-recorded `oracle/regression` traces. Limits: one player, x86-64 only, build pinned by the referenced `sts2.dll`, props beyond ints/bools/strings not injectable.

**Fuzzing.** `tools/fuzz_gen_mix.py` generates realistic random A10 scenarios (any character, act-scaled picks, colorless/curses/statuses, upgrades, enchantments, 3-8 relics with injected counters, 0-2 potions, any implemented encounter, random policy), runs the oracle `batch` command per chunk and `sts2diff run` per trace. Verdicts: ok, mismatch, sim-error, oracle-error, unimplemented, arena-full; failures stay in `--out`. Flags: `--focus mix|colorless|junk|gen|turn|cross` (turn = turn-start auto-play/decisions; cross = another character's mechanic, `docs/research/game_code.md` B), `--cross P`, `--mode uniform|greedy|stall|deep` (deep: huge HP, long fights), `--enchant P`, `--each-potion`, `--each-relic`, `--force-relics/-potions/-cards`, `--character`, `--encounter`, `--act`, `--keep-ok`, `--gen-only`. Example: `python tools/fuzz_gen_mix.py --n 1500 --seed r3 --out target/fz/r3 --jobs 6`. Needs the catalog (`oracle.sh catalog`, auto when missing; copy in `data/catalog.json`). Do not rebuild the oracle or `sts2diff` mid-round (pass a copied binary via `STS2DIFF=`). Last full round: ~250,000 fights, 0 residual mismatches. Rerun after any content or engine change.

## Relics (was docs/relics.md)
`content/relics/*.rs`, one listener per class; constants from `content/gen_relics.rs` (`gen_relics::kunai::CARDS`; never hand-copy); relic-only powers `content/powers/relic_powers.rs`. Files group by trigger (`shared_start`, `shared_play`, `shared_cards`, `shared_damage`, `shared_misc`, `shared_choice` = decisions/auto-play, `pets_forge` + `monsters/relic_pets.rs`) plus character pools and generated `runlevel.rs` (hook-less run-level relics). Which relics the network observes: `relic_mask.rs` (generated by `tools/gen_relic_mask.py` from `data/relic_classes.json`).

**State** `Relic { counter: i32, flags: u8, aux: i32 }`: every private/saved C# field maps to `counter`/`aux` (ints) or a `flags` bit (bools); mapping documented above each listener. Static metadata (no dispatch): `meta_props()` = `[SavedProperty]` list (`relic_props![PropDef::int("TurnsSeen", Slot::Counter), PropDef::flag("WasUsed", 0)]`, `SaveIfNotTypeDefault`, `PropDef::constant`; the oracle injects/dumps props through it, `sts2diff` converts both ways); `meta_display()` = `ShowCounter ? DisplayAmount : none` (dumped as `counter`); `meta_initial()` = non-zero initialisers. `RelicInit` carries state into `Combat::new`; `Combat::rel(me)` / `rel_mut(me)`.

**Engine.** Relic-specific hook: `modify_gold_gained` (`Combat::gain_gold`). `Combat` fields: `gold` (default 99), `room_type` (0/1/2 from encounter suffix), `deck_upgradable`, `cur_power_card`, `auto_select`; `spend_resources`; helpers in `engine/relic_cmds.rs`. Decision hooks (Gambling Chip, Toasty Mittens, Toolbox, Choices Paradox): `ask_*`, on `Ask::Pending` set `hook_ctx` + `Stage::AwaitChoice`; `resume_after_decision` -> `resume_hook` -> `resume_turn_start`. History Course auto-plays via the stack. Whispering Earring sets `Combat::auto_select` (selections take the first `max` candidates, never suspend). Pets: Byrdpip / Pael's Legion via `Combat::add_pet` (9999-HP pet at `BeforeCombatStart`); Fencing Manual `Combat::forge`; Pael's Eye `should_take_extra_turn` + `before_side_turn_end_early`.

**Limits.** Paper Phrog / Paper Krane live in `VulnerablePower` / `WeakPower` (relic listeners empty). Monster-added statuses via `add_generated_card` count as "not created by the player" only during the enemy turn (Regalite).

## Colorless, curses, statuses, tokens (was docs/colorless.md)
Code: `content/cards/{colorless_a,colorless_b,curses_pool,status,tokens,curses}.rs`, `content/powers/colorless_basic.rs`, `engine/cardcmds.rs`, `engine/autoplay.rs`, `engine/potion_gen.rs`.
- Turn-end-in-hand cards implement `on_turn_end_in_hand`. Persistent per-card state in `Card::counter` (Regret hand size, Wither fake-upgrade level, Beat Down picks packed per byte, The Ball damage, Sovereign Blade damage + repeat override); instanced power state in `Power::aux` (Automation, Panache, The Bomb, Vigor); Calamity via the per-play table.
- Calculated damage (Rend, Mind Blast, Gold Axe, Gang Up) = `CalcBase + ExtraDamage * mult`, computed before `execute_attack` with the single target.
- Multiplayer-only cards: single-player behaviour; `AnyAlly` ones never playable solo.
- Entropy transforms in `resume_hook` in click order. Stratagem pauses in place only in the turn-start hand draw and Foregone Conclusion's shuffle; elsewhere replayed (`Combat::replay_prompt`).
- Known divergence: listeners after a suspending one in the same turn-start pass run before the answer (the game awaits): Entropy + a board-changing turn-start effect (Rolling Boulder) can diverge.
