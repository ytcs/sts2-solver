# 05 - Character subsystems and combat input schema (STS2 v0.111.0 public-beta)

Scope: (A) character-specific subsystems (orbs, Osty, stars, Forge, Poison/Doom, tokens, starter data);
(B) everything that defines a combat's initial condition and therefore must be an input to the Rust simulator.
VFX/SFX/animation/waits/Godot/networking/multiplayer are ignored throughout. Single-player only.

Citation convention: `C/` = `decomp/MegaCrit/Sts2/Core/`. `File.cs:N` are line numbers in the decompiled sources.
"Hook order" always means `CombatState.IterateHookListeners` order (section 12).

Table of contents
1. Combat-start timeline (with RNG streams consumed)
2. Combat input schema (exhaustive)
3. RNG streams (names, seeding, who consumes what in combat)
4. Characters and starter data
5. Orbs (Defect)
6. Osty / pets / summons (Necrobinder)
7. Stars (Regent)
8. Forge / Sovereign Blade (Regent)
9. Poison and Doom
10. Shivs, Souls and other token cards
11. Potions
12. Hook-listener order (affects everything above)
13. Per-card / per-relic / per-enchantment persistent state
14. Ascension, modifiers, gold, act/floor in combat
15. Outputs that persist after combat
16. Flagged uncertainties / open questions

---------------------------------------------------------------------------------------------------

## 1. Combat-start timeline (what happens between "room entered" and "player may act")

Source of truth: `C/Rooms/CombatRoom.cs:StartCombat (~L197-228)`, `C/Combat/CombatManager.cs:SetUpCombat (L444)`,
`StartCombatInternal (L576)`, `StartTurn (L688)`, `SetupPlayerTurn (L877)`.

```
T0  Encounter.GenerateMonstersWithSlots(runState)            [CombatRoom.cs ~L199]
      encounter._rng = Rng(runState.Rng.Seed + TotalFloor + xxhash64(encounterId.Entry))   (EncounterModel.cs ~L261-264)
      -> encounter-specific RNG used by GenerateMonsters() (e.g. random monster variants). Stream: ENCOUNTER-LOCAL (not a run stream).
T1  for (monster, slot) in encounter.MonstersWithSlots:      [CombatRoom.cs ~L205-213]
      creature = CombatState.CreateCreature(monster, Enemy, slot)   [CombatState.cs L232-248]
        monster.RunRng = runState.Rng
        Creature ctor: maxHp = curHp = monster.MaxInitialHp                  [Creature.cs L347-362]
        SetUniqueMonsterHpValue(enemiesSoFar, runState.Rng.Niche)            [Creature.cs L372-384]   *** consumes Niche ***
            candidates = {MinInitialHp..MaxInitialHp} minus MaxHp of already-created same-side creatures (ascending order)
            if candidates empty: hp = Niche.NextInt(min, max+1) else hp = Niche.NextItem(candidates)
            NextItem always draws one NextInt(0,count) even if count==1.  -> EVERY enemy consumes >=1 Niche draw.
        ScaleMonsterHpForMultiplayer: no-op when 1 player.
        monster.Rng = Rng(runSeed + mapCol + mapRow + actIndex + creature.CombatId)   (cosmetic only, see s3.4)
      CombatState.AddCreature(creature)  -> appended to Enemies
T2  CombatManager.SetUpCombat(state)                         [CombatManager.cs L444-466]
      player.ResetCombatState(): new PlayerCombatState; OrbQueue.Clear(); OrbQueue.AddCapacity(Player.BaseOrbSlotCount)
                                 Energy=0, Stars=0, TurnNumber=1, piles empty, no pets.   [PlayerCombatState.cs L129-140]
      player.PopulateCombatState(runState.Rng.Shuffle, state)   [Player.cs L806-815]
           for card in Deck.Cards (deck order): clone into DrawPile (clone.DeckVersion = deck card)
           DrawPile._cards.UnstableShuffle(Rng.Shuffle)                         *** consumes Shuffle: (n-1) draws ***
           Hook.ModifyShuffleOrder(isInitialShuffle:true)  (listeners may reorder; e.g. cards forcing top/bottom)
      CombatManager.AddCreature(creature) for each state.Creatures (registers with CombatStateTracker etc.; the Allies/Enemies lists were ALREADY populated by CombatState.AddPlayer / CombatState.AddCreature in T1 and run setup: Allies=[player], Enemies=[...] in roster/slot order)  [CombatManager.cs L462-465]
T3  Hook.AfterRoomEntered(runState, room)  - run-level listeners ONLY (childCombatState==null):
      deck cards + deck enchantments, relics (unmelted), potions, modifiers.
      Combat-relevant examples: DivineRight (+3 Stars), Pantograph/Vajra/etc. (see relic list in s13.4).
T4  AfterCombatRoomLoaded -> turn loop -> StartCombatInternal:
      foreach creature: AfterCreatureAdded
      Hook.BeforeCombatStart (+Late pass): listeners incl. combat state. BoundPhylactery -> OstyCmd.Summon(1) here.
T5  StartTurn(player side, turn 1):
      creature.BeforeTurnStart (power.AmountOnTurnStart snapshot)
      Hook.BeforeSideTurnStart  -> CrackedCore channels 1 Lightning (TurnNumber<=1)           [CrackedCore.cs]
      phase=Start; enemies PrepareForNextTurn (roll first move; consumes MonsterAi where move RNG is used)
      creature.AfterTurnStart(): player skips ClearBlock on TurnNumber==1                       [Creature.cs L686-697]
      Hook.AfterBlockCleared
      SetupPlayerTurn:                                                                          [CombatManager.cs L877-925]
        energy = MaxEnergy (Hook.ModifyMaxEnergy) unless Hook.ShouldPlayerResetEnergy==false (then energy += MaxEnergy)
        Hook.AfterEnergyReset (+Late)   (BoundPhylactery: re-summon only if TurnNumber != 1)
        Hook.BeforeHandDraw
        handDraw = Hook.ModifyHandDraw(5)    (RingOfTheSnake +2 on TurnNumber<=1, etc.)
        if TurnNumber==1:
            cards whose Enchantment.ShouldStartAtBottomOfDrawPile -> MoveToBottom (in pile order)
            cards with Innate (excluding the above) -> MoveToTop each in pile order  [MoveToTop = Insert(0): the LAST moved ends up on top]
            handDraw = max(handDraw, innateCount); handDraw = min(handDraw, 10)
        CardPileCmd.Draw(handDraw, fromHandDraw:true)   (draw takes drawPile.Cards.First() = index 0 = "top")
        Hook.AfterPlayerTurnStart
      Hook.AfterSideTurnStart  (InfusedCore-type relics channel here, TurnNumber<=1)
      OrbQueue.AfterTurnStart  (Plasma orbs trigger)   [CombatManager.cs L792]   <- AFTER draw
      then phase=AutoPrePlay -> CheckForEmptyHand -> Hook.AfterAutoPrePlayPhaseEntered -> phase=Play
```

RNG-stream consumption summary for the *start* of combat: `Niche` (monster HP rolls, 1+ draw per enemy in encounter slot
order), `Shuffle` (deck.Count-1 draws), plus anything BeforeCombatStart / AfterRoomEntered / first-turn hooks consume.
The only deck-order-dependent consumption is the initial `UnstableShuffle`: **the deck must be passed as an ordered list**
(see s2).

Mid-combat reshuffle is different: `CardPileCmd.Shuffle` (CardPileCmd.cs L1076-1100) builds list = discard ++ (draw-pile cards
via a HashSet enumeration), then `StableShuffle` (sorts by `CardModel.CompareTo` = ModelId ordinal compare then upgrade level, then
`UnstableShuffle`) -> result independent of prior order. Not this document's topic; flagged in s16.

---------------------------------------------------------------------------------------------------

## 2. Combat input schema (exhaustive)

Everything the game reads at combat entry that can differ between two combats. Fields marked `[ORD]` are order-sensitive
(order feeds RNG or hook-order). `[RNG]` = full stream state required (counter is informational only; the 4x u64 state is what matters).

### 2.1 Run-level / environment

| Field | Type | Where read | Why it matters |
|---|---|---|---|
| `game_version` | "0.111.0" | - | content tables are version-specific |
| `ascension` | u8 0..10 | `IRunState.AscensionLevel`; `AscensionHelper.GetValueIfAscension` (`C/Helpers/AscensionHelper.cs`) via `RunManager.HasAscension` (`>=` compare) | monster HP/damage (levels 8 = ToughEnemies, 9 = DeadlyEnemies read inside combat); levels 4/5 are already baked into player slot count/deck (s14.1) |
| `act_index` | u8 0-based | `RunState.CurrentActIndex`; `Models/Monsters/DecimillipedeSegment.cs:130`, `SpoilsMap`, `LanternKey`, `GoldenCompass`, `FurCoat`, `LavaRock`... | per-monster/relic/card conditions |
| `total_floor`, `act_floor` | i32 | `EncounterModel._rng` seed uses `TotalFloor` (EncounterModel.cs ~L261) | encounter-local RNG seed (T0 above) |
| `map_coord` (col,row) | i32,i32 | `CombatState.cs:245` (monster.Rng seed) | only if monster `Rng` is gameplay-relevant - it is NOT (s3.4); can be omitted |
| `room_type` / `is_elite` / `is_boss` / is_last_act_boss | enum | relics (AmethystAubergine, SwordOfStone, etc.), `EndCombatInternal` | relic conditions |
| `game_mode` (standard/daily/custom) + `modifiers[]` | list of {id, props} | `CombatState.Modifiers` are hook listeners (CombatState.cs L471) | combat-relevant modifiers: Murderous, Terminal, Hoarder (see s14.2) |
| `unlock_state` | set of revealed epochs | `PotionPoolModel.GetUnlockedPotions`, `CardPoolModel.GetUnlockedCards` | filters + ORDERS in-combat random generation pools (s11.5). Recommended: declare "all epochs revealed" explicitly. |
| `card_multiplayer_constraint` | = SingleplayerOnly | `IRunState.CardMultiplayerConstraint` (IRunState.cs ~L190) | removes MultiplayerOnly cards from pools |
| `player_count` | = 1 | HP scaling, Hibernate etc. | fixed |
| `encounter` | see 2.3 | | |

### 2.2 Player (from `SerializablePlayer`, `C/Saves/Runs/SerializablePlayer.cs`; consumed by `Player.FromSerializable`, Player.cs L318-326)

| Field | Type | Notes |
|---|---|---|
| `character_id` | ModelId | Ironclad/Silent/Defect/Necrobinder/Regent (s4) |
| `current_hp`, `max_hp` | i32 | combat starts at the player's current HP (no reset); Creature ctor Creature.cs L364-370 |
| `max_energy` | i32 | normally 3; `Player.MaxEnergy` is never modified at run time (grep: no writes besides load); relics change it only via `Hook.ModifyMaxEnergy` (PlayerCombatState.cs L101) |
| `max_potion_slot_count` | i32 | default 3 (Player.cs L28); A4 (TightBelt) -> 2 (AscensionManager.cs ApplyEffectsTo); relics/events change it (`PlayerCmd.GainMaxPotionCount/LoseMaxPotionCount`, Player.cs L566-578). NB: PotionOfCapacity is NOT one of these: its `OnUse` is `OrbCmd.AddSlots(target.Player, Repeat)` (orb slots, in combat only) |
| `gold` | i32 | combat-relevant: Debt (loses gold), ThieveryPower, SpoilsMap, Royalties, MawBank, etc. (grep `\.Gold` in Cards/Powers/Monsters/Relics) |
| `base_orb_slot_count` | i32 | Defect 3, others 0; never mutated by game code found (s5.1) |
| `net_id` / slot index | = 0 | slot index feeds `Rng(player, id)` constructors (cosmetic skins: Byrdpip/PaelsLegion/FurCoat) |
| `deck` `[ORD]` | list<SerializableCard> | s2.4 |
| `relics` `[ORD]` | list<SerializableRelic> | s2.5; order = hook order |
| `potions` | list<{id, slot_index}> | s2.6; slot order = hook order and use-index |
| `rng` (PlayerRngSet), `odds` (PlayerOddsSet), `relic_grab_bag`, `discovered_*`, `unlock_state`, `extra_fields` | - | `PlayerRng` (rewards/shops/transformations), odds, grab bag, discovered lists: NOT read inside combat (s3.3). `ExtraPlayerFields` (CardShopRemovalsUsed, WongoPoints, CccomboBadgeUnlocked, DamageDealt, DebuffsApplied; `ExtraPlayerFields.cs`) are bookkeeping only - `DamageDealt` is only written by `CreatureCmd.Damage` (L~350). Not inputs. |

### 2.3 Encounter / enemies

| Field | Notes |
|---|---|
| `encounter_id` | `EncounterModel`; determines monsters, slots, custom rules (`Encounter.SaveCustomState` is serialized in `SerializableRoom.EncounterState`, CombatRoom.cs ToSerializable) |
| `monsters[]` `[ORD]` | ordered list (monster_id, slot_name). Order = `CombatState.Enemies` order = turn order = hook order; `SortEnemiesBySlotName` reorders by `Encounter.Slots` (CombatState.cs ~L505) |
| per-enemy `max_hp` | rolled in T1 from `Niche`; the Rust env can either take pre-rolled HPs or the Niche stream state. Ascension 8 raises Min/MaxInitialHp inside monster classes. |
| per-enemy pre-set state | (e.g. starting powers, move) comes from monster `AfterAddedToRoom`/`GenerateMoveStateMachine`; owned by the monsters spec |
| `encounter._rng` seed inputs | `run_seed` (u64 hash), `total_floor`, `encounter_id` -> formula in T0 |

### 2.4 Deck card (`SerializableCard`, `C/Saves/Runs/SerializableCard.cs`; `CardModel.ToSerializable` CardModel.cs L2228, `FromSerializable` L2246)

```
card {
  id: ModelId
  current_upgrade_level: u8          // ALWAYS 0 or 1. No card has MaxUpgradeLevel > 1 (grep: only 0/1; default CardModel.cs L782 = 1).
                                     //  => there is NO Searing Blow analogue in this version.
  enchantment: Option<{ id, amount: i32, props: Option<SavedProperties> }>   // SerializableEnchantment
  props: Option<SavedProperties>     // [SavedProperty] fields of the card class (s13.1)
  floor_added_to_deck: Option<i32>   // bookkeeping only (starter cards=1) - not read in combat except by few relics (ignore)
}
```
Not serialized (so NOT inputs): `Affliction` (combat-only), temporary cost modifiers, keywords added in combat, `DeckVersion`.
FromSerializable order: Props.Fill -> AfterDeserialized -> EnchantInternal(+ModifyCard) -> `UpgradeInternal` x level. Upgrade is
applied AFTER enchantment/props are filled.
`EnchantmentModel.Status` (Disabled) and enchantment-private fields (`Glam._usedThisCombat`, `Momentum._extraDamage`, `Slither`)
are not serialized (s13.3).

### 2.5 Relic (`SerializableRelic`: id, props, floor_added; `RelicModel.ToSerializable` RelicModel.cs L556)
`props` = `[SavedProperty]` fields (s13.4, includes `IsWax`, `IsMelted` on the base class RelicModel.cs L256-270).
`RelicModel.Status` (Normal/Active/Disabled) is NOT serialized: it is recomputed by relic setters/hooks. Rust: store Status
but initialize to Normal except where a saved property implies Disabled (LizardTail.WasUsed, MawBank.HasItemBeenBought,
SilkenTress.IsUsed, LavaRock.HasTriggered... verify per relic spec).
Melted relics (`IsMelted`) are excluded from hook iteration (CombatState.cs ~L440, RunState.cs ~L569).

### 2.6 Potion (`SerializablePotion`: id, slot_index; `PotionModel.ToSerializable` PotionModel.cs ~L300)
No per-potion state at all. Potion belt = fixed-length array of `Option<PotionId>` of length `max_potion_slot_count`.

### 2.7 RNG state
12 run-level streams (full state each) - s3. Player streams not used in combat.

### 2.8 Things that are NOT inputs (verified)
`ExtraPlayerFields`, `PlayerOddsSet`, `RelicGrabBag`, `DiscoveredX`, `MaxAscensionWhenRunStarted`, map layout, `RunState.ExtraFields`
(StartedWithNeow, TestSubjectKills, FreedRepy; only text of TestSubject monster reads progress `SaveManager.Progress.TestSubjectKills`,
Monsters/TestSubject.cs:72 - cosmetic), `Rng.Chaotic` (non-deterministic; used only for VFX/skin/shake).

---------------------------------------------------------------------------------------------------

## 3. RNG streams

### 3.1 Algorithm
- `Rng` (C/Random/Rng.cs) wraps `MegaRandom` = xoshiro256** (C/Random/MegaRandom.cs). Init from u64 seed: 4x `Splitmix64` (L~85-100):
  `x += 0x9E3779B97F4A7C15; z=(x^(x>>30))*0xBF58476D1CE4E5B9; z=(z^(z>>27))*0x94D049BB133111EB; z^(z>>31)` (constants as printed in source:
  11400714819323198485, 13787848793156543929, 10723151780598845931).
- `NextULongInner`: `result = rotl(s1*5,7)*9; t = s1<<17; s2^=s0; s3^=s1; s1^=s2; s0^=s3; s2^=t; s3=rotl(s3,45)`.
- `NextDouble = (u64>>11) * 2^-53` (1.1102230246251565E-16). `Next(max) = (int)(NextDouble()*max)`; `Next(min,max) = Next(max-min)+min`.
  `Rng.NextInt(min,max)` always consumes exactly one draw (also for range 1). `NextBool = (Next(2)==0)` (NOT MegaRandom.NextBool).
  `NextFloat(min,max) = (float)(NextDouble()*(max-min)+min)`. `NextItem(items)`: if count==0 -> null with NO draw; else `NextInt(0,count)` (one draw, even for count==1).
  `UnstableShuffle`: `for n=count-1 down to 1: j=NextInt(n+1); swap(list[n],list[j])` (ListExtensions.cs L45-60). `Rng.Shuffle` (Rng.cs ~L330) is the same loop.
  `TakeRandom(count)`: `ToList().UnstableShuffle(rng).Take(count)` (IEnumerableExtensions.cs L17).
- The `counter` field is bookkeeping only (serialized); state = (s0..s3).

### 3.2 Seeding
- Run seed string -> `Seed = XxHash64(UTF8(seed), 0)` (u64) (`StringHelper.GetDeterministicHashCode`, StringHelper.cs L139-152); strings starting with "old" use a legacy 32-bit hash (RunRngSet.cs L106-113).
- Run stream k: `Rng(seed + XxHash64(snake_case(name)))` (wrapping u64 add) (`Rng(ulong,string)`, Rng.cs L54; `RunRngSet.CreateRng` RunRngSet.cs L139).
- `snake_case(name)` = regex `([A-Za-z0-9]|\G(?!^))([A-Z])` -> `$1_$2`, then lowercase (StringHelper.cs L34, L90). Exact names:

| enum `RunRngType` | name hashed | Accessor | Used in combat by |
|---|---|---|---|
| UpFront | `up_front` | `Rng.UpFront` | not in combat (run/encounter generation: RunManager/RunState) |
| Shuffle | `shuffle` | `Rng.Shuffle` | initial deck shuffle; mid-combat reshuffle (CardPileCmd.cs L1088); StampedePower:28 picks a card via `Shuffle.NextItem`; Catastrophe, BeatDown, Uproar |
| UnknownMapPoint | `unknown_map_point` | - | not in combat |
| CombatCardGeneration | `combat_card_generation` | `Rng.CombatCardGeneration` | Attack/Skill/Power/Colorless/CosmicConcoction/OrobicAcid potions, Discovery, InfernalBlade, Distraction, Abundance, JackOfAllTrades, MadScience, Splash, Metamorphosis, CallOfTheVoid, CreativeAI, Calamity, HelloWorld, SpectrumShift, BigHat, Crossbow, OrangeDough, VexingPuzzlebox, Toolbox, ChoicesParadox, ThievingHopper... |
| CombatPotionGeneration | `combat_potion_generation` | `Rng.CombatPotionGeneration` | Alchemize, EntropicBrew, PhialHolster, AlchemicalCoffer, DelicateFrond |
| CombatCardSelection | `combat_card_selection` | `Rng.CombatCardSelection` | TrueGrit (random exhaust), Thrash, SeekerStrike, HiddenGem, DrainPower, Anointed, Cinder, relic Bookmark/MummifiedHand/JeweledMask/PowerCell/StoneCracker, Aggression/Entropy/Improvement powers, CardPileCmd draw-random-position (L1163) |
| CombatEnergyCosts | `combat_energy_costs` | `Rng.CombatEnergyCosts` | ConfusedPower (`NextInt(4)`, ConfusedPower.cs:53), SneckoOil, Slither enchantment |
| CombatTargets | `combat_targets` | `Rng.CombatTargets` | random-target attacks (`AttackCommand.cs:617`), **LightningOrb** (LightningOrb.cs ApplyLightningDamage), BouncingFlask, TheBall, BeatDown, Juggernaut/Cacophony/Countdown/Haunt/SerpentForm powers, Kusarigama/ParryingShield/Tingsha/WhisperingEarring/ForgottenSoul relics (CardCmd.cs) |
| MonsterAi | `monster_ai` | `Rng.MonsterAi` | `MonsterModel.cs:418` `MoveStateMachine.RollMove(targets, creature, RunRng.MonsterAi)`; FlutterPower; Fabricator |
| Niche | `niche` | `Rng.Niche` | **every Enemy-side creature creation** (`CombatState.CreateCreature` L232-248 -> `SetUniqueMonsterHpValue(_enemies, Niche)`), i.e. the T1 roster AND every mid-combat spawn (`CreatureCmd.Add<T>/Add(monster)`: SurprisePower->FatGremlin/SneakyGremlin, StockPower->Axebot, InfestedPower->Wriggler, TwoTailedRat, Fabricator, Fogmog->EyeWithTeeth, LivingFog->GasBomb, Ovicopter->ToughEgg ...; the uniqueness set is the CURRENT Enemies list incl. dead-but-present ones); ToughEgg hatchling HP (ToughEgg.cs:172); many relic pickups (out of combat). Osty/pet creation (Player side) does NOT draw Niche. |
| CombatOrbs | `combat_orbs` | `Rng.CombatOrbGeneration` | Chaos card, TrashToTreasurePower: `OrbModel.GetRandomOrb(rng)` = `rng.NextItem([Lightning, Frost, Dark, Plasma, Glass])` (OrbModel.cs L16-21, L~133) |
| TreasureRoomRelics | `treasure_room_relics` | - | not in combat |

Player streams (`PlayerRngSet`, seed = `XxHash64(seedString) + playerSlotIndex` (Player.cs L328-332), stream names `rewards`, `shops`, `transformations`):
**not consumed by any in-combat path found** (grep of Cards/Powers/Potions/Commands/Combat/Hooks for `PlayerRng`: none; CardFactory
`player.PlayerRng.Rewards` fallbacks are only reached from `CreateForReward`/transform-for-reward callers, none in combat; all in-combat generation passes an explicit `CombatCardGeneration`/`Niche` Rng).
Therefore `PlayerRng`/`PlayerOdds` are NOT combat inputs (but see s16 for the one residual risk: relic-triggered transformations).

### 3.3 State vs seed
Streams advance across the whole run, so a combat's initial condition requires each stream's full `{state0..state3}` (SerializableRng: `counter,state0..3`, `C/Saves/SerializableRng.cs`), not just the seed. For a fresh run, derive from the seed string at run start. In-combat streams that actually matter: Shuffle, CombatCardGeneration, CombatPotionGeneration, CombatCardSelection, CombatEnergyCosts, CombatTargets, MonsterAi, Niche, CombatOrbs (9 streams). Other three may be ignored/zeroed.

### 3.4 Per-monster `Rng` (MonsterModel.Rng) is cosmetic
Set in `CombatState.CreateCreature` (L245). Only `ToughEgg` skin uses it (Models/Monsters/ToughEgg.cs:118). grep over Models/Monsters, MonsterMoves, Models/Powers found no gameplay use (all gameplay randomness uses `RunRng.MonsterAi`, `RunRng.Niche`, `RunState.Rng.*`).
=> `map_coord` and `creature.CombatId` need not be inputs. (`CombatId` still defines creature ids = order of attachment: player=0, then monsters in order, then later spawns, Osty when summoned.)

---------------------------------------------------------------------------------------------------

## 4. Characters and starter data

Source: `C/Models/Characters/*.cs`, `C/Models/CharacterModel.cs L93-115`.
Common defaults (CharacterModel.cs): `MaxEnergy => 3` (L97), `BaseOrbSlotCount => 0` (L101), `StartingPotions => []` (L115, no character overrides it),
starting gold 99 for all; `Player.CreateForNewRun` passes `potionSlotCount = 3` (Player.cs L307), then `AscensionManager.ApplyEffectsTo` (RunManager.cs:1718): A4 -> -1 potion slot (=2), A5 -> adds `AscendersBane` curse to deck end (FloorAddedToDeck=1).
Starting deck is cloned `ToMutable()` in listed order with `FloorAddedToDeck=1` (Player.cs L711-721). Starting relics likewise (Player.cs L743-752).

| Character | Class | HP / MaxHP | Gold | MaxEnergy | OrbSlots | Starting relic | Starting deck (ordered, all +0) |
|---|---|---|---|---|---|---|---|
| Ironclad | Ironclad.cs L33-57 | 80 | 99 | 3 | 0 | BurningBlood | StrikeIronclad x5, DefendIronclad x4, Bash |
| Silent | Silent.cs L34-60 | 70 | 99 | 3 | 0 | RingOfTheSnake | StrikeSilent x5, DefendSilent x5, Neutralize, Survivor |
| Defect | Defect.cs L27-63 | 75 | 99 | 3 | 3 | CrackedCore | StrikeDefect x4, DefendDefect x4, Zap, Dualcast |
| Necrobinder | Necrobinder.cs L32-62 | 66 | 99 | 3 | 0 | BoundPhylactery | StrikeNecrobinder x4, DefendNecrobinder x4, Bodyguard, Unleash |
| Regent | Regent.cs L32-58 | 75 | 99 | 3 | 0 | DivineRight | StrikeRegent x4, DefendRegent x4, FallingStar, Venerate |

(Non-playable: Deprived [HP 1000, energy 100, empty deck/relics - test], RandomCharacter, DeprecatedCharacter - ignore.)
Character card/relic/potion pools: `CharacterModel.CardPool/RelicPool/PotionPool` (L105-109) -> `CardPoolModel.GetUnlockedCards` (CardPoolModel.cs L101-114: epoch filter then multiplayer filter, list order = `AllCards` generation order).
Character-specific special fields: only `BaseOrbSlotCount` (Defect) and `ShouldAlwaysShowStarCounter` (Regent, UI only). No other per-character combat field (grep `Models/Characters`).

### 4.1 Starter cards (canonical, from `C/Models/Cards/*.cs`)
Base cost / type / target / values (all upgrade max 1):

| Card | Cost | Type/Target | Effect | Upgrade (+1) |
|---|---|---|---|---|
| Strike{Ironclad,Silent,Defect,Necrobinder,Regent} | 1 | Attack/AnyEnemy, tag Strike | 6 dmg | +3 dmg |
| Defend{...} | 1 | Skill/Self, tag Defend | 5 block | +3 block |
| Bash | 2 | Attack/AnyEnemy | 8 dmg, Vulnerable 2 | +2 dmg, +1 Vuln |
| Neutralize | 0 | Attack/AnyEnemy | 3 dmg, Weak 1 (damage first, then Weak) | +1 dmg, +1 Weak |
| Survivor | 1 | Skill/Self | 8 block, then discard 1 card from hand (player choice; none if hand empty) | +3 block |
| Zap | 1 | Skill/Self | channel Lightning (OrbCmd.Channel) | cost -1 (->0) |
| Dualcast | 1 | Skill/Self | if orbs>0: evoke front orb (no dequeue) then evoke front orb again (dequeue) | cost -1 |
| Bodyguard | 1 | Skill/Self | Summon 5 (OstyCmd.Summon) | Summon +2 |
| Unleash | 1 | Attack/AnyEnemy, tag OstyAttack | Osty attacks target; `CalculatedDamageVar(Move).FromOsty()`: base `CalculationBase` 6 + `ExtraDamage` 1 x multiplier, where multiplier = Osty.CurrentHp (0 if dead) (Unleash.cs); requires Osty alive else the card does nothing (still paid) | base +3 |
| FallingStar | 0 energy + 2 stars | Attack/AnyEnemy | 8 dmg, Weak 1, Vulnerable 1 (order: damage, Weak, Vulnerable) | +4 dmg |
| Venerate | 1 | Skill/Self | gain 2 Stars | +1 star |

### 4.2 Starter relics (canonical)
- BurningBlood: `AfterCombatVictory`: if player alive, heal 6 (output of combat; irrelevant to in-combat logic).
- RingOfTheSnake: `ModifyHandDraw`: +2 cards while `PlayerCombatState.TurnNumber <= 1` (CardsVar 2). Note: handDraw floor for Innate is applied after (s1 T5).
- CrackedCore: `BeforeSideTurnStart`: if player's creature is among participants and `TurnNumber <= 1`: channel 1 Lightning (DynamicVar "Lightning"=1). Occurs BEFORE energy reset/draw of turn 1.
- BoundPhylactery: `BeforeCombatStart`: `OstyCmd.Summon(1)`; `AfterEnergyResetLate` if `TurnNumber != 1`: Summon(1) again (so Osty grows by 1 max HP / is revived with 1 HP each turn from turn 2).
- DivineRight: `AfterRoomEntered` if room is CombatRoom: `PlayerCmd.GainStars(3)` (runs before BeforeCombatStart, after SetUpCombat).

---------------------------------------------------------------------------------------------------

## 5. Orbs (Defect)

Files: `C/Entities/Orbs/OrbQueue.cs`, `C/Commands/OrbCmd.cs`, `C/Models/OrbModel.cs`, `C/Models/Orbs/{Lightning,Frost,Dark,Plasma,Glass}Orb.cs`, `PlayerCombatState.cs L121-139`.

### 5.1 Slots and queue
State: `OrbQueue { orbs: Vec<Orb> (index 0 = FRONT = next to be evoked; new orbs appended at the back), capacity: i32 }`, `maxCapacity=10` (OrbQueue.cs L15).
- Combat start: `Clear(); AddCapacity(Player.BaseOrbSlotCount)` (PlayerCombatState.cs L137-139). `BaseOrbSlotCount` is a saved player field; game code never changes it (only load/save), so per-run extra slots come from in-combat effects (OrbCmd.AddSlots: e.g. cards/potions - PotionOfCapacity, Capacitor-like), which are reset each combat.
- `OrbCmd.AddSlots(p, n)` (OrbCmd.cs L23-31): no-op if combat over/ending; `n = min(10 - capacity, n)`; `capacity += n`.
- `OrbCmd.RemoveSlots(p, n)` (L41-49): `n = min(capacity, n)` -> `RemoveCapacity`: `capacity = max(0, capacity-n)`; `while orbs.len() > capacity: remove LAST orb` - silently dropped: NOT evoked, no hook, `HasBeenRemovedFromState` not set (OrbQueue.cs L41-48).
- `Insert(idx, orb)`: throws if `idx >= capacity` (L71); used by effects that insert at front.
- Queue cleared and capacity zeroed when the player dies (CreatureCmd.cs ~L575).

### 5.2 Channel
```
fn channel(player, orb):                                  // OrbCmd.Channel L68-91
  if combat over/ending: return
  if player.Character.BaseOrbSlotCount == 0 && queue.capacity == 0:   // CHARACTER constant, not queue state
        add_slots(1)                                               // non-Defect characters get a slot lazily
  orb.owner = player
  if queue.orbs.len() >= queue.capacity:
        evoke_next(dequeue=true)                                   // evokes FRONT orb (no-op if queue empty)
  if queue.try_enqueue(orb):                                       // returns false iff capacity==0 -> orb silently lost
        history.orb_channeled; Hook.AfterOrbChanneled(orb)
```
- Defect with capacity 0 (all slots removed) and a Channel: front-evoke is a no-op (queue empty), `TryEnqueue` returns false -> orb lost, no hook.
- `TryEnqueue` throws if `orbs.len() >= capacity` after the evoke (can only happen via re-entrancy: an `AfterOrbEvoked` listener channeling another orb). Rust: treat as unreachable/panic, flag if a listener exists.
- Hook `AfterOrbChanneled` fires after the orb is in the queue.

### 5.3 Evoke
```
fn evoke(orb, dequeue):                                  // OrbCmd.Evoke L125-152
  if combat over/ending: return; if queue empty: return
  removed = false
  if dequeue: removed = queue.remove(orb)                // removal happens BEFORE the effect
  effect = orb.evoke()                                   // may damage / gain block / gain energy
  if player.creature.combat_state != null:
        Hook.AfterOrbEvoked(orb, targets)
        if removed: orb.RemoveInternal()                 // HasBeenRemovedFromState = true
fn evoke_next(dequeue=true)  = if !orbs.empty { evoke(orbs.first(), dequeue) }
fn evoke_last(dequeue=true)  = if !orbs.empty { evoke(orbs.last(),  dequeue) }
```
`dequeue=false` (Dualcast's FIRST evoke, `Dualcast.cs`): effect runs, orb stays, `RemoveInternal` is NOT called.
Orb "listeners": orbs are in the hook-listener list (OrbModel.ShouldReceiveCombatHooks = true, OrbModel.cs; CombatState.cs ~L449 adds `OrbQueue.Orbs`); in current content they only override passive/evoke/turn triggers.

### 5.4 Values and Focus
`ModifyOrbValue(x) = Hook.ModifyOrbValue(x)`: folds every listener's `ModifyOrbValue(orb, value)` in hook order (Hook.cs L1882-1890).
- `FocusPower.ModifyOrbValue`: if `power.Owner.Player == orb.Owner` -> `max(value + Amount, 0)` (FocusPower.cs; Focus may be negative, floored at 0 at that step).
- `InfusedCore` relic (+`ExtraDamage` to LIGHTNING orbs only, `InfusedCore.cs:46-58`; also channels `Lightning` DynamicVar orbs in `AfterSideTurnStart` when `TurnNumber<=1`).
- Since powers precede relics in hook order, Focus is applied first, then relic additions.
- `ModifyOrbPassiveTriggerCounts` (AbstractModel.cs L1524): `GoldPlatedCables` returns `count+1` when `orb == Orbs[0]` (front orb); `Hook.ModifyOrbPassiveTriggerCount` folds all (Hook.cs L1863-1876).

### 5.5 Orb definitions

| Orb | Passive (value) | Evoke (value) | Trigger timing |
|---|---|---|---|
| Lightning (LightningOrb.cs) | damage `ModifyOrbValue(3)` | damage `ModifyOrbValue(8)` | passive at BeforeTurnEnd |
| Frost (FrostOrb.cs) | block `ModifyOrbValue(2)` | block `ModifyOrbValue(5)` | passive at BeforeTurnEnd |
| Dark (DarkOrb.cs) | `_evokeVal += ModifyOrbValue(6)` (no direct effect) | damage `_evokeVal` (NOT focus-modified at evoke time) | passive at BeforeTurnEnd |
| Plasma (PlasmaOrb.cs) | gain 1 energy (not Focus-modified) | gain 2 energy | passive at AfterTurnStart |
| Glass (GlassOrb.cs) | see below | see below | passive at BeforeTurnEnd |

Details:
- **Lightning**: `ApplyLightningDamage(value, target, isEvoke)`:
  `opps = CombatState.GetOpponentsOf(owner).filter(IsHittable)`; if empty -> return (no draw). Targets = `[target]` if given (TeslaCoil passes the played card's target) else `[ CombatTargets.NextItem(opps) ]` (**one `combat_targets` draw, even with one enemy**). `CreatureCmd.Damage(targets, value, ValueProp.Unpowered, dealer=owner.Creature)` - Unpowered => Strength/Weak/Vulnerable-type "powered attack" modifiers do not apply (IsPoweredAttack requires Move && !Unpowered, ValuePropExtensions.cs L5-12); Block applies (no Unblockable flag).
- **Frost**: `GainBlock(owner.Creature, val, Unpowered, cardPlay=null)`: Dexterity does not apply (Unpowered) but `Hook.ModifyBlock` listeners still run. (Hibernate/multiplayer branch ignored.) Passive with non-null `target` throws.
- **Dark**: `Passive`: `_evokeVal += PassiveVal` where `PassiveVal=ModifyOrbValue(6)` (Focus counted when it accumulates). Starting `_evokeVal = 6`. `Evoke`: `hittable = CombatState.HittableEnemies` (Enemies order); if empty -> nothing; target = `MinBy(CurrentHp)` (first minimal in list order); `Damage(target, _evokeVal, Unpowered, dealer=owner)`.
- **Plasma**: `PlayerCmd.GainEnergy` -> `Hook.ModifyEnergyGain` -> energy += amount (PlayerCmd.cs L~21-36); `GainEnergy` no-ops if combat ending or amount<=0.
- **Glass**: state `_passiveVal=4` (base). `PassiveVal = ModifyOrbValue(_passiveVal)`; `EvokeVal = PassiveVal*2`.
  Passive: `targets = HittableEnemies`; `v = PassiveVal`; if `v <= 0` -> nothing, else `_passiveVal = max(0, _passiveVal-1)` (decrement of BASE, v was computed before) then `Damage(targets, v, Unpowered, dealer=owner)`.
  Evoke: if `EvokeVal <= 0` nothing, else damage all hittable enemies `EvokeVal` (Unpowered).
- `OrbModel.GetRandomOrb(rng)` = `rng.NextItem([Lightning, Frost, Dark, Plasma, Glass])` (OrbModel.cs L16-22, L~133).

### 5.6 Timing
- Turn start: `CombatManager.StartTurn` -> after `SetupPlayerTurn` (energy reset, draw) AND after `Hook.AfterSideTurnStart`: `OrbQueue.AfterTurnStart` iterates `Orbs.ToList()` front->back calling `AfterTurnStartOrbTrigger` (only Plasma overrides: `TriggerPassive`) (CombatManager.cs L783-793; OrbQueue.cs L92-102). Aborts if `CombatState==null`.
- Turn end: `DoTurnEnd` (CombatManager.cs L1599-1612) first line is `OrbQueue.BeforeTurnEnd`: front->back snapshot, each calls `BeforeTurnEndOrbTrigger` (Lightning/Frost/Dark/Glass: `TriggerPassive`). This runs after `AfterAutoPostPlayPhaseEntered` and `Hook.BeforeSideTurnEnd` (L1555), and BEFORE ethereal exhaust and end-of-turn-in-hand card effects (L1608+). If the combat ends mid-way (enemy dies to Lightning), `if (!turnState.IsInProgress || IsCombatEnding) return` (L1603-1606).
- `OrbModel.TriggerPassive` (OrbModel.cs L244-261): `count = Hook.ModifyOrbPassiveTriggerCount(1)`; `for i in 0..count: orb.Passive(target)`.
- Direct passive triggers bypassing count hooks (`OrbCmd.Passive(..., countAffectedByHooks:false)` -> `orb.Passive` once): LoopPower (front orb, `LoopPower.cs:21`), Darkness card, TeslaCoil card (twice if upgraded, with target). Using `countAffectedByHooks:true` (EmotionChip.cs:54) goes through `TriggerPassive`.
- Evoke triggers: cards (Dualcast, Multicast, etc.), overflow in Channel, potions, etc.

### 5.7 Hook points to model
`AfterOrbChanneled(player, orb)`, `AfterOrbEvoked(orb, targets)`, `ModifyOrbValue`, `ModifyOrbPassiveTriggerCounts` + `AfterModifyingOrbPassiveTriggerCount`, `History.OrbChanneled`.

---------------------------------------------------------------------------------------------------

## 6. Osty / pets / summons (Necrobinder)

Files: `C/Commands/OstyCmd.cs`, `C/Models/Monsters/Osty.cs`, `C/Models/Powers/DieForYouPower.cs`, `C/Entities/Players/PlayerCombatState.cs L17,L229-258,L281-292`, `Player.cs L124-142`, `CreatureCmd.cs L285-330`, `AttackCommand.cs L238-251`.
(No "souls"/"doom" subsystem outside power classes - Soul is just a token card, s10; Doom is a power, s9.)

### 6.1 Representation
Osty is a `Creature` whose `Monster` is `Osty : MonsterModel`, on the Player side (`Side = Player`), `PetOwner = player`.
- `Osty.MinInitialHp = MaxInitialHp = 1` (Osty.cs L25-27) but actual HP is set by `Summon`.
- Move machine: a single no-op "NOTHING_MOVE" looping on itself (Osty.cs L40-46). Osty never acts on its own; it only "attacks" when a Necrobinder card does `DamageCmd.Attack(...).FromOsty(Owner.Osty, card, cardPlay)`.
- Creature list position: `Allies = [player, ..., osty]` (`AddCreature` appends, CombatState.cs L535-548). Created via `PlayerCmd.AddPet<Osty>` (PlayerCmd.cs ~L261-282): `CombatState.CreateCreature(Osty.ToMutable(), Player side)` (so `CombatId` = next id) then `PlayerCombatState.AddPetInternal`, `CreatureCmd.Add` (-> `Hook.AfterCreatureAddedToCombat`).
- `Player.Osty = PlayerCombatState.GetPet<Osty>()` = first pet whose monster is Osty (dead Osty stays in `_pets` because it is not "removed from combat": DieForYou `ShouldCreatureBeRemovedFromCombatAfterDeath` returns false for Osty, `OnPetDied` keeps it, PlayerCombatState.cs L281-292). `IsOstyAlive = Osty?.IsAlive`; `IsOstyMissing = !IsOstyAlive` (Player.cs L132-142).
- Pets are cleared at `PlayerCombatState.AfterCombatEnd` (L142-151): Osty does not persist between combats; it is re-summoned by BoundPhylactery each combat.
- Powers on Osty: `DieForYouPower` (amount 1, applied at first creation, `OstyCmd.cs L74`). `ShouldPowerBeRemovedAfterOwnerDeath=false` so it survives Osty's death (and revival).

### 6.2 Summon (OstyCmd.cs L35-91)
```
fn summon(player, amount, source):
  amount = Hook.ModifySummonAmount(player, amount, source)         // folds listeners
  if amount == 0: return
  osty = Allies.find(c => c.Monster is Osty && c.PetOwner == player)
  if player.IsOstyAlive:
        CreatureCmd.GainMaxHp(osty, amount)        // maxHp += amount AND heal by the max-hp delta (CreatureCmd.cs L838-851: SetMaxHp then Heal(delta))
  else:
        reviving = osty != null
        if reviving: PlayerCombatState.AddPetInternal(osty)        // no-op (already in pets)
        else:        osty = AddPet<Osty>(player); apply DieForYouPower(1) to osty
        CreatureCmd.SetMaxHp(osty, amount)         // maxHp = amount (if <=0 -> Kill)
        CreatureCmd.Heal(osty, amount)             // currentHp = min(amount, maxHp) -> full HP at the NEW max
        if reviving: Hook.AfterOstyRevived(osty)
  History.Summoned; Hook.AfterSummon(player, amount)
```
Consequences: reviving resets max HP to `amount` (does NOT keep the old max); a living Osty accumulates max HP (and current HP by the same delta).
Sources of Summon: Bodyguard card (5, upgrade 7), BoundPhylactery (1 at combat start; 1 each turn >=2 after energy reset), PhylacteryUnbound relic (StartOfCombat/StartOfTurn vars), SummonNextTurnPower (Invoke card: `Summon` next turn + `EnergyNextTurnPower`), DevourLifePower, SicEmPower (on Osty hit), BoneBrew potion, etc.

### 6.3 Damage involving Osty (CreatureCmd.Damage, CreatureCmd.cs L285-330)
For each target (alive):
```
modified = Hook.ModifyDamage(...)                       // dealer-specific powers (below)
Hook.BeforeDamageReceived
blockOwner = target.PetOwner?.Creature ?? target        // *** damage to Osty consumes the PLAYER's block ***  (L285)
blocked   = blockOwner.DamageBlockInternal(modified)    // unless Unblockable
unblocked = Hook.ModifyHpLost(target, max(modified-blocked,0), phase=BeforeOsty)
unblockedTarget = Hook.ModifyUnblockedDamageTarget(target, ...)   // DieForYou: redirect to Osty
unblocked = Hook.ModifyHpLost(unblockedTarget, unblocked, phase=AfterOsty)
result = unblockedTarget.LoseHpInternal(unblocked)      // result.OverkillDamage = max(unblocked - hpBefore, 0) if killed
if target != unblockedTarget (Osty absorbed):
        spill = Hook.ModifyHpLost(target, result.OverkillDamage, AfterOsty)   // overkill goes back to the PLAYER
        if spill > 0: target.LoseHpInternal(spill)
```
- `DieForYouPower.ModifyUnblockedDamageTarget` (L15-33): redirect iff `target == Owner.PetOwner.Creature` (i.e., the player), Osty is alive, and `props.IsPoweredAttack()` (Move flag set and Unpowered NOT set). So **only monster attack moves** (powered) are absorbed; Unpowered damage (orbs, poison, thorns, self damage), `Unblockable`, etc. hit the player directly.
- `DieForYouPower.ShouldAllowHitting(creature)` = `creature.IsAlive`; it is a listener in the combat hook list whenever Osty is in the creature list. Dead Osty is not hittable (`IsHittable` false) and is excluded from `GetPossibleTargets` filters `c.IsAlive`.
- Monster multi-target attacks use `CombatState.PlayerCreatures` (the player only), never Osty directly (AttackCommand.cs L171-174). Card AOE `GetOpponentsOf(attacker)`.
- Potions/cards that target "AnyPlayer"/"AnyAlly": in single-player the only valid player target is the player (PotionModel.IsValidTarget L~215-235: AnyPlayer requires `target.IsPlayer`); `TargetType.Osty` exists in the enum (TargetType.cs L34) but is **referenced by no card/potion** in Models (only UI NCardPlay.cs:359). Osty can still be healed/killed by card code directly (`Spur`: `Heal(Owner.Osty)`; `BoneShards`, `Sacrifice`: `Kill(Owner.Osty)`).
- Osty's own `Block` field exists but damage always uses the owner's block; `Creature.AfterTurnStart` clears block for every creature on the side (Osty included) - harmless.
- Osty death: `CreatureCmd.Kill` -> HP to 0, `Died` event, not removed from combat; `RemoveAllPowersAfterDeath` removes all powers except those with `ShouldPowerBeRemovedAfterOwnerDeath()==false` (DieForYou). When the player dies, `if player.IsOstyAlive: Kill(Osty)` (CreatureCmd.cs ~L578).
- `PlayerCombatState.Pets` is a list; only Osty exists as a pet in the content shipped for Necrobinder. Relics `SpawnsPets` (BoundPhylactery, Byrdpip, PaelsLegion, PhylacteryUnbound) - Byrdpip/PaelsLegion are event/ancient pets (check relic spec; `Player.HasEventPet`, Player.cs L248-255).

### 6.4 How Osty attacks (cards)
`AttackCommand.FromOsty(osty, card, cardPlay)` (AttackCommand.cs L238-251): requires `osty.Monster is Osty`; `Attacker = osty`, model source = card. Cards guard with `Osty.CheckMissingWithAnim(Owner)` (= `owner.IsOstyMissing`): if Osty is dead/missing the card's effect is **skipped** (card is still played and its cost paid; for `Unleash`/`Poke` etc.).
Damage computation uses `Hook.ModifyDamage(runState, combatState, target, dealer = Osty creature, damage, props, card, ...)` (OstyDamageVar.cs L36-52; CalculatedDamageVar `FromOsty`): additive/multiplicative modifiers whose predicate is `dealer == Owner` (StrengthPower.cs, WeakPower.cs: `Owner != dealer -> no effect`) apply only if the Owner is **Osty**; i.e. the PLAYER's Strength/Weak do NOT affect Osty attacks. Vulnerable on target applies normally. Osty can only gain Strength etc. via effects that explicitly target it (none found for the base content; grep `Owner.Osty`).
Osty-specific modifiers found: `CalcifyPower` (`dealer.Monster is Osty`), `PersonalHivePower` (`dealer.Monster is Osty`), `BoneFlute` relic (block when Osty attacks), `PenNib`/`TheBoot` (`dealer == Owner.Creature || dealer == Owner.Osty`), `NecroMasteryPower` (Osty HP loss -> hits enemies; checks `creature.Monster is Osty && PetOwner == Owner.Player`), `SicEmPower`, `SandpitPower`, `BeatingRemnant`, `TungstenRod`, `Intangible/Buffer/HardenedShell/Slippery` (use the BeforeOsty/AfterOsty `ModifyHpLost` phases: s6.3).
"Osty has attacked this turn" queries use the combat history: `CreatureAttackedEntry.Actor == Owner.Osty && HappenedThisTurn` (Flatten, Rattle) - so the sim must keep a per-turn attack history of actor ids.
`CardTag.OstyAttack` cards: Poke (6, upg 9), Flatten (12 upg 16, costs 0 after Osty attacked this turn via `EnergyCost.SetThisTurn(0)`), Unleash, Fetch, Protector, RightHandHand, HighFive, Squeeze, Rattle, SweepingGaze, SicEm, Snap, BoneShards, ... (content spec).

---------------------------------------------------------------------------------------------------

## 7. Stars (Regent)

Files: `PlayerCombatState.cs L23, L103-119, L190-227`, `PlayerCmd.cs L63-115`, `CardModel.cs L410-470, L1541-1600, L1795-1850`, `CombatManager.cs L1229`, `Hook.cs L2339-2362`.
- State: `PlayerCombatState.Stars: i32`, starts at 0 each combat (fresh `PlayerCombatState`). Persists across turns (no per-turn reset found; only `SetStars(0)` at player death, CombatManager.cs L1229). No upper bound.
- `PlayerCmd.GainStars(n)`: no-op if combat ending or `Hook.ShouldGainStars(n)` false; `Stars = max(Stars+n, 0)`; then `Hook.AfterStarsGained`. `LoseStars`: `Stars = max(Stars-n,0)` (no hook). `SetStars` = Gain/Lose difference. `History.StarsModified` recorded on every change (PlayerCombatState.cs L113-116).
- Card star cost: `CanonicalStarCost` default -1 (no star cost; FallingStar overrides 2). `CurrentStarCost = temp-cost (last) else BaseStarCost` (CardModel.cs L451-466). `GetStarCostWithModifiers()`: X-cost cards (`HasStarCostX`) return all current Stars; else `Hook.ModifyStarCost(card, CurrentStarCost)` if the card is in a combat pile. Temporary star-cost API: `SetStarCostUntilPlayed/ThisTurn/ThisCombat` (CardModel.cs L1279-1291).
- Playability (`PlayerCombatState.HasEnoughResourcesFor`, L190-209): `energyCost = max(0, EnergyCost.GetWithModifiers(All))`, `starCost = max(0, GetStarCostWithModifiers())`; if `energyCost > Energy` and `Hook.ShouldPayExcessEnergyCostWithStars`: `starCost += (energyCost - Energy)*2; energyCost = Energy`. Fails with `EnergyCostTooHigh` / `StarCostTooHigh`.
- Spending (`CardModel.SpendResources`, L1805-1850): same excess conversion; energy first (`History.EnergySpent`, `LoseEnergy`, `Hook.AfterEnergySpent`), then stars (`LastStarsSpent`, `LoseStars`, `Hook.AfterStarsSpent`).
- Stars sources: Venerate (2, upg 3), DivineRight (+3 on entering a combat room; happens after `SetUpCombat`, before `BeforeCombatStart` = effectively initial Stars=3), potions (StarPotion), many Regent cards/powers (`GalacticDust` relic counts stars spent: `[SavedProperty] StarsSpent`).

---------------------------------------------------------------------------------------------------

## 8. Forge / Sovereign Blade (Regent)

Files: `C/Commands/ForgeCmd.cs`, `C/Models/Cards/SovereignBlade.cs`.
```
fn forge(amount, player, source):                             // ForgeCmd.cs L35-52
  if combat over/ending: return []
  blades = player.PlayerCombatState.AllCards (hand,draw,discard,exhaust,play) .filter(SovereignBlade && !IsDupe && pile != Exhaust)
  if blades.empty():
        b = CombatState.CreateCard<SovereignBlade>(player); b.CreatedThroughForge = true
        CardPileCmd.AddGeneratedCardToCombat(b, Hand, player)       // appended to hand (position Bottom); hand full -> goes to Discard, see s10
  // damage increase applies to ALL blades INCLUDING exhausted ones, excluding IsDupe:
  for b in AllCards.filter(SovereignBlade && !IsDupe):   b.AddDamage(amount); b.AfterForged()
  Hook.AfterForge(amount, player, source)
```
SovereignBlade (`SovereignBlade.cs`): rarity Token, cost 2 (upgrade: cost -1 => 1), type Attack, Retain keyword, base Damage 10 (`DamageVar(10)`), `RepeatVar(1)` = number of hits (`WithHitCount(Repeat.IntValue)`), dynamic TargetType: AnyEnemy, or AllEnemies when owner has `SeekingEdgePower`. `GainsBlock` if owner has `ParryPower` amount>0: after the attack gains `CalculatedBlock` = `CalculationBase 0 + CalculationExtra 1 x ParryAmount`. `AddDamage` mutates `DynamicVars.Damage.BaseValue` and the private `CurrentDamage` mirror (so a card re-created from `AfterDowngraded` restores it). `SetRepeats(n)` for multi-hit variants. `AfterCloned` resets `CreatedThroughForge=false` (clones via `Clone`/dupe keep current damage in DynamicVars).
No persistent (cross-combat) state: SovereignBlade is generated in combat.
`IsDupe`/`DupeOf` are card flags for duplicated cards (copies created by Dupe-like effects); the sim must carry `is_dupe`.

---------------------------------------------------------------------------------------------------

## 9. Poison and Doom (both fully inside power classes; summarized for completeness)

Source: `C/Models/Powers/PoisonPower.cs`, `DoomPower.cs`.
- **Poison** (counter, debuff): `AfterSideTurnStart(side, participants)`: if Owner in participants -> `Trigger()`. `TriggerCount = min(Amount, 1 + sum(AccelerantPower amount of living opponents of Owner))`; for each iteration: `Damage(Owner, Amount, Unblockable|Unpowered, dealer=null)` then, if Owner still alive, `PowerCmd.Decrement(this)` (Amount-1; removed at 0). The loop reads the current `Amount` each iteration. (Poison on the player works identically for enemy-applied Poison.) `CalculateTotalDamageNextTurn` is a preview that runs `Hook.ModifyDamage` per tick with dealer null.
- **Doom** (counter, debuff): enemy-side doom: `BeforeSideTurnEnd` for `side != Player`; player-side: `AfterSideTurnEnd` for `side != Enemy`. Triggers only for the FIRST doomed creature of that side (`doomed.First() == Owner`) and only if `Owner.CurrentHp <= Amount`; then `DoomKill(all doomed creatures on the side)`: for each `CreatureCmd.Kill(creature)` (normal kill: Fatal/death-prevention listeners run; reviving powers like ReattachPower matter), then `Hook.AfterDiedToDoom(creatures)`. `EndOfDays` card calls `DoomKill` directly. No decay of Doom stacks. Not triggered if combat is over/ending.
- Nothing about these two lives in Commands/Combat besides generic hook dispatch (grep `PoisonPower|DoomPower` in Combat/Commands/Hooks/Entities: no hits).

---------------------------------------------------------------------------------------------------

## 10. Shivs, Souls and other token cards

- Token rarity (`CardRarity.Token`): Shiv, Soul, SovereignBlade, Luminesce, Fuel, MinionDiveBomb, MinionSacrifice, MinionStrike, GiantRock (grep `CardRarity.Token`). Tokens do not appear in reward pools; all are created with `CombatState.CreateCard<T>(owner)` and inserted by `CardPileCmd.AddGeneratedCardsToCombat` (CardPileCmd.cs L281-312): no-op if combat not in progress; throws if the card already has a pile; for each: `History.CardGenerated`, `Add(card, pile, position=Bottom)`, `Hook.AfterCardGeneratedForCombat(card, creator)`. Hand capacity is `CardPile.MaxCardsInHand = 10` (CardPile.cs L21). **Overflow**: in `CardPileCmd.Add` (CardPileCmd.cs L484-490) `isFullHandAdd = target is Hand && hand.Count >= 10` -> the card is redirected to the DISCARD pile (position unchanged, normally Bottom = end of list). A generated Shiv/Soul/Sovereign Blade into a full hand therefore lands in discard (and Forge still finds it: `GetSovereignBlades` scans all combat piles). `Draw` simply stops when the hand is full (`DrawInternal` L1030-1058: no draw, no shuffle).
- **Shiv** (`Models/Cards/Shiv.cs`): cost 0, Attack, `AnyEnemy` (`AllEnemies` if the owner has `FanOfKnivesPower`: hits all enemies, `WithHitVfxNode`), 4 damage (`DamageVar(4, Move)`), upgrade +2, keyword Exhaust, tag `Shiv`. `Shiv.CreateInHand(owner, count, combatState, creator)` creates `count` separate cards. No RNG.
- **Soul** (`Soul.cs`): cost 0, Skill/Self, Exhaust, draws `CardsVar(2)` (upgrade +1). `Soul.CreateInHand/Create`.
- Necrobinder "souls": there is no separate resource - Soul is only this token card.
- Card-instance flags the sim must carry on generated cards: `owner`, `is_dupe` / `dupe_of`, keywords added locally (Ethereal etc.), enchantment, `energy_cost` local modifiers (`EnergyCost.SetThisTurn`, `SetThisCombat`, `SetThisTurnOrUntilPlayed`), `deck_version` link (for cards that write back to the deck: TheScythe/GeneticAlgorithm).

---------------------------------------------------------------------------------------------------

## 11. Potions

Files: `C/Models/PotionModel.cs`, `C/Commands/PotionCmd.cs`, `C/GameActions/UsePotionAction.cs`, `DiscardPotionGameAction.cs`, `C/Entities/Players/Player.cs L28, L42, L585-706`, `C/Factories/PotionFactory.cs`, `C/Nodes/Potions/NPotionPopup.cs L214-260, L426-457` (UI gating, which defines legality), `C/Models/PotionPools/*`.

### 11.1 Belt
`Player._potionSlots: Vec<Option<Potion>>` of length `MaxPotionCount` (default 3; A4: 2).
- `AddPotionInternal(potion, slot=-1)`: slot -1 -> first `null` slot; fails (`TooFull`) when no empty slot or the specified slot is occupied. `PotionCmd.TryToProcure`: first `Hook.ShouldProcurePotion` (relic Sozu etc. -> `NotAllowed`), then add, then `Hook.AfterPotionProcured`.
- Shrinking slot count (`SetMaxPotionCountInternal`, Player.cs L585-619): from the back, a potion in a removed slot moves to the first empty slot below the new count, else it is discarded (`DiscardPotionInternal`, with discard event).
- Growing adds empty slots.
- `Potion.Owner` fixed on add; `RemovePotionInternal` nulls the slot.

### 11.2 Using a potion in combat (the RL "use potion" action)
Action = `UsePotionAction(potion, target, isCombatInProgress)`; `ActionType = CombatPlayPhaseOnly` (UsePotionAction.cs L10-20) when enqueued in combat.
Legality (UI gate `NPotionPopup.RefreshButtons` L426-457 + action validation):
1. `player.CanUseOrRemovePotions` (true in combat; only 3 events set it false).
2. Potion `Usage`:
   - `CombatOnly` (60 potions): requires `CombatManager.IsInProgress && CombatState.CurrentSide == player side && creature.IsAlive && !InCardSelectScreen && !PlayerActionsDisabled` => usable during the player's play phase only (also not after pressing End Turn; not during enemy turn).
   - `AnyTime` (Ambergris, BloodPotion, EntropicBrew, FoulPotion, FruitJuice): usable anytime incl. combat player phase (FoulPotion also at merchant); `UsePotionAction` type still `CombatPlayPhaseOnly` when enqueued in combat.
   - `Automatic` (FairyInABottle only): never manually usable; triggered by death-prevention hook (`ShouldDie` returns false for the owner -> `AfterPreventingDeath` -> `OnUseWrapper`, heals `max(30% MaxHp, 1)` (`Math.Max(MaxHp*0.3m, 1m)`); target = the owner creature).
3. `Potion.PassesCustomUsabilityCheck` (default true; FoulPotion overrides).
4. Not `IsQueued`.
5. Target validity (`PotionModel.IsValidTarget`, L~195-235) - note the semantic differs from cards:
   - `AnyEnemy`: target alive and `target.Side != owner.Side` (so a specific living enemy must be chosen).
   - `AllEnemies` (ExplosiveAmpoule, PotionOfBinding, ShacklingPotion): no target (`target==null` is valid since not single-target).
   - `AnyPlayer` (51 potions): in single-player, `EnqueueManualUse` / `ExecuteAction` automatically substitute `Owner.Creature` (UsePotionAction.cs ExecuteAction: `creature = Player.Creature` when single-target and no TargetId). Osty is never a valid target (`IsPlayer` false).
   - `Self`: owner only. `AnyAlly`: non-owner ally (n/a in single-player).
   - `TargetedNoCreature` (FoulPotion at merchant/events): n/a in combat.
   - A dead/invalid target -> `Cancel()` and the potion stays in the slot.
6. No energy cost, no per-turn limit, any number of potions may be used per turn (also while holding 0 energy). Potions can be used while a card is mid-resolution only after it finishes (action queue serialises).

Execution (`PotionModel.OnUseWrapper`, PotionModel.cs L~245-295):
```
RemoveBeforeUse()                 // potion leaves its slot IMMEDIATELY (slot becomes None), HasBeenRemovedFromState
Hook.BeforePotionUsed(potion, target)
OnUse(choiceContext, target)      // potion effect (may use CombatCardGeneration / CombatPotionGeneration / etc.)
if owner not dead:
    History.PotionUsed; Hook.AfterPotionUsed(potion, target)
    CombatManager.CheckForEmptyHand(...)
```
Potions are also combat-hook listeners while in the belt (`ShouldReceiveCombatHooks=true`; belt order in the listener list, s12). `BeforeUse` just notifies UI.

### 11.3 Discarding a potion
`DiscardPotionGameAction(player, slotIndex, isCombatInProgress)` (`ActionType = CombatPlayPhaseOnly` in combat): if slot empty -> cancel; else `PotionCmd.Discard`: `potion.Discard()` (slot -> None via `Player.DiscardPotionInternal`) then `Hook.AfterPotionDiscarded`. No effect, no cost. Blocked only if `!CanUseOrRemovePotions`. Typical use: free a slot for a later pickup (Alchemize/EntropicBrew). Allowed anytime in the player's phase (button always enabled, NPotionPopup.cs L433).

### 11.4 Potion generation in combat
`PotionFactory.CreateRandomPotionInCombat(player, rng=RunState.Rng.CombatPotionGeneration, blacklist)` (PotionFactory.cs):
```
options = character_pool.GetUnlockedPotions(unlock) ++ SharedPotionPool.GetUnlockedPotions(unlock)   // concatenation ORDER matters
          .filter(p => p.CanBeGeneratedInCombat)          // excludes FairyInABottle, FruitJuice, RegenPotion (grep CanBeGeneratedInCombat => false)
f = rng.NextFloat()                                       // draw #1
rarity = f <= 0.10 ? Rare : f <= 0.35 ? Uncommon : Common
pick = rng.NextItem(options.filter(rarity == rarity))     // draw #2 (null if the bucket is empty -> would crash; buckets are non-empty in practice)
```
(`CreateRandomPotions(count)` repeats this, removing the picked potion from the list each time.) `PotionRarity` per potion in the table below. **EntropicBrew** (Models/Potions/EntropicBrew.cs) is different: `while (target.HasOpenPotionSlots) { p = PotionFactory.CreateRandomPotionOutOfCombat(player, Rng.CombatPotionGeneration); if (!TryToProcure(p).success) break; }` - i.e. it uses the OUT-of-combat generator (does NOT filter `CanBeGeneratedInCombat`, so it can produce FairyInABottle/FruitJuice/RegenPotion) but still draws from the `combat_potion_generation` stream, 2 draws per slot filled (rarity float, then item). `Alchemize` uses `CreateRandomPotionInCombat` (filtered).
Potions given to the belt: `PotionCmd.TryToProcure(potion.ToMutable(), player)` (fails if belt full -> potion wasted).

### 11.5 Pools (epoch-gated; list order = pool generation order)
- Character pools only contain 3 potions each and are returned only if the character's "4th" epoch is revealed (`IroncladPotionPool.GetUnlockedPotions` etc.):
  - Ironclad (Ironclad4Epoch): BloodPotion, SoldiersStew, Ashwater
  - Silent (Silent4Epoch): PoisonPotion, GhostInAJar, CunningPotion
  - Defect (Defect4Epoch): FocusPotion, EssenceOfDarkness, PotionOfCapacity
  - Necrobinder (Necrobinder4Epoch): PotionOfDoom, PotOfGhouls, BoneBrew
  - Regent (Regent4Epoch): StarPotion, CosmicConcoction, KingsCourage
- `SharedPotionPool` (45 potions, alphabetical, SharedPotionPool.cs): AttackPotion, BeetleJuice*, BlessingOfTheForge, BlockPotion, BottledPotential, Clarity, ColorlessPotion, CureAll, DexterityPotion, DistilledChaos, DropletOfPrecognition*, Duplicator, EnergyPotion, EntropicBrew, ExplosiveAmpoule, FairyInABottle, FirePotion, FlexPotion, Fortifier, FruitJuice, FyshOil, GamblersBrew, GigantificationPotion, HeartOfIron, LiquidBronze, LiquidMemories, LuckyTonic, MazalethsGift*, OrobicAcid, PotionOfBinding, PowderedDemise**, PowerPotion, RadiantTincture, RegenPotion, ShacklingPotion, ShipInABottle**, SkillPotion, SneckoOil, SpeedPotion, StableSerum, StrengthPotion, SwiftPotion, TouchOfInsanity**, VulnerablePotion, WeakPotion. (* removed unless `Potion1Epoch` revealed; ** removed unless `Potion2Epoch` revealed.) Other pools (Event, Token, Deprecated) are never generated in combat.
- Not generated in combat via pools: Ambergris, FoulPotion, GlowwaterPotion (Event), PotionShapedRock (Token).
- Rarity / usage / target table (from each potion file):

| Potion | Rarity | Usage | Target | CanBeGeneratedInCombat |
|---|---|---|---|---|
| Ambergris | Event | AnyTime | AnyPlayer | yes |
| Ashwater | Uncommon | CombatOnly | AnyPlayer | yes |
| AttackPotion | Common | CombatOnly | AnyPlayer | yes |
| BeetleJuice | Rare | CombatOnly | AnyEnemy | yes |
| BlessingOfTheForge | Uncommon | CombatOnly | AnyPlayer | yes |
| BlockPotion | Common | CombatOnly | AnyPlayer | yes |
| BloodPotion | Common | AnyTime | AnyPlayer | yes |
| BoneBrew | Uncommon | CombatOnly | AnyPlayer | yes |
| BottledPotential | Rare | CombatOnly | AnyPlayer | yes |
| Clarity | Uncommon | CombatOnly | AnyPlayer | yes |
| ColorlessPotion | Common | CombatOnly | AnyPlayer | yes |
| CosmicConcoction | Rare | CombatOnly | AnyPlayer | yes |
| CunningPotion | Uncommon | CombatOnly | AnyPlayer | yes |
| CureAll | Uncommon | CombatOnly | AnyPlayer | yes |
| DexterityPotion | Common | CombatOnly | AnyPlayer | yes |
| DistilledChaos | Rare | CombatOnly | AnyPlayer | yes |
| DropletOfPrecognition | Rare | CombatOnly | AnyPlayer | yes |
| Duplicator | Uncommon | CombatOnly | AnyPlayer | yes |
| EnergyPotion | Common | CombatOnly | AnyPlayer | yes |
| EntropicBrew | Rare | AnyTime | AnyPlayer | yes |
| EssenceOfDarkness | Rare | CombatOnly | AnyPlayer | yes |
| ExplosiveAmpoule | Common | CombatOnly | AllEnemies | yes |
| FairyInABottle | Rare | Automatic | Self | NO |
| FirePotion | Common | CombatOnly | AnyEnemy | yes |
| FlexPotion | Common | CombatOnly | AnyPlayer | yes |
| FocusPotion | Common | CombatOnly | AnyPlayer | yes |
| Fortifier | Uncommon | CombatOnly | AnyPlayer | yes |
| FoulPotion | Event | AnyTime | (none) | yes |
| FruitJuice | Rare | AnyTime | AnyPlayer | NO |
| FyshOil | Uncommon | CombatOnly | AnyPlayer | yes |
| GamblersBrew | Uncommon | CombatOnly | AnyPlayer | yes |
| GhostInAJar | Rare | CombatOnly | AnyPlayer | yes |
| GigantificationPotion | Rare | CombatOnly | AnyPlayer | yes |
| GlowwaterPotion | Event | CombatOnly | AnyPlayer | yes |
| HeartOfIron | Uncommon | CombatOnly | AnyPlayer | yes |
| KingsCourage | Uncommon | CombatOnly | AnyPlayer | yes |
| LiquidBronze | Uncommon | CombatOnly | AnyPlayer | yes |
| LiquidMemories | Rare | CombatOnly | AnyPlayer | yes |
| LuckyTonic | Rare | CombatOnly | AnyPlayer | yes |
| MazalethsGift | Rare | CombatOnly | AnyPlayer | yes |
| OrobicAcid | Rare | CombatOnly | AnyPlayer | yes |
| PoisonPotion | Common | CombatOnly | AnyEnemy | yes |
| PotionOfBinding | Uncommon | CombatOnly | AllEnemies | yes |
| PotionOfCapacity | Uncommon | CombatOnly | AnyPlayer | yes |
| PotionOfDoom | Common | CombatOnly | AnyEnemy | yes |
| PotionShapedRock | Token | CombatOnly | AnyEnemy | yes |
| PotOfGhouls | Rare | CombatOnly | AnyPlayer | yes |
| PowderedDemise | Uncommon | CombatOnly | AnyEnemy | yes |
| PowerPotion | Common | CombatOnly | AnyPlayer | yes |
| RadiantTincture | Uncommon | CombatOnly | AnyPlayer | yes |
| RegenPotion | Uncommon | CombatOnly | AnyPlayer | NO |
| ShacklingPotion | Rare | CombatOnly | AllEnemies | yes |
| ShipInABottle | Rare | CombatOnly | AnyPlayer | yes |
| SkillPotion | Common | CombatOnly | AnyPlayer | yes |
| SneckoOil | Rare | CombatOnly | AnyPlayer | yes |
| SoldiersStew | Rare | CombatOnly | AnyPlayer | yes |
| SpeedPotion | Common | CombatOnly | AnyPlayer | yes |
| StableSerum | Uncommon | CombatOnly | AnyPlayer | yes |
| StarPotion | Common | CombatOnly | AnyPlayer | yes |
| StrengthPotion | Common | CombatOnly | AnyPlayer | yes |
| SwiftPotion | Common | CombatOnly | AnyPlayer | yes |
| TouchOfInsanity | Uncommon | CombatOnly | AnyPlayer | yes |
| VulnerablePotion | Common | CombatOnly | AnyEnemy | yes |
| WeakPotion | Common | CombatOnly | AnyEnemy | yes |

(`DeprecatedPotion` excluded.) Potion effects themselves belong to the content spec.

### 11.6 In-combat card generation by potions/cards (the RNG pattern)
`CardFactory.GetDistinctForCombat(player, pool, count, rng)` (CardFactory.cs ~L105-114): pool = `Character.CardPool.GetUnlockedCards(unlock, constraint)` filtered by caller (type/rarity), then `FilterForPlayerCount`, `FilterForCombat` (drops `CanBeGeneratedInCombat==false`), then `TakeRandom(count, rng)` = `UnstableShuffle` of the **whole filtered list** (list.len()-1 draws!) and take the first `count`. So the pool ORDER (AllCards generation order) and its length determine both outcomes and stream advancement. Callers pass `RunState.Rng.CombatCardGeneration`. `CardRarity`/`PlayerOdds` are NOT involved in this path. (`CreateForReward` and its `PlayerRng.Rewards` rolls are reward-only.)

---------------------------------------------------------------------------------------------------

## 12. Hook-listener order (`CombatState.IterateHookListeners`, CombatState.cs L411-501)

Used by every `Hook.*` dispatch (via `IterateCombatHookListeners`; yields nothing while combat is over/ending unless starting, Hook.cs L53-63).
For damage/block/etc. modification hooks the dispatcher is `runState.IterateHookListeners(combatState)` (RunState.cs L545-596) which first yields run-level listeners (for each active player: deck cards + deck enchantments; with `childCombatState==null` also relics/potions/modifiers/badges), THEN the combat listeners.
Combat listener list is built as:
```
for creature in Allies ++ Enemies (Allies order: player, then pets/Osty in add order; Enemies in slot order):
    list += creature.Powers (in application order)
    if creature is a monster (incl. Osty): list += creature.Monster        // Osty's Monster model; its powers (DieForYou) already added
    else (player):  if !player.IsActiveForHooks: skip rest
        list += player.Relics (list order; skip IsMelted)
        list += player.PotionSlots (slot order, non-null)
        if player.PlayerCombatState: list += OrbQueue.Orbs (front->back)
             list += for pile in [Hand, Draw, Discard, Exhaust, Play]: for card in pile (pile order): card, card.Affliction?, card.Enchantment?
list += CombatState.Modifiers; list += BadgeModels; list += MultiplayerScalingModel
then yields only models for which Contains(model) (still attached to state)
```
Therefore the following are INPUTS to ordering: relic list order, potion slot index order, deck order (via initial pile order), power application order, enemy slot order.
Each dispatch snapshots the list at its start (`list` is materialised before yielding, but `Contains` is evaluated lazily at yield time - a model removed mid-iteration is skipped).

---------------------------------------------------------------------------------------------------

## 13. Persistent per-card / per-relic / per-enchantment state

### 13.1 Cards with `[SavedProperty]` (full list, grep of `Models/`)
| Card | Saved fields | Meaning |
|---|---|---|
| TheScythe | `CurrentDamage:int` (init 13), `IncreasedDamage:int` | permanent damage growth; the **Ritual-Dagger analogue** (increase 5 per "Increase" var, written to the deck card via `DeckVersion`) |
| GeneticAlgorithm | `CurrentBlock:int`, `IncreasedBlock:int` | permanent block growth |
| Dowsing | `RoomsEntered:int` | counter |
| Guilty | `CombatsSeen:int` | counter |
| SpoilsMap | `SpoilsActIndex:int` | act marker |
| MadScience | `TinkerTimeType:CardType` (default -1), `TinkerTimeRider` | player-chosen card mode |
There is no Searing Blow analog and no card with `MaxUpgradeLevel > 1`.
Combat cards carry `DeckVersion` (the deck card); effects that grow a card permanently (`TheScythe`, `GeneticAlgorithm`) update `DeckVersion` - they are *outputs* of combat that must be written back to the deck by the env (s15).
Cards in combat also hold non-serialized state: `EnergyCost` local modifiers, `Keywords` added locally, `Affliction` (Bound/Entangled/Galvanized/Hexed/Ringing/Smog/Tainted - applied by monsters in combat only, never persisted: not in `SerializableCard`), `Enchantment.Status`, `ReplayCount`, `CurrentTarget`, `Drawn`.

### 13.2 Enchantments (id + amount, `SerializableEnchantment`)
Ids: Adroit, Clone, Corrupted, Glam, Goopy, Imbued, Inky, Instinct, Momentum, Nimble, PerfectFit, RoyallyApproved, Sharp, Slither, SlumberingEssence, SoulsPower, Sown, Spiral, Steady, Swift, TezcatarasEmber, Vigorous. `amount` is the stack/strength. Combat-time private state (NOT serialized, resets each combat via cloning `ClonePreservingMutability`): `Glam._usedThisCombat`, `Momentum._extraDamage` (accumulates within combat), `Slither` (random cost via `CombatEnergyCosts`), `EnchantmentModel._status` (Normal/Disabled). `ShouldStartAtBottomOfDrawPile` (turn-1 reordering, s1 T5) is an enchantment property.

### 13.3 Enchantment on deck vs combat
Combat clone: `CloneCard` -> `ClonePreservingMutability`: the clone gets a cloned enchantment via `EnchantInternal(clone, amount)` (CardModel.cs L1195-1222).

### 13.4 Relic persistent state (`[SavedProperty]`, full grep)
Counters/flags that carry across combats (initialize from save): ArchaicTooth (`StarterCard`,`AncientCard`: SerializableCard), BoneTea/EmberTea/TeaOfDiscourtesy (`CombatsLeft`), Byrdpip/PaelsLegion (`Skin: string`, cosmetic), BookOfFiveRings (`CardsAdded`), FakeHappyFlower/HappyFlower/Pendulum/PollinousCore (`TurnsSeen`), DustyTome (`AncientCard: ModelId`), GoldenCompass (`GoldenPathAct`), FishingRod/ToyBox (`CombatsSeen`), FakeVenerableTeaSet/VenerableTeaSet (`GainEnergyInNextCombat:bool`), Nunchaku/PenNib (`AttacksPlayed`), MawBank (`HasItemBeenBought`), IronClub (`CardsPlayed`), LastingCandy (`CombatRewardsSeen`), FurCoat (`FurCoatActIndex`, `FurCoatCoordCols/Rows`, `FurCoatCoordsSet`), JossPaper (`CardsExhausted`), LavaLamp (`TookDamageThisCombat`), GalacticDust (`StarsSpent`), LavaRock (`HasTriggered`), Girya (`TimesLifted`), LizardTail (`WasUsed`), SeaGlass (`CharacterId`), TuningFork (`SkillsPlayed`), PumpkinCandle (`KindleCount`), TouchOfOrobas (`StarterRelic`,`UpgradedRelic`), PaelsTooth (`SerializableCards: list`), WingedBoots/SilverCrucible (`TimesUsed`, `TreasureRoomsEntered`), SwordOfStone (`ElitesDefeated`), WongosMysteryTicket (`CombatsFinished`,`GaveRelic`), PaelsWing (`RewardsSacrificed`), SilkenTress (`IsUsed`), base RelicModel (`IsWax`, `IsMelted`: melted relics are skipped in hook iteration).
Relics that count (Nunchaku, PenNib, TuningFork, IronClub...) carry the counter across combats via these saved props - the Rust relic implementations must take these as initial values and write them back. Whether OTHER (non-`[SavedProperty]`) relic fields persist in memory across combats of a live run (and would therefore diverge from a save-derived input) was NOT verified - see s16 item 16.
Relics with combat-start effects (hooks that fire before the first player action): `BeforeCombatStart(+Late)`: Anchor, BeltBuckle, BoundPhylactery, Byrdpip, DelicateFrond, FakeAnchor, FakeSneckoEye, FurCoat, Kusarigama, LetterOpener, MeatOnTheBone, PaelsFlesh, PaelsLegion, Pantograph, PetrifiedToad, PhylacteryUnbound, SneckoEye, TeaOfDiscourtesy, UnsettlingLamp, Vambrace; powers GalvanicPower, VitalSparkPower. `AfterRoomEntered`: BigMushroom, BronzeScales, BurningSticks, DataDisk, DivineRight, EmberTea, EternalFeather, FakeVenerableTeaSet, GhostSeed, Girya, Gorget, LavaLamp, LordsParasol, MawBank, MealTicket, Metronome, OddlySmoothStone, Pantograph, Permafrost, PhilosophersStone, Planisphere, RedSkull, RegalPillow, SilverCrucible, SlingOfCourage, StoneCalendar, StoneCracker, SwordOfJade, ThrowingAxe, Vajra, VelvetChoker, VenerableTeaSet, WingedBoots (many are room-type-gated; consult relic spec). Energy/draw modifiers: `ModifyMaxEnergy`/`ShouldPlayerResetEnergy`/`ModifyHandDraw` overriders: BagOfPreparation, BigMushroom, BlessedAntler, BloodSoakedRose, BoomingConch, Bread, Ectoplasm, Fiddle, IceCream, PaelsBlood, PaelsFlesh, Pendulum, PhilosophersStone, Pocketwatch, PollinousCore, PrismaticGem, PumpkinCandle, RingOfTheDrake, RingOfTheSnake, SneckoEye, Sozu, SpikedGauntlets, VelvetChoker, WhisperingEarring.

---------------------------------------------------------------------------------------------------

## 14. Ascension, modifiers, gold, act/floor inside combat

### 14.1 Ascension (`C/Entities/Ascension/AscensionLevel.cs`; effects `AscensionManager.cs`, `AscensionHelper.cs`)
Enum value = level: 1 SwarmingElites (map elite count x1.6, MapPointTypeCounts.cs:14), 2 WearyTraveler (AncientEventModel.cs:180), 3 Poverty (gold x0.75 in EncounterModel rewards), 4 TightBelt (-1 potion slot), 5 AscendersBane (curse in starting deck), 6 Inflation (shop), 7 Scarcity (card odds), 8 ToughEnemies, 9 DeadlyEnemies, 10 DoubleBoss (RunManager.cs:761).
Inside a combat only 8 and 9 act, through `AscensionHelper.GetValueIfAscension(level, ascVal, base)` in monster classes (`Models/Monsters/*`: HP ranges, damages, plating amounts etc.; check is `runAscension >= level`). Levels 4 and 5 are already visible as inputs (potion slot count / deck). Level 10's second boss is a map matter. Level 1-3,6,7,10 do not change combat.

### 14.2 Modifiers (custom/daily runs) with combat effect
`Murderous`: applies +3 Strength to all creatures on room entry and to every enemy on `AfterCreatureAddedToCombat`; `Terminal`: -1 max HP at run base room, +5 Plating to player creatures on each combat room entry; `Hoarder`: clones cards added to deck (deck mutation, no combat effect); `CursedRun`: adds a random curse per act (out of combat; `Niche`). Others only change rewards/map/Neow (AllStar, BigGameHunter, CharacterCards, DeadlyEvents, Draft, Flight, Insanity, Midas, NightTerrors, SealedDeck, Specialized, Vintage). Modifiers are listeners (hook order, s12).

### 14.3 Gold in combat
Read by: Debt (`min(DynamicVars.Gold, Owner.Gold)` lost), ThieveryPower (monster steals `min(Amount, gold)`), SpoilsMap/Royalties/SurprisePower/Skewer (gain), relics MawBank, OldCoin, GoldenPearl, LuckyFysh, CursedPearl, SealOfGold, SignetRing, SilkenTress, AmethystAubergine. `PlayerCmd.GainGold` goes through `Hook.ModifyGoldGained`. Gold is thus an input (and an output).

### 14.4 Act/floor
`CurrentActIndex` used by DecimillipedeSegment, SpoilsMap, LanternKey (`2 != act`), GoldenCompass, FurCoat, LavaRock (`act != 0`), AmethystAubergine; `TotalFloor` for the encounter-local RNG seed; `ActFloor` unused in combat. MapPoint coord unused (s3.4).

---------------------------------------------------------------------------------------------------

## 15. Outputs that persist after combat (for the RL env's next-combat input)

`Player.AfterCombatEnd` (Player.cs L837-842): `Creature.RemoveAllPowersInternalExcept()`; `PlayerCombatState.AfterCombatEnd()` (clears all piles, pets, unsubscribes); `Creature.LoseBlockInternal(Block)`. Persisting: `current_hp` (post BurningBlood-like `AfterCombatVictory` heals), `max_hp` (potion/relic max-HP changes), `gold`, potion belt (used potions removed; potions gained), relic saved props (s13.4), deck (cards added/removed/upgraded/transformed in combat, `DeckVersion` write-backs: TheScythe, GeneticAlgorithm), RNG stream states (all 12 + player streams if rewards/transform), relic `Status` resets. Energy, Stars, orbs, Osty, block, powers, piles are all discarded.

---------------------------------------------------------------------------------------------------

## 16. Flagged uncertainties / open questions

1. **Initial shuffle depends on deck order** (UnstableShuffle) and on `Hook.ModifyShuffleOrder(isInitialShuffle:true)` listeners - list the overriding models in the hooks spec. Mid-combat shuffles are `StableShuffle` (sorted by `CardModel.CompareTo`: `ModelId` ordinal order then upgrade level, ModelId.cs L42-50) after `list = discard ++ HashSet<draw cards>`; the HashSet enumeration order of draw-pile cards before the sort is irrelevant because of the sort, but the sort must be stable w.r.t. equal keys (identical cards are interchangeable in outcome except for per-card state - e.g. enchantments/TheScythe - which is NOT compared by CompareTo; flag potential divergence for non-identical cards that compare equal).
2. (resolved) Full-hand adds go to the discard pile (s10); still unverified: whether any `Hook` listener (e.g. ModifyCardPile-type hooks) can alter the target pile before this check.
3. **Run-level hook listeners include deck cards** (`RunState.IterateHookListeners` yields `Deck.Cards` + enchantments for damage hooks). Verify `Contains()`/`ShouldReceiveCombatHooks` for deck-version cards so as not to double-count with combat clones (RunState.cs L545-596).
4. **Orb corner cases**: re-entrant channel from `AfterOrbEvoked` (TryEnqueue throws "OrbQueue is full"); whether any content does this. `OrbQueue.Insert` callers not enumerated.
5. **`Player.BaseOrbSlotCount` mutability across a run**: no game code writes it, but check events/relics (e.g. orb-slot relics) that might call `Player.BaseOrbSlotCount` via other paths (grep found none outside Characters/OrbCmd). Treat as a saved constant.
6. **Relic `Status` serialization**: not saved; relic code recomputes it on load? Check relic implementations (e.g. LizardTail/Girya) for status init on combat entry.
7. **Osty damage "dealer" semantics**: Strength/Weak confirmed owner-only (`Owner != dealer`), so player buffs don't affect Osty. Other powers (e.g., Vigor? PenNib/TheBoot explicitly include Osty) must be audited individually; the safe rule is "powers apply to the dealer creature's own powers".
8. **DieForYou is only a hook**: relies on `props.IsPoweredAttack()`; verify monster attack damage uses `ValueProp.Move` (AttackCommand) so redirects always occur for normal attacks; thorns/poison do not redirect.
9. **PlayerRng leakage**: Claws/LeafyPoultice/Astrolabe/PandorasBox transforms are out-of-combat; combat card transforms (Begone, Guards, PrimalForce, Compact) pass explicit replacement cards (null rng) - Compact's `Transform(list, null)` with a null Rng: confirm replacements are pre-supplied. If any in-combat path used `PlayerRng.Transformations`, that stream would become an input.
10. **Niche draws for fixed-HP enemies**: confirmed `NextItem` draws even when min==max. Also, ToughEgg hatchling HP uses `Niche` mid-combat; Fabricator/Flutter use `MonsterAi`.
11. **Potion pool epoch gating**: the character pool returns 0 potions unless the 4th epoch is "revealed" - decide in the sim whether the player has all epochs (assumed yes: list in s11.5).
12. **Encounter-local RNG** (`EncounterModel._rng`, seeded from run seed + TotalFloor + encounter id hash) feeds `GenerateMonsters()` - needed only if the env generates monster rosters instead of taking them as input; recommended: take the roster as input.
13. **`SerializableCard.Props` fill semantics** (`SavedProperties.Fill`) for card defaults (`SerializationCondition.SaveIfNotTypeDefault` / `AlwaysSave -1`) - e.g. MadScience `TinkerTimeType` default -1 is always saved.
14. **ExtraPlayerFields.DamageDealt / DebuffsApplied** are accumulated during combat but affect only achievements/badges (not gameplay) - safe to ignore (verify `CCCCombo` etc. no gameplay).
16. **Non-saved relic fields**: a relic with a private non-`[SavedProperty]` field that is not reset in `AfterCombatEnd`/`BeforeCombatStart` would behave differently live vs. loaded-from-save. Audit per relic in the relic spec.
17. Some line numbers above (marked `~`) were taken from outputs without explicit numbering; function names are authoritative.
