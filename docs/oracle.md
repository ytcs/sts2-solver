# Combat oracle (real game code, differential testing target)

Code: `oracle/combat/` (build and run: `oracle/combat/README.md`). A plain `dotnet` console app references `sts2.dll` + `GodotSharp.dll` and runs the game's own combat logic (`CombatManager`, `ActionExecutor`, hooks, cards, powers, monsters, RNG) on top of `ModelDb` / `RunState` / `Player`. No game file is modified. The Godot engine is not needed as long as every call that crosses into it is stubbed.

The fallback, not needed so far, is a real headless game plus a mod.

## 1. Stub layer
| Problem | Fix |
|---|---|
| Any call into a Godot native method segfaults (null function pointer in `NativeFuncs`) | `GodotStub.cs`: mmap an 11-byte x86-64 stub (`mov rax, <self>; ret`) and feed `NativeFuncs.Initialize` a callback table where every entry points at it. Native calls become no-ops returning a non-null value, so method-bind lookups in static constructors succeed |
| Godot singletons (`OS`, `Time`, `Engine`) cannot be materialised | Harmony patches (`Patches.cs`): `OS.GetCmdlineArgs/HasFeature/GetExecutablePath`, `Time.GetTicksMsec/Usec` |
| `TestMode` | `TestMode.TurnOnInternal()` (the game's unit-test switch). `NonInteractiveMode.IsActive` becomes true so `Cmd.Wait`, `CustomScaledWait`, SFX, VFX and creature nodes are skipped. Audit below |
| `ModManager` not initialised, `ModelDb.Init` throws | `ModManager.State = Skipped` by reflection; `AssemblyInfo.Init()` |
| `ModelDb.InitIds` needs `ModelIdSerializationCache.Init()` | called as in `OneTimeInitialization.ExecuteEssential` (without atlas, localization and Godot loaders) |
| Logging uses `GD.Print` | `ConsoleLogPrinter.Print` patched to stderr; `Log.LogCallback` turns any game `Log.Error` into a fatal oracle error (the game swallows exceptions in fire-and-forget tasks via `TaskHelper.RunSafely`) |
| Localization tables are in the `.pck` | `LocString.GetFormattedText/GetRawText` return `table.key`; `LocString.Exists` is false. Text is never gameplay-relevant |
| `PeerVersionInfo.LocalDefault()` pulls `PlatformUtil` / `ReleaseInfo` | patched to a constant |
| `PreloadManager.LoadRoomCombatAssets` | patched to a completed task |
| Post-combat persistence (`WriteReplay`, `SaveManager.SaveRun/UpdateProgressAfterCombatWon/SaveProgressFile`) | patched to no-ops; `run.Map = MockSinglePointActMap` |
| `LocalContext.NetId` unset makes `StartTurn` skip `SetupPlayerTurn` (no draw, turn loop hangs) | `LocalContext.NetId = 1` |
| `await` continuations run on arbitrary pool threads in a console app | `Pump.cs`: single-threaded `SynchronizationContext`; every continuation and `Task.Delay` wake-up runs FIFO on the main thread; the driver pumps until the game waits for input |
| `RunManager.ApplyAscensionEffects` (adds Ascender's Bane, removes a potion slot) | skipped unless the scenario sets `apply_ascension_effects: true` (the scenario deck and slots are authoritative) |

Start-up costs about 1.7 s (JIT + `ModelDb.Init`); a random fight adds 0.1-0.7 s. A fuzz over every encounter (85 encounter ids at that run, incl. event and boss encounters) x 5 characters with random decks/relics/potions (about 1,900 fights) ends with 0 oracle errors. The one failure class seen, `Tried to pop model ... (Mayhem auto-play)`, is a benign bookkeeping `Log.Error` the game itself emits and is whitelisted in `Boot.cs`.

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
  "apply_ascension_effects": false,
  "script": [ {"play":{"hand_pos":2,"target":0}}, {"choose":[1]}, {"end_turn":true}, {"use_potion":{"slot":0,"target":0}} ],
  "policy": {"kind":"random|playall|stall","seed":7,"endw":1,"atkw":1,"potw":1,"max_steps":400,"max_rounds":60}   // batch mode without a script
}
```
Omitting `deck` / `relics` / `potions` keeps the character's starting inventory. Scenarios for the target use `ascension: 10`, 2 potion slots and Ascender's Bane appended to the deck. The nine streams are `shuffle, combat_card_generation, combat_potion_generation, combat_card_selection, combat_energy_costs, combat_targets, monster_ai, niche, combat_orbs` (snake_case of `RunRngType`); streams not listed come from `RunRngSet(seed)`.

### Actions
* `play`: `hand_pos` = index in the CURRENT hand; `target` = index in `enemies` (as dumped, dead-but-listed enemies included) for `AnyEnemy` cards; `target_ally` = index in `[player, pets...]` for `AnyAlly` cards; both null otherwise. Runs the real path `CardModel.TryManualPlay` -> `PlayCardAction` (energy and hook checks included); an illegal play is an oracle error.
* `end_turn`: `EndPlayerTurnAction`. The record is written when the player is next in the play phase (after the enemy turn and the next draw/start effects) or when combat ended.
* `use_potion`: `slot`, optional `target` / `target_ally`, via `PotionModel.EnqueueManualUse`.
* `choose`: answers a card-selection prompt (anything through `CardSelectCmd` + the game's `ICardSelector`). Entries directly FOLLOW the action that raises the prompt and are consumed in order; entries at the start of the script answer prompts raised during combat setup. `[i,...]` index the option list exactly as the game hands it to the selector (draw-pile prompts are sorted by rarity then id first; the options are echoed in the trace, so replay picks, not card identities). Too few or unused entries are oracle errors.
* Random driver (`--random S`): uniform over all legal actions (every playable (card, target), every legal potion use, `end_turn`); prompts are answered uniformly with a count in [min, max]. `--record` stores the resulting script.

## 3. Trace schema (JSONL, snake_case)
Record 0 is the state after combat setup and the first turn start (`action: null`), then one record per action. All fields are present in every record. `Dump.cs` is the one place to add fields.

| field | meaning |
|---|---|
| `step`, `action` | index; the action as in the script |
| `choices` | prompts raised while executing the action: `[{min,max,options:[{id,upgrade,pile}],picked:[idx]}]` |
| `log` | history entries since the previous record (nested plays `play*`, draws, ...) |
| `combat_in_progress`, `combat_over`, `result` | `result` ("win"/"loss") only when over; the last state is post-victory (Burning Blood already healed, enemies may be empty) |
| `round`, `side`, `turn`, `phase` | `CombatState.RoundNumber`, `CurrentSide`, `PlayerCombatState.TurnNumber`, `PlayerTurnPhase` |
| `energy`, `max_energy`, `stars`, `gold` | |
| `player` | `{hp,max_hp,block,alive,powers:[{id,amount,(amount_on_turn_start)}]}`, powers in list order |
| `pets` | Osty etc.: `{id,combat_id,hp,max_hp,block,alive,powers}` |
| `enemies` | `CombatState.Enemies` order: `{id,combat_id,hp,max_hp,block,alive,powers,index,slot?,next_move,intents:[{type,damage,hits,total_damage}]}`; damage values are the game's own intent damage for the player |
| `hand` | cards in order: `{id,upgrade,cost,keywords[],enchantment?,props?}` (`cost` = resolved now, -1 for X; `props` = card [SavedProperty] values) |
| `draw`, `discard`, `exhaust`, `play_pile` | `{id,upgrade}` in pile order; **draw is top first**, discard/exhaust in insertion order |
| `orbs`, `orb_capacity` | Defect orb queue `{id,passive,evoke}`, front first |
| `relics` | hook order: `{id,props?,counter?}` (`props` = [SavedProperty] values = persistent counters and flags) |
| `potions`, `potion_slots` | `[{slot,id}]` |
| `rng` | the nine streams `{counter,s0,s1,s2,s3}` (xoshiro256** words as JSON integers; `counter` = the game's call counter) |

## 4. Determinism checks
* The same scenario twice gives a byte-identical trace; a random run vs replay of its `--record`ed script (including `choose`) is byte-identical; a fight inside a 170-fight fuzz process equals the same scenario in a fresh process.
* `check-shuffle`: hand + draw == `UnstableShuffle(deck)` with `Rng(GetDeterministicHashCode(seed), "shuffle")`; at record 0 `shuffle.counter == deck.Count-1` and `niche.counter == #enemies`, i.e. nothing else drew before combat.
* `dump-rng 1` fed back through `"rng"` reproduces the seed-only trace byte for byte.
* Ironclad starter vs Nibbit: enemy HP 46 (A0), Butt 12 -> block 5 -> 7 damage, Burning Blood +6 after the kill.
* The oracle executes the game's own C#, so it is the reference by construction; the TestMode audit below is the argument that the stub layer does not change behaviour. A comparison against traces from the running game is not done.

## 5. Gotchas for faithful injection
* **Injection path** = the game's save-load code: `Player.FromSerializable(SerializablePlayer)` (cards: enchantment first, then `UpgradeInternal` x level; relics via `SavedProperties.Fill`; potions by slot), `RunState.CreateForTest`, `RunRngSet.GetRng(t).LoadFromSerializable(...)`, `RunManager.SetUpTest`, `EnterRoomDebug(Monster, encounter.ToMutable())`. `Deck.Cards` order = scenario order = input to the initial `UnstableShuffle`. `FloorAddedToDeck` is 1 unless given.
* **Run seed is a string**: `RunRngSet.Seed = GetDeterministicHashCode(seed)` (u64); streams are `Rng(Seed, snake_case(name))`. With explicit `rng` states the seed string still matters for the encounter-local Rng (`Seed + TotalFloor + xxhash64(encounter id)`, about 14 encounters draw from it). `EnterRoomDebug` appends the current floor to the map history before generating monsters, so the oracle pre-appends `total_floor-1` dummy entries to make the encounter see `TotalFloor = total_floor`.
* **Ascension** sets monster HP/damage scaling. Player effects (A4 potion slot -1, A5 Ascender's Bane) apply only with `apply_ascension_effects: true`.
* **Pre-combat RNG consumption**: monster HP rolls draw `niche` (1+ per enemy, uniqueness retries), initial `monster_ai` rolls, `shuffle` n-1. All happen before record 0; give a scenario the pre-combat stream states, not post-draw states.
* Single player, `LocalContext.NetId = 1`, `NetSingleplayerGameService`, multiplayer scaling off. Prefs are in-memory defaults (`FastMode=Instant`). `SaveManager.Progress` is NOT initialised: anything reading unlock/progress state would fail loudly (none seen). `Rng.Chaotic` (wall clock) only feeds visuals.

### TestMode audit
`TestMode.IsOn` is required to skip Godot nodes. Every `TestMode.IsOn/IsOff` in gameplay code gates VFX/SFX/animation/UI waits except:
1. `RollingBoulderPower.AfterPlayerTurnStart`: in test mode it damages all hittable enemies in one batch; the real game's VFX-driven version hits creatures as the boulder reaches them. Damage ORDER among enemies could differ in multi-enemy fights; per-enemy results are the same.
2. `CardSelectCmd.FromChooseABundleScreen` takes `bundles[0]` (not reachable by a scripted choice).
3. `Slither` enchantment `TestEnergyCostOverride` (unused).

No in-combat RNG draw is gated on TestMode.

## 6. Limits
* One player; Linux x86-64 only (the stub is raw x86-64 machine code); the game build is pinned by the `sts2.dll` it references (v0.111.0).
* Records are taken at player decision points only. Mid-enemy-turn states are not dumped (the `log` field localises mismatches inside an enemy turn).
* Persistent-state injection beyond ints/bools/strings (`SavedProperties` int arrays, cards-in-props) is not wired in `Scenario.cs`.
* Decision candidate lists are capped at `MAX_PICK` (64); a fight may create at most `MAX_CARDS` (160) card instances (a stalling policy for 50+ rounds). Both are flagged, never silent.

## 7. Randomized differential fuzzing
Per-entity sweeps validate one card, relic or monster at a time; the fuzzers validate their interactions. Each generator builds realistic random A10 scenarios (starter deck + act-scaled picks from the character pool and colorless, upgrades, enchantments, curses/statuses, relics with injected counters including stress relics that raise decisions or auto-play, 0-2 potions, any implemented encounter, a random driver policy), runs them through the oracle's `batch` command (one process per chunk, 10-50 ms per fight) and diffs every trace with `sts2diff`. Verdicts: `ok`, `mismatch`, `sim-error` (Rust panic), `oracle-error` (the real game threw), `unimplemented` (`Combat::missing`), `arena-full` (more than `MAX_CARDS` instances). Mismatching scenarios and traces stay in the output directory.

| tool | what |
|---|---|
| `tools/fuzz_gen.py` | `gen`, `run`, `triage` (re-diff failing scenarios), `rediff` (re-run only the diff phase after an oracle crash or a Rust fix), `freeze`, `regress`. Ironclad / Silent by default (`--characters`). Options `--relic-mode runlevel\|many`, `--each-card`, `--force-relics/-potions/-cards`, `--policy` |
| `tools/fuzz_gen_mix.py` | any character, act-scaled; `--focus mix\|colorless\|junk\|gen\|turn` (`turn` = turn-start auto-play and decisions: Mayhem, Imbued, Earring), `--mode uniform\|greedy\|stall\|deep` (`deep` gives 400-999 HP so fights reach turn 10+), `--enchant P`, `--each-potion`, `--each-relic`, `--keep-ok`, `--gen-only` |
| `tools/fuzz_gen_orb_pet.py` | Defect and Necrobinder (orbs, Osty) |

```
python3 tools/fuzz_gen.py run --n 5000 --seed 1 --out /tmp/fz --jobs 6 [--relic-mode runlevel|many] [--force-relics A,B] [--force-cards X]
python3 tools/fuzz_gen.py triage --out /tmp/fz
python3 tools/fuzz_gen.py freeze --base /tmp/fz/job0/f1_17 --name my_regression --note "..."   # -> oracle/regression_scripted/my_regression.scenario.json
python3 tools/fuzz_gen.py regress                       # replay every oracle/regression_scripted/*.scenario.json (scripted, policy independent)
python3 tools/fuzz_gen_mix.py --n 1500 --seed r3 --out /tmp/fz/r3 --jobs 6
python3 tools/fuzz_gen_mix.py --n 300 --seed t --out /tmp/fz/t --focus turn --enchant 0.1
python3 tools/fuzz_gen_mix.py --n 700 --seed p --out /tmp/fz/p --each-potion
```
Policy keys (`scenario.policy`): `kind` (`random|playall|stall`), `endw` (weight of `end_turn`; 0 = never end the turn while anything else is legal), `atkw` (attack weight; <1 stalls), `potw`, `max_steps`, `max_rounds`. `fuzz_gen.py` reads `tools/fuzz_relic_props.json` (relic saved properties for counter injection) and `tools/fuzz_pools.json` (pools, cached `dump-pools`). `oracle.sh catalog --out F` dumps pools and encounters (the generator input; a copy is in `data/catalog.json`).

Debug aids: `STS2DIFF_DUMP=N` prints the Rust and oracle snapshot of record N. `STS2DIFF=<path>` selects the `sts2diff` binary; copy the binary and pass it this way, and do not rebuild the oracle or `sts2diff` while a round runs. `oracle/regression/*.{scenario.json,jsonl}` are oracle-recorded traces (possibly truncated) that `crates/sts2diff/tests/` replays on every `cargo test`.

Residual classes are flagged (`unimplemented` or an error), never silent: decision candidate lists beyond `MAX_PICK` and fights beyond `MAX_CARDS` (Limits).

Conventions that came out of the fuzzing (per-play power state, `card_def` per instance, nested auto-plays, resumable passes, Stratagem replay) are in `docs/porting-guide.md`. `sts2diff` compares keywords and enchantments of every pile and fails on oracle prompts the simulator never asked.
