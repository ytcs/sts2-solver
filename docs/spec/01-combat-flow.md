# 01 - Combat Flow and Turn Loop (STS2 v0.111.0 public-beta, single-player combat only)

Scope: the pure-logic skeleton of one combat: how it is created from an encounter, how the
turn loop is sequenced, which hooks fire in which order, which RNG streams are drawn and when,
and when win/loss is evaluated. VFX/SFX/Godot/UI/network/Sentry/logging/`Cmd.Wait` are ignored.
All paths are relative to `decomp/MegaCrit/Sts2/Core/`. `file:line` cites are to that tree.
`CM` = `Combat/CombatManager.cs`.

Conventions in pseudocode: `await X` means "run X to completion, in order". The real game is
async, but with no player choice pending every `await` is a plain sequential call (see section
14 for the only places where async order is observable). `side` is `Player` or `Enemy`.
"listener order" = `IterateHookListeners` order (section 12).

Single-player simplifications are noted explicitly where the multiplayer branch differs.

---------------------------------------------------------------------------------------------

## 1. State that the flow reads and writes

### 1.1 CombatState (`Combat/CombatState.cs`)
| Field | Initial | Notes |
|---|---|---|
| `RoundNumber` | 1 (`:157`) | ++ only on Enemy->Player switch when no extra turn (`CM:1883`). |
| `CurrentSide` | `Player` (`:158`) | |
| `_allies` | [] | Player creatures first, then pets (Osty etc.), in add order. |
| `_enemies` | [] | Order = add order, re-sorted by encounter slot on each add (section 3.4). |
| `Creatures` | `_allies ++ _enemies` (`:71`) | **Allies first, then enemies.** Fresh list each access. |
| `Players` | allies filtered by `IsPlayer`, mapped to Player (`:81`) | Order = `AddPlayer` order = `RunState.Players` order. |
| `_nextCreatureId` | 0 | `CombatId` is assigned by `AttachCreature` (`:257`): players get 0..n-1 (AddPlayer in `CombatRoom.EnterInternal`), then monsters in creation order, then mid-combat spawns. Monsters' `monster.Rng` seed depends on it. |

`CreaturesOnCurrentSide` = `Allies` if side is Player, else `Enemies` (`:378-385`). Note `Allies` includes pets.

### 1.2 CombatTurnState (`Combat/CombatTurnState.cs`) - one per combat, fresh at `SetUpCombat`
`IsInProgress=false`, `IsStarting=true`, `IsEnemyTurnStarted=false`, `EndingPlayerTurnPhaseOne/Two=false`,
`PendingLoss=null`, `PlayersReadyToEndTurn={}`, `PlayersReadyToBeginEnemyTurn={}`,
`PlayersTakingExtraTurn=[]`, two signal sources (recreated at each player turn start).
`IsLive = IsInProgress && !cancelled`.

### 1.3 PlayerCombatState (`Entities/Players/PlayerCombatState.cs`) - recreated per combat by `Player.ResetCombatState` (`Player.cs:797`)
| Field | Initial | Notes |
|---|---|---|
| `TurnNumber` | 1 (`:37`) | Per-player. Incremented by `SwitchSides` (all players on normal Enemy->Player; only extra-turn players on extra-turn switch). **Not** equal to `RoundNumber` when extra turns happen. |
| `Phase` | `None` | See section 11. |
| `Energy` | 0 | `Stars` 0. Stars are NOT reset per turn (only zeroed on player death, `CM:1229`). |
| piles | Hand, Draw, Discard, Exhaust, Play (`AllPiles` order, `:76`) | Index 0 of `_cards` is the TOP of the pile. `AddInternal(card)` appends at the end (`CardPile.cs:83`). |
| `OrbQueue` | `Clear()` then `AddCapacity(player.BaseOrbSlotCount)` (`:137-139`) | |
| `Pets` | [] | |
| `MaxEnergy` | `Hook.ModifyMaxEnergy(state, player, player.MaxEnergy)` evaluated on every read (`:101`) | `player.MaxEnergy` is run-level (character base + permanent changes). |
| max hand size | `CardPile.MaxCardsInHand = 10` (`CardPile.cs:21`) | |

### 1.4 Creature (`Entities/Creatures/Creature.cs`)
Monster ctor sets `_maxHp = _currentHp = Monster.MaxInitialHp` (`:359-360`) which is later overwritten by the HP roll (3.3). `Block=0`.
`IsAlive = CurrentHp > 0`. `IsPrimaryEnemy` = enemy side and no power with `OwnerIsSecondaryEnemy` (`:253-279`).
`Monster.SpawnedThisTurn` (`MonsterModel.cs:248`): set true by `SetUpForCombat`, set false by `OnSideSwitch` on EVERY side switch.
Player creature: `CombatState` is set by `AttachCreature` when added to a combat.

---------------------------------------------------------------------------------------------

## 2. RNG streams touched by the combat flow

All run-level RNGs live in `RunRngSet` (`Runs/RunRngSet.cs`): one `Rng` per `RunRngType`, created as
`new Rng(RunSeed, snake_case(enumName))` = `MegaRandom(seed + XxHash64(utf8(name), seed 0))`
(`Rng.cs:`ctor `(ulong seed,string name)`, `Helpers/StringHelper.cs:139`). `RunSeed =
XxHash64(utf8(seedString))` (`RunRngSet.cs:~99`). Streams persist for the whole run (they are NOT reset per combat):
**the state of every stream at combat entry depends on everything drawn earlier in the run**. For an RL env that
only simulates combats, start each combat from a fresh `RunRngSet(seed)` or an explicitly provided stream state.

`MegaRandom` = xoshiro256** seeded by 4x splitmix64 (`Random/MegaRandom.cs:73-95`). Primitives that matter here
(each `Rng.*` call also increments an unused `_counter`):
* `Next(max)` / `NextInt(max)`: `(int)(NextDouble() * max)`; **always consumes exactly one 64-bit output, even when max==1**. `NextInt(min,max)`: `min + (int)(NextDouble()*(max-min))`.
* `NextDouble() = (u64 >> 11) * 2^-53`. `NextFloat(min,max) = (float)(NextDouble()*(double)(max-min) + min)` (`Rng.cs`). `NextBool()` = `Next(2)==0` (NOT `MegaRandom.NextBool`).
* `NextItem(items)`: `NextInt(0,count)` then `ElementAt` (no draw if empty). `Rng.Shuffle`: Fisher-Yates from the top `for i=n-1..1: j=NextInt(i+1); swap`.
* `UnstableShuffle(list, rng)` (`Extensions/ListExtensions.cs:~40`): `i=n; while i>1 { i--; j=rng.NextInt(i+1); swap(list[i],list[j]) }` (identical sequence to Fisher-Yates above).
* `StableShuffle`: first `list.Sort()` by `CardModel.CompareTo` (ModelId category then entry, ordinal string compare; then `CurrentUpgradeLevel`; `Models/CardModel.cs:2272`, `AbstractModel.cs:96`, `ModelId.cs:42`), then `UnstableShuffle`.

Draws in the combat flow, in order of occurrence:

| # | When | Stream | Draw |
|---|---|---|---|
| R0 | Before combat: `EncounterModel.GenerateMonstersWithSlots` (`EncounterModel.cs:254-272`) | **Encounter-local `Rng`** seeded `(ulong)((long)RunSeed + (long)runState.TotalFloor) + XxHash64(utf8(encounter.Id.Entry))` (`:263`) | Encounter-specific (e.g. `Rng.NextItem`, `NextInt(3)` for `StarterMoveIdx`, `NextBool`); about 14 of 92 encounters draw. Only created if `_rng==null`. Never persisted. |
| R1 | Per enemy at `CombatState.CreateCreature` (`:233-248`) | `RunState.Rng.Niche` | **Exactly 1 draw per enemy** (`Creature.SetUniqueMonsterHpValue`, `Creature.cs:372-384`): see 3.3. |
| R2 | `SetUpCombat` -> `Player.PopulateCombatState` (`Player.cs:806`) | `RunState.Rng.Shuffle` (shared by all players, players in `state.Players` order) | `UnstableShuffle` of the cloned draw pile: `n-1` draws for n cards (0 draws if n<=1). |
| R3 | `StartCombatInternal`: `AfterCreatureAdded` for each enemy, and again at each player turn start via `PrepareForNextTurn` | `RunRng.MonsterAi` (passed to `MoveStateMachine.RollMove`) | Per `RollMove` that actually transitions: one `NextFloat(max)` per `RandomBranchState` visited (`RandomBranchState.cs:~110`). `ConditionalBranchState` draws nothing. See 5.2 for when RollMove is a no-op. |
| R4 | Mid-combat shuffle (draw pile empty) | `RunState.Rng.Shuffle` | `StableShuffle(discard ++ drawPile)` (`CardPileCmd.cs:1076-1090`): `n-1` draws. Out of scope of this doc beyond the call site. |
| R5 | `CombatState.CreateCreature` `monster.Rng = new Rng((ulong)((long)RunSeed + col + row + ActIndex + CombatId))` (`:245`) | per-monster `monster.Rng` | **Visual-only in all current monsters** (skins, e.g. `ToughEgg.cs:118`). Safe to omit. `Rng.Chaotic` is wall-clock-seeded and visual-only. |

Other streams used by effects (not by the flow itself): `CombatCardGeneration`, `CombatCardSelection`, `CombatEnergyCosts`,
`CombatTargets`, `CombatOrbs`, `CombatPotionGeneration`, `Niche` (e.g. ToughEgg hatchling HP, `ToughEgg.cs:172`), `MonsterAi`
(e.g. `Fabricator.cs:115`). `PlayerRngSet` streams (Rewards/Shops/Transformations) are not used in combat.

**Order sensitivity:** `Shuffle`, `Niche`, `MonsterAi` are each a single shared stream, so the *order of calls across entities*
matters: players in `Players` order for R2; enemies in creation order for R1 (= `MonstersWithSlots` order); enemies in
`State.Creatures` order for the initial R3 and in `State.Enemies` order for per-turn R3.

---------------------------------------------------------------------------------------------

## 3. Combat setup (room entry to `SetUpCombat`)

Entry: `CombatRoom.EnterInternal` (`Rooms/CombatRoom.cs:119`) then `StartCombat` (`:194`).
Precondition: a mutable `EncounterModel` and a `CombatState(encounter, runState, modifiers, badgeModels, mpScaling)` already exist (`:80-84`;
ctor `CombatState.cs:153`).

### 3.1 Ordered steps
```
EnterInternal(runState):
  if CombatState.Players.Count == 0:
     for p in runState.Players:  CombatState.AddPlayer(p)        # AttachCreature (CombatId = 0..), then add to _allies   (CombatState.cs:222)
  StartCombat(runState):
    if !Encounter.HaveMonstersBeenGenerated:
        Encounter.GenerateMonstersWithSlots(runState)             # R0, encounter-local Rng (3.2)
    for (monster, slot) in Encounter.MonstersWithSlots:           # in list order
        creature = CombatState.CreateCreature(monster, Enemy, slot)   # 3.3 (R1 draw happens here)
        CombatState.AddCreature(creature)                         # append to _enemies (CombatState.cs:535)
    CombatManager.SetUpCombat(CombatState)                        # 3.4
    Hook.AfterRoomEntered(runState, room)                         # 3.5
    CombatManager.AfterCombatRoomLoaded()                         # launches turn loop (section 4)
```
(`CombatRoom.cs:194-228`)

### 3.2 Encounter monster list
`GenerateMonsters()` returns `(MonsterModel mutable, string? slotName)[]`. Slot names come from the encounter's `Slots`
list (default empty, `EncounterModel.cs:171`). Encounter-specific randomness uses the encounter-local `Rng` (R0). Ascension
adjustments are NOT applied here: they are baked into per-monster properties via `AscensionHelper.GetValueIfAscension`
(only `ToughEnemies` for HP and `DeadlyEnemies` for damage/values are used by monsters; `HasLevel(l) = ascension >= l`,
`AscensionManager.cs:44`). Player-side ascension effects (`TightBelt` potion slot, `AscendersBane` card) are applied at run
start, not here. `SwarmingElites` only affects map generation (`MapPointTypeCounts.cs:14`); `DoubleBoss` act structure.

### 3.3 Monster creation and HP (`CombatState.CreateCreature`, `CombatState.cs:233-248`)
```
creature = new Creature(monster, side, slot)        # hp = monster.MaxInitialHp provisional; monster.RunRng = RunState.Rng
creaturesOnSide = _enemies                           # enemies ALREADY added (earlier ones in this loop)
creature.SetUniqueMonsterHpValue(creaturesOnSide, RunState.Rng.Niche):
    S = { MinInitialHp .. MaxInitialHp }             # inclusive ints, ascending
    S -= { e.MaxHp for e in creaturesOnSide except self }
    hp = S.nonEmpty ? Niche.NextItem(S) : Niche.NextInt(MinInitialHp, MaxInitialHp+1)
    # NextItem = ToArray (ascending: HashSet built from ascending range, removals leave order intact), idx = Niche.NextInt(0, |S|)
    MonsterMaxHpBeforeModification = CurrentHp = MaxHp = hp
creature.ScaleMonsterHpForMultiplayer(...)           # no-op when 1 player (playerCount != 1 only)
AttachCreature(creature)                             # CombatId = _nextCreatureId++
monster.Rng = new Rng(...)                           # R5 (visual only)
Encounter.OnCreatureSpawned(creature)                # bookkeeping: SpawnedEnemies list (for gold proportion)
```
Key facts: (a) exactly one `Niche` draw per enemy; (b) HP uniqueness is against earlier-created enemies' **MaxHp** on the same side
(compares post-roll max HP, so two same-type enemies get different HP whenever range size > count); (c) `MinInitialHp/MaxInitialHp`
are virtual properties evaluated at this moment (ascension dependent); (d) there is no further HP hook (no
"ModifyMonsterHp" hook exists, grep confirms only `Creature.cs:98` / `NCombatRoom.cs:889`).
In multiplayer, HP is scaled by `hp * playerCount * {1.1 act0, 1.2 act1, 1.3 act2 boss / 1.2 act2 otherwise}` in decimal (`Creature.cs:749`, `MultiplayerScalingModel.cs:70`); single-player skips it.

### 3.4 `CombatManager.SetUpCombat(state)` (`CM:444-468`)
```
assert _turnState == null
_turnState = new CombatTurnState(state)      # IsStarting = true, IsInProgress = false
state.MultiplayerScalingModel?.OnCombatEntered(state)       # MP only
StateTracker.SetState(state)                 # UI only
for p in state.Players:  p.ResetCombatState()                # new PlayerCombatState (1.3)
for p in state.Players:  p.PopulateCombatState(p.RunState.Rng.Shuffle, state)   # 3.6 (R2)
for c in state.Creatures:                                    # allies first, then enemies in current order
    AddCreature(c):                                          # CM:1096
        c.Monster?.SetUpForCombat()                          #   MoveStateMachine = GenerateMoveStateMachine(); SpawnedThisTurn = true
        if c.SlotName != null: state.SortEnemiesBySlotName() #   stable in practice, see below
        StateTracker.Subscribe(c); CreaturesChanged event    #   UI
CombatSetUp event
```
`SortEnemiesBySlotName` (`CombatState.cs:496`): `_enemies.Sort(by Encounter.Slots.IndexOf(slotName))`. `List.Sort` is introsort but for
<=16 elements it is an insertion sort (stable); combats have <=16 enemies, so treat as a **stable sort by slot index; unknown/null
slot => index -1 (sorts first)**. When `Encounter.Slots` is empty all keys are equal, order is creation order.
`SetEnemyIndex` (`:504`) can reorder only for encounters with no slots.
Because the sort runs at each `AddCreature` with a non-null slot, **spawned-in enemies with a slot name are re-sorted into slot order**; a spawn with a null slot is just appended and no sort runs (see Q3).

### 3.5 `Hook.AfterRoomEntered(runState, room)` (`Hooks/Hook.cs:1128`)
Dispatched over `runState.IterateHookListeners(null)` (deck cards+enchantments, then relics, potions, modifiers...) with **no combat-ending guard**; runs while `IsInProgress=false, IsStarting=true`.
Some relics act here (`BigMushroom`, `BurningSticks`, `DivineRight`, `DataDisk`, `BronzeScales`, `EmberTea`, `EternalFeather`, `Gorget`, `Metronome`...). This is BEFORE `AfterCreatureAdded`/initial `RollMove` and BEFORE `BeforeCombatStart`.

### 3.6 Draw pile construction (`Player.PopulateCombatState`, `Player.cs:806-815`)
```
for card in Deck.Cards (deck order):
    clone = state.CloneCard(card)          # ClonePreservingMutability; added to state._allCards; clone.DeckVersion = card
    DrawPile.AddInternal(clone)            # appended; index 0 = top
DrawPile.RandomizeOrderInternal(player, rng=RunState.Rng.Shuffle, state):   # CardPile.cs:69
    _cards.UnstableShuffle(rng)            # n-1 draws; NOTE: UNSTABLE (depends on deck order), not StableShuffle
    Hook.ModifyShuffleOrder(state, player, _cards, isInitialShuffle:true)   # listeners incl. enchantment PerfectFit; IsStarting exempts the "combat ending" guard
```
Deck order in = `Player.Deck.Cards` order. Innate/bottom-of-pile reordering is NOT done here but at turn 1 hand draw (6.2).
No energy, orb, or star setup here beyond 1.3 defaults. Relic/power based start-of-combat effects come from `BeforeCombatStart` (4).

---------------------------------------------------------------------------------------------

## 4. Combat start (`CM:576-626` `StartCombatInternal`)
```
AfterCombatRoomLoaded():  _turnLoopTask = RunTurnLoopAfter(prev)     # CM:470; just starts the loop; waits for previous loop to die (ignore)
StartCombatInternal(ts):
  # (actions executor unpause/finish: ignore)
  for c in state.Creatures (snapshot; allies then enemies):
      await c.AfterAddedToRoom()               # enemies only: Monster.AfterAddedToRoom() (virtual; e.g. apply starting powers, summon)
      if c.IsEnemy && state.CurrentSide == Player:
          c.Monster.RollMove(state.Players.Select(p => p.Creature))   # R3 (first roll)
  ts.IsInProgress = true; ts.IsStarting = false
  await Hook.BeforeCombatStart(runState, state)      # 4.1
  CombatBegan event
  (banner/ftue/Cmd.CustomScaledWait: ignore)
  await StartTurn(ts)                                 # first player turn (section 6)
  signal = await AwaitTurnEndAndSwitchSides(ts)       # section 8-9
  while ts.IsLive:
      if state.CurrentSide == Player: await StartTurn(ts); signal = await AwaitTurnEndAndSwitchSides(ts)
      else:                           await StartTurn(ts, signal); signal = null
```
Notes:
* `AfterAddedToRoom` hooks run while `IsInProgress=false` but `IsStarting=true`, so `IterateCombatHookListeners` still dispatches (`Hook.cs:~40`). Powers applied here exist before relic `BeforeCombatStart` effects.
* The creature snapshot is taken once: creatures that `AfterAddedToRoom` spawns are NOT iterated in this loop (their own add path runs `CreatureCmd.Add` -> section 13).
* The loop alternates: after the player turn finishes, `CurrentSide==Enemy`, so the next iteration runs `StartTurn(enemy)`, which itself runs the whole enemy turn and ends by switching back to Player; then the loop sees `CurrentSide==Player`.

### 4.1 `Hook.BeforeCombatStart(runState, combatState)` (`Hook.cs:311`)
Two full passes over `runState.IterateHookListeners(combatState)` (re-enumerated for the second pass): pass 1 `BeforeCombatStart()` on every listener, pass 2 `BeforeCombatStartLate()` (only `PetrifiedToad` overrides Late). Listener order for this iterator (`Runs/RunState.cs:545-596`):
1. Each active player's **Deck** cards (non-combat instances!) each followed by its enchantment, in player order.
2. (relics/potions/modifiers are skipped when a child combat state is given; they come from 3.)
3. Mod subscribers (ignore).
4. `combatState.IterateHookListeners()` (section 12).
Each listener is checked with `Contains(item)` at yield time (removed models skipped; `RunState.cs:577`).
Relics with `BeforeCombatStart`: Anchor, BoundPhylactery, BeltBuckle, Vambrace, Pantograph, SneckoEye, Kusarigama, FurCoat, MeatOnTheBone, LetterOpener, ... (relics topic). Powers: Galvanic, VitalSpark.

---------------------------------------------------------------------------------------------

## 5. Turn loop invariants

### 5.1 Who calls what
```
Player turn i:   StartTurn(Player)  ->  [player acts]  ->  AwaitTurnEndAndSwitchSides:
                    wait EndTurnSignal -> AfterAllPlayersReadyToEndTurn (phase one) -> [ReadyToBeginEnemyTurn action]
                    -> AfterAllPlayersReadyToBeginEnemyTurn (phase two: flush, AfterSideTurnEnd, SwitchFromPlayerToEnemySide)
Enemy turn i:    StartTurn(Enemy) -> ExecuteEnemyTurn -> EndEnemyTurn -> (EndEnemyTurnInternal, CheckWinCondition, SwitchSides)
```
Only the turn loop advances turns.

### 5.2 Monster move state machine, relevant to flow (`MonsterMoves/MonsterMoveStateMachine/MonsterMoveStateMachine.cs:41-67`)
`RollMove` is a **no-op (no RNG drawn)** when `!current.CanTransitionAway` or `(!_performedFirstMove && current.IsMove)`.
`_performedFirstMove` is set by `OnMovePerformed` after the monster's first `PerformMove`. Therefore:
* Turn 1: the initial `RollMove` in `AfterCreatureAdded` (R3, section 4 step 2): if the machine's initial state is a branch it is resolved to a move (RNG draw for a `RandomBranchState`); if it is already a move it is returned unchanged (no draw). The second call at turn-1 `PrepareForNextTurn` is a no-op either way (current state is a move and no move has been performed yet). `NextMove` is always set from the return value (`MonsterModel.cs:418`).
* Later turns: `PrepareForNextTurn` at each player turn start transitions to the next move: `current.GetNextState()` repeatedly until a MOVE state is reached (`RandomBranchState`: 1 `NextFloat` draw with float weights; `MoveState`: `FollowUpState`; `ConditionalBranchState`: first true condition).
* **Intents shown during the player's turn are chosen at the START of that player turn, before the player acts**; conditional/weight lambdas see pre-action state. The move performed in the enemy phase is whatever `NextMove` was at that time (unless changed by `SetMoveImmediate`/stun during the player turn).
* `MoveState.CanTransitionAway` is false until performed if `MustPerformOnceBeforeTransitioning` (used by stun `STUNNED` state, `Creature.cs:525-545`).

---------------------------------------------------------------------------------------------

## 6. Turn start (generic `StartTurn(ts, actionDuringEnemyTurn)`, `CM:688-851`)

### 6.1 Ordered steps (both sides unless marked)
```
0. if !ts.IsInProgress: return
1. SetPhaseForAllPlayers(None)                                              # CM:695
2. under ReadyLock:
     isExtra = PlayersTakingExtraTurn.Count > 0
     if side==Player && isExtra:  creaturesStartingTurn = [p.Creature for p in PlayersTakingExtraTurn]; playersStartingTurn = PlayersTakingExtraTurn
     else:                        creaturesStartingTurn = state.CreaturesOnCurrentSide (Allies incl. pets | Enemies);
                                  playersStartingTurn   = side==Player ? state.Players : []
3. for c in creaturesStartingTurn:  c.BeforeTurnStart(side)                 # each power: AmountOnTurnStart = Amount   (Creature.cs:678)
4. await Hook.BeforeSideTurnStart(state, side, creaturesStartingTurn)       # single pass, listener order; each listener awaited with its own choice context
5. if side == Player:
     5a. SetPhaseForAllPlayers(Start); PlayerActionsDisabled = false
     5b. clear PlayersReadyToEndTurn, PlayersReadyToBeginEnemyTurn; new EndTurnSignalSource, BeginEnemyTurnSignalSource
     5c. if !isExtra:  for e in state.Enemies: e.PrepareForNextTurn(state.PlayerCreatures)   # RollMove -> R3 (MonsterAi); intents roll BEFORE any player draw
   else (Enemy): (banner only)
6. for c in creaturesStartingTurn:  await c.AfterTurnStart(side)            # block clear; skipped for a Player creature whose PlayerCombatState.TurnNumber == 1 (Creature.cs:686-697)
7. for c in creaturesStartingTurn:  await Hook.AfterBlockCleared(state, c)  # fires even if the clear was prevented
8. for p in playersStartingTurn:    SetupPlayerTurn(p)                       # 6.2 (async per player; pausing on a choice lets the next player's setup begin)
9. await Hook.AfterSideTurnStart(state, side, creaturesStartingTurn)        # pass 1 AfterSideTurnStart, pass 2 AfterSideTurnStartLate  (AFTER the hand draw!)
10. if side == Player:
      10a. for p in playersStartingTurn (if p.PlayerCombatState != null): await p.OrbQueue.AfterTurnStart(ctx)   # start-of-turn orb passives, in orb order; AFTER draw and AfterSideTurnStart
      10b. for p in state.Players:  if p.Creature.IsDead || p not in playersStartingTurn:
               SetReadyToEndTurn(p, canBackOut:false)
               if AllPlayersReadyToEndTurn():  return          # SINGLE-PLAYER: AllPlayersReadyToEndTurn() is unconditionally TRUE (CM:1062-1070) => StartTurn returns here whenever the (only) player is dead; the turn-start tail below is skipped
      10c. for p in playersStartingTurn where p alive:  RunAutoPrePlayPhase(p)    # 6.3
      10d. await CheckWinCondition(ts)
      10e. if ts.IsInProgress: IsEnemyTurnStarted = false; TurnStarted event   # play phase is now open (ActionSynchronizer -> PlayPhase)
    else (Enemy):
      10f. IsEnemyTurnStarted = true; TurnStarted event
      10g. await CheckWinCondition(ts)
      10h. if ts.IsInProgress: await ExecuteEnemyTurn(ts, actionDuringEnemyTurn)   # section 10
```
Notes:
* Block clear is per-side at that side's turn start: **enemies lose block at the start of the enemy turn (before acting); the player loses block at the start of the player turn** (except the player's very first turn, `TurnNumber==1`). Extra-turn players clear block again at the start of the extra turn (their `TurnNumber` is >1).
* `Creature.ClearBlock` (`:723`): `if Hook.ShouldClearBlock(state, creature, out preventer): Block = 0; else await Hook.AfterPreventingBlockClear(state, preventer, creature)`. `ShouldClearBlock` = AND over listeners (first `false` wins and is the "preventer"): Barricade, Blur, Burrowed (powers), SturdyClamp (relic).
* `AfterBlockCleared` is dispatched for all `creaturesStartingTurn` (pets included).
* Phase transitions: `None` (step 1) -> `Start` (5a) -> `AutoPrePlay`/`Play` in 10c. During the enemy turn phases are `None`.

### 6.2 `SetupPlayerTurn(ts, player, ctx)` (`CM:877-924`)
```
if player.Creature.IsDead: return;  if player.PlayerCombatState == null: return
if Hook.ShouldPlayerResetEnergy(state, player):  PCS.ResetEnergy()  # Energy = MaxEnergy          (ShouldPlayerResetEnergy = AND over listeners; IceCream returns false when TurnNumber>1)
else:                                            PCS.AddMaxEnergyToCurrent()   # Energy += MaxEnergy
await Hook.AfterEnergyReset(state, player)            # pass 1 AfterEnergyReset, pass 2 AfterEnergyResetLate
await Hook.BeforeHandDraw(state, player, ctx)         # pass 1 BeforeHandDraw, pass 2 BeforeHandDrawLate (can do async choices, e.g. Toolbox)
handDraw = Hook.ModifyHandDraw(state, player, 5, out modifiers)   # decimal; pass 1 ModifyHandDraw over listeners sequentially (chained), pass 2 ModifyHandDrawLate (no overrides exist)
                                                      # a listener is added to `modifiers` iff (int)before != (int)after
await Hook.AfterModifyingHandDraw(state, modifiers)   # only for those modifier listeners, in listener order
if PCS.TurnNumber == 1:                               # PLAYER's own turn number, not RoundNumber
    pile = DrawPile
    bottom = [c in pile where c.Enchantment?.ShouldStartAtBottomOfDrawPile]          # Imbued enchantment only (Imbued.cs:11)
    for c in bottom (pile order): pile.MoveToBottomInternal(c)                       # remove + append => relative order kept
    innate = [c in pile where Keywords has Innate] minus bottom                      # evaluated AFTER the bottom moves, pile order
    for c in innate (pile order): pile.MoveToTopInternal(c)                          # remove + Insert(0): so LAST innate in pile order ends up ON TOP (reversed order)
    handDraw = max(handDraw, innate.Count);  handDraw = min(handDraw, 10)
await CardPileCmd.Draw(ctx, handDraw, player, fromHandDraw:true)    # see 6.4
await Hook.AfterPlayerTurnStart(state, ctx, player)   # pass AfterPlayerTurnStartEarly, pass AfterPlayerTurnStart, pass AfterPlayerTurnStartLate
```
Overrides of note: `AfterPlayerTurnStart` (relics Bellows, BoneTea, EmotionChip, MercuryHourglass, GamblingChip, ...; powers Loop, RollingBoulder, SummonNextTurn, ToolsOfTheTrade, Entropy, Inferno...), `AfterPlayerTurnStartLate` (BloodVial), `AfterEnergyReset` (ArtOfWar, VenerableTeaSet; powers EnergyNextTurn, StarNextTurn, Radiance, Genesis, LightningRod, Spinner), `ModifyHandDraw` (many; see grep list in section 15).
Note: **no shuffle happens at combat start**; the draw pile was shuffled once in `PopulateCombatState` (3.6). `ModifyMaxEnergy` is read at `ResetEnergy`/`AddMaxEnergyToCurrent` time.
Innate processing only happens on the player's `TurnNumber == 1` (so an extra turn does not repeat it). Innate with draw count: if more innate than 5 (or modified draw), draw grows up to 10.

### 6.3 `RunAutoPrePlayPhase` (`CM:859-866`) per alive player in `playersStartingTurn`, in order
```
await setupPlayerTurnTask                                   # finishes SetupPlayerTurn if it was paused
Phase = AutoPrePlay
await CheckForEmptyHand(ts, ctx, player):                    # CM:1171
        if ts.IsInProgress && !IsExecutingCardOrPotionEffect(player) && Hand is empty: await Hook.AfterHandEmptied(state, ctx, player)   # UnceasingTop (valid only in phases AutoPrePlay/Play/AutoPostPlay)
await Hook.AfterAutoPrePlayPhaseEntered(ctx, state, player)  # passes: ...EnteredEarly, ...Entered, ...EnteredLate; (HistoryCourse, Mayhem, Imbued, WhisperingEarring...)
Phase = Play
```
Card auto-plays here can end the turn (e.g. Void Form via `PlayerCmd.EndTurn`); the end-turn signal is **held until `StartTurn` returns** and consumed by `AwaitTurnEndAndSwitchSides` (`CombatTurnState.cs` doc, `CM:656`).
`CheckForEmptyHand` is otherwise also called after each card play / potion use (not after end turn).

### 6.4 `CardPileCmd.Draw` as called for the hand draw (`Commands/CardPileCmd.cs:1010-1066`) - flow-critical summary only
```
if IsOverOrEnding: return []                        # no draw once the last enemy died
if !Hook.ShouldDraw(state, player, fromHandDraw): AfterPreventingDraw; return []
n = ceil(count) (0 if count<=0);  room = max(0, 10 - hand.Count);  if room==0: return
for i in 0..n:  break if room<=0 or IsOverOrEnding
   break unless (draw.Count + discard.Count > 0) and hand.Count < 10
   ShuffleIfNecessary:  if draw empty && discard nonempty -> Shuffle()  # R4: StableShuffle(discard ++ draw) on RunState.Rng.Shuffle; Hook.ModifyShuffleOrder(isInitial=false); add to draw pile bottom; Hook.AfterShuffle
   card = draw.Cards[0]; hand.Add(card)  (CardPileCmd.Add; may trigger move hooks)
   History.CardDrawn(state, card, fromHandDraw); await Hook.AfterCardDrawn(...); card.InvokeDrawn()
   room = 10 - hand.Count
```
(Draw/shuffle details belong to the card-pile spec; listed here because the hand draw is the first RNG-consuming step after combat start.)

---------------------------------------------------------------------------------------------

## 7. Player play phase and the end-turn trigger

* Play phase exists from step 10e until end-turn phase one begins. Player-driven actions (`PlayCardAction`, `UsePotionAction`, `EndPlayerTurnAction`, `UndoEndPlayerTurnAction`, `DiscardPotionGameAction`) are `GameActionType.CombatPlayPhaseOnly` and are only dispatched while the action synchronizer is in `PlayPhase` (`ActionQueueSynchronizer.cs:121,149`). Requests made earlier are deferred to the player turn; at `EndTurnPhaseOne` queued player-driven actions are cancelled and only the currently running one finishes (`CM:1475-1500`).
  => Legal-action rule for the sim: card play, potion use and end-turn are legal only when `Phase == Play`, `IsInProgress`, and no `PendingLoss`. After the end-turn is committed no further plays occur.
* `PlayerCmd.EndTurn(player, canBackOut, hook)` (`PlayerCmd.cs:286`): `if !IsPlayerReadyToEndTurn(player): OnEndedTurnLocally() (PlayerActionsDisabled = true); SetReadyToEndTurn(...)`. `EndPlayerTurnAction` ignores a stale action whose turn number != `PCS.TurnNumber`.
* `SetReadyToEndTurn` (`CM:932-964`): add to `PlayersReadyToEndTurn`; `PlayerEndedTurn` event; **if `AllPlayersReadyToEndTurn()` then complete `EndTurnSignalSource`** with `(runningPlayerDrivenAction, TurnNumber, player, hook)`. In single-player/fake-MP `AllPlayersReadyToEndTurn()` is always true (`CM:1062`), so the first ready completes the signal.
* Effects can end the turn: `VoidForm` (`PlayerCmd.EndTurn`, `Models/Cards/VoidForm.cs:26`) and player death during the player side (`CreatureCmd.cs:495-503`, multiplayer only matters).
* `UndoReadyToEndTurn` is multiplayer-only UX; ignore.

---------------------------------------------------------------------------------------------

## 8. Player end turn

### 8.1 `AwaitTurnEndAndSwitchSides(ts)` (`CM:635-686`)
```
if !IsInProgress or CurrentSide != Player: return null
sig = await EndTurnSignal                      # may already be complete (end turn during auto-pre-play)
if !IsInProgress: return null
if sig.RunningAction != null: await its completion   # the card/potion whose effect ended the turn (e.g. Void Form) finishes first
await AfterAllPlayersReadyToEndTurn(ts, sig)   # phase one  (8.2)
if !IsInProgress: return null
hook = await BeginEnemyTurnSignal              # completed by ReadyToBeginEnemyTurnAction (enqueued at end of phase one)
if !IsInProgress: return null
await AfterAllPlayersReadyToBeginEnemyTurn(ts) # phase two  (8.3)
return hook
```

### 8.2 `AfterAllPlayersReadyToEndTurn` + `EndPlayerTurnPhaseOneInternal` (`CM:1437-1473, 1515-1589`)
```
EndingPlayerTurnPhaseOne = true
wait until player-driven actions are drained            # queue semantics; in a sync sim: nothing pending
playersEndingTurn = PlayersTakingExtraTurn.Count>0 ? PlayersTakingExtraTurn : state.Players
for p in playersEndingTurn:  p.Phase = AutoPostPlay; await Hook.AfterAutoPostPlayPhaseEntered(ctx, state, p)    # Stampede, IAmInvincible, HowlFromBeyond, ...  (single pass)
for p in playersEndingTurn:  await ctx.WaitForCompletion(); p.Phase = End
await Hook.BeforeSideTurnEnd(state, Player, [p.Creature for p in playersEndingTurn])
        # 3 passes in order: BeforeSideTurnEndVeryEarly, BeforeSideTurnEndEarly, BeforeSideTurnEnd (Hook.cs:1244)
if await CheckWinCondition(ts): return                  # win or pending loss: skip the rest (incl. discard and phase two)
for p in playersEndingTurn:  await DoTurnEnd(ts, p)     # 8.4  (concurrently started, sequential in a sync sim)
if await CheckWinCondition(ts): return
for p in playersEndingTurn:  await Hook.BeforeFlush(state, p)     # passes BeforeFlush, BeforeFlushLate (Hook.cs:544); SlumberingEssence enchantment
await CheckWinCondition(ts)
if ts.IsInProgress:  enqueue ReadyToBeginEnemyTurnAction  -> SetReadyToBeginEnemyTurn -> BeginEnemyTurnSignal result
EndingPlayerTurnPhaseOne = false
```
* Because `ReadyToBeginEnemyTurnAction` goes through the action queue, other actions that were already queued (e.g. actions enqueued by end-of-turn effects) run first. A sync sim can treat it as immediate; see Q5.
* `BeforeSideTurnEnd` and `AfterSideTurnEnd` are the hooks that implement most "at end of turn" effects (e.g. Metallicize/Plated Armor-like powers). `participants` = the ending players' creatures only (NOT pets).

### 8.3 `AfterAllPlayersReadyToBeginEnemyTurn` (phase two, `CM:1707-1730`)
```
EndingPlayerTurnPhaseTwo = true
AboutToSwitchToEnemyTurn event (UI); Task.Yield()
if IsInProgress && CurrentSide == Player:
    await EndPlayerTurnPhaseTwoInternal(ts)    # 8.5
    await SwitchFromPlayerToEnemySide(ts)      # 9.2
EndingPlayerTurnPhaseTwo = false
```
`ReadyToBeginEnemyTurnAction` -> `SetReadyToBeginEnemyTurn` (`CM:1003`): single-player: first ready completes the signal.

### 8.4 `DoTurnEnd(ts, player)` (`CM:1599-1625`) - runs inside phase one, per player in `playersEndingTurn`
```
await player.OrbQueue.BeforeTurnEnd(ctx)                 # for each orb (snapshot, orb order): orb.BeforeTurnEndOrbTrigger   (end-of-turn passives: Frost, Lightning, ...)
if !IsInProgress || IsCombatEnding(ts): return           # orb damage may have killed the last enemy
turnEndCards = []; etherealCards = []
for card in Hand.Cards (hand order):
    if card.HasTurnEndInHandEffect:                 turnEndCards.Add(card)          # takes precedence over Ethereal (else-if!)
    elif Keywords has Ethereal && Hook.ShouldEtherealTrigger(state, card):   etherealCards.Add(card)
for c in etherealCards (hand order): await CardCmd.Exhaust(ctx, c, causedByEthereal:true)     # all ethereal exhausts first
await DoTurnEndCards(turnEndCards):                      # CM:1633
    for c in turnEndCards (hand order):
        CardPileCmd.Add(c, Play)                         # move to play pile
        await c.OnTurnEndInHandWrapper(ctx)              # e.g. curses (Burn, Decay, Regret, ...)
        if c has Ethereal: CardCmd.Exhaust(ctx, c, causedByEthereal:true, skipVisuals)    # NOTE: no ShouldEtherealTrigger check for these
        else:              CardPileCmd.Add(c, Discard, Bottom)
```
(The reference implementation interleaves these asynchronously with visual delays; the intended semantics are sequential in hand order.)
`ShouldEtherealTrigger` is "AND over listeners" (no current override found outside the abstract). `ShouldRetainThisTurn` is NOT consulted here.

### 8.5 `EndPlayerTurnPhaseTwoInternal` and `FlushPlayerHand` (`CM:1746-1819`)
```
playersEndingTurn = (same rule as 8.2)
for p in playersEndingTurn (alive): await FlushPlayerHand(ts, p)
        flush = Hook.ShouldFlush(state, p)                    # AND over listeners: RunicPyramid, RingingTriangle, RetainHand, WellLaidPlans powers/relics (veto the flush)
        for card in Hand.Cards (hand order):  if !flush || card.ShouldRetainThisTurn: retain.Add(card) else: toFlush.Add(card)
        if toFlush nonempty: await CardPileCmd.Add(toFlush, Discard)         # default position Bottom; order = hand order
        await Hook.AfterFlush(state, p, ctx, toFlush, retain)                # Bookmark relic etc.
        p.PlayerCombatState.EndOfTurnCleanup()                               # for every card in ALL piles: ExhaustOnNextPlay=false; HasSingleTurnRetain=false; HasSingleTurnSly=false;
                                                                               # remove EndOfTurn-expiring local energy-cost modifiers; remove temp star costs that clear at turn end   (CardModel.cs:1611, CardEnergyCost.cs:331)
await Hook.AfterSideTurnEnd(state, Player, [p.Creature for p in playersEndingTurn])    # passes AfterSideTurnEnd, then AfterSideTurnEndLate (Hook.cs:1279)
```
`ShouldRetainThisTurn = Keywords has Retain || HasSingleTurnRetain` (`CardModel.cs:591`). **Retained cards stay in hand; there is no per-turn hand limit adjustment**.
After this, players' `Phase` is still `End` until the next `StartTurn` sets `None`.

---------------------------------------------------------------------------------------------

## 9. Side switching, turn counters, extra turns

### 9.1 `SwitchSides(ts)` (`CM:1857-1895`)
```
extra = PlayersTakingExtraTurn.Count > 0
if CurrentSide == Player && !extra:
    CurrentSide = Enemy
else:                                                   # either Enemy->Player, or Player->Player (extra turn)
    CurrentSide = Player
    if extra:  list = PlayersTakingExtraTurn            # RoundNumber unchanged
    else:      list = state.Players; RoundNumber++
    for p in list: p.PlayerCombatState.IncrementTurnNumber()
for c in state.Creatures (allies then enemies): c.OnSideSwitch()      # monsters: SpawnedThisTurn = false; players: no-op
TurnEnded event
```
Counter semantics:
* `RoundNumber` starts at 1 and increments once per full (player+enemy) round, on the Enemy->Player switch. It does not change on extra turns.
* `PlayerCombatState.TurnNumber` starts at 1; the first player turn runs with 1, the second with 2... Incremented in `SwitchSides` **before** `StartTurn`, so inside turn n hooks see `TurnNumber == n` (many relics check `TurnNumber == 1/2/3`, `<= 1`, `> 1`).
* The combat-wide "turns taken" stat is the local player's `TurnNumber` at combat end (`CM:1301`).

### 9.2 `SwitchFromPlayerToEnemySide` (`CM:1825-1855`)
```
PlayersTakingExtraTurn.Clear()
for p in state.Players:  if Hook.ShouldTakeExtraTurn(state, p): PlayersTakingExtraTurn.Add(p)     # OR over listeners; PaelsEye, AmbergrisPower (Amount>0 && player==Owner.Player)
list = PlayersTakingExtraTurn.ToList()
SwitchSides(ts)
for p in list: await Hook.AfterTakingExtraTurn(state, p)         # AmbergrisPower decrements; PaelsEye bookkeeping
```
* The list is recomputed at every player->(enemy|extra) transition; it is cleared there, not after the extra turn starts. During an extra turn, `PlayersTakingExtraTurn` is non-empty, so `StartTurn`, phase one and phase two all use it as the participant set.
* **Extra turn flow:** after `SwitchSides` the side is still `Player`, so the turn loop immediately runs `StartTurn(Player)`: participants are the extra-turn players only; step 5c (enemy `PrepareForNextTurn`/RollMove) is **skipped**; enemies do not act; enemy block is not cleared; `RoundNumber` unchanged; `TurnNumber` of participants +1 so `AfterTurnStart` clears their block and innate processing is skipped. Non-participating players (multiplayer) are immediately set ready (10b).
* `EndCombatInternal` clears `PlayersTakingExtraTurn` (`CM:1307`).
* Every `SwitchSides` fires `OnSideSwitch` for all creatures, so monsters spawned during the player turn lose `SpawnedThisTurn` at the player->enemy switch and DO act that enemy turn; monsters spawned during the enemy turn keep `SpawnedThisTurn = true` until the enemy->player switch and are skipped by `TakeTurn` that turn.

---------------------------------------------------------------------------------------------

## 10. Enemy turn

### 10.1 `StartTurn(ts, hook)` with `CurrentSide == Enemy` - see 6.1: steps 1-4, 6, 7, 9, 10f-10h (no player setup; `participants = Enemies`).
Concretely: `BeforeTurnStart` (power `AmountOnTurnStart = Amount`) for every enemy -> `Hook.BeforeSideTurnStart(Enemy, enemies)` -> `Creature.AfterTurnStart(Enemy)` for each enemy in `Enemies` order (**block clear**) -> `Hook.AfterBlockCleared(enemy)` each -> `Hook.AfterSideTurnStart(Enemy, enemies)` (two passes) -> `IsEnemyTurnStarted = true`, `TurnStarted` -> `CheckWinCondition` -> `ExecuteEnemyTurn`.
(The poison tick etc. of enemies lives in power `AfterSideTurnStart`/`BeforeTurnStart` hooks; they run in listener order.)

### 10.2 `ExecuteEnemyTurn` (`CM:1402-1435`)
```
(test-only actionDuringEnemyTurn hook; ignore)
for enemy in state.Enemies.ToList():                    # snapshot at enemy-turn start, in Enemies order
    if !state.ContainsCreature(enemy): continue          # removed (dead/escaped) during this enemy turn
    await enemy.TakeTurn():                              # Creature.cs:711
         if !SpawnedThisTurn: await Monster.PerformMove()
    # NOTE: no IsAlive check anywhere in this loop or in TakeTurn.
    await CheckWinCondition(ts)                          # after EVERY enemy's turn
    if !ts.IsInProgress: return
RunManager checksum
await EndEnemyTurn(ts)
```
`Monster.PerformMove` (`MonsterModel.cs:435-458`):
```
move = NextMove; targets = state.PlayerCreatures
await move.PerformMove(targets)                  # sets move._performedAtLeastOnce; runs the move's lambda (damage/block/powers/summons)
MoveStateMachine.OnMovePerformed(move)           # _performedFirstMove = true
History.MonsterPerformedMove(state, monster, move, targets)
IsPerformingMove = false
if Creature.IsDead && Hook.ShouldCreatureBeRemovedFromCombatAfterDeath(state, Creature): state.RemoveCreature(Creature)
```
Facts to replicate: (a) the monster's NEXT move is NOT rolled here; it is rolled at the next player-turn start (5c). (b) Enemies spawned during this loop are not in the snapshot and additionally have `SpawnedThisTurn` set. (c) An enemy that dies mid-move stays in `Enemies` until its move finishes (`IsPerformingMove` check in `CreatureCmd.Kill`, `CreatureCmd.cs:~565`), then is removed by `PerformMove` above.
(d) Order between enemies is fixed by list order, including damage ordering (e.g. thorns kills). (e) If the player dies during an enemy's move, `LoseCombat` marks `PendingLoss` (13.2); the next `CheckWinCondition` (right after that enemy's `TakeTurn`) processes it and stops the loop.

### 10.3 `EndEnemyTurn` (`CM:1073-1094`) and `EndEnemyTurnInternal` (`CM:1696-1705`)
```
assert CurrentSide == Enemy
await EndEnemyTurnInternal(ts):
    enemies = state.CreaturesOnCurrentSide.ToList()
    await Hook.BeforeSideTurnEnd(state, Enemy, enemies)       # 3 passes (VeryEarly, Early, normal)
    for p in state.Players:  p.PlayerCombatState.EndOfTurnCleanup()     # SAME cleanup as the flush (all cards, all piles)
    await Hook.AfterSideTurnEnd(state, Enemy, enemies)        # passes AfterSideTurnEnd, AfterSideTurnEndLate
await CheckWinCondition(ts)
if !IsCombatEnding(ts):  SwitchSides(ts)                      # Enemy -> Player: RoundNumber++, every player TurnNumber++, OnSideSwitch on all creatures
```
Quirk: `IsCombatEnding` returns false once `IsInProgress` is false, so after a win/loss detected by the `CheckWinCondition` just above, `SwitchSides` still executes (increments counters; harmless but visible if counters are read later). `EndCombatInternal` reads `TurnNumber` before this.
Enemy-turn `AfterSideTurnEnd` participants are enemies; powers like player-side "end of enemy turn" decay (e.g. Weak/Vulnerable decrement) are implemented by powers listening on those hooks (powers topic).

---------------------------------------------------------------------------------------------

## 11. PlayerTurnPhase table (`Combat/PlayerTurnPhase.cs`, set in `CM`)

| Value | Int | Set where | When | What happens / what is gated |
|---|---|---|---|---|
| `None` | 0 | `StartTurn` top (`CM:695`) for BOTH sides; `Reset`; `EndCombatInternal` | Enemy turn, between turns, before/after combat | No player-driven actions. `UnceasingTop` ignores it. |
| `Start` | 1 | `CM:725` (player side only) | From turn start until `RunAutoPrePlayPhase` | Block clear, `AfterSideTurnStart`, energy reset, hand draw, `AfterPlayerTurnStart`, start-of-turn orb passives. A paused setup (player choice) keeps the player in `Start`. |
| `AutoPrePlay` | 2 | `RunAutoPrePlayPhase` (`CM:862`) | After setup completes | `CheckForEmptyHand`, then `Hook.AfterAutoPrePlayPhaseEntered` (Early/normal/Late): HistoryCourse, Imbued, Mayhem, WhisperingEarring. |
| `Play` | 3 | `CM:865` | After pre-play hooks; ActionSynchronizer enters `PlayPhase` after `CheckWinCondition` (`CM:832`) | The ONLY phase where card play, potion use and End Turn are accepted. |
| `AutoPostPlay` | 4 | `EndPlayerTurnPhaseOneInternal` (`CM:1540`) | First thing in end-turn phase one | `Hook.AfterAutoPostPlayPhaseEntered`: Stampede, IAmInvincible, HowlFromBeyond. |
| `End` | 5 | `CM:1550` | After auto-post-play of all players; stays through flush and the side switch | `BeforeSideTurnEnd`, orb end-of-turn triggers, ethereal/turn-end-in-hand cards, `BeforeFlush`, flush, `AfterFlush`, `AfterSideTurnEnd`. |

`UnceasingTop.IsValidPhase` accepts phases 2..4 only (`Models/Relics/UnceasingTop.cs`), so hand-empty draws never fire during `Start`, `End`, or the enemy turn.
Enum values: `None=0, Start=1, AutoPrePlay=2, Play=3, AutoPostPlay=4, End=5`.

---------------------------------------------------------------------------------------------

## 12. Hook dispatch order and "combat ending" suppression

### 12.1 `CombatState.IterateHookListeners` (`CombatState.cs:411-494`) - listener order
```
list = []
for creature in (Allies ++ Enemies):                      # allies (players, pets) first, then enemies
    list += creature.Powers                               # in the creature's power-list order
    if creature.Player == null:  list += creature.Monster        # monsters AND pets (pets are monster creatures on the ally side)
    else:
        if !player.IsActiveForHooks: continue             # dead player: skip everything below for this creature
        list += player.Relics where !IsMelted             # relic list order
        list += non-null PotionSlots                      # slot order
        if player.PlayerCombatState == null: continue
        list += OrbQueue.Orbs
        for pile in [Hand, Draw, Discard, Exhaust, Play]: for card in pile.Cards: list += card; list += card.Affliction?; list += card.Enchantment?
list += Modifiers; list += BadgeModels; list += MultiplayerScalingModel?
for item in list:  if Contains(item): yield item          # liveness re-checked lazily at yield time (removed models are skipped)
yield ModHelper subscribers                               # ignore
```
`Contains` rules: power: owner in a combat and (owner not a player or player active); relic/potion/card/orb/etc.: not `HasBeenRemovedFromState` and owner active; monster: creature has a CombatState (`:550-601`).
The list is built once at the start of enumeration, so models added during a dispatch are not visited by that dispatch, and removed models are skipped.
`Early`/`Late` variants (e.g. `AfterPlayerTurnStartEarly`/`Late`, `AfterEnergyResetLate`, `BeforeSideTurnEndVeryEarly/Early`) are separate **full passes** over a fresh listener enumeration: pass order is Early -> normal -> Late for the whole listener set.

### 12.2 The combat-ending guard (`Hook.cs:~40`, `IterateCombatHookListeners`)
Most combat hooks iterate through `IterateCombatHookListeners`, which **yields nothing if `IsOverOrEnding && !IsStarting` at the moment enumeration begins** (not re-checked per listener).
`IsOverOrEnding = IsEnding || !IsInProgress`; `IsEnding` (`IsCombatEnding`, `CM:417-436`) = `IsInProgress && (PendingLoss != null || (no alive primary enemy && !Hook.ShouldStopCombatFromEnding))`.
Consequences a sim must replicate:
* As soon as the last primary enemy dies (HP 0 and removed or not), hooks dispatched via this iterator stop firing, **before** `CheckWinCondition` formally ends combat. Examples: `AfterCardDrawn`, `AfterSideTurnStart`, `ModifyHandDraw` etc.
* The following hooks bypass the guard on purpose: `AfterBlockBroken`, `AfterCreatureAddedToCombat`, `ShouldCreatureBeRemovedFromCombatAfterDeath`, `ShouldStopCombatFromEnding`, and the run-level iterations (`BeforeCombatStart`, `AfterCombatEnd`, `AfterCombatVictory`, `AfterRoomEntered`, `ShouldDie`, death hooks...).
* A dispatch that begins while combat is live still runs for all listeners even if one of them ends combat.
* During setup (`IsStarting`) the guard is exempt, so `ModifyShuffleOrder(isInitial)`, `AfterAddedToRoom` powers etc. dispatch before `IsInProgress` becomes true.
* `CardPileCmd.Draw`, `Shuffle`, `CreatureCmd.GainBlock` additionally check `IsOverOrEnding` themselves and return early (draw returns `[]`, block returns 0).
* Hook predicates are AND-combined (`ShouldClearBlock`, `ShouldFlush`, `ShouldPlayerResetEnergy`, `ShouldEtherealTrigger`, `ShouldDraw`) except `ShouldTakeExtraTurn` and `ShouldStopCombatFromEnding` (OR).

### 12.3 Hook invocation index for this flow (in the order they occur)
Setup: `ModifyShuffleOrder(initial)` -> [`AfterRoomEntered`] -> `Monster.AfterAddedToRoom` -> (`RollMove`) -> `BeforeCombatStart` -> `BeforeCombatStartLate`.
Player turn start: `BeforeSideTurnStart` -> [power AmountOnTurnStart snapshot happens before it] -> `ShouldClearBlock`/`AfterPreventingBlockClear` -> `AfterBlockCleared` -> `ShouldPlayerResetEnergy`, `ModifyMaxEnergy` -> `AfterEnergyReset`/`Late` -> `BeforeHandDraw`/`Late` -> `ModifyHandDraw`, `AfterModifyingHandDraw` -> `ShouldDraw` -> (`ModifyShuffleOrder`, `AfterShuffle`) -> `AfterCardDrawn` per card -> `AfterPlayerTurnStartEarly`/normal/`Late` -> `AfterSideTurnStart`/`Late` -> `OrbQueue.AfterTurnStart` -> `AfterHandEmptied` (if hand empty) -> `AfterAutoPrePlayPhaseEnteredEarly`/normal/`Late`.
Player turn end: `AfterAutoPostPlayPhaseEntered` -> `BeforeSideTurnEndVeryEarly`/`Early`/normal -> `OrbQueue.BeforeTurnEnd` -> `ShouldEtherealTrigger` (+ exhaust hooks) -> turn-end-in-hand cards -> `BeforeFlush`/`Late` -> `ShouldFlush` -> `AfterFlush` -> `EndOfTurnCleanup` -> `AfterSideTurnEnd`/`Late` -> `ShouldTakeExtraTurn` -> `AfterTakingExtraTurn`.
Enemy turn: `BeforeSideTurnStart(Enemy)` -> block clear -> `AfterBlockCleared` -> `AfterSideTurnStart(Enemy)`/`Late` -> per enemy: move effects (attack/power hooks) -> `BeforeSideTurnEnd(Enemy)` -> `EndOfTurnCleanup` (players) -> `AfterSideTurnEnd(Enemy)`/`Late`.

---------------------------------------------------------------------------------------------

## 13. Win/loss, death handling, mid-combat creature add/remove, combat end

### 13.1 `CheckWinCondition(ts)` (`CM:1387-1400`) and where it runs
```
if ts.PendingLoss != null:  ProcessPendingLoss(): PendingLoss=null; IsInProgress=false; CombatEnded event; return true     # loss: NO EndCombatInternal, no AfterCombatEnd/Victory hooks
if IsCombatEnding(ts):      await EndCombatInternal(ts); return true                                                           # victory
return false
```
Call sites (in turn order): (1) `StartTurn` player side after `RunAutoPrePlayPhase` (`CM:827`); (2) `StartTurn` enemy side after `TurnStarted` (`:844`); (3) after **each** enemy's `TakeTurn` (`:1426`); (4) phase one: after `BeforeSideTurnEnd` (`:1557`), after all `DoTurnEnd` (`:1576`), and after `BeforeFlush` (`:1588`); (5) `EndEnemyTurn` after `EndEnemyTurnInternal` (`:1086`); (6) **`ActionExecutor.ExecuteActions` (`GameActions/ActionExecutor.cs:~160-171`) calls `CheckWinCondition()` after EVERY executed game action while `IsInProgress`, except `EndPlayerTurnAction` and `ReadyToBeginEnemyTurnAction`.** The player-driven actions (`PlayCardAction`, `UsePotionAction`, ...) therefore get a win/loss check immediately after each card play / potion use (after the whole card effect, including auto-plays it triggered, has finished). Between the killing blow and that check, `IsEnding` is already true and hooks/draws/block gain are suppressed (12.2). In a sync sim: run `CheckWinCondition` after every top-level card play / potion use / discard-potion action.

### 13.2 Loss
* `CreatureCmd.Kill`: after killing, if ALL `runState.Players` creatures are dead -> `CombatManager.LoseCombat()` (`CreatureCmd.cs:~479`): sets `PendingLoss` if none. The loss is applied at the next `CheckWinCondition`. Between those, `IsEnding` is true (hooks suppressed).
* Player death handling (`CreatureCmd.KillWithoutCheckingWinCondition`, section 13.4): `OrbQueue.Clear()`, kill Osty, `DeactivateHooks()` (player's relics/potions/orbs/cards stop listening), `HandlePlayerDeath` (`CM:1218`): only when NOT all players are dead: remove their cards from combat, set energy 0, stars 0. Single-player: the all-dead branch runs, so none of that cleanup is executed and combat is just lost.
* If both a pending loss and "all enemies dead" hold, loss wins (checked first).
* Also: `Reset(graceful)` (`CM:1184`) tears down on exit.

### 13.3 Victory: `EndCombatInternal` (`CM:1297-1360`)
```
IsInProgress = false;  PlayersTakingExtraTurn.Clear();  Phase = None for all;  PlayerActionsDisabled = false
for p in Players: await p.ReviveBeforeCombatEnd()           # heals 1 if dead (multiplayer only in practice)
await Hook.AfterCombatEnd(runState, state, room)            # run-level iteration (deck cards, combat listeners incl. relics/powers/piles) - NO guard; relic bookkeeping resets (CentennialPuzzle, ...)
History.Clear()
room.OnCombatEnded()                                         # GoldProportion = 1 - escaped/spawned
for p in Players: p.AfterCombatEnd()                         # Creature.RemoveAllPowersInternalExcept() (no AfterRemoved), PlayerCombatState.AfterCombatEnd() (clear all piles, pets), Block := 0
await Hook.AfterCombatVictory(runState, state, room)         # AfterCombatVictoryEarly pass then AfterCombatVictory pass (e.g. post-combat heals, gold)
(stats/save/achievements/rewards screens: ignore)
CombatWon, CombatEnded events
```
Note the order: powers and combat piles are already cleared when `AfterCombatVictory` runs; relics still listen. Post-combat heal relics therefore act after power removal. `Player.AfterCombatEnd` does not touch HP.

### 13.4 Death/removal during combat (`CreatureCmd.Kill`, `CreatureCmd.cs:445-560` - summary of flow-relevant parts)
```
for each creature (given list order):  KillWithoutCheckingWinCondition(c):
    LoseHpInternal(all hp); Hook.AfterCurrentHpChanged
    Hook.BeforeDeath
    if force || MaxHp<=0 || Hook.ShouldDie(...)  (AND over run+combat listeners; preventers e.g. Fairy):
        c.InvokeDiedEvent(); shouldRemove = Hook.ShouldCreatureBeRemovedFromCombatAfterDeath(state, c)   # AND; vetoed by Reattach, Illusion, DieForYou, SteamEruption, PainfulStabs, Adaptable, ...
        Hook.AfterDeath(wasRemovalPrevented:false)
        teammates = alive teammates of c
        if shouldRemove && c is enemy && in state.Enemies:
              CombatManager.RemoveCreature(c)  (Monster.BeforeRemovedFromRoom(); ResetStateMachine())
              if !c.Monster.IsPerformingMove: state.RemoveCreature(c)        # else removed at end of PerformMove (10.2)
        c.RemoveAllPowersAfterDeath() then AfterRemoved for each (powers vetoing removal stay)
        if c enemy && c was primary && teammates nonempty && all teammates are secondary: Kill(teammates)       # minions die with the last primary
        elif c is player: OrbQueue.Clear(); kill Osty; DeactivateHooks(); HandlePlayerDeath
    else: Hook.AfterDeath(wasRemovalPrevented:true); Hook.AfterPreventingDeath; if still dead recurse (max 10)
after loop: if all players dead: LoseCombat() ; elif combat in progress: for dead players on the Player side: PlayerCmd.EndTurn(player)
```
A dead enemy that is NOT removed (`ShouldCreatureBeRemoved...` false) remains in `Enemies` with 0 HP; it is excluded from "alive primary enemy" tests but is still iterated by `ExecuteEnemyTurn` (see Q1).
`IsPrimaryEnemy` is evaluated BEFORE powers are removed (it needs the minion/illusion power).

### 13.5 Mid-combat creature add (`CreatureCmd.Add`, `CreatureCmd.cs:38-80`)
```
creature = state.CreateCreature(monster, Enemy, slot)      # R1 draw (Niche) + CombatId = next + monster.Rng (R5)
state.AddCreature(creature)                                # append to _enemies
CombatManager.AddCreature(creature)                        # SetUpForCombat (SpawnedThisTurn = true); SortEnemiesBySlotName if slot != null
await CombatManager.AfterCreatureAdded(creature)           # Monster.AfterAddedToRoom(); if enemy && CurrentSide==Player: RollMove (R3)
if CurrentSide != Enemy && monster: PrepareForNextTurn(rollNewMove:false)   # UI refresh only
await Hook.AfterCreatureAddedToCombat(state, creature)     # unguarded dispatch
```
Spawned during the player turn: gets an intent immediately (RollMove) and acts that enemy turn. Spawned during the enemy turn: no RollMove at spawn (side is Enemy), `SpawnedThisTurn` = true so it skips this enemy turn; its first RollMove happens... at the next player-turn start via `PrepareForNextTurn` (and `RollMove` on a fresh machine whose current state is a move is a no-op, so its initial move is its machine's initial state; see 5.2). `Encounter.GetNextSlot(state)` picks the first free slot name (`EncounterModel.cs:240`).
`CombatState.RemoveCreature` / `CreatureEscaped` simply remove from the list (escape: `Creature.RemoveAllPowersInternalExcept()`, recorded in `EscapedCreatures`).

---------------------------------------------------------------------------------------------

## 14. Async/pause points that are observable (what a sync sim must be able to suspend at)

The game awaits player choices inside hooks via `PlayerChoiceContext`. A choice inside `SetupPlayerTurn` pauses only that player's setup; the rest of `StartTurn` continues for other players and for non-player listeners. In single-player this reduces to: **the sim must be able to pause at any hook that can request a choice** and resume exactly there. Hooks that carry a choice context in this flow:
`BeforeHandDraw` (Toolbox, NightmarePower, ForegoneConclusion, CallOfTheVoid, Bolas/ThrummingHatchet cards in hand, ...), `AfterPlayerTurnStart*`, `AfterAutoPrePlayPhaseEntered*` (Mayhem/History Course auto-play with targeting), `AfterAutoPostPlayPhaseEntered*`, `AfterHandEmptied`, `DoTurnEnd` (orbs, turn-end cards, `AfterFlush`), `BeforeFlush`, `AfterSideTurnEnd*`, `BeforeSideTurnStart/End*`.
Within `Hook.BeforeSideTurnStart/BeforeSideTurnEnd/AfterSideTurnEnd/BeforeFlush` each listener's task is *started* and awaited until it completes OR pauses (`AssignTaskAndWaitForPauseOrCompletion`), then the next listener starts; paused listeners are completed afterwards (`Task.WhenAll`). Unpaused behaviour = strictly sequential in listener order.
Everything else is deterministic sequential code.

---------------------------------------------------------------------------------------------

## 15. History queries that gameplay code reads (`Combat/History/`)

* `CombatHistory` is one list of entries (cleared by `Reset` and `EndCombatInternal`, NOT between turns). Entry kinds: BlockGained, CardAfflicted, CardDiscarded, CardDrawn(fromHandDraw), CardExhausted, CardGenerated, CardPlayStarted, CardPlayFinished(WasEthereal), CreatureAttacked, DamageReceived, EnergySpent, MonsterPerformedMove, OrbChanneled, PotionUsed, PowerReceived, StarsModified (auto-logged by `PlayerCombatState.Stars` setter), Summoned. "Each entry is logged immediately after the event, before its After-hook".
* Each entry snapshots `RoundNumber`, `CurrentSide`, and **every player's `TurnNumber`** (`CombatHistoryEntry.cs:60-70`).
* `HappenedThisTurn(state)` = same `RoundNumber` && same `CurrentSide` && every snapshotted player still has the same `TurnNumber`. => "this turn" never includes the opposite side's events, and an extra turn is a distinct "turn" even though `RoundNumber` is equal. Used by ~34 call sites (cards-played-this-turn counters, Kunai/OrnamentalFan-like relics etc.).
* `HappenedLastPlayerTurn(player)` = entry's snapshot `TurnNumber == current TurnNumber - 1` for that player; **side is not checked**, so entries made during the enemy turn that followed (still carrying turn number n) also count as "last turn" for turn n+1 (`EmotionChip`, `HistoryCourse`, `Bolas`, `ThrummingHatchet`).
* Because `TurnNumber` is incremented in `SwitchSides` (before the next `StartTurn`), draw/play entries made during `StartTurn` of turn n carry turn n and Player side.
* The history stores references (cards, creatures, damage results); a Rust port needs only the fields queried by the card/relic/power specs plus (round, side, per-player turn number).
* Entries are only appended when `combatState.IsLiveCombat()` (always true for real combats).

---------------------------------------------------------------------------------------------

## 16. Things a faithful sim must replicate that are easy to miss

1. **Initial draw-pile order**: clone deck in deck order, `UnstableShuffle` with the run-level `Shuffle` stream (n-1 draws, Fisher-Yates from the top, swap with `NextInt(i+1)`), then `ModifyShuffleOrder(initial)`; top of pile = index 0. Mid-combat reshuffles use `StableShuffle` (sort by ModelId then upgrade level first), so they are independent of previous pile order but initial shuffle is NOT.
2. The `Shuffle`, `Niche`, `MonsterAi` streams are **run-persistent and shared across entities**; combat start state is whatever the run left. Consumption order: encounter-local Rng (R0) -> `Niche` per enemy in creation order -> `Shuffle` per player -> `MonsterAi` per enemy in `Creatures` order (initial) -> turn-start `RollMove`s.
3. **Monster HP**: one `Niche` draw per enemy with "unique among earlier-created enemies' MaxHp" semantics via an ascending-ordered `HashSet`; ranges with a single value still consume a draw; ascension changes min/max.
4. **Move rolling timing**: intents roll at the START of each player turn (before draw), in `Enemies` order; the very first roll is during `AfterCreatureAdded` (before `BeforeCombatStart`); the second roll on turn 1 is a no-op. Not rolled for extra turns. Moves performed are the pre-rolled `NextMove`; the following move is not rolled right after performing it.
5. **AfterSideTurnStart fires after the hand draw and `AfterPlayerTurnStart`**, and orb start-of-turn passives fire after `AfterSideTurnStart`. Block clear (`AfterTurnStart`) happens before energy reset and draw.
6. Block timing: each side clears its own block at the START of its own turn (`AfterTurnStart`), before it acts. Enemy block gained during the enemy turn persists through the following player turn and is cleared when the next enemy turn starts; the player's block gained during the player turn persists through the enemy turn and is cleared at the next player turn start. The player's block is NOT cleared on the player's first turn (`TurnNumber==1`).
7. Innate: only on `TurnNumber == 1`; bottom-of-pile enchantment cards first, then innate cards moved to top one by one (so their on-top order is the REVERSE of their pile order); hand draw = clamp(max(modifiedDraw, innateCount), .., 10).
8. `ShouldPlayerResetEnergy == false` (Ice Cream) means energy is ADDED (`+= MaxEnergy`), not kept as-is; `MaxEnergy` is itself hook-modified at read time.
9. Phase one vs two: ethereal exhaust / turn-end-in-hand cards / orb end-of-turn triggers happen in phase one, BEFORE `BeforeFlush` and flush; `AfterSideTurnEnd(Player)` happens AFTER the flush. A win during phase one skips flush and the entire enemy turn.
10. A card with a turn-end-in-hand effect that is also Ethereal is resolved as a turn-end card (else-if) and exhausted without the `ShouldEtherealTrigger` check.
11. `EndOfTurnCleanup` (reset `ExhaustOnNextPlay`, single-turn Retain/Sly, end-of-turn cost modifiers, temp star costs) runs twice per round: in `FlushPlayerHand` after `AfterFlush`, and again for all players at the end of the enemy turn (before `AfterSideTurnEnd(Enemy)`).
12. `ShouldFlush == false` retains EVERYTHING (not just Retain cards); retained cards are still subject to ethereal exhaust earlier. Cards flushed go to the discard pile bottom in hand order; there is no shuffle at turn end.
13. Hooks vanish when combat is "ending" (last primary dead or pending loss) even before `CheckWinCondition` runs; draws and block gain also bail. Victory/loss is only *processed* at the call sites in 13.1.
14. Loss processing skips `AfterCombatEnd`/`AfterCombatVictory` entirely. Victory processing clears powers and combat piles BEFORE `AfterCombatVictory`.
15. Extra turns: no enemy turn, no RoundNumber change, no new intents, players' TurnNumber +1 (block clear again, innate not repeated, `IceCream`-like `TurnNumber==1` checks false), `PlayersTakingExtraTurn` recomputed on every player-turn end via `ShouldTakeExtraTurn` (Ambergris/PaelsEye), participant set for phases = extra-turn players only.
16. `SpawnedThisTurn` semantics (section 9.2/13.5). `OnSideSwitch` runs at every switch, including player->player extra-turn switches and the harmless post-win switch.
17. `ExecuteEnemyTurn` iterates a snapshot of `Enemies` and calls `CheckWinCondition` after every enemy; it neither checks liveness nor stops for dead-but-retained enemies.
18. `BeforeCombatStart` iterates **deck** card instances first (runState iterator), then combat listeners; `AfterRoomEntered` runs before enemy initial `RollMove`.
19. Power `AmountOnTurnStart` snapshot happens in `BeforeTurnStart` (before `BeforeSideTurnStart`) for the creatures starting the turn only (so the enemy snapshot happens at the enemy turn, the ally snapshot at the player turn).
20. `HappenedThisTurn` also compares all players' turn numbers, and `HappenedLastPlayerTurn` ignores side.
21. Hand size cap 10 applies in `Draw` (loop stops when full); the turn-1 innate clamp to 10 is separate.
22. Retain/flush uses `card.ShouldRetainThisTurn` evaluated at flush time (after `ShouldFlush`), and single-turn retain is cleared at the same flush by `EndOfTurnCleanup` (after `AfterFlush`), so `AfterFlush` hooks still see it.
23. Single-player: `AllPlayersReadyToEndTurn()` always true; `SetReadyToBeginEnemyTurn` completes on the first call. If the (only) player is dead at the end of turn-start setup, `StartTurn` returns early (10b) without running `CheckWinCondition` - but combat is lost via `PendingLoss` anyway.
24. Combat IDs and `TotalFloor`-dependent encounter RNG matter only for encounter generation (R0) and visuals (R5).

---------------------------------------------------------------------------------------------

## 17. Uncertainties and open questions

* **Q1 - dead-but-retained enemies in `ExecuteEnemyTurn`.** `Creature.TakeTurn` only checks `SpawnedThisTurn`, not `IsAlive`. Monsters that stay in `Enemies` after death (those whose powers veto `ShouldCreatureBeRemovedFromCombatAfterDeath`: Reattach, Illusion, SteamEruption, DieForYou, PainfulStabs, Adaptable) presumably have a "dead/revive" `NextMove` set by their own death/revive hooks. Not verified per monster; the monster/powers specs must confirm that their `NextMove` is a harmless or revive move.
* **Q2 - RESOLVED**: win/loss after card play comes from `ActionExecutor` (13.1 item 6). Still unverified: whether any card/potion effect that is *not* executed as a top-level game action (e.g. hook-driven actions via `GenericHookGameAction`) gets its own check; the check after the enclosing action covers it.
* **Q3 - enemy ordering after a spawn.** `SortEnemiesBySlotName` sorts only when the added creature has a non-null slot; spawn paths with `slotName == null` append to the end. For creatures with slots not in `Encounter.Slots` (`IndexOf == -1`) they sort before slotted ones. Relies on `List.Sort` being stable for <=16 items (insertion sort); not verified at runtime.
* **Q4 - `StableShuffle` ties.** Identical-key cards (same ModelId and upgrade level, e.g. 5 Strikes) are ordered by .NET's introsort (insertion sort for <=16 elements, but a real combat deck can exceed 16 cards, so introsort/heapsort partitions apply). The resulting arrangement of tied elements is implementation-defined. If tied cards differ in hidden state (enchantments/afflictions/cost mods) this changes behaviour; to be bit-exact a Rust port must replicate .NET `List<T>.Sort` (IntroSort with the CoreCLR partition/insertion thresholds) rather than use a stable sort. Not verified here.
* **Q5 - `ReadyToBeginEnemyTurnAction` queue ordering.** Phase two begins only after this action is dequeued; any actions already queued (e.g. enqueued by end-of-turn effects) run first. I could not verify whether any end-of-turn hook enqueues game actions rather than awaiting directly; assumed none for a sync sim.
* **Q6 - `AfterCombatVictory` and healing order** (Burning Blood etc.) is described structurally only; individual relic behaviour is in the relics spec.
* **Q7 - `TotalFloor` and `CurrentMapCoord`** are run-state values required for R0/R5; an RL env that does not model the map must supply them (R5 can be ignored; R0 needs `TotalFloor` for bit-exactness of encounter-internal randomness such as which slime variants/starter move indices appear).
* **Q8 - XxHash64 of the seed string** (`StringHelper.GetDeterministicHashCode`, `Helpers/StringHelper.cs:139`) is the root of every stream seed; `old`-prefixed seeds use the legacy hash (`:157`). RNG spec owner should confirm xxHash64 variant (seed 0) and the exact `snake_case` conversion of enum names (`CombatCardGeneration -> combat_card_generation`, `CombatOrbs -> combat_orbs`, `MonsterAi -> monster_ai`).
* **Q9 - multiplayer-only branches** (`ReviveBeforeCombatEnd`, per-player ready sets, `HandlePlayerDeath`, HP scaling, `MultiplayerScalingModel`) are documented but out of scope; the single-player path was traced assuming `IsSingleplayerOrFakeMultiplayer == true`.
* **Q10 - pets.** Pets (Osty etc.) are ally-side monster creatures: they get `BeforeTurnStart`, block clear and `AfterBlockCleared` at player turn start (they are in `Allies`) and are hook listeners as monsters; they never run `TakeTurn` (enemy-only) nor `RollMove` via the turn loop. Not traced through `PlayerCmd.AddPet` (creature creation/slot, `AfterAddedToRoom`).
