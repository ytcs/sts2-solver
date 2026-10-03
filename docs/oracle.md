# Combat oracle (real game code, differential testing target)

Status: **Route A (in-process, no Godot runtime) works.** Code: `oracle/combat/`. Build/run: `oracle/combat/README.md`.

## 1. Feasibility study

### Route A: plain `dotnet` console app referencing `sts2.dll` + `GodotSharp.dll` (CHOSEN)

The game's combat logic (`CombatManager`, `ActionExecutor`, hooks, cards, powers, monsters, RNG) is pure C# on top of
`ModelDb`/`RunState`/`Player`. The Godot native engine is not needed as long as every call that would cross into the
engine is stubbed. What had to be done (all in `oracle/combat/`, no game file is modified):

| Problem | Fix |
|---|---|
| Any call into a Godot native method segfaults (null function pointer in `NativeFuncs`) | `GodotStub.cs`: mmap a 11-byte x86-64 stub (`mov rax, <self>; ret`) and feed `NativeFuncs.Initialize` a callback table where **every** entry points at it. Native calls become no-ops that return a non-null value (so method-bind lookups in static ctors succeed). |
| Godot singletons (`OS`, `Time`, `Engine`) cannot be materialised (needs a managed peer for the native object) | Harmony patches (`Patches.cs`): `OS.GetCmdlineArgs/HasFeature/GetExecutablePath`, `Time.GetTicksMsec/Usec`. |
| `TestMode` | `TestMode.TurnOnInternal()` (the game's own unit-test switch). It makes `NonInteractiveMode.IsActive` true so `Cmd.Wait`, `CustomScaledWait`, SFX, VFX, creature nodes are skipped. Audited divergences: see section 4. |
| `ModManager` not initialised -> `ModelDb.Init` throws | set `ModManager.State = Skipped` by reflection; `AssemblyInfo.Init()`. |
| `ModelDb.Init` -> `ModelDb.InitIds` needs `ModelIdSerializationCache.Init()` | called as in `OneTimeInitialization.ExecuteEssential` (sans atlas/localization/Godot loaders). |
| Logging uses `GD.Print` | `ConsoleLogPrinter.Print` patched to stderr; `Log.LogCallback` turns any game `Log.Error` into a fatal oracle error (fail fast; the game swallows exceptions in fire-and-forget tasks via `TaskHelper.RunSafely`). |
| Localization tables are in the `.pck` | `LocString.GetFormattedText/GetRawText` return `table.key`; `LocString.Exists` false. Text is never gameplay-relevant. |
| `PeerVersionInfo.LocalDefault()` pulls `PlatformUtil`/`ReleaseInfo` | patched to a constant. |
| `PreloadManager.LoadRoomCombatAssets` | patched to a completed task. |
| Post-combat persistence (`WriteReplay`, `SaveManager.SaveRun/UpdateProgressAfterCombatWon/SaveProgressFile`) | patched to no-ops. `run.Map = MockSinglePointActMap` (the game's test map). |
| `LocalContext.NetId` must be set or `StartTurn` silently skips `SetupPlayerTurn` (hand never drawn, turn loop hangs) | `LocalContext.NetId = 1`. |
| Determinism: in a console app `await` continuations run on arbitrary pool threads | `Pump.cs`: single-threaded `SynchronizationContext`; every continuation and `Task.Delay` wake-up runs FIFO on the main thread; the driver is "pump until the game waits for input". |
| `RunManager.ApplyAscensionEffects` (adds Ascender's Bane, removes a potion slot) | skipped unless scenario sets `apply_ascension_effects: true` (scenario deck/slots are authoritative). |

Route B (real headless game + mod, prior harness at `a1a341a:mods/DataDumper`) was **not needed**; it remains a fallback
if a future game build makes the stubbing approach infeasible.

Start-up cost: about 1.7 s (JIT + `ModelDb.Init`), a whole random fight adds ~0.1-0.7 s.


Robustness: `fuzz` over every encounter (85 ids, incl. event/boss encounters) x 5 characters with random decks/relics/potions
(about 1,900 random fights) ends with 0 oracle errors. The single failure class seen, `Tried to pop model ... (Mayhem
auto-play)`, is a benign bookkeeping `Log.Error` the game itself emits and is whitelisted in `Boot.cs`.

## 2. Scenario input (JSON)

```jsonc
{
  "name": "...",                       // free text
  "ascension": 0,                      // RunState.AscensionLevel (monster HP/damage via AscensionHelper)
  "encounter": "NIBBITS_WEAK",         // ModelId entry (ENCOUNTER.X also accepted)
  "character": "IRONCLAD",             // IRONCLAD|SILENT|DEFECT|NECROBINDER|REGENT
  "hp": 80, "max_hp": 80,              // default: character StartingHp
  "max_energy": 3, "gold": 99, "max_potion_slots": 3, "base_orb_slots": 0,   // optional, default = character values
  "seed": "1",                         // run seed STRING -> RunRngSet(seed): Seed = StringHelper.GetDeterministicHashCode(seed)
  "rng": { "shuffle": {"counter":9,"s0":..,"s1":..,"s2":..,"s3":..}, ... },  // optional per-stream FULL state override (u64 as number or string)
  "total_floor": 1,                    // floor number of this combat (1-based); feeds the encounter-local Rng seed
  "act": 0,                            // CurrentActIndex
  "deck":   ["STRIKE_IRONCLAD", "BASH+", {"id":"X","upgrade":1,"enchantment":{"id":"SHARP","amount":2},"props":{...}}],  // ORDERED
  "relics": ["BURNING_BLOOD", {"id":"X","props":{"counter":3}}],                // ORDERED (hook order); "props" = the relic's [SavedProperty] values
  "potions": ["FIRE_POTION", {"id":"X","slot":2}],
  "apply_ascension_effects": false,    // see gotchas
  "script": [ {"play":{"hand_pos":2,"target":0}}, {"choose":[1]}, {"end_turn":true}, {"use_potion":{"slot":0,"target":0}} ]
}
```
Omitting `deck`/`relics`/`potions` keeps the character's starting inventory. The names of the nine streams:
`shuffle, combat_card_generation, combat_potion_generation, combat_card_selection, combat_energy_costs, combat_targets,
monster_ai, niche, combat_orbs` (same snake_case as `RunRngType`). Streams not listed come from `RunRngSet(seed)`.

### Actions
* `play`: `hand_pos` = index in the CURRENT hand list; `target` = index in `enemies` (the list as dumped, dead enemies that are still
  listed included) for `AnyEnemy` cards; `target_ally` = index in the allies list (`[player, pets...]`) for `AnyAlly` cards; both
  null otherwise. Goes through the real path `CardModel.TryManualPlay` -> `PlayCardAction` (energy/hook checks included); an illegal
  play is an oracle error, not a no-op.
* `end_turn`: `EndPlayerTurnAction` (real UI path). The record is written when the player is next in the play phase (after the whole
  enemy turn and the next turn's draw/start effects) or when combat ended.
* `use_potion`: `slot` = potion slot index, optional `target`/`target_ally` as above (self-targeted potions: null) via `PotionModel.EnqueueManualUse`.
* `choose`: answers a card-selection prompt (Armaments, Headbutt, Burning Pact, Toolbox, ... anything that goes through
  `CardSelectCmd` + the game's `ICardSelector` hook). `choose` entries directly FOLLOW the action that raises the prompt and are
  consumed in order; `choose` entries at the very start of the script answer prompts raised during combat setup. `[i,...]` are indices into the
  option list exactly as the game hands it to the selector (note: for draw-pile prompts the game sorts options by rarity then id
  before handing them over; the options are echoed in the trace, so replay the picks, not card identities). Too few
  `choose` entries = oracle error with the prompt text; unused entries = error.
* Random driver (`--random S`): at each step picks uniformly among all legal actions (every playable (card, target) pair, every legal potion use,
  and `end_turn`); prompts are answered uniformly with a count in [min,max]. `--record` stores the resulting script.

## 3. Trace schema (JSONL, one object per step; snake_case)

Record 0 is the state after combat setup + first turn start (`action: null`); then one record per action. All fields are
present in every record.

| field | meaning |
|---|---|
| `step`, `action` | index, the action as in the script (`null` for record 0) |
| `choices` | prompts raised while executing the action: `[{min,max,options:[{id,upgrade,pile}],picked:[idx]}]` |
| `combat_in_progress`, `combat_over`, `result` | `result` ("win"/"loss") only when over. After the last action the state is post-victory (e.g. Burning Blood already healed, enemies list may be empty) |
| `round`, `side`, `turn`, `phase` | `CombatState.RoundNumber`, `CurrentSide`, player `PlayerCombatState.TurnNumber`, `PlayerTurnPhase` |
| `energy`, `max_energy`, `stars`, `gold` | |
| `player` | `{hp,max_hp,block,alive,powers:[{id,amount,(amount_on_turn_start)}]}` powers in the creature's list order |
| `pets` | creatures (Osty ...): `{id,combat_id,hp,max_hp,block,alive,powers}` |
| `enemies` | list order = `CombatState.Enemies`: `{id,combat_id,hp,max_hp,block,alive,powers,index,slot?,next_move,intents:[{type,damage,hits,total_damage}]}`; `damage`/`total_damage` are the game's own intent damage for the player (with current powers) |
| `hand`, | cards in order: `{id,upgrade,cost,keywords[],enchantment?,props?}` (`cost` = resolved energy cost now, -1 for X; `props` = card [SavedProperty] values) |
| `draw`, `discard`, `exhaust`, `play_pile` | `{id,upgrade}` in pile order; **draw is top first (index 0 = next card drawn)**, discard/exhaust in insertion order |
| `orbs`, `orb_capacity` | Defect orb queue `{id,passive,evoke}` front first |
| `relics` | in hook order: `{id,props?,counter?}` (`props` = relic [SavedProperty] values = its persistent counters/flags) |
| `potions`, `potion_slots` | `[{slot,id}]` |
| `rng` | the nine streams `{counter,s0,s1,s2,s3}` (u64 as JSON integers; xoshiro256** state words; `counter` is the game's call counter) |

`Dump.cs` is the single place to add fields (e.g. power internals, card dynamic vars).

## 4. Determinism and verification done

* Same scenario twice -> byte-identical trace; random run vs replay of its `--record`ed script (incl. `choose` entries) ->
  byte-identical; a fight run inside a 170-fight fuzz process vs the same scenario in a fresh process -> identical (28/28 replays with prompts).
* Opening state: `check-shuffle` confirms hand+draw == `UnstableShuffle(deck)` with `Rng(GetDeterministicHashCode(seed),"shuffle")` for the starter
  deck, seeds 1 and 2, with `shuffle.counter == deck.Count-1` (9) and `niche.counter == #enemies` (1) at record 0, i.e. nothing else drew before combat.
* Explicit RNG injection: feeding `dump-rng 1`'s nine streams back via `"rng"` reproduces the seed-only trace byte for byte.
* Sanity: Ironclad starter vs Nibbit: enemy HP 46 (A0), Butt 12 -> block 5 -> 7 damage, Burning Blood +6 after the kill, turn structure as in spec 01.
* NOT yet done: comparison against a real-game (Route B) trace. The oracle executes the game's own C# so it is the reference by construction,
  but the stub layer (below) could in principle change behaviour; the audit below is the argument that it does not.

## 5. Gotchas for faithful scenario injection

* **Injection path** = the game's own save-load code: `Player.FromSerializable(SerializablePlayer)` (cards: enchantment applied first, then
  `UpgradeInternal` x level; relics via `SavedProperties.Fill`; potions by slot), then `RunState.CreateForTest`,
  `RunRngSet.GetRng(t).LoadFromSerializable(...)`, `RunManager.SetUpTest`, `EnterRoomDebug(Monster, encounter.ToMutable())`.
  `Deck.Cards` order = scenario order = input to the initial `UnstableShuffle` (order matters!). `FloorAddedToDeck` is set to 1 unless given.
* **Run seed is a string**; `RunRngSet.Seed = GetDeterministicHashCode(seed)` (u64). Streams are created `Rng(Seed, snake_case(name))`. With explicit `rng`
  states the seed string still matters for the encounter-local Rng (`Seed + TotalFloor + xxhash64(encounter id)`, ~14 encounters draw from it) and
  for nothing else in combat. The old `total_floor` semantics: `EnterRoomDebug` appends the current floor to the map history before generating monsters,
  so `TotalFloor` seen by the encounter = `total_floor` (the oracle pre-appends `total_floor-1` dummy entries).
* **Ascension**: `ascension` sets monster HP/damage scaling. The ascension *player* effects (A4 potion slot -1, A5 Ascender's Bane in deck) are NOT applied
  unless `apply_ascension_effects: true`; the scenario deck/`max_potion_slots` are taken as authoritative.
* **Pre-combat consumption**: monster HP rolls draw `niche` (1+ per enemy, uniqueness retries), initial `monster_ai` rolls, `shuffle` n-1. All happen before record 0 and are
  already reflected in `rng` of record 0 (give the scenario the pre-combat stream states, not post-draw states).
* `LocalContext.NetId=1`, single player, `NetSingleplayerGameService`; multiplayer scaling is off.
* Game prefs are in-memory defaults (`FastMode=Instant`); `SaveManager.Progress` is NOT initialised (anything reading unlock/progress state would fail loudly; none seen).
* Localization is absent: any gameplay path that depended on string content would differ (none known; text only feeds UI).
* `Rng.Chaotic` (wall-clock) is only used for visuals (FakeMerchant lines etc.), never run streams.

### TestMode audit (the only place game behaviour differs from a normal client)
`TestMode.IsOn` is required to skip Godot nodes. Grepping all `TestMode.IsOn/IsOff` in gameplay code: all branches are VFX/SFX/animation/UI waits **except**
(1) `RollingBoulderPower.AfterPlayerTurnStart`: in test mode it damages all hittable enemies in one batch, the real game's VFX-driven version hits creatures as the boulder reaches them
(damage ORDER among enemies could differ for multi-enemy fights; per-enemy results are the same); (2) `CardSelectCmd.FromChooseABundleScreen` takes `bundles[0]` in test mode (not reachable by a scripted choice);
(3) `Slither` enchantment `TestEnergyCostOverride` (unused here). No in-combat RNG draw is gated on TestMode.

## 6. Limitations / next steps

* One player, Linux x86-64 only (the stub is raw x86-64 machine code), game build pinned by the `sts2.dll` it references (v0.111.0).
* Records are only taken at player decision points. Mid-enemy-turn states are not dumped; an optional event log (e.g. from `CombatManager.History`) could be added to `Dump.cs`
  to localise mismatches inside an enemy turn.
* Potion/relic/card persistent-state injection beyond ints/bools/strings (`SavedProperties` int arrays, cards-in-props) is not wired in `Scenario.cs`.
* Suggested follow-up: replay a handful of old real-game traces (git history `data/combat_traces`, older build) through scenarios to cross-check the stub layer end to end.

## 7. Randomized differential fuzzing (`tools/fuzz_gen.py`)

Per-entity sweeps validate one card / relic / monster at a time; the fuzzer validates their interactions. It generates realistic A10
Ironclad / Silent runs (starter deck + 5-25 additions from the character pool + colorless, upgrades ~40%, some enchanted cards, curses,
themed duplicates, 0-6 relics incl. "stress" relics that raise decisions / auto-play, relic counters, 0-2 potions, hp 50-90 or a "tank"
pool, every encounter incl. event encounters, per-act floors), runs them through the oracle with the `random` / `playall` / `stall`
policies in ONE process per job (`oracle.sh batch`, ~10 ms per fight) and diffs every trace against `sts2diff`.

```
python3 tools/fuzz_gen.py run --n 5000 --seed 1 --out /tmp/fz --jobs 6 [--relic-mode runlevel|many] [--force-relics A,B] [--force-cards X]
python3 tools/fuzz_gen.py triage --out /tmp/fz            # re-diff the failing scenarios kept under /tmp/fz/jobK (first differences)
python3 tools/fuzz_gen.py freeze --base /tmp/fz/job0/f1_17 --name my_regression --note "..."   # -> oracle/regression_scripted/my_regression.scenario.json
python3 tools/fuzz_gen.py regress                         # replay every oracle/regression_scripted/*.scenario.json (scripted, policy independent)
```
Verdicts: `ok`, `mismatch`, `sim-error` (Rust panic), `oracle-error` (the real game threw; e.g. Inky on a non-targeted card), `unimplemented`
(content / engine rule flagged as not ported: `Combat::missing`), `arena-full` (more than `MAX_CARDS` card instances in one fight).
Debug aids: `STS2DIFF_DUMP=1` (full Rust + oracle record of the first diverging step), `STS2DIFF_DUMP=all`, `STS2_TRACE=1` (debug builds:
every card play / draw / history entry of the Rust side), `tools/fuzz_relic_props.py` (relic saved properties used for counter injection).

Known residual classes (flagged as `unimplemented`, never silent):
* A decision raised inside a draw loop (Stratagem after a reshuffle, a Hellraiser auto-played prompt card) is resumable only in the turn-start
  hand draw and in effects that draw through `Combat::draw_cards_s` / `draw_then_done` (`PH_DRAW_TAIL`: Shrug It Off, Pommel Strike, Backflip,
  Acrobatics, Prepared, Dagger Throw, Battle Trance, Offering, Burning Pact, Drum of Battle, Finesse, ... and the draw potions). Everything
  else that can draw (hook-driven draws such as Dark Embrace / Feel No Pain, `AutoPlayFromDrawPile` of Havoc / Cascade, Expertise, Escape Plan,
  Pillage, Thinking Ahead, Calculated Gamble, the Swift enchantment, Clarity / Snecko Oil) flags `Combat::missing` when such a decision occurs.
  ~0.1% of fights of the fuzz distribution.
* Decision candidate lists are capped at `MAX_PICK` (64): a draw/discard pile larger than that (very long fights) is not fully selectable.
* A fight may create at most `MAX_CARDS` (160) card instances (a stalling policy for 50+ rounds).

## 8. Randomized differential fuzzing, mixed-deck / Regent / all-relic variant (`tools/fuzz_gen_mix.py`)

`tools/fuzz_gen_mix.py` generates realistic random A10 runs (any character, act-scaled decks with colorless / event / curse / status cards,
random upgrades and enchantments, 3-8 random relics with injected counters, 0-2 potions, any implemented encounter, a random driver policy),
runs them through the oracle's **`batch`** command (one process per chunk, ~50 ms per fight) and replays each trace with `sts2diff`:

```
python3 tools/fuzz_gen_mix.py --n 1500 --seed r3 --out /tmp/fz/r3 --jobs 6            # mixed round; non-ok scenarios stay in the dir
python3 tools/fuzz_gen_mix.py --n 300 --seed t --out /tmp/fz/t --focus turn --enchant 0.1   # turn-start auto-play / decisions (Mayhem, Imbued, Earring, ...)
python3 tools/fuzz_gen_mix.py --n 700 --seed p --out /tmp/fz/p --each-potion               # every potion, round robin (also --each-relic)
python3 tools/fuzz_triage.py /tmp/fz/r3          # re-run sts2diff on the kept scenarios (shows which a fix repaired)
python3 tools/fuzz_show.py /tmp/fz/r3/fm_r3_17 0 5   # compact oracle trace view;  STS2DIFF_DUMP=N prints the Rust/oracle snapshot of record N
python3 tools/fuzz_keep.py /tmp/fz/r3/fm_r3_17 NAME --upto 12 --note "..."   # promote a finding to oracle/regression/
```
Policies (`scenario.policy`, read by the oracle driver): `endw` (weight of `end_turn`; 0 = never end the turn while anything else is legal),
`atkw` (attack weight; <1 stalls), `potw`, `max_steps`, `max_rounds`; `deep` mode also gives the player 400-999 HP so fights reach turn 10+.
`oracle.sh catalog --out F` dumps pools / encounters (the generator input). Do not rebuild the oracle or `sts2diff` while a round is running
(copy the binary and pass it through `STS2DIFF`). `oracle/regression/*.{scenario.json,jsonl}` are oracle-recorded traces (possibly truncated)
that `crates/sts2diff/tests/regression.rs` replays on every `cargo test`.

Rules that fell out of the fuzzing (the commit messages name each): per-play power state lives in `hist.remember_play/take_play` (plays nest through
auto-play); every card-type/target query of a history entry uses `cx.card_def(e.card)` (Mad Science is per instance); an effect that auto-plays a card
must return `Suspend` or `Done` and the engine suspends the outer play while the nested one waits (`run_play_at`); a `Resolved` choice (Whispering
Earring's selector) must apply its result exactly like a resumed one; turn-start passes that auto-play (`AfterAutoPrePlayPhaseEntered*`) and
`AfterShuffle` are `dispatch_resumable`. The default arena is `MAX_CARDS` = 160 / `MAX_POWERS` = 16; an overflowing fight raises the sticky
overflow flag (`util::raise_overflow`, `ov::*`) and `sts2diff` reports it as an error, never a silent mismatch. Stratagem prompts (an
`AfterShuffle` decision raised inside a draw / shuffle) are resumable for: the turn-start hand draw, a plain draw that is the last thing a
card / potion effect does (`draw_cont`), Foregone Conclusion's own `BeforeHandDraw` shuffle (`hook_shuffle` / `hook_after`) and the
hand-empty draw at the very end of an outermost card play / potion (Unceasing Top: `hand_check`, via `draw_cont`).
Known gaps (flagged unimplemented): a draw started from inside another hook (Iteration's `AfterCardDrawn`, Centennial Puzzle ...), a draw that is
not the effect's last action (Battle Trance, Acrobatics, Prophesize, Bottled Potential ...) and `AutoPlayFromDrawPile` (Mayhem, Cascade, Havoc ...)
that reshuffle while Stratagem is active; traces of the last two kinds are kept in `oracle/regression_pending/`. The oracle trace has a `log` field per record (history entries since the previous record: nested plays `play*`,
draws, ...), and `sts2diff` compares keywords/enchantments of every pile and fails on oracle prompts the simulator never asked.
