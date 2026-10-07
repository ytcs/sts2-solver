# Game-code research: enemy moves, cross-character cards, run generation

Read-only research against the decompiled game (`decomp/`), the Rust simulator (`crates/sts2sim`), the bridge
(`mods/AgentBridge`), fuzzers and the oracle. No code was changed.

**Citation convention.** `X/File.cs:N` = `decomp/MegaCrit.Sts2.Core.X/File.cs` line N (e.g. `Models.Monsters/Queen.cs:145`).
Repo files are given with their full path from the repo root. Rust paths without a crate are under `crates/sts2sim/src/`.

---

## A. Enemy move determination

**Scope (per the user's clarification).** The exact RNG state is hidden from the player. Future enemy moves are public
only as *pattern knowledge*: deterministic where the pattern is fixed, odds at random branches. Option (b) (realized
branch outcomes under the current RNG) is therefore **excluded**. The question is how completely and accurately the
simulator's multi-turn look-ahead represents the pattern.

### A1. When and how moves are rolled

- **One run-wide stream.** `RunRngSet` builds one `Rng` per `RunRngType` at run start:
  `new Rng(Seed + xxHash64(snake_case(name)))` (`Runs/RunRngSet.cs:117-121, 139-143`). `MonsterAi` is one of them
  (`Runs/RunRngSet.cs:77`). Every monster in every fight draws from this same stream (`Models/MonsterModel.cs:418`).
  It is not reset per combat.
- **Timing.**
  - The state machine is built in `SetUpForCombat` (`Models/MonsterModel.cs:410-414`).
  - Every enemy calls `PrepareForNextTurn` and rolls at the start of each player turn (`Combat/CombatManager.cs:739-744`,
    `Entities.Creatures/Creature.cs:547-553`). This happens after the start-of-turn hooks (`Combat/CombatManager.cs:721`).
  - A monster added mid-turn rolls immediately (`Combat/CombatManager.cs:1131-1133`).
  - Consequences:
    - Conditions are read after the player's previous turn and the enemy turn have resolved.
    - Nothing the player does while an intent is shown changes that intent, except code that forces a move (A2).
- **The roll** (`MonsterMoves.MonsterMoveStateMachine/MonsterMoveStateMachine.cs:54-80`):
  - If the current state cannot be left yet, or is the not-yet-performed opening move, no draw happens (`:60-63`).
  - Otherwise `GetNextState` is followed until a `MoveState` is reached. Only the first loggable state is appended to
    `StateLog` (`:73-78`).
- **Node types.** There are exactly four classes, all in `MonsterMoves.MonsterMoveStateMachine/`:
  - `MonsterState`: the base class (`MonsterState.cs:7-28`).
  - `MoveState`:
    - Fixed follow-up, no RNG (`MoveState.cs:67-70`).
    - "Must perform once before leaving" (`MoveState.cs:27-37`).
  - `ConditionalBranchState`: first branch whose predicate is true, no RNG (`ConditionalBranchState.cs:50-60`).
  - `RandomBranchState`:
    - Always exactly one draw, even if only one branch has positive weight (`RandomBranchState.cs:115-128`).
    - Repeat rules (`RandomBranchState.cs:130-167`, enum `MoveRepeatType.cs:3-9`):
      - UseOnce: weight 0 once the move is in the log.
      - CannotRepeat / repeat up to N: weight 0 if the last N logged moves are all this move.
      - Cooldown K: weight 0 if the move appears among the last K logged moves.

### A2. Is the future sequence a pure function of RNG + monster state?

**No.** It is a function of RNG, the monster's own state (log, counters, powers), and live combat state that the player
can change. The rest of this subsection lists where.

**Conditional branches reading live state** (all in `Models.Monsters/`):

| Monster | Predicate | Player influence | Cite |
|---|---|---|---|
| FrogKnight | own HP < 50% | damage pushes it into the other branch | `FrogKnight.cs:75-76` |
| LagavulinMatriarch | has `AsleepPower` | unblocked damage wakes it; otherwise Asleep counts down | `LagavulinMatriarch.cs:173-174` |
| SlumberingBeetle | has `SlumberPower` | same as LagavulinMatriarch | `SlumberingBeetle.cs:115-116` |
| BowlbugRock | off-balance (Imbalanced) | fully blocking its attack | `BowlbugRock.cs:76-77`, `Models.Powers/ImbalancedPower.cs:19-28` |
| LivingShield | ally count | killing allies | `LivingShield.cs:45-46` |
| Nibbit | alone / in front | killing the others | `Nibbit.cs:77-82` |
| Toadpole | in front | killing the front monster | `Toadpole.cs:79-80` |
| Queen | Amalgam dead | killing Amalgam | `Queen.cs:145-149` |
| Fabricator | can still fabricate (alive count) | killing bots | `Fabricator.cs:63-64` |
| Ovicopter | can still lay (alive count) | killing eggs | `Ovicopter.cs:71-72` |
| TestSubject | respawn count | killing it | `TestSubject.cs:210-211` |
| KnowledgeDemon | own curse counter | none (internal) | `KnowledgeDemon.cs:139-140` |

- Slot-based branches (Exoskeleton, Myte, PhantasmalGardener, Wriggler) branch on a fixed encounter slot. They are
  static, e.g. `Exoskeleton.cs:62-65`.
- The only state-dependent *weight* is TwoTailedRat's "can summon". It reads a turn countdown, the call count, free
  slots, and the other rats' *pending* moves (`TwoTailedRat.cs:125-128, 194-218`).

**Moves forced outside the machine.**

- **Stun.** A stun replaces the pending move and must be performed once (`Entities.Creatures/Creature.cs:525-544`).
  Triggers (powers in `Models.Powers/`):
  - `AsleepPower` and `SlumberPower`: unblocked damage (`Models.Powers/AsleepPower.cs:23-33`, `SlumberPower.cs:23-29`).
  - `BurrowedPower`: its block is broken (`BurrowedPower.cs:25-33`).
  - `FlutterPower`: powered attacks (`FlutterPower.cs:42-49`).
  - `PlowPower` and `ShriekPower`: HP thresholds (`PlowPower.cs:30-45`, `ShriekPower.cs:27-30`).
  - `RavenousPower`: an ally dies (`RavenousPower.cs:26-32`).
  - The Whistle card (`Models.Cards/Whistle.cs:33`).
- **Wake-up without the player.** `AsleepPower` decrements every enemy turn and calls `WakeUpMove` at 0
  (`Models.Powers/AsleepPower.cs:46-55`); `SlumberPower` does the same (`SlumberPower.cs:40-45`). This is predictable and
  needs no player input.
- **Direct `SetMoveImmediate` calls.**
  - Queen → Enrage when Amalgam dies while Burn Bright is pending (`Models.Monsters/Queen.cs:230-232`).
  - On death or knockout (monsters in `Models.Monsters/`, powers in `Models.Powers/`): TestSubject (`TestSubject.cs:171`), WaterfallGiant (`WaterfallGiant.cs:308`),
    `IllusionPower` (`Models.Powers/IllusionPower.cs:88`), `ReattachPower` (`ReattachPower.cs:62`).
  - ToughEgg after hatching (`Models.Monsters/ToughEgg.cs:139`).

### A3. Other consumers of the MonsterAi stream

- Fabricator picks which bot to spawn with `MonsterAi` (`Models.Monsters/Fabricator.cs:115`). The simulator mirrors this
  at `content/monsters/glory_a.rs:213`.
- `FlutterPower` passes the stream into `GetNextState` of the last logged move. That is a `MoveState`, so it draws
  nothing (`Models.Powers/FlutterPower.cs:48`, `MoveState.cs:67-70`).
- The stream is shared across all monsters and all fights of the run. Killing, stunning or summoning therefore changes
  which monster consumes which draw. That affects realized outcomes only, which are out of scope; the *pattern odds*
  are unaffected.

### A4. What the simulator exposes, and how faithful the look-ahead is

**Mechanism.**

- `look_paths` steps a copy of each monster's machine forward `LOOK_H = 3` turns past the shown intent and merges
  identical states (`engine/monster.rs:458-459, 715-742`).
  - Random nodes fork into every positive-weight branch with probability weight/sum. If every weight is 0, it takes the
    first branch, as the game does (`engine/monster.rs:516-535`).
  - Conditional nodes take the first true predicate (`engine/monster.rs:508-515`).
  - A pending stun follows its stored next move (`engine/monster.rs:546-548`).
  - It draws **no RNG**; a test enforces this (`crates/sts2sim/tests/lookahead.rs:56-72`).
- `lookahead` → per future turn: 16 move-slot probabilities plus expected attack damage (`engine/monster.rs:463-469, 785-826`).
- `Combat::intent_plan` → `(move, probability, text)`. Effects are measured by performing each move on a copy
  (`engine/monster.rs:569-625, 630-693`).
  - Exposed to Python at `crates/sts2py/src/sim.rs:285-303`.
  - Shown in live play at `agent/live.py:46-48, 227`.
- Observation ("expert look-ahead"): rows for up to 8 enemies (`observe.rs:17, 43-46, 423-437`). The ~98.7% coverage
  claim is at `docs/env-api.md:50-51`.
- **Result: per-monster pattern odds, not the realized sequence.** This is the right object under the clarification.

**Simulator RNG.**

- `rng.rs` is a bit-exact port of the game RNG: xoshiro256**, splitmix seeding, xxh64 stream names, and a draw counter
  (`rng.rs:1-5, 85-115`).
- The monster stream is `Rng::named(seed, "monster_ai")` (`state.rs:306`), drawn in `next_state`
  (`engine/monster.rs:293`).
- Live play has no seed. The replayer resamples seeds until the simulated intents match the game
  (`agent/fight.py:57-75, 120-154`). The machine state is therefore exact, and the RNG is a consistent sample.

**Fidelity checklist by node type.**

| Pattern feature | Represented? | Notes / cite |
|---|---|---|
| Fixed cycles (`MoveState` follow-ups) | **Exact** | Nibbit is tested (`crates/sts2sim/tests/lookahead.rs:38-53`). |
| Random weights | **Exact** for static weights | Fogmog 0.4/0.6 (`content/monsters/fogmog.rs:37` vs `Models.Monsters/Fogmog.cs:45-51`). |
| CannotRepeat / max-repeat / cooldown / use-once | **Exact** | Evaluated on the *projected* move log (`engine/monster.rs:221-265, 495-499`). The log keeps 8 entries (`:163`); no content needs more than 3. |
| Conditional branches on live state | **Partial** | Predicates read the *current* combat, not the projected one (`engine/monster.rs:510`; the comment at `:455-456` says so). |
| State-dependent weights | **Partial** | Weights see only the projected log (`engine/monster.rs:221-222, 260-263`). |
| Own counters / powers ticking over the horizon | **Missing** | E.g. Asleep countdown, TwoTailedRat countdown. |
| Forced moves (stun, wake, death, Enrage) | **Missing** beyond a stun already pending | `engine/monster.rs:546-548` handles only the current stun. |
| Summoned monsters | **Missing** until they exist | No rows for future spawns. |
| Multi-monster joint distribution | **Missing** | Each monster is projected independently. |
| Damage with buffs / debuffs gained over the horizon | **Missing** | Uses today's modifiers (`engine/monster.rs:463-464, 696-711`). |

Spot checks where the decomp and the simulator agree exactly:

- FrogKnight: `content/monsters/glory_a.rs:357-363` vs `FrogKnight.cs:74-80`.
- KnowledgeDemon: `content/monsters/hive_b.rs:423-461` vs `KnowledgeDemon.cs:130-146`.
- TwoTailedRat (1/12 vs 0.75, Screech cooldown 3): `content/monsters/underdocks_a.rs:374-389, 421-438` vs `TwoTailedRat.cs:124-128, 194-218`.
- Queen: `content/monsters/glory_b.rs:269-305` vs `Queen.cs:143-152`.
- LagavulinMatriarch: `content/monsters/underdocks_b.rs:205-208` vs `LagavulinMatriarch.cs:168-174`.
- BowlbugRock: `content/monsters/hive_a.rs:72-93` vs `BowlbugRock.cs:72-77`.

**Concrete shortfalls.**

1. **Unprojected self-state gives probability-0 misses even with no player input.**
   - *LagavulinMatriarch / SlumberingBeetle.* Asleep counts down each enemy turn and wakes the monster at 0
     (`Models.Powers/AsleepPower.cs:46-55`). The look-ahead sees Asleep and predicts Sleep for all of +1..+3. With
     Asleep 3 at turn 1, the real move at +3 is Slash.
   - *TwoTailedRat.* The summon countdown advances only when a move is performed
     (`content/monsters/underdocks_a.rs:390-392`). The projection is one tick behind even at +1: with one tick left and
     Scratch pending, the real +1 summon chance is 0.75 and the look-ahead says 0. Reaching the call cap, and other
     rats' calls, are not projected either.
   - *Ovicopter / Fabricator.* The "can lay / fabricate" checks count living enemies
     (`content/monsters/hive_a.rs:358-360`, `content/monsters/glory_a.rs:197-199`). Spawns inside the horizon are not
     counted.
   - *KnowledgeDemon* is right only by luck: its counter changes 4 steps from its branch, which is beyond `LOOK_H`.
2. **Player-controllable branches are shown as status quo.**
   - FrogKnight's half-HP branch uses current HP.
   - Queen, LivingShield, Nibbit and Toadpole assume the current line-up survives.
   - Nothing represents "if I do X". The all-branches superset in `bound_enter` (`engine/monster.rs:838-875`) is the
     closest thing.
3. **Expected damage ignores horizon buffs.** Strength from the monster's own buff moves (FrogKnight For the Queen +5,
   Fogmog Swipe +1, KnowledgeDemon Ponder) is ignored, and so is Weak or Vulnerable expiring
   (`engine/monster.rs:463-464, 696-711`). Non-attack effects appear only as the move identity, or as `intent_plan`
   text measured on today's state.
4. **No joint projection.** A predicate that reads another monster sees its current state. For example, TwoTailedRat
   reads the other rats' pending move (`content/monsters/underdocks_a.rs:382`).
5. **Truncation and slots.**
   - The horizon is 3 turns.
   - Paths are capped at 48 and the rest are silently dropped (`engine/monster.rs:501`), with merging only afterwards
     (`:724-732`). Rows are only tested to sum to ≤ 1 (`crates/sts2sim/tests/lookahead.rs:95-96`).
   - The stun shares slot 15 with move node 15 (`engine/monster.rs:816`).
   - Enemies past the 8th get no rows.
6. **Possible stale cache.** The key covers other enemies' id, slot, alive flag and HP, but not their pending move
   (`engine/monster.rs:767-771`). TwoTailedRat's weight reads that pending move. The cache test only covers the training
   mix (`crates/sts2env/tests/lookahead_cache.rs:1-43`).
7. **The test measures coverage, not calibration.** `realized_moves_were_predicted` uses random play with 400 HP, skips
   stunned monsters, and allows 3% misses (`crates/sts2sim/tests/lookahead.rs:126-188`). The Lagavulin and rat misses
   above count against that 3%.
8. **The observation lacks the pending node.** It has intent types plus the last 4 performed moves (`observe.rs:311-343`),
   but not the current machine node or the stored follow-up.

### A5. What the bridge exports

- Per enemy: the `next_move` id and intents with damage and hits (`mods/AgentBridge/src/Snap.cs:200-229`).
- The seed is a deliberate placeholder (`mods/AgentBridge/src/Snap.cs:78, 322`).
- The game's `Rng` is serialisable and clonable (`Random/Rng.cs:32-36, 356-364`). The bridge *could* export it, or
  pre-roll N moves on a cloned machine. That would be option (b), which is hidden information and excluded by the
  clarification.
- Nothing extra is needed from the bridge for pattern knowledge. The simulator reconstructs the machine state from move
  history.

### A6. Recommendation: representing future enemy actions as public info

- **(a) Deterministic futures for fixed patterns.** Already exact. Keep `lookahead` / `intent_plan`.
- **(b) Realized branch outcomes.** Excluded: the RNG is hidden. The current design already avoids this, and a test
  enforces it.
- **(c) Distribution: keep per-monster pattern odds, and fix what is predictable without RNG.**
  1. **Project the monster's own state per path.** Tick Asleep and Slumber, TwoTailedRat's countdown and call count, and
     KnowledgeDemon's counter. Apply the projected move's own-side effects (Strength or block on itself, summons into
     the alive count) before evaluating later predicates and damage. This removes the probability-0 misses.
  2. **Project the encounter jointly**, or at least feed projected allies into predicates that read them.
  3. **Flag player-controllable branches** instead of silently assuming the status quo. For each monster, emit a small
     set of "what-if" rows (HP below a threshold, woken, ally dead, attack fully blocked), using `bound_enter`'s support
     as the outer bound. A predictor then sees both the status-quo distribution and what the player can force.
  4. **Add the current node and the stored follow-up to the observation.** This makes stun and death recovery
     inferable.
  5. **Add other enemies' pending moves to the look-ahead cache key.**
  6. **Upgrade the test to measure calibration**: binned predicted vs realized frequency, including probability-0 misses.
  7. **Consider `LOOK_H` 4-5.** KnowledgeDemon's counter-driven branch is 4 steps out (`Models.Monsters/KnowledgeDemon.cs:130-146`),
     beyond today's horizon. Pair it with a path cap that merges before truncating (`engine/monster.rs:501, 724-732`).

---

## B. Cross-character mechanics

**Bottom line.**

- The game has exactly **one** character-dependent combat rule: Channel with no orb slots.
- Stars, Osty/Summon, Doom, Focus and Shiv/Sly are all character-agnostic.
- The simulator matches every path on reading.
- The gap is **verification**: no oracle diff covers deck cards from another character's pool on the wrong character,
  yet the RL curriculum trains on exactly those decks.

### B1. Orb cards on a non-Defect

**Game:**

- Base slots: `CharacterModel.BaseOrbSlotCount => 0` (`Models/CharacterModel.cs:101`). Only Defect overrides it, to 3
  (`Models.Characters/Defect.cs:63`). It is copied to the player at run start (`Entities.Players/Player.cs:266, 307`).
- Combat start: `OrbQueue.Clear(); AddCapacity(BaseOrbSlotCount)` (`Entities.Players/PlayerCombatState.cs:137-139`).
  The cap is 10 (`Entities.Orbs/OrbQueue.cs:15`).
- `OrbCmd.Channel` (`Commands/OrbCmd.cs:68-92`):
  - If the *character constant* is 0 and capacity is 0, it first adds 1 slot (`:74-77`).
  - If full, it evokes the front orb (`:80-83`).
  - `TryEnqueue` fails at capacity 0, so the orb is lost with no history entry and no hook (`Entities.Orbs/OrbQueue.cs:52-55`).
- What that means in play:
  - Ironclad: the first Channel creates 1 slot. Each later Channel evokes and replaces the orb.
  - A Defect reduced to 0 slots (e.g. Bulk Up's `RemoveSlots`, `Models.Cards/BulkUp.cs:37`) **loses** channeled orbs.
  - A non-Defect taken back to 0 slots gets 1 slot again on the next Channel.
- Other orb effects:
  - Capacitor and Potion of Capacity add slots (`Models.Cards/Capacitor.cs:22`, `Models.Potions/PotionOfCapacity.cs:28`).
  - Dualcast does nothing with no orbs (`Models.Cards/Dualcast.cs:23`).
  - Essence of Darkness channels one Dark per slot, so 0 on a fresh non-Defect (`Models.Potions/EssenceOfDarkness.cs:32-36`).
- Orb passives fire for every player (`Combat/CombatManager.cs:792, 1604`).
- Focus checks only the owner (`Models.Powers/FocusPower.cs:14-21`).
- Death clears the queue and zeroes capacity (`Commands/CreatureCmd.cs:581`, `Entities.Orbs/OrbQueue.cs:30-34`).

**Simulator:** matches.

- `channel` (`engine/orbs.rs:142-162`): lazy slot at `:146-148` via `character_base_orb_slots()`, which is 3 only for
  character 2 (`engine/orbs.rs:129-131`); evoke when full at `:149-151`; orb lost at 0 capacity at `:152-155`.
- Slot add and remove: `engine/orbs.rs:106-126`. Focus: `content/powers/defect.rs:14-21`. Essence of Darkness:
  `content/potions/defect.rs:11`. Death: `engine/death.rs:95`.
- Starting slots come from the scenario (`scenario.rs:368`).

### B2. Regent star-cost cards without Stars / Divine Right

**Game:**

- Stars are a plain int on every `PlayerCombatState` (`Entities.Players/PlayerCombatState.cs:23, 103-119`).
- Gain and loss clamp at 0 (`:211-227`). `PlayerCmd.GainStars` checks `ShouldGainStars` and then fires `AfterStarsGained`
  (`Commands/PlayerCmd.cs:90-97`).
- Stars are not reset each turn. The only `SetStars(0)` is a multiplayer death (`Combat/CombatManager.cs:1232`).
- Insufficient stars → `StarCostTooHigh` → unplayable (`Entities.Players/PlayerCombatState.cs:190-209`,
  `Models/CardModel.cs:1738`).
- Non-Regents gain stars from Venerate, Glow, Gather Light, Star Potion etc. with no character check.
- Divine Right is a relic giving +3 stars on entering combat (`Models.Relics/DivineRight.cs:16-21`).
- Regent's only character flag is a UI one (`Models.Characters/Regent.cs:36`). Sovereign Blade's check is only an
  animation (`Models.Cards/SovereignBlade.cs:124-125`).

**Simulator:** matches with no character checks.

- Stars: `engine/energy.rs:96-136`. Star costs: `engine/energy.rs:144-231`.
- Playability: `engine/play.rs:80-88`.
- Stars start at 0 (`scenario.rs:356`). Divine Right: `content/relics/shared_misc.rs:21-23`. Forge: `engine/regent.rs:40-54`.

### B3. Necrobinder Osty / Summon / Doom without Osty

**Game:**

- `OstyCmd.Summon` works for anyone (`Commands/OstyCmd.cs:35-91`):
  - alive Osty → gains max HP (`:48-51`);
  - dead Osty still in allies → revived (`:54-62`);
  - otherwise a new Osty pet is created with `DieForYouPower` (`:65-74`).
- Necrobinder gets Osty only from Bound Phylactery (`Models.Relics/BoundPhylactery.cs:22-42`). So an Ironclad playing
  Bodyguard gets a working Osty.
- Osty attacks with Osty missing stay playable but do nothing (`Models.Cards/Poke.cs:29-35`,
  `Models.Monsters/Osty.cs:74-78`).
  - Bone Shards' block and kill are both inside the Osty branch (`Models.Cards/BoneShards.cs:33-43`).
  - Snap still asks for a Retain choice (`Models.Cards/Snap.cs:40-44`).
  - Only High Five is unplayable without Osty (`Models.Cards/HighFive.cs:18`).
- Doom is generic and side-based (`Models.Powers/DoomPower.cs:60-73, 146`).

**Simulator:** matches.

- Osty and summon: `engine/pets.rs:17-34, 69-105`.
- Osty cards (`content/cards/necrobinder_osty.rs`): Poke `:124-133`, Snap `:136-150`, High Five `:244-247`, Bone Shards `:262-276`.
- Doom: `content/powers/necrobinder.rs:229` onward.

### B4. Silent Shiv / Sly, and generators

- Shiv's only character check is an animation (`Models.Cards/Shiv.cs:71-74`).
- **Generators use the owner's pool, not the card's pool.** `Owner.Character.CardPool` drives Infernal Blade
  (`Models.Cards/InfernalBlade.cs:22`), White Noise (`Models.Cards/WhiteNoise.cs:23`), Distraction, Discovery, Jackpot,
  Metamorphosis, Calamity, Creative AI, and the Attack/Skill/Power potions.
  - So White Noise on an Ironclad yields an Ironclad Power.
  - Splash takes from the *other* pools (`Models.Cards/Splash.cs:28`).
- The simulator matches through `character_pool()` (`engine/cmds.rs:25-34`; e.g. `content/cards/ironclad_b1.rs:33`) and
  Splash (`content/cards/colorless_a.rs:732-743`).
- One comment is stale: `content/cards/defect_skills.rs:231` says "Defect pool", but the code calls `character_pool()`.
- The design spec agrees that `BaseOrbSlotCount` is the only per-character combat field
  (`docs/spec/05-characters-and-inputs.md:247, 291-292`).

### B5. Fuzzer / oracle coverage of cards from other characters

- **`tools/fuzz_gen.py`**
  - Ironclad and Silent only (`:23-27`). Own pool plus colorless (`:142-158`). Other characters' relics are filtered out
    (`:85-87`). `base_orb_slots` is hard-coded to 0 (`:212`).
  - Other characters' cards appear only via `--force-cards` (`:173-175, 412`).
- **`tools/fuzz_gen_mix.py`**
  - Five characters, with correct orb slots (Defect 3, `:22-28, 254`).
  - The deck is own pool, colorless, event, curse, status **plus token** (`:111-131`). So Shiv, Sovereign Blade,
    Minion* and Sweeping Gaze land on any character.
  - About 7% of relic draws come from another character (`:169, 177`).
  - The starter relic is dropped 15% of the time (`:186`), which tests Regent without Divine Right and Necrobinder
    without Phylactery.
  - Potions are own plus shared only (`:203`), so orb and summon potions never reach other characters.
  - Splash is in `gen_ids` (`:101`).
  - **No deck cards from another character's pool.**
- **`tools/fuzz_gen_orb_pet.py`**: Defect and Necrobinder, own pool only (`:83-112`).
- **oracle `fuzz`** (`oracle/combat/Fuzz.cs:17-66`): own pool plus colorless, and it writes **no `base_orb_slots`**.
- **Oracle-checked cases from other characters:**
  - `oracle/templates/engine_sly.json`: Silent Sly cards on Ironclad.
  - `oracle/templates/engine_void_form.json`, `engine_cards_a.json`: Regent cards on Ironclad.
  - `oracle/templates/cc/t_tokens.json`: every token on Ironclad.
  - `oracle/regression/thrash_exhausts_osty_attack`: an Ironclad Splash-generated a Necrobinder Osty attack, and a real
    divergence was fixed.
- The diff compares `orbs`, `orb_capacity` and `stars` (`crates/sts2diff/src/snapshot.rs:164, 188-198`).
- None of the 506 scenario JSONs or 59 regression traces puts a Defect orb card or an Osty-pool card in another
  character's deck. This is weak evidence on its own, because clean fuzz runs are deleted (`tools/fuzz_gen_mix.py:11`).

### B6. Fidelity risks

1. **RL curriculum trains on unverified decks (main risk).** In 30% of non-easy fights, `tools/gen_curriculum.py:128-132`
   adds 1-3 cards from another character's pool. `tools/certify_fights.py` (docstring `:6`) treats the simulator as
   ground truth.
   - Example decks: Ironclad + Zap/Glacier, Silent + Bodyguard/Poke, Defect + star cards.
   - None of these has been diffed against the oracle.
   - **Fix:** add a weight for cards from other characters to `fuzz_gen_mix.make_deck` that mirrors the curriculum draw.
2. **Missing `base_orb_slots` defaults disagree.**
   - The simulator converter uses `unwrap_or(0)` (`crates/sts2diff/src/convert.rs:115`), and Python uses the same path
     (`crates/sts2py/src/lib.rs:21`).
   - The oracle keeps the character's own value, 3 for Defect (`oracle/combat/Setup.cs:60, 68`).
   - Defect scenarios from oracle `fuzz` (`oracle/combat/Fuzz.cs:53-66`) therefore start the simulator at 0 slots
     against the game's 3.
3. **Paths that match on reading but have never been diffed:**
   - lazy 1-slot Channel on non-Defects, including slots regained after Bulk Up;
   - a Defect at 0 slots losing the orb;
   - Capacitor, Focus and Essence of Darkness on non-Defects (these potions never reach other characters in fuzzing);
   - Summon creating Osty for a non-Necrobinder;
   - star gain plus star-cost cards on non-Regents (only Minion-token star costs are partly covered);
   - owner-pool generation from another character's generator card.
4. **Minor edge cases:**
   - On a re-entrant full queue the game throws (`Entities.Orbs/OrbQueue.cs:57-59`), while the simulator silently drops
     the orb (`engine/orbs.rs:152-155`).
   - The simulator's `summon` has no `source` argument. This is harmless because nothing overrides `ModifySummonAmount`.

For how cards from other characters enter a deck at run level, see C2 (the authoritative list).

---

## C. Run-level generation for a macro simulator

### C0. RNG streams and seeding

- **Generator.** `MegaRandom` is xoshiro256** seeded through splitmix64 (`Random/MegaRandom.cs:48-66, 168-187`).
  - `NextFloat(min,max)` = `(float)(NextDouble*(max-min)+min)` (`Random/Rng.cs:170-177`).
  - `NextItem` = `NextInt(0,n)` (`Random/Rng.cs:289-298`).
  - `UnstableShuffle` is a back-to-front Fisher-Yates (`Extensions/ListExtensions.cs:45-59`). `StableShuffle` sorts
    first, then shuffles (`:22-31`).
- **Run seed.** `xxHash64(utf8 seed)` (`Runs/RunRngSet.cs:115`). Named streams: `Seed + xxHash64(snake_case(name))`
  (`:139-143`).
- **The 12 run streams** (`Entities.Rngs/RunRngType.cs`): up_front, shuffle, unknown_map_point, combat_card_generation,
  combat_potion_generation, combat_card_selection, combat_energy_costs, combat_targets, monster_ai, niche, combat_orbs,
  treasure_room_relics.
- **Per-player streams.** Player seed = `hash(seed) + slot` (`Entities.Players/Player.cs:330`). Streams: rewards, shops,
  transformations (`Random/PlayerRngSet.cs:19-46`). Card-rarity and potion odds both use `Rewards`
  (`Odds/PlayerOddsSet.cs:22-26`).
- **Ad-hoc streams:**
  - Map: `"act_{i+1}_map"` (`Map/StandardActMap.cs:113`).
  - Events: `Seed + (IsShared ? 0 : slot) + hash(id)` (`Models/EventModel.cs:234`).
  - Act selection: `"act_selection"` (`Multiplayer.Game.Lobby/StartRunLobby.cs:471`).
- **Up-front draw order.** The shared relic bag, then each player bag (`Runs/RunManager.cs:522-526`), then
  `GenerateRooms` (`Runs/RunManager.cs:743-766`, `Models/ActModel.cs:331-386`).
- **Already ported:** `crates/sts2sim/src/rng.rs` is bit-exact.

### C1. Map generation (seed-exact: hard; distribution-only: medium)

**Classes:** `StandardActMap`, `MapPointTypeCounts`, `MapPathPruning`, `MapPostProcessing`. Golden Compass uses
`GoldenPathActMap` (`Models.Relics/GoldenCompass.cs:39-46`).

**Size and paths**
- Grid is 7 columns × (rooms+1) rows (`Map/StandardActMap.cs:89-91`).
- Rooms: Overgrowth 15, Underdocks 15, Hive 14, Glory 13 (`Models.Acts/Overgrowth.cs:47`, `Underdocks.cs:42`,
  `Hive.cs:47`, `Glory.cs:43`).
- The ancient is at row 0, the boss at the end, and A10 adds a second boss (`Map/StandardActMap.cs:94-99, 226-229`).
- 7 walks from row 1 using shuffled steps of {-1, 0, +1} that never cross an existing edge (`:145-221`).

**Room counts**
- Rests: Gaussian(7,1) clamped to [6,7] for Overgrowth and Underdocks, Gaussian(6,1) clamped to [6,7] for Hive,
  `NextInt(5,7)` for Glory (`Models.Acts/Overgrowth.cs:136`, `Hive.cs:119`, `Glory.cs:108`).
- Unknowns: Gaussian(12,1) clamped to [10,14] (`Map/MapPointTypeCounts.cs:516-519`), minus 1 in Hive and Glory.
- Shops 3. Elites 5, or 8 at A1 (`Map/MapPointTypeCounts.cs:500-502`).

**Fixed rows** (`Map/StandardActMap.cs:248-275`)
- Row 1 Monster, Treasure at rooms+1−7, Rest on the last room row.

**Placement**
- The rest are placed over 3 passes on shuffled nodes (`:276-300, 364-385`).
- Rules: no Rest or Elite below row 6, no repeat of Elite/Rest/Treasure/Shop from parent to child, no siblings of the
  same type (`:405-485`).
- Then duplicate-path pruning with repair (`Map/MapPathPruning.cs:18-97, 254-258`), and centring and straightening.

**Port:** the counts and rules are straightforward. Seed-exactness depends on pruning, stable sorts and HashSet/LINQ
iteration order, which makes it hard. A distribution-only clone is medium.

### C2. Card rewards (medium)

**Classes:** `RewardsSet`, `CardReward`, `CardFactory`, `CardRarityOdds`, `CardCreationOptions`.

**What a fight gives** (`Rewards/RewardsSet.cs:206-246`)
- Monster: gold, potion roll, 3 cards. Elite: the same plus a relic. Boss: gold, potion roll, 3 cards (all rare).

**Rarity roll** (`Odds/CardRarityOdds.cs:96-111`)
- One `NextFloat` u. Rare if u < baseRare + offset; uncommon if below that plus baseUncommon; otherwise common.

| Odds type | Rare (normal / A7) | Uncommon |
|---|---|---|
| Regular | 3% / 1.49% | 37% |
| Elite | 10% / 5% | 40% |
| Shop | 9% / 4.5% | 37% |
| Boss | 100% | — |

Source: `Odds/CardRarityOdds.cs:13-41`.

**Pity offset**
- Starts at −5% and resets to −5% after a rare. Otherwise it grows +1% (+0.5% at A7), capped at +40%
  (`Odds/CardRarityOdds.cs:27-31, 69-81`).
- It advances only for encounter-sourced Regular/Elite/Boss rewards (`Factories/CardFactory.cs:244-260`).
- Event and relic rewards roll at base odds and leave the offset untouched (`Odds/CardRarityOdds.cs:120-134`). The shop
  reads the offset but never changes it (`Factories/CardFactory.cs:50`).
- An empty rarity moves to the next one, with wrap-around (`Factories/CardFactory.cs:263-281`).
- No duplicates on one screen (`:91-97, 217`).

**Upgrade roll** (`Factories/CardFactory.cs:23, 283-305`)
- Non-rare cards: actIndex × 0.25 (× 0.125 at A7), so 0 / 25% / 50% across acts 1-3. Rare cards: 0.
- The float is drawn before the upgradability check.

**RNG order and gold**
- Order on `PlayerRng.Rewards`: potion drop decision, then gold, potion identity, and per card rarity → item → upgrade,
  then the relic (`Rewards/RewardsSet.cs:132-135`, `Rewards/GoldReward.cs:71-72`).
- Gold: Monster 10-20, Elite 35-45, Boss 100, × 0.75 at A3 (`Models/EncounterModel.cs:64-99`).

**Every source of other-character cards** (authoritative list):
- **Prismatic Gem** (Orobas ancient relic): card rewards draw from all unlocked character pools
  (`Models.Relics/PrismaticGem.cs:28-46`; flag set at `Rewards/CardReward.cs:114-115`).
- **Sea Glass** (Orobas): 15 cards (5/5/5) from one other character's pool, take any number
  (`Models.Relics/SeaGlass.cs:68-90`). Orobas offers Prismatic Gem with probability 1/3, otherwise Sea Glass
  (`Models.Events/Orobas.cs:188-211`).
- **Kaleidoscope** (Neow, only with all pools unlocked): 2 rewards, each with 1 card from each of 3 other pools
  (`Models.Relics/Kaleidoscope.cs:22-48`).
- **Colorful Philosophers** (Hive event): pick one of up to 3 other colours, which gives 3 rewards (common, uncommon,
  rare) (`Models.Events/ColorfulPhilosophers.cs:29-68`).
- **Splash** (in combat): other pools (`Models.Cards/Splash.cs:25-30`).
- **CharacterCards** custom-run modifier (`Models.Modifiers/CharacterCards.cs:35-56`).
- **Transforms** stay in the original card's pool, so off-class stays off-class. Quest/Event/Ancient/Token cards become
  Colorless (`Factories/CardFactory.cs:170-211`). Streams: `PlayerRng.Transformations` or `Niche`
  (`Models.Relics/Astrolabe.cs:25`, `PandorasBox.cs:24`).
- **Potions:** none. Combat generation uses the owner's pool (`Factories/CardFactory.cs:119-162`).
- **Colorless sources:** Dingy Rug (`Models.Relics/DingyRug.cs:13-27`), the shop's 2 colorless slots, and transforms.

**Port:** the odds are trivial. Exact pool ordering (unlock state, ModelDb order) and the hook modifiers are the work.

### C3. Shop (easy-medium)

**Classes:** `MerchantInventory`, `MerchantCardEntry`, `MerchantRelicEntry`, `MerchantPotionEntry`,
`MerchantCardRemovalEntry`.

**Stock**
- 5 character cards: Attack, Attack, Skill, Skill, Power. Shop rarity odds, no duplicates, and one random slot at half
  price (`Entities.Merchant/MerchantInventory.cs:15-22, 98-112`).
- 2 colorless cards: one Uncommon, one Rare (`:24-28, 114-124`).
- 3 relics: rarity, rarity, Shop tier (`:126-139`).
  - Relic rarity uses the 50 / 33 / 17 roll on **`PlayerRng.Rewards`**, not Shops (`Factories/RelicFactory.cs:80-94`).
  - Pulled from the back of the player bag, filtered by `IsAllowedInShops` (`Entities.Merchant/MerchantRelicEntry.cs:39`).
- 3 potions (`Entities.Merchant/MerchantInventory.cs:141-148`), and card removal.

**Prices**
- Cards: 50 / 75 / 150, × 1.15 for colorless, × U(0.95, 1.05), halved on sale
  (`Entities.Merchant/MerchantCardEntry.cs:38-51, 112-128`). The sale recomputation spends an extra float.
- Relics: Common 175, Uncommon 225, Rare 275, Shop 200 (`Models/RelicModel.cs:304-315`), × U(0.85, 1.15)
  (`Entities.Merchant/MerchantRelicEntry.cs:42-44`).
- Potions: 50 / 75 / 100, × U(0.95, 1.05) (`Entities.Merchant/MerchantPotionEntry.cs:50-70`).
- Removal: 75 + 25 per removal used, or 100 + 50 at A6 (`Entities.Merchant/MerchantCardRemovalEntry.cs:20-32`).
- Price hooks such as Membership Card: `Entities.Merchant/MerchantEntry.cs:19-30`.

**Quirk:** shop cards never upgrade, but each still consumes a Rewards float (`Factories/CardFactory.cs:58, 283-290`).

### C4. Potion drops (easy)

- `PotionRewardOdds.Roll` (`Odds/PotionRewardOdds.cs:54-72`): chance = current + 0.125 on elites. It starts at 0.40;
  −0.10 after a drop, +0.10 after a miss, with no clamp.
- The game's own doc comment says "25% bonus" for elites (`Odds/PotionRewardOdds.cs:51`), but the code adds
  `0.25 * 0.5` = 12.5% (`:61-63`). Trust the code.
- Rolled after every Monster, Elite and Boss fight (`Rewards/RewardsSet.cs:229-256`). White Beast Statue forces a drop
  (`Models.Relics/WhiteBeastStatue.cs:17`).
- Rarity: Rare ≤ 0.10, Uncommon ≤ 0.35, otherwise Common. Drawn without replacement from the character pool plus the
  shared pool (`Factories/PotionFactory.cs:76-95`).
- Stream: `PlayerRng.Rewards` (`Rewards/PotionReward.cs:56-59`).

### C5. Unknown-room odds (easy)

- `UnknownMapPointOdds`: base Monster 0.10, Treasure 0.02, Shop 0.03, Elite disabled (−1); Event takes the remainder
  (`Odds/UnknownMapPointOdds.cs:21-47, 97`).
- One float, walked cumulatively (`:140-158`). The rolled type resets to its base; every other allowed type gains its
  base (`:159-175`).
- Resets each act (`Runs/RunManager.cs:1385`). Shop is excluded after a shop, or if all children are shops
  (`Runs/RunManager.cs:660-668`).
- Hooks: Juzu Bracelet removes Monster (`Models.Relics/JuzuBracelet.cs:17-27`); Golden Compass forces Event
  (`Models.Relics/GoldenCompass.cs:48-55`); DeadlyEvents enables Elite (`Models.Modifiers/DeadlyEvents.cs:18-26`).
- Stream: `UnknownMapPoint` (`Runs/RunState.cs:275`).

### C6. Events (selection easy; event bodies hard)

- **Pools.** Act lists: Overgrowth 13, Underdocks 10, Hive 10, Glory 7 (`Models.Acts/Overgrowth.cs:28-42`,
  `Underdocks.cs:26-37`, `Hive.cs:31-42`, `Glory.cs:30-38`). Plus 18 shared events (`Models/ModelDb.cs:157-176`).
- **Selection.**
  - Each act's list plus the shared list, minus unrevealed epochs, is shuffled with `UpFront` at run start
    (`Models/ActModel.cs:334-347`).
  - A cursor walks forward, skipping events that fail `IsAllowed` or were already visited anywhere in the run, so there
    is no repeat across acts (`Rooms/RoomSet.cs:70, 98, 110-125`).
  - The `ModifyNextEvent` hook applies (Lantern Key → War Historian Repy; `Models/ActModel.cs:437-443`).
- **Conditions.** About 36 events override `IsAllowed`. Examples:
  - Crystal Sphere: act ≥ 2 and gold ≥ 100 (`Models.Events/CrystalSphere.cs:49-56`).
  - Relic Trader: act ≥ 2 and 5 tradable relics (`Models.Events/RelicTrader.cs:98-105`).
  - Unrest Site: HP ≤ 70% (`Models.Events/UnrestSite.cs:28`).
  - Tea Master: act < 3 and gold ≥ 150 (`Models.Events/TeaMaster.cs:34-41`).
  - Doll Room: act 2 only (`Models.Events/DollRoom.cs:81`).
- **Port:** selection is easy. Each event's choices and outcomes are bespoke code, which is the bulk of the work.

### C7. Rest sites (easy)

- `RestSiteOption.Generate`: Heal and Smith (+ Mend in multiplayer), then the `ModifyRestSiteOptions` hook
  (`Entities.RestSite/RestSiteOption.cs:53-74`).
- Heal is 30% of max HP (`Entities.RestSite/HealRestSiteOption.cs:90-92`). Smith is disabled if nothing can be upgraded
  (`Entities.RestSite/SmithRestSiteOption.cs:29, 49`).
- Options added by relics:
  - Shovel → Dig (`Models.Relics/Shovel.cs:24`).
  - Girya → Lift, up to 3 times (`Models.Relics/Girya.cs:65-69`).
  - Meat Cleaver → Cook: remove 2 cards, +5 max HP (`Entities.RestSite/CookRestSiteOption.cs:15, 45-58`).
  - Pumpkin Candle → Kindle (`Models.Relics/PumpkinCandle.cs:83`).
  - Pael's Growth → Clone (`Models.Relics/PaelsGrowth.cs:37`).
  - Byrdonis Egg → Hatch (`Models.Relics/ByrdonisEgg.cs:25`).
- The Midas modifier removes Smith (`Models.Modifiers/Midas.cs:31-38`).

### C8. Ancients (easy; data already in repo)

- **Which ancient:**
  - Act 1: Neow (`Models.Acts/Overgrowth.cs:26`).
  - Act 2: Orobas / Pael / Tezcatara (`Models.Acts/Hive.cs:24-28, 98-105`).
  - Act 3: Nonupeipe / Tanx / Vakuu (`Models.Acts/Glory.cs:23-27`).
  - The shared ancient Darv (`Models/ModelDb.cs:189`) is distributed by a random prefix split
    (`Runs/RunManager.cs:745-752`). The act's ancient is `UpFront.NextItem` (`Models/ActModel.cs:385`).
- **Neow:** a curse option, coin-flip pairs (Lava Rock/Small Capsule, Oyster/Humidifier, Talisman/Pomander), and 2
  shuffled positives (`Models.Events/Neow.cs:220-284`).
- **Determinism:** an event's options are a pure function of seed and ancient id (`Models/EventModel.cs:234`).
- **Repo data:** `data/ancients.json` already holds pools, conditions, exclusions and ancient weights.

### C9. Act transitions (easy)

- `EnterAct` → `SetActInternal`: set the act index, reset unknown odds, generate the map
  (`Runs/RunManager.cs:1344-1355, 1379-1388`).
- `AncientEventModel.BeforeEventStarted` (`Models/AncientEventModel.cs:170-190`) heals missing HP in full, or **80%** of
  it at A2 WearyTraveler.
  - Neow first sets HP to 0 (`:174-177`), so an A2+ run starts at 80% of max HP.
  - The act 2 and 3 ancients heal 80% of missing HP at A2+.
- After the final boss comes `TheArchitect` (`Runs/RunManager.cs:1316-1325`).
- Encounters and bosses are fixed at run start. At A10 the second boss differs from the first (`Runs/RunManager.cs:761-765`).

### C10. Boss rewards and treasure (easy-medium)

- **No boss relic.** `RelicRarity` has no Boss tier (`Entities.Relics/RelicRarity.cs:3-13`).
  - A boss gives 100 gold, a potion roll and 3 rare cards (`Rewards/RewardsSet.cs:238-242`).
  - The StS1 boss-relic role is filled by the next act's ancient.
- **Treasure chests.**
  - Rarity 50 / 33 / 17 on `TreasureRoomRelics` (`Multiplayer.Game/TreasureRoomRelicSynchronizer.cs:105-108`).
  - Pulled from the front of the shared bag, which holds no character relics (`Runs/RunManager.cs:522`).
  - Gold 42-52, × 0.75 at A3 (`Multiplayer.Game/OneOffSynchronizer.cs:133-137`).
- **Elite relics.** From the front of the player bag (shared and character relics), same rarity roll on `Rewards`
  (`Runs/RelicGrabBag.cs:69-92`, `Factories/RelicFactory.cs:21-28, 80-94`).
- **Bag mechanics.** Tiers are shuffled once with `UpFront` (`Runs/RelicGrabBag.cs:16-22`).
  - Relics that fail `IsAllowed` are purged at pull time (`:250-263`).
  - An empty tier falls back Shop → Common → Uncommon → Rare (`:218-243`), with Circlet last (`Factories/RelicFactory.cs:13`).
- **Port:** medium, because of the per-relic `IsAllowed` checks.

### C11. Port summary

| System | Difficulty | Why |
|---|---|---|
| RNG | done | `crates/sts2sim/src/rng.rs` |
| Map | hard (exact) / medium (distribution) | pruning, sort-based shuffles, collection iteration order |
| Card rewards | medium | odds trivial; pool ordering, unlocks, hooks |
| Shop | easy-medium | streams split across Shops and Rewards |
| Potions | easy | |
| Unknown rooms | easy | |
| Event selection | easy | about 36 `IsAllowed` predicates |
| Event bodies | hard | bespoke per event |
| Rest sites | easy | |
| Ancients | easy | `data/ancients.json` exists |
| Act transitions | easy | |
| Boss / treasure relics | medium | relic `IsAllowed` checks, bag order |

Existing repo knowledge:
- RNG stream list: `docs/spec/03-cards-piles.md:522`.
- A7 rarity numbers: `.claude/skills/sts2-pathing/SKILL.md:29`.
- Encounter bags: `.claude/skills/sts2-acts/SKILL.md:8`.
- `tools/gen_curriculum.py:128-132` only approximates cards from other characters (30%, 1-3 cards) and does not model
  their real sources (C2).
