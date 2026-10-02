# 04 - Monsters, move state machines, encounters (STS2 v0.111.0 public-beta, combat only)

Scope: pure game-logic semantics of enemy AI, enemy spawn/HP, encounters and act pools. VFX/SFX/animation/Godot/UI/logging and
multiplayer scaling are ignored (single-player only; `playerCount==1` paths).

Conventions
- `C/` = `decomp/MegaCrit/Sts2/Core/`. Citations are `C/path:line`. Line numbers refer to the decompiled files as they exist today.
- Move ids are the exact string literals from the source (they are observable and are referenced by other code, e.g. `"SLASH_MOVE"`).
- "A8" = Ascension >= 8 (`AscensionLevel.ToughEnemies`), "A9" = Ascension >= 9 (`AscensionLevel.DeadlyEnemies`). "x/y" = base/ascended.
- "RNG" below always means the xoshiro256** `Rng` class (section 1.6). `Rng.NextFloat(max)` etc. are `Rng` methods, not `MegaRandom` methods.
- Intent tags: `A(d)`=SingleAttack, `A(dxn)`=MultiAttack, `Bf`=Buff, `Db`=Debuff, `Dbs`=Debuff(strong), `Df`=Defend, `Hl`=Heal, `Sm`=Summon, `Sl`=Sleep,
  `St`=Stun, `Sc(n)`=StatusCard(n), `Cd`=CardDebuff, `Dth(d)`=DeathBlow, `Esc`=Escape, `Hid`=Hidden.

--------------------------------------------------------------------------------------------------------------------------------

## 1. Move state machine and AI semantics

### 1.1 Objects

`MonsterModel` (C/Models/MonsterModel.cs) is one instance per creature (canonical model `.ToMutable()`d). Relevant fields:
`MoveStateMachine` (built once by `SetUpForCombat`, :410), `NextMove: MoveState` (the *pending/visible* move, initialised to a placeholder
`new MoveState()` = id `"UNSET_MOVE"`, performing it throws, :240), `SpawnedThisTurn` (:248), `Rng` (per-monster stream, **cosmetic only** - the only
user is ToughEgg's skin choice, C/Models/Monsters/ToughEgg.cs:118), `RunRng` (the run's `RunRngSet`, assigned in `CombatState.CreateCreature`, C/Combat/CombatState.cs:235).
Each concrete monster implements `GenerateMoveStateMachine()` which builds the graph *fresh per creature* (closures capture `this`).

State types (C/MonsterMoves/MonsterMoveStateMachine/):
- `MoveState(id, onPerform, intents...)` (MoveState.cs). Has `FollowUpState`/`FollowUpStateId` (unconditional successor, `GetNextState` returns it, no RNG, :67),
  `MustPerformOnceBeforeTransitioning` (default false) and a private `_performedAtLeastOnce` flag (set in `PerformMove`, cleared in `OnExitState`, :62).
  `CanTransitionAway = !MustPerformOnce || _performedAtLeastOnce` (:27-36). Always `ShouldAppearInLogs`.
- `RandomBranchState(id)` (RandomBranchState.cs): list of `StateWeight{stateId, repeatType, maxTimes, weightLambda, cooldown}`. Not logged.
- `ConditionalBranchState(id)` (ConditionalBranchState.cs): ordered list of `(target, Func<bool>)`; first predicate that is true wins; throws if none. Draws **no** RNG. Not logged.
- Several MoveStates may share one perform delegate but be distinct states (e.g. `THRASH_MOVE` vs `THRASH2_MOVE`): they are distinct objects in the log, which is
  how repeat limits are made to "reset" (see 1.5).

### 1.2 Core algorithm (`MonsterMoveStateMachine`, MonsterMoveStateMachine.cs)

Construction (:16-32): register all states by id; `_currentState = _initialState`; **if the initial state is loggable (i.e. a MoveState) it is appended to
`StateLog` immediately**. An initial branch state is not logged.

`RollMove` (:34) = `FindNextMoveState(logMove:true)` then asserts `_currentState` is a MoveState and returns it.

```
FindNextMoveState():                                   // :54
  if !cur.CanTransitionAway  ||  (!_performedFirstMove && cur.IsMove):  return        // :60  no-op, NO rng draw
  first_logged = null
  loop:                                                   // do { } while (!cur.IsMove)
     id = cur.GetNextState(owner, rng)                    // MoveState -> FollowUp id; Random -> weighted pick (1 draw); Conditional -> first true
     cur.OnExitState();  cur = (id empty ? initial : States[id]);  cur.OnEnterState()   // SetCurrentState :84
     if first_logged == null && cur.ShouldAppearInLogs: first_logged = cur
  StateLog.add(first_logged)      // only the first *loggable* state visited, i.e. the move finally chosen (branches are never logged)
```
`_performedFirstMove` becomes true on the first `OnMovePerformed` (called after every performed move, including stun wrapper moves, MonsterModel.cs:449).
Consequences:
1. The **first** RollMove of a combat does not transition: the initial MoveState simply becomes `NextMove`. If the initial state is a branch, the first RollMove
   walks it (RNG draw for a RandomBranch) and logs the resulting move.
2. The RollMove issued at the start of player turn 1 (after the one at combat setup) is a no-op for the same reason (no draw), as long as the monster has not yet performed a move.
3. A creature that has a MoveState with `MustPerformOnceBeforeTransitioning` that has not yet been performed never transitions (stun/revive/dead states rely on this).
4. `ForceCurrentState(state)` (:44) = `SetCurrentState(state)` (calls `OnExitState` on old - clears its performed flag - and `OnEnterState` on new). The forced state is **not** logged.

`StateLog` is a list of the *MoveState objects* chosen by RollMove (plus the initial one). It is the only memory used by repeat rules; it can contain the same state many times and
also contains moves that were rolled but never performed (e.g. replaced by a stun).

### 1.3 RandomBranchState weight algorithm (RandomBranchState.cs:115-166) - bit-exact details

```
GetNextState(owner, rng):
   max = (float) Sum_i GetStateWeight(i)           // LINQ Enumerable.Sum over float; see float note
   r   = rng.NextFloat(max)                        // = (float)( MegaRandom.NextDouble() * (double)max )   ONE 53-bit draw, even if only one branch has weight>0
   for s in States in insertion order:
        r -= GetStateWeight(s)                     // f32 subtraction; weight lambdas are RE-EVALUATED here
        if r <= 0f: return s.stateId
   throw
```
- Exactly **one** `MonsterAi` draw per RandomBranch traversal (consumes `rng._counter++` and one xoshiro step), regardless of how many branches are live. If all weights are 0,
  `max=0`, `r=0`, the first branch is returned (`0-0<=0`) - do not special-case.
- Float note (flag): `Rng.NextFloat(min,max)` is `(float)(NextDouble()*(double)(max-min)+(double)min)` (Rng.cs, `NextFloat`) i.e. it uses the *double* draw
  `(next_u64>>11)*2^-53`, not `MegaRandom.NextFloat`. `Enumerable.Sum(float)` accumulates in `double` and casts to `float` at the end (believed, .NET Core 3.0+; only
  matters for non-dyadic weights, i.e. TwoTailedRat's 1/12 and Fogmog's 0.4/0.6) - verify when porting; the per-branch subtraction is plain f32.
- `GetStateWeight(s)` (:130): `w = base_multiplier(repeat rules) * s.weightLambda()` (or 0 if the cooldown rule fires first):
  - `UseOnlyOnce`: multiplier 0 if the state object appears **anywhere** in `StateLog`, else 1.
  - `CanRepeatForever`: 1.
  - `CannotRepeat` (treated as N=1) and `CanRepeatXTimes(N)`: multiplier 1 if `StateLog.Count < N`; otherwise 1 iff any of the last N log entries is a *different* state object,
    i.e. multiplier 0 iff the last N entries are all this very state. (For N=1: 0 iff the most recent log entry is this state.)
  - `cooldown>0`: weight forced to 0 if a move whose `Id == stateId` appears in the last `cooldown` MoveState entries of the log (`Where(IsMove).Reverse().Take(cooldown)`).
- `AddBranch` overloads (RandomBranchState.cs:46-112) - **read the int argument by position**: `(state, int maxRepeats)`, `(state, int maxRepeats, float w)` and
  `(state, int maxRepeats, Func<float>)` mean `CanRepeatXTimes(maxRepeats)`; `(state, int cooldown, MoveRepeatType)` and `(state, int cooldown, MoveRepeatType, float|Func)` mean a
  **cooldown** (e.g. Flyconid `AddBranch(m, 3, CannotRepeat)` = cooldown 3 + CannotRepeat, *not* weight 3). Plain `(state, MoveRepeatType[, w])` = no cooldown, weight 1 unless given.
  Catalog notation: `RAND{A:CNR, B:x2, C:w0.4:CNR, D:cd3+CNR, E:ONCE, F:INF}` where CNR=CannotRepeat, xN=CanRepeatXTimes(N), cdN=cooldown N, ONCE=UseOnlyOnce, INF=CanRepeatForever, wX=weight lambda.

### 1.4 ConditionalBranchState
`GetNextState` evaluates predicates in registration order, returns the first true (`Evaluate()>0`), throws otherwise (ConditionalBranchState.cs:44-52). Predicates read live creature
state at *roll time* (HP, powers, `Creature.SlotName`, ally counts, monster-private flags such as `IsOffBalance`). They are never re-evaluated when the move actually executes.

### 1.5 StateLog consequences worth remembering
- A move that is replaced by a stun/forced move before being performed is still in the log (it was rolled), and is logged **again** if it is re-entered after the stun.
- Branch states are never logged, so `CannotRepeat` on branch edges compares against the last *move*.
- Re-using the same MoveState object in two places (self-loop `moveState.FollowUpState = moveState`) is allowed; `OnExitState` clears `_performedAtLeastOnce` even for a self-transition.
- Two distinct MoveState objects with identical behaviour (`SWIPE_MOVE`/`SWIPE_RANDOM_MOVE`, `UNLOAD_MOVE`/`UNLOAD_MOVE_2`, `ZOOM_MOVE`/`ZOOM_MOVE_2`, `THRASH_MOVE`/`THRASH_MOVE_2`,
  `TACKLE_2_MOVE`/`TACKLE_3_MOVE`/`TACKLE_4_MOVE`) are used to build multi-turn fixed cycles; port them as separate states.

### 1.6 RNG streams that touch monsters (all are `Rng` instances; none belongs to the combat)

`RunRngSet` (C/Runs/RunRngSet.cs) owns 12 run-scoped streams; stream `t` is `new Rng(Seed, snake_case(t))` = `new Rng(Seed + xxHash64_utf8(name), 0 seed)` (RunRngSet.cs:139-143),
where `Seed = xxHash64(seedString)` (XxHash64, hash seed 0; StringHelper.cs:139) and names are `up_front, shuffle, unknown_map_point, combat_card_generation, combat_potion_generation,
combat_card_selection, combat_energy_costs, combat_targets, monster_ai, niche, combat_orbs, treasure_room_relics` (C/Entities/Rngs/RunRngType.cs; `SnakeCase` regex StringHelper.cs:34,92).
`Rng(seed)` = `MegaRandom` = xoshiro256** whose 4 state words come from 4 splitmix64 steps of the seed (C/Random/MegaRandom.cs:~95-110). Streams persist across the whole run
(`SerializableRng{counter,state0..3}`), so a combat-only environment **must be able to start a combat from an arbitrary stream state** (open question 6.1).

| Stream | Who draws | Draws per event |
|---|---|---|
| `RunRng.MonsterAi` (`monster_ai`) | `RandomBranchState.GetNextState` via `MonsterModel.RollMove` (MonsterModel.cs:418); `Fabricator.SpawnBot` (C/Models/Monsters/Fabricator.cs:115) | 1 per RandomBranch traversal; 1 per bot spawned. `FlutterPower` (C/Models/Powers/FlutterPower.cs:48) passes it to `GetNextState` of the last-logged *MoveState*, which never draws. |
| `RunRng.Niche` (`niche`) | monster initial HP (`Creature.SetUniqueMonsterHpValue`, C/Entities/Creatures/Creature.cs:372, called from `CombatState.CreateCreature` :241); `ToughEgg.Hatch` hatchling HP (C/Models/Monsters/ToughEgg.cs:172) | exactly 1 draw per monster *creation* (also for min==max monsters and for mid-combat summons); 1 per hatch. Niche is also used by other (non-monster) code, so it is shared state. |
| `RunRng.CombatCardGeneration` | `ThievingHopper.ThieveryMove` picks the card(s) to steal (ThievingHopper.cs:223) | 1 `NextItem` per living target that has >=1 eligible card; candidates = Draw pile cards then Discard pile cards (shuffle-dependent order) filtered by the first non-empty priority tier |
| Encounter `Rng` (`EncounterModel._rng`) | encounter composition in `GenerateMonsters()` (section 4.2) | per encounter, see table; seed = `(ulong)((long)runSeed + (long)TotalFloor) + xxHash64(Id.Entry)` (EncounterModel.cs:254-262), `Id.Entry = Slugify(ClassName)` e.g. `SLIMES_WEAK` |
| `Creature.Monster.Rng` | cosmetic (skins) | - |
| `Rng.Chaotic` | cosmetic only (skins, VFX, dialogue pick) | **never** gameplay; ignore |

`NextItem(list)` = `NextInt(0,n)` = `floor(NextDouble()*n)` on the (possibly filtered) list order; 1 draw even if n==1 (n==0 returns default without a draw).
`NextInt(a,b)`=`a+floor(NextDouble()*(b-a))`; `NextBool()` = `MegaRandom.NextInt(2)==0` (Rng.cs: `_random.Next(2)==0`, i.e. `floor(d*2)==0`).

### 1.7 When intents are decided relative to the turn flow (this is what makes them observable)

Combat start (C/Rooms/CombatRoom.cs:194-225, C/Combat/CombatManager.cs:576-600, 1096-1131):
1. `CombatRoom.StartCombat`: `Encounter.GenerateMonstersWithSlots` (encounter-RNG draws), then for each `(monster, slot)` **in encounter list order**: `CombatState.CreateCreature`
   (-> Niche HP draw, `monster.RunRng`, per-monster Rng) then `CombatState.AddCreature` (append to `_enemies`; makes this creature visible to later HP-uniqueness checks).
2. `CombatManager.SetUpCombat` -> `AddCreature(creature)` for every creature: `monster.SetUpForCombat()` (= `GenerateMoveStateMachine()` + `SpawnedThisTurn=true`), then `SortEnemiesBySlotName()` if the creature has a slot.
3. `StartCombatInternal` loops `state.Creatures` (= allies then enemies in current list order) calling `AfterCreatureAdded`: for each enemy **in that order**:
   `Monster.AfterAddedToRoom()` (applies spawn powers / block / etc.), *then immediately* `Monster.RollMove()` (only if `CurrentSide==Player`, which is true at combat start).
   So per enemy the order is `[afterAdded_i, roll_i]`, interleaved, enemies in slot-sorted order. A later enemy's spawn hook runs after earlier enemies already rolled.
4. `Hook.BeforeCombatStart` (relics etc.), then player turn 1 `StartTurn`.

Every player turn (C/Combat/CombatManager.cs:~700-745): `Hook.BeforeSideTurnStart` -> **for each creature in `State.Enemies` in list order: `Creature.PrepareForNextTurn` -> `Monster.RollMove()`** (skipped on extra player turns) ->
(then `AfterTurnStart`: block clear of the *player* side, player draw/energy, etc.). Therefore intents for turn *t* are fixed **before the player draws/acts**, are visible all turn,
and the AI RNG is consumed in enemy-list order at the start of each player turn. Dead-but-retained creatures (Reattach/Adaptable/Illusion revivers) also roll.

Enemy turn (C/Combat/CombatManager.cs:1402-1430): iterates `Enemies.ToList()` (snapshot at turn start): for each still-contained creature `Creature.TakeTurn()` (:711) which runs
`Monster.PerformMove()` unless `SpawnedThisTurn`; then `CheckWinCondition` after **every** enemy. After all enemies, `EndEnemyTurn` -> `SwitchSides` -> `Creature.OnSideSwitch()` for all creatures (`SpawnedThisTurn=false`, MonsterModel.cs:483).
Enemy block is cleared in `AfterTurnStart(Enemy)` (Creature.cs:686-696) i.e. at the *start* of the enemy side's turn, before any move executes (subject to `ShouldClearBlock` hooks such as `BurrowedPower`).

Spawn timing rules (`CreatureCmd.Add`, C/Commands/CreatureCmd.cs:38-80):
- `CreateCreature` (Niche HP draw + unique-HP rule) -> `CombatState.AddCreature` -> `CombatManager.AddCreature` (`SetUpForCombat`, slot sort) -> `AfterCreatureAdded` (`AfterAddedToRoom`, and `RollMove` **only if the current side is Player**) ->
  if side != Enemy: `PrepareForNextTurn(rollNewMove:false)` (refresh only).
- Spawned **during the enemy turn**: no roll now (`NextMove` stays `UNSET_MOVE`), not in the turn snapshot so it does not act this turn; its first roll happens at the next player `StartTurn`; acts the following enemy turn.
- Spawned **during the player turn** (e.g. Axebot respawn via `StockPower` when killed by the player): rolls immediately (initial move becomes visible); `SpawnedThisTurn` is cleared at the side switch, so it **does act** in the enemy turn that follows.

### 1.8 Stun, forced moves, revival ("interrupts")

- `MonsterModel.SetMoveImmediate(state, forceTransition=false)` (MonsterModel.cs:421): only if `NextMove.CanTransitionAway || forceTransition`: `NextMove = state; machine.ForceCurrentState(state)`. Not logged. (A creature already holding an un-performed MustPerform state ignores a second stun.)
- `CreatureCmd.Stun(creature, [stunMove], nextMoveId)` -> `Creature.StunInternal` (Creature.cs:525-545): ignored if dead/no combat; `nextMoveId` defaults to **`StateLog.Last().Id`** (the last *rolled* move, which is the pending move that gets interrupted);
  builds a fresh `MoveState("STUNNED", stunMove, StunIntent){FollowUpStateId=nextMoveId, MustPerformOnceBeforeTransitioning=true}` and `SetMoveImmediate`s it. `Creature.IsStunned` = `NextMove.Id=="STUNNED"`.
  Timeline: stun during the player turn replaces the visible intent with St; the STUNNED move executes in the enemy turn (does nothing but the optional `stunMove` side effect, then `OnMovePerformed`); next player-turn roll transitions via `FollowUpStateId`
  (a MoveState id **or a branch id**; a branch is then walked with its RNG draw) and logs the resulting move. With the default `nextMoveId` the interrupted move is therefore re-logged and executed one turn late.
  Special `nextMoveId` choices matter: `AsleepPower`->`"SLASH_MOVE"`, `SlumberPower`->`"ROLL_OUT_MOVE"`, `BurrowedPower`->`"BITE_MOVE"`, `PlowPower`->`BEAST_CRY_MOVE`, `ShriekPower`->`TERROR_MOVE`,
  `FlutterPower`->`StateLog.Last().GetNextState(...)` i.e. the **successor of the pending move** (the pending move is skipped; ThievingHopper's pending move after FLUTTER is HAT_TRICK, so a broken Flutter continues with NAB).
- Player card `Whistle` (C/Models/Cards/Whistle.cs:33) also calls `CreatureCmd.Stun(target)`.
- Edge case (must be reproduced for bit-exactness): `IllusionPower`/`ReattachPower` set `isReviving=true` and then call `SetMoveImmediate(REVIVE/DEAD state)` **without** `force`. If the creature is holding an un-performed `STUNNED` (e.g. Whistle earlier this turn) when it dies, the revive state is ignored (`CanTransitionAway` is false) but `isReviving` stays set (unhittable); it then performs `STUNNED`, rolls into `StateLog.Last()`'s move while dead and never reaches REVIVE_MOVE/REATTACH_MOVE. `TestSubject`/`WaterfallGiant` use `forceTransition:true` and are immune.
- Revive-style forced states: `IllusionPower` (REVIVE_MOVE, heal intent, `MustPerformOnce`, follow-up = last logged id), `ReattachPower` (`DEAD_MOVE` -> `REATTACH_MOVE`), `AdaptablePower`/`TestSubject` (`RESPAWN_MOVE` via `forceTransition:true`),
  `WaterfallGiant` (`ABOUT_TO_BLOW_MOVE` via `forceTransition:true`), `Queen` (`SetMoveImmediate(ENRAGE_MOVE)` when Amalgam dies while BURN_BRIGHT is pending, Queen.cs:230). While reviving, `ShouldAllowHitting` is false (the creature is unhittable).

### 1.9 Move execution (`MonsterModel.PerformMove`, MonsterModel.cs:435-457)
`IsPerformingMove=true; move = NextMove; targets = CombatState.PlayerCreatures` (the player creatures, not pets) `; await move.PerformMove(targets)` (sets `_performedAtLeastOnce`, runs closure) `; machine.OnMovePerformed(move)`; history entry;
`IsPerformingMove=false`; if the creature died during its own move and removal is allowed it is removed from combat *after* the move (this is why GasBomb/Waterfall explode and Tunneler/Bowlbug self-stuns work).
`NextMove` is **not** advanced here - only `RollMove` (next player turn start) advances it. Damage numbers shown in the intent are recomputed live (below) but the *executed* damage is recomputed through the normal damage pipeline at execution time.

### 1.10 Minions, secondary enemies, win condition, escape
- `IsPrimaryEnemy` = enemy side without any power whose `OwnerIsSecondaryEnemy` is true (`MinionPower`, and `IllusionPower` which auto-applies `MinionPower`, C/Entities/Creatures/Creature.cs:253-280).
- `CombatManager.IsCombatEnding` (C/Combat/CombatManager.cs:417): combat continues while any *primary* enemy is alive, or while any hook returns `ShouldStopCombatFromEnding()=true`
  (`SurprisePower` on GremlinMerc, `StockPower` on Axebot, `InfestedPower` on PhrogParasite, `SteamEruptionPower` on WaterfallGiant, `AdaptablePower` on TestSubject).
  Revivers that stay in `Enemies` while dead (Reattach/Illusion/Adaptable/SteamEruption owners return `false` from `ShouldCreatureBeRemovedFromCombatAfterDeath`) are simply *not alive*, so they do not keep combat open by themselves.
- When the last primary enemy dies, `CreatureCmd.KillWithoutCheckingWinCondition` kills all remaining secondary teammates (CreatureCmd.cs:~565-575).
- Death hooks run **before** removal (`Hook.AfterDeath`, then `RemoveCreature` unless `ShouldCreatureBeRemovedFromCombatAfterDeath` says no) so death-triggered spawns (Stock, Infested, Surprise) see the dead creature's slot still occupied.
- `CreatureCmd.Escape` (CreatureCmd.cs:609): strips all powers, `RemoveCreature`, adds to `CombatState.EscapedCreatures` (used by gold-proportion logic `EncounterModel.CalculateGoldProportion`, EncounterModel.cs:~375, overridden by `GremlinMercNormal`).

### 1.11 Intents (C/MonsterMoves/Intents/*) - what the player can observe
`IntentType`: Attack, Buff, Debuff, DebuffStrong, Defend, Escape, Heal, Hidden, Summon, Sleep, Stun, StatusCard, CardDebuff, DeathBlow, Unknown (`UnknownIntent` is **unused** by any monster).
A move has 0..n intents (`new MoveState(id, fn, intent, intent, ...)`; e.g. `A + Bf`). The intent list is **static per move state**; only attack numbers are dynamic.
- `SingleAttackIntent(int|Func<decimal> dmg)`: Repeats=1. `MultiAttackIntent(int dmg, int|Func<int> repeats)`: shows per-hit and hits. `DeathBlowIntent(Func)`: a SingleAttack whose `IntentType=DeathBlow` (the attacker dies). `IntendsToAttack` = any Attack or DeathBlow intent (used by `GoForTheEyes`, MonsterModel.cs:242).
- Displayed damage = `Hook.ModifyDamage(me, target=player, dealer=monster, base, ValueProp.Move, ...)` for the local player (so it includes the monster's Strength, player's Vulnerable/Weak etc.), floored at 0 (`AttackIntent.GetSingleDamage`, AttackIntent.cs:~95-108). Total = per-hit x repeats.
- `StatusIntent(n)` exposes the card count `n` (`CardCount`). `DebuffIntent(strong)` exposes only the strong flag. All other intents expose only their type. Hidden/Sleep/Stun/Summon/Escape/Heal/Buff/Defend/CardDebuff expose nothing numeric.
- Dynamic attack values (lambda-backed): TheForgotten DREAD (`base + monster's Dexterity`), WaterfallGiant PRESSURE_GUN (grows +5 each use) and EXPLODE (eruption counter), TestSubject MULTI_CLAW (hit count `3 + ExtraMultiClawCount`).
For an RL observation per enemy: `[(intent_type, per_hit_dmg, hits)...]`, `move_id`, `is_stunned`, `intends_to_attack`.

### 1.12 Slots and enemy order
- `EncounterModel.Slots` (EncounterModel.cs:~215) is an ordered list of slot names; monsters carry `SlotName`. `CombatState.SortEnemiesBySlotName()` (CombatState.cs:496) sorts `_enemies` by `Slots.IndexOf(slot)` (insertion-sort/stable for <=16 elements; encounters with
  no `Slots` list but named slots - e.g. KnightsElite - keep creation order). **Enemy list order (after the slot sort) = enemy-turn order = player-turn roll order.** HP uniqueness/draw order instead follows *creation* order (encounter list order before slot sorting, or spawn order mid-combat); the two coincide for every shipped encounter except the Gremlin Merc death spawn.
- `GetNextSlot(combatState)` = first slot in `Slots` order not occupied by a current enemy (or "" if none) (EncounterModel.cs:~235). Summoners use either `GetNextSlot` (LivingFog, Fabricator) or `Slots.LastOrDefault(free)` (Ovicopter, TwoTailedRat),
  a fixed slot (`"illusion"`, `"fat"`, `"sneaky"`, `"wrigglerN"`), or the dead creature's own slot (Axebot respawn).
- Several monsters branch on `Creature.SlotName` at roll time (Exoskeleton, Myte, PhantasmalGardener, Wriggler).

### 1.13 Initial HP
`Creature(monster, side, slot)` ctor sets `maxHp=curHp=MaxInitialHp` (Creature.cs:347-358) and then, for enemies, `CombatState.CreateCreature` (CombatState.cs:233-247) calls
`SetUniqueMonsterHpValue(creaturesOnSide=_enemies so far, RunState.Rng.Niche)` (Creature.cs:372-384):
```
set = { Min .. Max }  (ascending ints)  minus { MaxHp of every other enemy currently in _enemies }
hp  = set.nonempty ? set[ floor(Niche.NextDouble()*|set|) ]  (NextItem over the HashSet, enumeration = ascending order)
                   : Niche.NextInt(Min, Max+1)               (also exactly 1 draw)
curHp = maxHp = hp ; MonsterMaxHpBeforeModification = hp
```
Exactly one Niche draw per creation. `ScaleMonsterHpForMultiplayer` is a no-op for 1 player. Uniqueness only considers creatures that have already passed `CombatState.AddCreature` (creatures that are merely *created* are invisible to it, e.g. Fat Gremlin vs Sneaky Gremlin below), so **creation order matters** and a lone-HP monster (min==max) still burns a draw.
Mid-combat summons use the same path (HP drawn from the same `Niche` stream, unique vs. current living enemies). Dead creatures that were removed no longer reserve HP values.
Some monsters then post-process HP in `AfterAddedToRoom` (DecimillipedeSegment: round max HP up to even and make all three segments' max HP distinct by +2 steps, wrapping to `ScaleHpForMultiplayer(MinInitialHp)` if it passes Max; PunchConstruct event reduction).

### 1.14 Porting notes (engine design)
- Model each monster as `{static graph table, private fields, perform(move_id, &mut CombatState)}`; branch predicates are small enums/closures over `(creature, combat)`; weight lambdas likewise.
- Keep `StateLog` as `Vec<StateId>` (the repeat rules need identity, not id strings), `performed_first_move`, `MustPerform`/`performed` flags per move state and the `NextMove` slot.
- `RollMove` order and the "first roll is a no-op for move-initial monsters" rule are required to be bit-exact with the AI stream.

--------------------------------------------------------------------------------------------------------------------------------

## 2. Ascension scaling mechanics

`AscensionLevel` (C/Entities/Ascension/AscensionLevel.cs): None=0, SwarmingElites=1, WearyTraveler=2, Poverty=3, TightBelt=4, AscendersBane=5, Inflation=6, Scarcity=7, ToughEnemies=8, DeadlyEnemies=9, DoubleBoss=10 (`maxAscensionAllowed=10`).
`AscensionManager.HasLevel(l)` = `current >= l` (cumulative); `AscensionHelper.GetValueIfAscension(level, ascValue, fallback)` (C/Helpers/AscensionHelper.cs:22-47) reads `RunManager.Instance.HasAscension(level)` (false when no run is in progress) and returns `ascValue` if active else `fallback`.

Combat-relevant effects (everything is baked into monster/encounter properties; no power/card/hook consults the ascension):
| Level | Effect on combat |
|---|---|
| A8 ToughEnemies | `MinInitialHp`/`MaxInitialHp` of nearly every monster (table in Appendix A), plus a handful of defensive constants (Plating/Block/Curl/Shriek/Hatchling HP/SiphonHeal etc., flagged "A8" in Appendix A). Gremlin Merc's *damage* values and `FrogKnight.PlatingAmount` also key off ToughEnemies. |
| A9 DeadlyEnemies | damage / multi-hit count / debuff-strength / strength-gain constants (Appendix A "A9"). Many entries are equal at both levels (`x→x`). |
| A10 DoubleBoss | the **last act** gets a second boss: `act.SetSecondBossEncounter(UpFront.NextItem(AllBossEncounters except first boss))` (C/Runs/RunManager.cs:761-765). Run-level; the combat itself is unchanged. |
| A1 SwarmingElites | map: elites per act = `round(5*1.6)=8` instead of 5 (C/Map/MapPointTypeCounts.cs:14). Not combat. |
| A3 Poverty | `EncounterModel.MinGoldReward/MaxGoldReward *= 0.75` (EncounterModel.cs:~70-95). Reward only. |
| A4 TightBelt, A5 AscendersBane | change the *inputs* of a combat, not monster behaviour: -1 max potion slot, an `AscendersBane` curse card in the starting deck (`AscensionManager.ApplyEffectsTo`, C/Entities/Ascension/AscensionManager.cs). |
| A2, A6, A7 | non-combat (Weary Traveler ancient gating, merchant card-removal price, card rarity odds). |

Defaults for gold (`RoomType`: Monster 10-20, Elite 35-45, Boss 100-100; `FakeMerchantEventEncounter` fixed 300) are encounter-level and only matter for rewards.
The HP of non-ascended values is the *range* `[Min,Max]` (A0) vs `[Min',Max']` (A8+) - both rolled uniquely per 1.13. Several monsters have `Max==Min` (fixed HP; the Niche draw is still consumed).

--------------------------------------------------------------------------------------------------------------------------------

## 3. Monster catalog

Legend for the `Cx` ("complex") column - `-` means plain data-driven monster (fixed state graph + simple powers/damage/block/status cards). Tags:
`SPAWN` creates creatures; `SHARED` reads/writes other creatures' state or an encounter field; `HOOK` behaviour driven by a custom power/hook (see the Powers spec); `CARD` adds generated cards to player piles
(status cards - note the pile and position); `CHOICE` blocks on a player choice; `STUN` self-stun / forced-move / wake-up logic; `PHASE` revive or HP-threshold phase; `ESC` escapes; `META` steals/returns run-level gold or deck cards; `DYN` dynamic intent values.
`HP` is `A0range | A8+range` (min-max, uniqueness-rolled per 1.13); numbers in moves are `A0/A9` unless stated; Appendix A lists every ascended constant exactly (authoritative if a typo is suspected).
Notation: `A -> B` fixed follow-up; `⟲` loops to itself/first state; `⇄` alternates; `INIT=` initial state. `RAND{...}` per 1.3 (NextFloat draw per traversal). "player" = target of the move (all `PlayerCreatures`).
Intents are listed in `[]` after the move id: the first tags are the intents shown to the player, text after `:` is the performed effect not visible in the intent.
Powers named here (Ritual, Artifact, Plating, Slippery, Thorns, ...) are defined in the Powers spec; "applier null" = applied with no source creature.

### 3.1 Act 1a "Overgrowth" monsters

| Class | Encounters | HP | Moves | Spawn powers / hooks / state | Cx |
|---|---|---|---|---|---|
| `AssassinRubyRaider` | RubyRaidersNormal | 18-23 / A8: 19-24 | INIT=`KILLSHOT_MOVE`[A(10/11)] ⟲ | - | - |
| `AxeRubyRaider` | RubyRaidersNormal | 20-22 / A8: 21-23 | INIT=`SWING_1`[A(5/6)+Df(5/6)] -> `SWING_2`[same] -> `BIG_SWING`[A(12/13)] -> SWING_1 | - | - |
| `BruteRubyRaider` | RubyRaidersNormal | 30-33 / A8: 31-34 | INIT=`BEAT_MOVE`[A(7/8)] ⇄ `ROAR_MOVE`[Bf: Str+3] | - | - |
| `CrossbowRubyRaider` | RubyRaidersNormal | 18-21 / A8: 19-22 | INIT=`RELOAD_MOVE`[Df: block 3] ⇄ `FIRE_MOVE`[A(14/16)] | `IsCrossbowReloaded` flag (cosmetic) | - |
| `TrackerRubyRaider` | RubyRaidersNormal | 21-25 / A8: 22-26 | INIT=`TRACK_MOVE`[Db: Frail 2] -> `HOUNDS_MOVE`[A(1 x8/9)] ⟲ | - | - |
| `CubexConstruct` | CubexConstructNormal; ConstructMenagerieNormal (x2, Act 3) | 65 / A8: 70 | INIT=`CHARGE_UP_MOVE`[Bf: Str+2] -> `REPEATER_BLAST_MOVE`[A(7/8)+Bf: Str+2] -> `REPEATER_BLAST_MOVE_2`[same] -> `EXPEL_MOVE`[A(5/6 x2)] -> REPEATER_BLAST_MOVE ... | spawn: GainBlock 13, Artifact 1; `IsBurrowed=true`, cleared by CHARGE_UP; HP-change handler is cosmetic | - |
| `FuzzyWurmCrawler` | FuzzyWurmCrawlerWeak; OvergrowthCrawlers | 55-57 / A8: 58-59 | INIT=`FIRST_ACID_GOOP`[A(4/6)] -> `INHALE`[Bf: Str+7] -> `ACID_GOOP`[A(4/6)] -> FIRST_ACID_GOOP (3-cycle) | `IsPuffed` cosmetic | - |
| `ShrinkerBeetle` | ShrinkerBeetleWeak; OvergrowthCrawlers | 38-40 / A8: 40-42 | INIT=`SHRINKER_MOVE`[Dbs: ShrinkPower -1 on player] -> `CHOMP_MOVE`[A(7/8)] -> `STOMP_MOVE`[A(13/14)] -> CHOMP ... | - | - |
| `Nibbit` | NibbitsWeak (IsAlone); NibbitsNormal (front+back) | 42-46 / A8: 44-48 | INIT_MOVE{alone: `BUTT`; else front: `SLICE`, back: `HISS`}; `BUTT_MOVE`[A(12/13)] -> `SLICE_MOVE`[A(6/7)+Df(5/6)] -> `HISS_MOVE`[Bf: Str+2/3] -> BUTT | encounter sets `IsAlone` / `IsFront` | - |
| `LeafSlimeS` | SlimesNormal, SlimesWeak (small) | 11-15 / A8: 12-16 | INIT=RAND{`TACKLE_MOVE`[A(3/4)]:CNR, `GOOP_MOVE`[Sc(1): Slimed->discard]:CNR}; both -> RAND (after the first pick it alternates, but a draw is still consumed) | - | CARD |
| `TwigSlimeS` | SlimesNormal, SlimesWeak (small) | 7-11 / A8: 8-12 | INIT=`TACKLE_MOVE`[A(4/5)] ⟲ | - | - |
| `LeafSlimeM` | SlimesNormal, SlimesWeak, FlyconidNormal, SlitheringStranglerNormal (medium) | 32-35 / A8: 33-36 | INIT=`STICKY_SHOT`[Sc(2): 2 Slimed->discard] ⇄ `CLUMP_SHOT`[A(8/9)] | - | CARD |
| `TwigSlimeM` | same pools as LeafSlimeM | 26-28 / A8: 27-29 | INIT=`STICKY_SHOT_MOVE`[Sc(1): 1 Slimed->discard] -> RAND{`POKEY_POUNCE_MOVE`[A(11/12)]:x2, STICKY_SHOT:CNR}; POKEY_POUNCE -> RAND | - | CARD |
| `Flyconid` | FlyconidNormal (+1 medium slime), SnappingJaxfruitNormal | 47-49 / A8: 51-53 | INIT=RAND_INITIAL{`FRAIL_SPORES_MOVE`:cd2+CNR, `SMASH_MOVE`:CNR}; every move -> RAND{`VULNERABLE_SPORES_MOVE`[Db: Vulnerable 2]:cd3+CNR, `FRAIL_SPORES_MOVE`[A(8/9)+Db: Frail 2]:cd2+CNR, `SMASH_MOVE`[A(11/12)]:CNR} | - | - |
| `Fogmog` | FogmogNormal (slot `fogmog`) | 74 / A8: 78 | INIT=`ILLUSION_MOVE`[Sm] -> `SWIPE_MOVE`[A(8/9)+Bf: Str+1] -> RAND{`SWIPE_RANDOM_MOVE`[=Swipe]:w0.4+CNR -> `HEADBUTT_MOVE`; `HEADBUTT_MOVE`[A(14/16)]:w0.6+CNR}; HEADBUTT -> SWIPE_MOVE | ILLUSION_MOVE: `CreatureCmd.Add<EyeWithTeeth>(slot "illusion")` (only in live combat) | SPAWN |
| `EyeWithTeeth` | spawned by Fogmog | 6 | INIT=`DISTRACT_MOVE`[Sc(3): 3 Dazed->discard] ⟲ | spawn: IllusionPower(1) (-> also MinionPower; revives via REVIVE_MOVE[Hl] after death, full heal) | PHASE, CARD, HOOK |
| `Inklet` | InkletsNormal x3 (middle one `MiddleInklet=true`) | 11-17 / A8: 12-18 | INIT=`JAB_MOVE`[A(3/4)] (middle: `WHIRLWIND_MOVE`[A(2/3 x3)]); JAB -> RAND{`PIERCING_GAZE_MOVE`[A(10/11)]:CNR, WHIRLWIND:CNR}; WHIRLWIND -> JAB; PIERCING_GAZE -> JAB | spawn: SlipperyPower(1). (An `INIT_RAND` branch object exists but is unreferenced.) | - |
| `Mawler` | MawlerNormal | 72 / A8: 76 | INIT=`CLAW_MOVE`[A(4/5 x2)]; all -> RAND{`RIP_AND_TEAR_MOVE`[A(14/16)]:CNR, `ROAR_MOVE`[Db: Vulnerable 3]:ONCE, CLAW:CNR} | - | - |
| `SlitheringStrangler` | SlitheringStranglerNormal | 53-55 / A8: 54-56 | INIT=`CONSTRICT`[Db: ConstrictPower 3] -> RAND{`THWACK`[A(7/8)+Df(5)]:INF, `LASH`[A(12/13)]:INF}; THWACK/LASH -> CONSTRICT | - | HOOK |
| `SnappingJaxfruit` | SnappingJaxfruitNormal | 31-33 / A8: 34-36 | INIT=`ENERGY_ORB_MOVE`[A(3/4)+Bf: Str+2] ⟲ | - | - |
| `VineShambler` | VineShamblerNormal | 61 / A8: 64 | INIT=`SWIPE_MOVE`[A(6/7 x2)] -> `GRASPING_VINES_MOVE`[A(8/9)+Cd: TangledPower 1] -> `CHOMP_MOVE`[A(16/18)] -> SWIPE | - | HOOK |
| `BygoneEffigy` (elite) | BygoneEffigyElite | 127 / A8: 132 | INIT=`SLEEP_MOVE`[Sl] -> `WAKE_MOVE`[Bf: Str+10] -> `SLASHES_MOVE`[A(13/15)] ⟲ (`SLEEP_MOVE_2` registered, unreachable) | spawn: SlowPower(1) | HOOK |
| `Byrdonis` (elite) | ByrdonisElite | 81-84 / A8: 90 | INIT=`SWOOP_MOVE`[A(17/19)] ⇄ `PECK_MOVE`[A(3/4 x3)] | spawn: TerritorialPower(1) (end of its side's turn: +Str Amount). A8: HP fixed 90 | HOOK |
| `PhrogParasite` (elite) | PhrogParasiteElite (slot `phrog`) | 61-64 / A8: 66-68 | INIT=`INFECT_MOVE`[Sc(3): 3 Infection->discard] ⇄ `LASH_MOVE`[A(4/5 x4)] (an unused `RAND` object exists) | spawn: InfestedPower(4). On death: InfestedPower spawns 4 `Wriggler`s (`StartStunned=true`) in slots `wriggler1..4` and keeps combat open | SPAWN, PHASE, CARD |
| `Wriggler` | spawned by Phrog death; DenseVegetationEventEncounter (4, `StartStunned=false`) | 17-21 / A8: 18-22 | INIT = `SPAWNED_MOVE`[St] if StartStunned else INIT_MOVE; SPAWNED -> INIT_MOVE{slot wriggler1/3: `NASTY_BITE_MOVE`[A(6/7)]; wriggler2/4: `WRIGGLE_MOVE`[Bf+Sc(1): 1 Infection->discard, Str+2]}; BITE ⇄ WRIGGLE | reads `Creature.SlotName` at roll time | CARD |
| `CeremonialBeast` (boss) | CeremonialBeastBoss | 252 / A8: 262 | INIT=`STAMP_MOVE`[Bf: PlowPower(150/160) on self] -> `PLOW_MOVE`[A(18/20)+Bf: Str+2 after] ⟲. Phase 2 via stun: `STUNNED`(MustPerform) -> `BEAST_CRY_MOVE`[Db: RingingPower 1] -> `STOMP_MOVE`[A(15/17)] -> `CRUSH_MOVE`[A(17/19)+Bf: Str+3/4] -> BEAST_CRY ... | PlowPower (counter = HP threshold): when it takes unblocked damage and `CurrentHp <= Amount`: remove Strength+TemporaryStrength, `IsInSecondPhase=true`, `Stun(StunnedMove, "BEAST_CRY_MOVE")`, remove Plow. `STUN_MOVE` state is unused except bestiary | HOOK, STUN, PHASE |
| `KinFollower` | TheKinBoss (slots `slot1`,`slot2`; slot1 `StartsWithDance`) | 58-59 / A8: 62-63 | INIT=`QUICK_SLASH_MOVE`[A(5)] -> `BOOMERANG_MOVE`[A(2 x2)] -> `POWER_DANCE_MOVE`[Bf: Str+2/3] -> QUICK_SLASH; StartsWithDance: INIT=POWER_DANCE | spawn: MinionPower | SHARED |
| `KinPriest` (boss leader) | TheKinBoss (slot `leaderSlot`) | 190 / A8: 199 | INIT=`ORB_OF_FRAILTY_MOVE`[A(8/9)+Db: Frail 1] -> `ORB_OF_WEAKNESS_MOVE`[A(8/9)+Db: Weak 1] -> `BEAM_MOVE`[A(3 x3)] -> `RITUAL_MOVE`[Bf: Str+2/3] -> ORB_OF_FRAILTY | `AfterDeath` of all followers only triggers dialogue (cosmetic). Primary enemy; followers die when it dies | SHARED |
| `Vantom` (boss) | VantomBoss | 173 / A8: 183 | INIT=`INK_BLOT_MOVE`[A(7/8)] -> `INKY_LANCE_MOVE`[A(6/7 x2)] -> `DISMEMBER_MOVE`[A(26/30)+Sc(3): 3 Wound->discard] -> `PREPARE_MOVE`[Bf: Str+2] -> INK_BLOT | spawn: SlipperyPower(8/9) | CARD, HOOK |

### 3.2 Act 1b "Underdocks" monsters

| Class | Encounters | HP | Moves | Spawn powers / hooks / state | Cx |
|---|---|---|---|---|---|
| `CorpseSlug` | CorpseSlugsNormal (3), CorpseSlugsWeak (2) | 25-27 / A8: 27-29 | `WHIP_SLAP_MOVE`[A(3 x2)] -> `GLOMP_MOVE`[A(8/9)] -> `GOOP_MOVE`[Db: Frail 2] -> WHIP_SLAP; INIT by `StarterMoveIdx%3` (0 WHIP, 1 GLOMP, 2 GOOP) | encounter: `n=EncounterRng.NextInt(3)`, slug k gets `StarterMoveIdx=n+k`. spawn: RavenousPower(4/5): when another creature on its side dies -> `IsRavenous`, `Stun(StunnedMove)` (default next = last logged) and `+Str Amount` | HOOK, STUN, SHARED |
| `Seapunk` | SeapunkNormal (+CalcifiedCultist), SeapunkWeak | 44-46 / A8: 47-49 | INIT=`SEA_KICK_MOVE`[A(11/13)] -> `SPINNING_KICK_MOVE`[A(2 x4)] -> `BUBBLE_BURP_MOVE`[Bf+Df: block 7/8, Str+1/2] -> SEA_KICK | - | - |
| `CalcifiedCultist` | CultistsNormal; SeapunkNormal | 38-41 / A8: 39-42 | INIT=`INCANTATION_MOVE`[Bf: Ritual 2 (self)] -> `DARK_STRIKE_MOVE`[A(9/11)] ⟲ | - | - |
| `DampCultist` | CultistsNormal | 51-53 / A8: 52-54 | INIT=`INCANTATION_MOVE`[Bf: Ritual 5/6] -> `DARK_STRIKE_MOVE`[A(1/3)] ⟲ | - | - |
| `FossilStalker` | FossilStalkerNormal | 51-53 / A8: 54-56 | INIT=`LATCH_MOVE`[A(12/14)]; all -> RAND{LATCH:x2, `TACKLE_MOVE`[A(9/11)+Db: Frail 1]:x2, `LASH_MOVE`[A(3/4 x2)]:x2} | spawn: SuckPower(3) | HOOK |
| `GremlinMerc` | GremlinMercNormal (slot `merc`) | 47-49 / A8: 51-53 | INIT=`GIMME_MOVE`[A(7/8 x2)] -> `DOUBLE_SMASH_MOVE`[A(6/7 x2)+Db: Weak 2] -> `HEHE_MOVE`[A(8/9)+Bf: Str+2] -> GIMME. (damage keys off **A8**, not A9) | spawn: SurprisePower(1), ThieveryPower(20) per player (each attack move calls `Steal()`: removes up to 20 gold, run-level). On death `SurprisePower.AfterDeath` (C/Models/Powers/SurprisePower.cs:18-40) does, in this order: `CreateCreature(FatGremlin,"fat")` (Niche HP draw #1; not yet in `_enemies`), apply `HeistPower` (gold stolen) to it, `CreatureCmd.Add<SneakyGremlin>("sneaky")` (Niche draw #2, **cannot see Fat's HP**, so equal HP is possible; added first), then `CreatureCmd.Add(fat)`. Resulting turn order: **Sneaky, Fat** (no `Slots` list -> stable creation order). Both are rolled immediately if the Merc died on the player's turn. Keeps combat open (`ShouldStopCombatFromEnding`). | SPAWN, META, HOOK |
| `FatGremlin` | spawned by GremlinMerc death (created before, added after, SneakyGremlin) | 13-17 / A8: 14-18 | INIT=`SPAWNED_MOVE`[St] -> `FLEE_MOVE`[Esc: `CreatureCmd.Escape`] ⟲ | HeistPower returns gold as a reward if it dies; escaping reduces gold proportion (`GremlinMercNormal.CalculateGoldProportion`) | ESC, META |
| `SneakyGremlin` | spawned by GremlinMerc death | 10-14 / A8: 11-15 | INIT=`SPAWNED_MOVE`[St] -> `TACKLE_MOVE`[A(9/10)] ⟲ | - | - |
| `HauntedShip` | HauntedShipNormal | 63 / A8: 67 | INIT=`HAUNT_MOVE`[Db+Sc(5): Weak 3, 5 Dazed->discard] -> `SWIPE_MOVE`[A(13/14)] -> `STOMP_MOVE`[A(4/5 x3)] -> SWIPE (HAUNT only once) | - | CARD |
| `LivingFog` | LivingFogNormal (slot `livingFog`; slots `bomb1..5` first) | 80 / A8: 82 | INIT=`ADVANCED_GAS_MOVE`[A(8/9)+Cd: SmoggyPower 1] -> `BLOAT_MOVE`[A(5/6)+Sm: spawns `GasBomb` x BloatAmount(=1) in `GetNextSlot`, then attacks] -> `SUPER_GAS_BLAST_MOVE`[A(8/9)] -> BLOAT -> SUPER ... | enemy order puts the Fog LAST (slot index 5) | SPAWN, HOOK |
| `GasBomb` | spawned by LivingFog | 7 / A8: 8 | INIT=`EXPLODE_MOVE`[Dth(8/9)]: attack then `CreatureCmd.Kill(self)` | spawn: MinionPower | - |
| `PunchConstruct` | PunchConstructNormal; ConstructMenagerieNormal; PunchOffEventEncounter (x2) | 55 / A8: 60 | INIT=`READY_MOVE`[Df: block 10] -> `FAST_PUNCH_MOVE`[A(5/6 x2)+Db: Frail 1] -> `STRONG_PUNCH_MOVE`[A(14/16)] -> READY. (`StartsWithFastPunch`: INIT=FAST_PUNCH) | spawn: Artifact 1; event: `StartingHpReduction` (EncounterRng.NextInt(2,10)): `CurrentHp = max(1, Hp - reduction)` | - |
| `SewerClam` | SewerClamNormal | 56 / A8: 58 | INIT=`JET_MOVE`[A(10/11)] ⇄ `PRESSURIZE_MOVE`[Bf: Str+4] | spawn: Plating(8/9) | HOOK |
| `SludgeSpinner` | SludgeSpinnerWeak | 37-39 / A8: 41-42 | INIT=`OIL_SPRAY_MOVE`[A(8/9)+Db: Weak 1]; all -> RAND{OIL_SPRAY:CNR, `SLAM_MOVE`[A(11/12)]:CNR, `RAGE_MOVE`[A(6/7)+Bf: Str+3]:CNR} | - | - |
| `Toadpole` | ToadpolesWeak (front, back) | 21-25 / A8: 22-26 | INIT_MOVE{front: `SPIKEN_MOVE`[Bf: Thorns+2]; back: `WHIRL_MOVE`[A(7/8)]}; WHIRL -> SPIKEN -> `SPIKE_SPIT_MOVE`[A(3/4 x3): Thorns-2 first] -> WHIRL | encounter sets `IsFront` | HOOK |
| `TwoTailedRat` | TwoTailedRatsNormal (3 in slots third/fourth/fifth) | 17-21 / A8: 18-22 | INIT = `StarterMoveIndex%3` -> SCRATCH/DISEASE_BITE/SCREECH (encounter rotation) else RAND. Every move -> RAND{`SCRATCH_MOVE`[A(8/9)]:CNR w(canSummon?1/12:1), `DISEASE_BITE_MOVE`[A(6/7)]:CNR same w, `SCREECH_MOVE`[Db: Frail 1]:cd3+CNR same w, `CALL_FOR_BACKUP_MOVE`[Sm]:ONCE w(canSummon?0.75:0)} | `_turnsUntilSummonable=2` (decremented by each non-summon move), `CallForBackupCount<3` (shared: after a call every rat gets `max+1`), `CanSummon` also needs a free slot and no teammate with `NextMove==CALL_FOR_BACKUP_MOVE`. Summon: new TwoTailedRat in `Slots.LastOrDefault(free)` (no MinionPower). Total live weight = 3/12+0.75 = 1.0 | SPAWN, SHARED, RNG(float weights) |
| `PhantasmalGardener` (elite) | PhantasmalGardenersElite x4 (slots first..fourth) | 26-31 / A8: 27-32 | INIT_MOVE{first: `FLAIL_MOVE`[A(1 x3)]; second: `BITE_MOVE`[A(5)]; third: `LASH_MOVE`[A(7)]; fourth: `ENLARGE_MOVE`[Bf: Str+2/3, `EnlargeTriggers++`]}; BITE -> LASH -> FLAIL -> ENLARGE -> BITE | spawn: SkittishPower(6/7): first powered-card hit each turn gives block Amount | HOOK, SHARED |
| `SkulkingColony` (elite) | SkulkingColonyElite | 75 / A8: 80 | INIT=`ZOOM_MOVE`[A(14/16)] -> `ZOOM_MOVE_2`[same] -> `INERTIA_MOVE`[A(9/11)+Bf: Str+2/4] -> `PIERCING_STABS_MOVE`[A(7/8 x2)] -> ZOOM_MOVE | spawn: HardenedShellPower(20) | HOOK |
| `TerrorEel` (elite) | TerrorEelElite | 140 / A8: 150 | INIT=`CRASH_MOVE`[A(16/18)] ⇄ `THRASH_MOVE`[A(3/4 x3)+Bf: Vigor 6]; `TERROR_MOVE`[Db: Vulnerable 99] only via stun -> CRASH | spawn: ShriekPower(70/75): on unblocked damage when `CurrentHp <= Amount` -> `Stun(next=TERROR_MOVE)`, power removed. `STUN_MOVE` state defined but unused | HOOK, STUN, PHASE |
| `LagavulinMatriarch` (boss) | LagavulinMatriarchBoss | 222 / A8: 233 | INIT=`SLEEP_MOVE`[Sl] -> SLEEP_BRANCH{`SLEEP_MOVE` while AsleepPower; else `SLASH_MOVE`[A(19/21)]}; SLASH -> `DISEMBOWEL_MOVE`[A(9/10 x2)] -> `SLASH2_MOVE`[A(12/14)+Df(12/14)] -> `SOUL_SIPHON_MOVE`[Db+Bf: player Str-2 Dex-2, self Str+2] -> SLASH | spawn: Plating(12), AsleepPower(3). Asleep: unblocked damage -> remove Plating, `IsAwake`, `Stun(WakeUpMove, next="SLASH_MOVE")`, remove Asleep; also decrements at the end of each of its side turns (3 -> sleeps through 3 enemy turns, SLASH on the 4th); at 0 `WakeUpMove` (no stun); Plating is removed by `BeforeSideTurnEndVeryEarly` when Amount<=1 | HOOK, STUN, PHASE |
| `SoulFysh` (boss) | SoulFyshBoss | 211 / A8: 221 | INIT=`BECKON_MOVE`[Sc(2): 1 Beckon->draw(random pos), 1 Beckon->discard] -> `DE_GAS_MOVE`[A(16/18)] -> `GAZE_MOVE`[A(7/8)+Sc(1): 1 Beckon->discard] -> `FADE_MOVE`[Bf: Intangible 2 self, `IsInvisible`] -> `SCREAM_MOVE`[A(13/15)+Db: Vulnerable 3] -> BECKON | - | CARD |
| `WaterfallGiant` (boss) | WaterfallGiantBoss | 240 / A8: 250 | INIT=`PRESSURIZE_MOVE`[Bf: SteamEruption +15/20] -> `STOMP_MOVE`[A(15/16)+Db+Bf: Weak 1, +3] -> `RAM_MOVE`[A(10/11)+Bf: +3] -> `SIPHON_MOVE`[Hl+Bf: heal 10/15 x players, +3] -> `PRESSURE_GUN_MOVE`[A(cur: 20/23, +5 per use)+Bf: +3] -> `PRESSURE_UP_MOVE`[A(13/14)+Bf: +3] -> STOMP ...; death -> `ABOUT_TO_BLOW_MOVE`[St, MustPerform: SteamEruptionDamage=counter, remove power] -> `EXPLODE_MOVE`[Dth(counter)] ⟲ (kills self) | SteamEruptionPower (counter builds +3/turn): on the Giant's death `TriggerAboutToBlowState()`: HP set to 999999999 (infinite), `SetMoveImmediate(ABOUT_TO_BLOW, force)`; stays in combat, keeps combat open | PHASE, STUN, HOOK, DYN |

### 3.3 Act 2 "Hive" monsters

| Class | Encounters | HP | Moves | Spawn powers / hooks / state | Cx |
|---|---|---|---|---|---|
| `BowlbugEgg` | BowlbugsNormal / BowlbugsWeak | 21-22 / A8: 23-24 | INIT=`BITE_MOVE`[A(7/8)+Df(7/8)] ⟲ | - | - |
| `BowlbugNectar` | BowlbugsNormal / Weak | 35-38 / A8: 36-39 | INIT=`THRASH_MOVE`[A(3)] -> `BUFF_MOVE`[Bf: Str+15/16] -> `THRASH2_MOVE`[A(3)] ⟲ | - | - |
| `BowlbugRock` | BowlbugsNormal / Weak (slot first/odd), SlumberingBeetleNormal | 45-48 / A8: 46-49 | INIT=`HEADBUTT_MOVE`[A(15/16)] -> POST_HEADBUTT{`DIZZY_MOVE`[St] if `IsOffBalance`; else HEADBUTT}; DIZZY -> HEADBUTT | spawn: ImbalancedPower(1): when its own damage is fully blocked sets `IsOffBalance` (other owners would `Stun`). HEADBUTT performs, then `if IsOffBalance: CreatureCmd.Stun(self, DizzyMove)` (self-stun *inside* its own move); DIZZY clears the flag | HOOK, STUN |
| `BowlbugSilk` | BowlbugsNormal, SlumberingBeetleNormal | 40-43 / A8: 41-44 | INIT=`TOXIC_SPIT_MOVE`[Db: Weak 1] ⇄ `THRASH_MOVE`[A(4/5 x2)] | - | - |
| `Chomper` | ChompersNormal x2 (second has `ScreamFirst`); TunnelerNormal (orphan, see 5) | 60-64 / A8: 63-67 | INIT=`CLAMP_MOVE`[A(8/9 x2)] ⇄ `SCREECH_MOVE`[Sc(3): 3 Dazed->discard]; ScreamFirst: INIT=SCREECH | spawn: Artifact 2 | CARD |
| `Tunneler` | TunnelerWeak; TunnelerNormal (orphan) | 87 / A8: 92 | INIT=`BITE_MOVE`[A(13/15)] -> `BURROW_MOVE`[Bf+Df: BurrowedPower, block 32/37] -> `BELOW_MOVE`[A(23/26)] ⟲; `DIZZY_MOVE`[St] -> BITE | BurrowedPower: owner's block is not cleared at turn start; when its block is broken -> `GetStunned`, `Stun(StillDizzyMove, next="BITE_MOVE")`, remove Burrowed (then block removed) | HOOK, STUN |
| `Exoskeleton` | ExoskeletonsWeak (3 slots), ExoskeletonsNormal (4) | 24-28 / A8: 26-30 | INIT_MOVE{first: `SKITTER_MOVE`[A(1 x3/4)]; second: `MANDIBLES_MOVE`[A(8/9)]; third: `ENRAGE_MOVE`[Bf: Str+2]; fourth: RAND}; SKITTER -> RAND; MANDIBLES -> ENRAGE; ENRAGE -> RAND; RAND{SKITTER:CNR, MANDIBLES:CNR} | spawn: HardToKillPower(9). Initial move chosen by slot name (slot order = turn order); `fourth` draws at the first roll | HOOK, SHARED |
| `HunterKiller` | HunterKillerNormal | 121 / A8: 126 | INIT=`TENDERIZING_GOOP_MOVE`[Db: TenderPower 1]; all -> RAND{`BITE_MOVE`[A(17/19)]:CNR, `PUNCTURE_MOVE`[A(7/8 x3)]:x2} | - | HOOK |
| `LouseProgenitor` | LouseProgenitorNormal | 134-136 / A8: 138-141 | INIT=`WEB_CANNON_MOVE`[A(9/10)+Db: Frail 2] -> `CURL_AND_GROW_MOVE`[Df+Bf: block 14/18, Str+5/7, `Curled=true`] -> `POUNCE_MOVE`[A(14/16)] -> WEB_CANNON | spawn: CurlUpPower(14/18); `Curled` reset when it attacks | HOOK |
| `Myte` | MytesNormal x2 (slots first/second) | 61-67 / A8: 64-69 | INIT_MOVE{first: `TOXIC_MOVE`[Sc(2): 2 Toxic->HAND]; second: `SUCK_MOVE`[A(4/6)+Bf: Str+2/3]}; TOXIC -> `BITE_MOVE`[A(13/15)] -> SUCK -> TOXIC | slot-dependent opener | CARD, SHARED |
| `Ovicopter` | OvicopterNormal (slot `ovicopter` last; `egg1..5` before) | 124-130 / A8: 126-132 | INIT=`LAY_EGGS_MOVE`[Sm: 3x `ToughEgg` into the last free slots, each +MinionPower] -> `SMASH_MOVE`[A(16/17)] -> `TENDERIZER_MOVE`[A(7/8)+Db: Vulnerable 2] -> SUMMON_BRANCH{LAY_EGGS if `CanLay`(alive teammates incl. self <= 3); else `NUTRITIONAL_PASTE_MOVE`[Bf: Str+3/4] -> SMASH} | slot choice `Slots.LastOrDefault(free)` | SPAWN, SHARED |
| `ToughEgg` | spawned by Ovicopter | 14-18 / A8: 15-19 | INIT=`HATCH_MOVE`[Sm] -> `NIBBLE_MOVE`[A(4/5)] ⟲ | spawn: HatchPower (display countdown: 1 if spawned on player's turn else 2) + MinionPower. HATCH: removes all non-Minion powers, HP := `Niche.NextInt(HatchlingMinHp, HatchlingMaxHp+1)` (19-22 / A8 20-23) + `SetMaxAndCurrentHp`; `AfterHatchedState=NIBBLE` | PHASE, RNG |
| `SlumberingBeetle` | SlumberingBeetleNormal (third slot) | 86 / A8: 89 | INIT=`SNORE_MOVE`[Sl] -> SNORE_NEXT{SNORE while SlumberPower; else `ROLL_OUT_MOVE`[A(16/18)+Bf: Str+2]}; ROLL_OUT ⟲ | spawn: Plating(15/18), SlumberPower(3). Unblocked damage decrements (0 -> `Stun(WakeUpMove, next="ROLL_OUT_MOVE")`); also decrements at the end of its side turn (0 -> `WakeUpMove`); WakeUp removes Plating | HOOK, STUN, PHASE |
| `SpinyToad` | SpinyToadNormal | 116-119 / A8: 121-124 | INIT=`PROTRUDING_SPIKES_MOVE`[Bf: Thorns+5, `IsSpiny`] -> `SPIKE_EXPLOSION_MOVE`[A(23/25): Thorns-5] -> `TONGUE_LASH_MOVE`[A(17/19)] -> SPIKES | - | HOOK |
| `TheObscura` | TheObscuraNormal (slots `illusion`,`obscura`) | 123 / A8: 129 | INIT=`ILLUSION_MOVE`[Sm: Parafright into slot `illusion`, `HasSummoned`] -> RAND; all -> RAND{`PIERCING_GAZE_MOVE`[A(10/11)]:CNR, `SAIL_MOVE`(WailMove)[Bf: Str+3 to ALL teammates]:CNR, `HARDENING_STRIKE_MOVE`[A(6/7)+Df(6/7)]:CNR} | - | SPAWN, SHARED |
| `Parafright` | spawned by TheObscura | 21 | INIT=`SLAM_MOVE`[A(16/17)] ⟲ | spawn: IllusionPower(1) -> Minion; revives (REVIVE_MOVE heal-to-full, MustPerform) after death | PHASE, HOOK |
| `ThievingHopper` | ThievingHopperWeak | 79 / A8: 84 | INIT=`THIEVERY_MOVE`[A(17/19)+Cd: steal cards] -> `FLUTTER_MOVE`[Bf: FlutterPower 5] -> `HAT_TRICK_MOVE`[A(21/23)] -> `NAB_MOVE`[A(14/16)] -> `ESCAPE_MOVE`[Esc] ⟲ | spawn: EscapeArtistPower(5). THIEVERY: for each living target picks cards from Draw+Discard that have a deck version, tiered by priority (uncommon non-Imbued > common/rare/event non-Imbued > basic/quest > ancient/Imbued), random pick via `RunRng.CombatCardGeneration`, removes them from combat AND from the deck, holds them in `SwipePower`, returned as a reward if the Hopper dies. FlutterPower: -50% powered-attack damage taken; each unblocked hit `-1`; at 0 `Stun(next=successor of pending move)`. ESCAPE: `CreatureCmd.Escape` | META, HOOK, STUN, ESC |
| `DecimillipedeSegment{Front,Middle,Back}` (elite) | DecimillipedeElite (slots segment1..3) | 40-46 / A8: 46-52 | `WRITHE_MOVE`[A(5/6 x2)], `BULK_MOVE`[A(6/7)+Bf: Str+2], `CONSTRICT_MOVE`[A(8/9)+Db: Weak 1]; cycle CONSTRICT -> BULK -> WRITHE -> CONSTRICT; INIT by `StarterMoveIdx%3` (0 WRITHE, 1 BULK, 2 CONSTRICT) with encounter `n=NextInt(3)`: front=n, middle=n+1, back=n+2. Death protocol below | spawn: Reattach(25); `AfterAddedToRoom` rounds max HP up to even and bumps by +2 until all three segments differ (wrap to Min). Death while another segment is alive: stays in `Enemies` (dead, unhittable), forced to `DEAD_MOVE`[no intent] (not MustPerform) -> next roll `REATTACH_MOVE`[Hl, MustPerform: heal 25 + revive unless all others dead] -> RAND{WRITHE:CNR, BULK:CNR, CONSTRICT:CNR}. Last segment death ends it | PHASE, SHARED, HOOK |
| `Entomancer` (elite) | EntomancerElite | 145 / A8: 165 | INIT=`BEES_MOVE`[A(3 x7/8)] -> `SPEAR_MOVE`[A(18/20)] -> `PHEROMONE_SPIT_MOVE`[Bf: if PersonalHive<3: Hive+1 & Str+1 else Str+2] -> BEES | spawn: PersonalHivePower(1): when it takes a powered attack from the player side, adds `Amount` Dazed to the draw pile at random positions | CARD, HOOK |
| `InfestedPrism` (elite) | InfestedPrismsElite | 161 / A8: 171 | INIT=`JAB_MOVE`[A(15/17)] -> `RADIATE_MOVE`[A(11/13)+Df(11/13)] -> `WHIRLWIND_MOVE`[A(5/6 x3)] -> `PULSATE_MOVE`[A(8/10)+Bf+Df: block 20/22, VitalSpark+] -> JAB | spawn: VitalSparkPower(2/3) | HOOK |
| `Crusher` (boss) | KaiserCrabBoss (slot `crusher`) | 209 / A8: 219 | INIT=`THRASH_MOVE`[A(12/14)] -> `ENLARGING_STRIKE_MOVE`[A(4)] -> `BUG_STING_MOVE`[A(6/7 x2)+Db: Weak 2, Frail 2] -> `ADAPT_MOVE`[Bf: Str+2/3] -> `GUARDED_STRIKE_MOVE`[A(12/14)+Df: block 18] -> THRASH | spawn: BackAttackLeftPower(1), CrabRagePower(1) (when a teammate dies: +Str 6, +99 block, power removed). Encounter has `FullyCenterPlayers`; the player gets SurroundedPower from Rocket (damage the player *takes* from the crab standing behind them is x1.5: with Facing=Right (initial) the BackAttackLeft crab (Crusher) is behind, with Facing=Left the BackAttackRight crab (Rocket); Facing flips when the player targets a card/potion at the crab with the opposite BackAttack power, and when a kill leaves all hittable enemies on one side) | SHARED, HOOK |
| `Rocket` (boss) | KaiserCrabBoss (slot `rocket`) | 199 / A8: 209 | INIT=`TARGETING_RETICLE_MOVE`[A(3/4)] -> `PRECISION_BEAM_MOVE`[A(18/20)] -> `CHARGE_UP_MOVE`[Bf: Str+2/3] -> `LASER_MOVE`[A(31/35)] -> `RECHARGE_MOVE`[Sl] -> TARGETING_RETICLE | spawn: SurroundedPower(1) applied to the opponents, BackAttackRightPower(1), CrabRagePower(1) | SHARED, HOOK |
| `KnowledgeDemon` (boss) | KnowledgeDemonBoss | 379 / A8: 399 | INIT=`CURSE_OF_KNOWLEDGE_MOVE`[Db] -> `SLAP_MOVE`[A(17/18)] -> `KNOWLEDGE_OVERWHELMING_MOVE`[A(8/9 x3)] -> `PONDER_MOVE`[A(11/13)+Hl+Bf: heal 30 x players, Str+2/3] -> BRANCH{CURSE if counter<3; else SLAP} | CURSE: for each player `CardSelectCmd.FromChooseACardScreen` between two cards from set[counter] = {Disintegration / MindRot}, {Disintegration / Sloth}, {Disintegration / WasteAway} (Disintegration damage var 6/7/8); chosen card's `OnChosen()` fires its effect; counter++. `IsBurnt` cosmetic | CHOICE, CARD |
| `TheInsatiable` (boss) | TheInsatiableBoss | 321 / A8: 341 | INIT=`LIQUIFY_GROUND_MOVE`[Bf+Sc(6): SandpitPower(4, target player) on self, 6 `FranticEscape` cards (3 draw random pos, 3 discard)] -> `THRASH_MOVE`[A(8/9 x2)] -> `LUNGING_BITE_MOVE`[A(28/31)] -> `SALIVATE_MOVE`[Bf: Str+2/3] -> `THRASH_MOVE_2` -> THRASH_MOVE ... | SandpitPower (countdown that kills the targeted player at 0, see Powers) | CARD, HOOK |

### 3.4 Act 3 "Glory" monsters

| Class | Encounters | HP | Moves | Spawn powers / hooks / state | Cx |
|---|---|---|---|---|---|
| `Axebot` | AxebotsNormal (slot `front`) | 70-78 / A8: 76-86 (+10 per respawn) | INIT=`HAMMER_UPPERCUT_MOVE`[A(14/18)+Db: Weak 2, Frail 2] -> `ONE_TWO_MOVE`[A(10/11 x2)] -> HAMMER ...; respawned instance: INIT=`BOOT_UP_MOVE`[Df+Bf: block 10/15, Str+(3/4 x RespawnCount)] -> HAMMER | spawn: StockPower(StockAmount=2). On death with Stock>0 spawns a new Axebot (StockAmount-1, +10 max HP per respawn, same slot/side) via `StockPower.AfterDeath` and keeps combat open | SPAWN, PHASE, HOOK |
| `DevotedSculptor` | DevotedSculptorWeak | 162 / A8: 172 | INIT=`FORBIDDEN_INCANTATION_MOVE`[Bf: Ritual 9 (applier null)] -> `SAVAGE_MOVE`[A(12/15)] ⟲ | - | - |
| `ScrollOfBiting` | ScrollsOfBitingWeak (3), ScrollsOfBitingNormal (4) | 30-37 / A8: 33-39 | INIT by `StarterMoveIdx%3` (0 `CHOMP`, 1 `CHEW`, 2 `MORE_TEETH`); `CHOMP`[A(14/16)] -> `MORE_TEETH`[Bf: Str+2] -> `CHEW`[A(5/6 x2)] -> RAND{CHOMP:CNR, CHEW:x2}; CHEW -> RAND | spawn: PaperCutsPower(2). Encounter: `n=NextInt(3)` ->  idx n, n+1, n+2 (Normal's 4th: idx 2) | HOOK |
| `LivingShield` | TurretOperatorWeak | 55 / A8: 65 | INIT=`SHIELD_SLAM_MOVE`[A(6)] -> BRANCH{SHIELD_SLAM if living allies>0; else `SMASH_MOVE`[A(16/18)+Bf: Str+3] ⟲} | spawn: RampartPower(25) | HOOK, SHARED |
| `TurretOperator` | TurretOperatorWeak | 41 / A8: 51 | INIT=`UNLOAD_MOVE`[A(3/4 x5)] -> `UNLOAD_MOVE_2`[same] -> `RELOAD_MOVE`[Bf: Str+1] -> UNLOAD_MOVE | - | - |
| `Fabricator` | FabricatorNormal (slots bot1,bot2,**fabricator**,bot3,bot4) | 150 / A8: 155 | INIT=`fabricateBranch`{RAND if `CanFabricate`(alive teammates incl. self <4) else DISINTEGRATE}; RAND{`FABRICATE_MOVE`[Sm]:INF, `FABRICATING_STRIKE_MOVE`[A(18/21)+Sm]:INF}; `DISINTEGRATE_MOVE`[A(11/13)]; all -> fabricateBranch | FABRICATE spawns 1 defensive {Guardbot,Noisebot} + 1 aggro {Zapbot,Stabbot}; FABRICATING_STRIKE attacks then spawns 1 aggro. Each pick: `items = set minus _lastSpawned` (one field for both sets, set order = declaration order), `RunRng.MonsterAi.NextItem(items)`, slot `GetNextSlot`, then `MinionPower` applied (applier = Fabricator) | SPAWN, SHARED, RNG |
| `Guardbot` | spawned by Fabricator | 16-20 / A8: 17-21 | INIT=`GUARD_MOVE`[Df]: 15 block (Unpowered) to every Fabricator on its side ⟲ | MinionPower from Fabricator | SHARED |
| `Noisebot` | spawned by Fabricator | 18-23 / A8: 19-24 | INIT=`NOISE_MOVE`[Sc(2)]: 1 Dazed->discard + 1 Dazed->draw(random pos) per target ⟲ | MinionPower | CARD |
| `Zapbot` | spawned by Fabricator | 18-23 / A8: 19-24 | INIT=`ZAP`[A(14/15)] ⟲ | spawn: HighVoltagePower(2); MinionPower | HOOK |
| `Stabbot` | spawned by Fabricator | 18-23 / A8: 19-24 | INIT=`STAB_MOVE`[A(11/12)+Db: Frail 1] ⟲ | MinionPower | - |
| `FrogKnight` | FrogKnightNormal | 191 / A8: 199 | INIT=`TONGUE_LASH`[A(13/14)+Db: Frail 2] -> `STRIKE_DOWN_EVIL`[A(21/23)] -> `FOR_THE_QUEEN`[Bf: Str+5] -> HALF_HEALTH{`BEETLE_CHARGE`[A(35/40)] if !HasBeetleCharged && `CurrentHp < MaxHp/2` (int div); else TONGUE_LASH}; BEETLE_CHARGE -> TONGUE_LASH | spawn: Plating(15/19). `HasBeetleCharged` set inside BEETLE_CHARGE | HOOK |
| `GlobeHead` | GlobeHeadNormal | 148 / A8: 158 | INIT=`SHOCKING_SLAP`[A(13/14)+Db: Frail 2] -> `THUNDER_STRIKE`[A(6/7 x3)] -> `GALVANIC_BURST`[A(16/17)+Bf: Str+2] -> SHOCKING_SLAP | spawn: GalvanicPower(6/8) | HOOK |
| `OwlMagistrate` | OwlMagistrateNormal | 231 / A8: 247 | INIT=`MAGISTRATE_SCRUTINY`[A(16/17)] -> `PECK_ASSAULT`[A(4 x6)] -> `JUDICIAL_FLIGHT`[Bf: SoarPower 1, `IsFlying`] -> `VERDICT`[A(33/36)+Db: Vulnerable 4; removes Soar] -> SCRUTINY | - | HOOK |
| `SlimedBerserker` | SlimedBerserkerNormal | 261 / A8: 281 | INIT=`VOMIT_ICHOR_MOVE`[Sc(10): 10 Slimed->discard] -> `FURIOUS_PUMMELING_MOVE`[A(4/5 x4)] -> `LEECHING_HUG_MOVE`[Db+Bf: Weak 3 (applier null), Str+3] -> `SMOTHER_MOVE`[A(30/33)] -> VOMIT | - | CARD |
| `TheLost` | TheLostAndForgottenNormal | 93 / A8: 99 | INIT=`DEBILITATING_SMOG`[Db+Bf: player Str-2 (self +2)] ⇄ `EYE_LASERS`[A(4/5 x2)] | spawn: PossessStrengthPower(1, applier null) | HOOK |
| `TheForgotten` | TheLostAndForgottenNormal | 106 / A8: 111 | INIT=`MIASMA`[Db+Df+Bf: player Dex-2, self block 8, self Dex+2] ⇄ `DREAD`[A(13/15 + self Dexterity)] | spawn: PossessSpeedPower(1, applier null). DREAD damage is dynamic (reads Dex) | HOOK, DYN |
| `FlailKnight` (elite) | KnightsElite (slot `first`) | 101 / A8: 108 | INIT=`RAM_MOVE`[A(15/17)]; all -> RAND{`WAR_CHANT`[Bf: Str+3]:CNR, `FLAIL_MOVE`[A(9/10 x2)]:x2, RAM:x2} | - | - |
| `SpectralKnight` (elite) | KnightsElite (slot `second`) | 93 / A8: 97 | INIT=`HEX`[Db: HexPower 2 on player] -> `SOUL_SLASH`[A(15/17)] -> RAND{SOUL_SLASH:x2, `SOUL_FLAME`[A(3/4 x3)]:CNR}; SOUL_FLAME -> RAND | - | HOOK |
| `MagiKnight` (elite) | KnightsElite (slot `third`) | 82 / A8: 89 | INIT=`POWER_SHIELD_MOVE`[A(6/7)+Df: block 5/9] -> `DAMPEN_MOVE`[Db: DampenPower on player, `AddCaster(self)`] -> `RAM_MOVE`(Spear)[A(10/11)] -> `PREP_MOVE`[Df: block 5/9] -> `MAGIC_BOMB`[A(35/40)] -> RAM -> PREP -> MAGIC_BOMB ... | DampenPower is shared among casters | HOOK, SHARED |
| `MechaKnight` (elite) | MechaKnightElite | 300 / A8: 320 | INIT=`CHARGE_MOVE`[A(25/30)] -> `FLAMETHROWER_MOVE`[A(8/12)+Sc(4): 4 Burn->HAND] -> `WINDUP_MOVE`[Df+Bf: block 15, Str+5, `IsWoundUp`] -> `HEAVY_CLEAVE_MOVE`[A(35/40)] -> FLAMETHROWER ... | spawn: Artifact 3 | CARD |
| `SoulNexus` (elite) | SoulNexusElite | 234 / A8: 254 | INIT=`SOUL_BURN_MOVE`[A(29/31)]; all -> RAND{SOUL_BURN:CNR, `MAELSTROM_MOVE`[A(6/7 x4)]:CNR, `DRAIN_LIFE_MOVE`[A(18/19)+Dbs: Vulnerable 2, Weak 2]:CNR} | - | - |
| `TorchHeadAmalgam` | QueenBoss (slot `amalgam`, created first) | 199 / A8: 211 | INIT=`STRONG_TACKLE_MOVE`[A(26/32)] -> `TACKLE_2_MOVE`[A(18/22)] -> `BEAM_MOVE`[A(8 x3)] -> `TACKLE_3_MOVE`[A(14/16)] -> `TACKLE_4_MOVE`[same] -> BEAM_MOVE ... | spawn: MinionPower | SHARED |
| `Queen` (boss) | QueenBoss (slot `queen`) | 400 / A8: 419 | INIT=`PUPPET_STRINGS_MOVE`[Cd: ChainsOfBindingPower 3] -> `YOU_ARE_MINE_MOVE`[Db: Frail/Weak/Vulnerable 99] -> YOURE_MINE_NOW_BRANCH{`BURN_BRIGHT_FOR_ME_MOVE`[Bf+Df: +1 Str to every other teammate, block 20] if !HasAmalgamDied; else `OFF_WITH_YOUR_HEAD_MOVE`}; BURN_BRIGHT -> BURN_BRIGHT_FOR_ME_BRANCH (same test); OFF_WITH_YOUR_HEAD[A(3/4 x5)] -> `EXECUTION_MOVE`[A(15/18)] -> `ENRAGE_MOVE`[Bf: Str+2] -> OFF_WITH_YOUR_HEAD ... | `Amalgam = first enemy that is a TorchHeadAmalgam` (AfterAddedToRoom). When the Amalgam dies: `HasAmalgamDied=true`; if `NextMove==BURN_BRIGHT` -> `SetMoveImmediate(ENRAGE_MOVE)` | SHARED, PHASE |
| `TestSubject` (boss) | TestSubjectBoss | 100 / A8: 111 | Phase 1: INIT=`BITE_MOVE`[A(20/22)] ⇄ `SKULL_BASH_MOVE`[A(14/16)+Db: Vulnerable 1]. Death (Adaptable): `TriggerDeadState` -> `SetMoveImmediate(RESPAWN_MOVE, force)`[Hl+Bf, MustPerform]: `Respawns++`, `Revive(SecondFormHp 200/212 then ThirdFormHp 300/313)` (SetMaxHp+Heal), r=1: +PainfulStabsPower(1); r=2: +NemesisPower(1), remove Adaptable & PainfulStabs. Then REVIVE_BRANCH{`MULTI_CLAW_MOVE`[A(10/11 x(3+ExtraMultiClawCount))] ⟲ (count +1 each use) if Respawns<2; else phase 3: `PHASE3_LACERATE_MOVE`[A(10/11 x3)] -> `BIG_POUNCE`[A(45)] -> `BURNING_GROWL_MOVE`[Sc+Bf: 3/5 Burn->discard, Str+2/3] -> PHASE3_LACERATE ...} | spawn: AdaptablePower(1), EnragePower(2/3). AdaptablePower: unhittable while reviving, keeps combat open, not removed at death, `RunState.ExtraFields.TestSubjectKills++` (meta). HP 100/111 -> 200/212 -> 300/313 (fixed) | PHASE, HOOK, DYN, CARD |
| `Aeonglass` (boss) | AeonglassBoss | 512 / A8: 535 | INIT=`EBB_MOVE`[A(22/26)+Df: block 33] -> `EYE_LASERS_MOVE`[A(11/12 x2)] -> `INCREASING_INTENSITY_MOVE`[Sc(1/2)+Bf: Wither card(s)->discard; Str += base(3/4)+AdditionalStrength; AdditionalStrength++] -> EBB ... | spawn: WitheringPresencePower(6) on each opponent (target=player), Artifact 3. INCREASING_INTENSITY first `FakeUpgrade`s every Wither already in the player's card pools, `WitherUpgradeCount++`; `AfterCardGeneratedForCombat(Wither)` fake-upgrades newly generated Withers to the same count | CARD, HOOK, SHARED |

### 3.5 Event-only combatants (real, but not in any act pool)

| Class | Encounter / event | HP | Moves | Notes | Cx |
|---|---|---|---|---|---|
| `BattleFriendV1/V2/V3` | BattlewornDummyEventV1/V2/V3Encounter (event `BattlewornDummy`) | 75 / 150 / 300 | `NOTHING_MOVE` (no intents) ⟲ | spawn: BattlewornDummyTimeLimitPower(3) -> `CreatureCmd.Escape` when it expires; encounter sets `RanOutOfTime`; `ShouldGiveRewards=false` | ESC, HOOK |
| `FakeMerchantMonster` | FakeMerchantEventEncounter (slot `merchant`, gold 300) | 165 / A8: 175 | INIT=`SWIPE_MOVE`[A(13/15)]; SWIPE/SPEW_COINS/ENRAGE -> RAND_MOVE{SWIPE:CNR, `SPEW_COINS_MOVE`[A(2 x8)]:CNR, `THROW_RELIC_MOVE`[A(9/10)+Db: Frail 1]:CNR, `ENRAGE_MOVE`[Bf: Str+2]:cd3+CNR}; THROW_RELIC -> RAND_ATTACK_MOVE{SWIPE, SPEW, THROW all CNR} | fixed HP (min=max) | - |
| `MysteriousKnight` (extends `FlailKnight`) | MysteriousKnightEventEncounter (event TheLanternKey) | HP = FlailKnight (101 / A8: 108) | same AI as FlailKnight | spawn: Strength 6 and Plating 6 | HOOK |
| `Architect` | TheArchitectEventEncounter | 9999 | `NOTHING` (Hid) | event dummy | - |
| `PunchConstruct` x2 | PunchOffEventEncounter | see 3.2 | one starts with FAST_PUNCH | `StartingHpReduction` each = `EncounterRng.NextInt(2,10)` (drawn in constructor order: fast-punch construct first) | - |

### 3.6 Player-side pets (monster models used as allies - not enemies)
`Osty` (HP set by `OstyCmd`, `NOTHING_MOVE`, Necrobinder), `Byrdpip` (relic pet, HP 9999), `PaelsLegion` (Pael relic pet, HP 9999): all `NOTHING_MOVE` with no intents; they are created on the *player* side (`CombatSide.Player`) and never roll AI. Skip for the enemy engine (they matter for the player/relic specs).

### 3.7 Test / unused / deprecated monsters (see section 5)
`BigDummy`(9999), `OneHpMonster`(1), `TenHpMonster`(10), `MultiAttackMoveMonster`(999, `POKE` 1x5), `SingleAttackMoveMonster`(999, `POKE` 1), `TheAdversaryMkOne/Two/Three` (unreferenced: Artifact 0/1/2; loops SMASH|BASH|CRASH -> BEAM|FLAME_BEAM -> BARRAGE), `DeprecatedMonster`, and `Mocks/*` (6).

--------------------------------------------------------------------------------------------------------------------------------

## 4. Encounters

### 4.1 Acts, pools and room selection (what is "real")

`ModelDb.Acts` = Overgrowth, Underdocks, Hive, Glory (C/Models/ModelDb.cs:~300-320). There are **four** acts: Act 1 has two alternatives with `Index==0` (Overgrowth - the default - or Underdocks, chosen in the run lobby,
C/Multiplayer/Game/Lobby/StartRunLobby.cs:498-511), Act 2 = Hive (`Index==1`), Act 3 = Glory (`Index==2`); `DeprecatedAct` (Index -1) is an empty save-compat stub.
Each act's `GenerateAllEncounters()` (C/Models/Acts/*.cs) is the authoritative pool; `RoomType` and `IsWeak` partition it: weak = `RoomType.Monster && IsWeak`, regular = `Monster && !IsWeak`, elite = `Elite`, boss = `Boss`
(C/Models/ActModel.cs:144-164).

| Act | Class (Index) | Rooms before boss (`BaseNumberOfRooms`, MP -1) | Weak slots at start | Weak pool | Regular pool | Elite pool | Boss pool |
|---|---|---|---|---|---|---|---|
| 1a | `Overgrowth` (0, default) | 15 | 3 | FuzzyWurmCrawlerWeak, NibbitsWeak, ShrinkerBeetleWeak, SlimesWeak | CubexConstructNormal, FlyconidNormal, FogmogNormal, InkletsNormal, MawlerNormal, NibbitsNormal, OvergrowthCrawlers, RubyRaidersNormal, SlimesNormal, SlitheringStranglerNormal, SnappingJaxfruitNormal, VineShamblerNormal | BygoneEffigyElite, ByrdonisElite, PhrogParasiteElite | CeremonialBeastBoss, TheKinBoss, VantomBoss |
| 1b | `Underdocks` (0) | 15 | 3 | CorpseSlugsWeak, SeapunkWeak, SludgeSpinnerWeak, ToadpolesWeak | CorpseSlugsNormal, CultistsNormal, FossilStalkerNormal, GremlinMercNormal, HauntedShipNormal, LivingFogNormal, PunchConstructNormal, SeapunkNormal, SewerClamNormal, TwoTailedRatsNormal | PhantasmalGardenersElite, SkulkingColonyElite, TerrorEelElite | LagavulinMatriarchBoss, SoulFyshBoss, WaterfallGiantBoss |
| 2 | `Hive` (1) | 14 | 2 | BowlbugsWeak, ExoskeletonsWeak, ThievingHopperWeak, TunnelerWeak | BowlbugsNormal, ChompersNormal, ExoskeletonsNormal, HunterKillerNormal, LouseProgenitorNormal, MytesNormal, OvicopterNormal, SlumberingBeetleNormal, SpinyToadNormal, TheObscuraNormal | DecimillipedeElite, EntomancerElite, InfestedPrismsElite | KaiserCrabBoss, KnowledgeDemonBoss, TheInsatiableBoss |
| 3 | `Glory` (2) | 13 | 2 | DevotedSculptorWeak, ScrollsOfBitingWeak, TurretOperatorWeak | AxebotsNormal, ConstructMenagerieNormal, FabricatorNormal, FrogKnightNormal, GlobeHeadNormal, OwlMagistrateNormal, ScrollsOfBitingNormal, SlimedBerserkerNormal, TheLostAndForgottenNormal | KnightsElite, MechaKnightElite, SoulNexusElite | AeonglassBoss, QueenBoss, TestSubjectBoss |

Pool sizes: 22 / 20 / 20 / 18 encounters (C/Models/Acts/{Overgrowth,Underdocks,Hive,Glory}.cs). `TunnelerNormal` is in **no** pool (see 5).

Room generation (out of combat but determines which encounter is fought; `ActModel.GenerateRooms`, C/Models/ActModel.cs:331-388, uses the `up_front` stream):
`normalEncounters` = `NumberOfWeakEncounters` picks from a weak "grab bag" (refilled when empty), then picks from a regular grab bag until `BaseNumberOfRooms` entries; each pick excludes (when possible) an encounter equal to, or sharing an `EncounterTag`
with, the previously picked one (`AddWithoutRepeatingTags`, :419); `eliteEncounters` = 15 bag picks with the same tag rule; `Boss = UpFront.NextItem(AllBossEncounters)`; with A10 the last act also gets `SecondBossEncounter` (RunManager.cs:761).
Encounters are consumed in list order as map nodes are entered (`PullNextEncounter`, :445). Tags (C/Entities/Encounters/EncounterTag.cs): Burrower, Chomper, Nibbit, Shrinker, Slimes, Thieves, Workers, Crawler, Mushroom, Knights, Scrolls, Seapunk, Slugs, Exoskeletons, Jaxfruit.
Unknown ("?") rooms may resolve to a Monster room (`RoomType.Monster`, normal pool) - not modelled here.

### 4.2 Per-encounter details (generation order = initial enemy list order before slot sorting)

`GenerateMonstersWithSlots` (C/Models/EncounterModel.cs:254-270) seeds the per-encounter RNG once, calls `GenerateMonsters()` once, asserts every monster is mutable. `Rng` below = that encounter RNG (seed in 1.6).
`RoomType` default gold: Monster 10-20, Elite 35-45, Boss 100 (x0.75 with A3); `ShouldGiveRewards` true unless noted. "Slots" = ordered slot list (turn order); "-" = no `Slots` list (creation order is turn order).

#### Act 1a Overgrowth
| Encounter | RT | Tags | Enemies (slot) and setup | Encounter-RNG draws |
|---|---|---|---|---|
| `FuzzyWurmCrawlerWeak` | Monster(weak) | Crawler | FuzzyWurmCrawler | - |
| `NibbitsWeak` | Monster(weak) | Nibbit | Nibbit (`IsAlone=true`) | - |
| `ShrinkerBeetleWeak` | Monster(weak) | Shrinker | ShrinkerBeetle | - |
| `SlimesWeak` | Monster(weak) | Slimes | [small1, medium, small2]: `small1=NextItem([LeafSlimeS,TwigSlimeS])`; remove it; `small2=NextItem(remaining)` (1 draw, n=1); `medium=NextItem([LeafSlimeM,TwigSlimeM])` (draw order small1, small2, medium) | 3 |
| `CubexConstructNormal` | Monster | - | CubexConstruct | - |
| `FlyconidNormal` | Monster | Mushroom, Slimes | [NextItem([LeafSlimeM,TwigSlimeM]), Flyconid] | 1 |
| `FogmogNormal` | Monster | - | Fogmog (`fogmog`); Slots [illusion, fogmog] (EyeWithTeeth later fills `illusion`, turn order illusion first) | - |
| `InkletsNormal` | Monster | - | Inklet, Inklet(`MiddleInklet`), Inklet (no slots) | - |
| `MawlerNormal` | Monster | - | Mawler | - |
| `NibbitsNormal` | Monster | - | Nibbit(`IsFront`, slot `front`), Nibbit (slot `back`); Slots [front, back] | - |
| `OvergrowthCrawlers` | Monster | Shrinker, Crawler | ShrinkerBeetle, FuzzyWurmCrawler | - |
| `RubyRaidersNormal` | Monster | - | 3 distinct raiders: loop x3 `NextItem(types not yet used)`, key order Axe, Assassin, Brute, Crossbow, Tracker | 3 (lists of 5,4,3) |
| `SlimesNormal` | Monster | Slimes | [TwigSlimeM, LeafSlimeM, a, b] where `flag=NextBool()`; a = flag ? LeafSlimeS : TwigSlimeS; b = the other small | 1 |
| `SlitheringStranglerNormal` | Monster | Jaxfruit, Slimes | `NextItem({SnappingJaxfruit, MediumSlime, SmallSlimes})`; Jaxfruit -> [Jaxfruit, Strangler]; MediumSlime -> [NextItem(medium), Strangler]; SmallSlimes -> [NextItem(small), NextItem(small), Strangler] (with replacement) | 1 + (0, 1 or 2) |
| `SnappingJaxfruitNormal` | Monster | Mushroom, Jaxfruit | SnappingJaxfruit, Flyconid | - |
| `VineShamblerNormal` | Monster | - | VineShambler | - |
| `BygoneEffigyElite` | Elite | - | BygoneEffigy | - |
| `ByrdonisElite` | Elite | - | Byrdonis | - |
| `PhrogParasiteElite` | Elite | - | PhrogParasite (`phrog`); Slots [phrog, wriggler1..4]; death spawns 4 Wrigglers (see monster) | - |
| `CeremonialBeastBoss` | Boss | - | CeremonialBeast | - |
| `TheKinBoss` | Boss | - | KinFollower(`StartsWithDance`)@slot1, KinFollower@slot2, KinPriest@leaderSlot; Slots [slot1, slot2, leaderSlot] | - |
| `VantomBoss` | Boss | - | Vantom | - |

#### Act 1b Underdocks
| Encounter | RT | Tags | Enemies (slot) and setup | Draws |
|---|---|---|---|---|
| `CorpseSlugsWeak` | Monster(weak) | Slugs | 2x CorpseSlug; `EnsureCorpseSlugsStartWithDifferentMoves`: `n=NextInt(3)`, slug k `StarterMoveIdx=(n+k)%3` | 1 |
| `SeapunkWeak` | Monster(weak) | Seapunk | Seapunk | - |
| `SludgeSpinnerWeak` | Monster(weak) | - | SludgeSpinner | - |
| `ToadpolesWeak` | Monster(weak) | - | Toadpole(`IsFront=true`), Toadpole(`IsFront=false`) | - |
| `CorpseSlugsNormal` | Monster | Slugs | 3x CorpseSlug (same start rotation) | 1 |
| `CultistsNormal` | Monster | - | CalcifiedCultist, DampCultist | - |
| `FossilStalkerNormal` | Monster | - | FossilStalker | - |
| `GremlinMercNormal` | Monster | - | GremlinMerc (`merc`); `GoldWasStolen` set by SurprisePower; `CalculateGoldProportion`: if a FatGremlin escaped: 0 if gold was stolen else 0.5; else 1 | - |
| `HauntedShipNormal` | Monster | - | HauntedShip | - |
| `LivingFogNormal` | Monster | - | LivingFog (`livingFog`); Slots [bomb1..bomb5, livingFog] (GasBombs fill bomb1.. in order; Fog acts last) | - |
| `PunchConstructNormal` | Monster | - | PunchConstruct | - |
| `SeapunkNormal` | Monster | Seapunk | CalcifiedCultist, Seapunk | - |
| `SewerClamNormal` | Monster | - | SewerClam | - |
| `TwoTailedRatsNormal` | Monster | - | 3x TwoTailedRat in slots third, fourth, fifth with `StarterMoveIndex = (n, n+1, n+2)%3`, `n=NextInt(3)`; Slots [first..fifth] | 1 |
| `PhantasmalGardenersElite` | Elite | - | 4x PhantasmalGardener (first..fourth) | - |
| `SkulkingColonyElite` | Elite | - | SkulkingColony | - |
| `TerrorEelElite` | Elite | - | TerrorEel | - |
| `LagavulinMatriarchBoss` | Boss | - | LagavulinMatriarch | - |
| `SoulFyshBoss` | Boss | - | SoulFysh | - |
| `WaterfallGiantBoss` | Boss | - | WaterfallGiant | - |

#### Act 2 Hive
| Encounter | RT | Tags | Enemies (slot) and setup | Draws |
|---|---|---|---|---|
| `BowlbugsWeak` | Monster(weak) | Workers | BowlbugRock@`odd`, `NextItem([BowlbugEgg, BowlbugNectar])`@`even` (no `Slots` list) | 1 |
| `ExoskeletonsWeak` | Monster(weak) | Exoskeletons | 3x Exoskeleton (first, second, third); Slots [first,second,third] | - |
| `ThievingHopperWeak` | Monster(weak) | Thieves | ThievingHopper | - |
| `TunnelerWeak` | Monster(weak) | Burrower | Tunneler | - |
| `BowlbugsNormal` | Monster | Workers | BowlbugRock@first + 2 picks from {Egg, Silk, Nectar} without repeating a type (`NextItem(types with count<1)` x2) @middle, @last | 2 |
| `ChompersNormal` | Monster | Chomper | Chomper, Chomper(`ScreamFirst`) | - |
| `ExoskeletonsNormal` | Monster | Exoskeletons | 4x Exoskeleton (first..fourth) | - |
| `HunterKillerNormal` | Monster | - | HunterKiller | - |
| `LouseProgenitorNormal` | Monster | - | LouseProgenitor | - |
| `MytesNormal` | Monster | - | Myte@first, Myte@second | - |
| `OvicopterNormal` | Monster | - | Ovicopter (`ovicopter`); Slots [egg1..egg5, ovicopter] (ToughEggs fill from the last free slot) | - |
| `SlumberingBeetleNormal` | Monster | Workers | BowlbugRock@first, BowlbugSilk@second, SlumberingBeetle@third | - |
| `SpinyToadNormal` | Monster | - | SpinyToad | - |
| `TheObscuraNormal` | Monster | - | TheObscura (`obscura`); Slots [illusion, obscura] | - |
| `DecimillipedeElite` | Elite | - | Front@segment1, Middle@segment2, Back@segment3; `n=NextInt(3)` -> `StarterMoveIdx` (n, n+1, n+2)%3; Slots [segment1..3] | 1 |
| `EntomancerElite` | Elite | - | Entomancer | - |
| `InfestedPrismsElite` | Elite | - | InfestedPrism | - |
| `KaiserCrabBoss` | Boss | - | Crusher@crusher, Rocket@rocket; Slots [crusher, rocket]; `FullyCenterPlayers` | - |
| `KnowledgeDemonBoss` | Boss | - | KnowledgeDemon | - |
| `TheInsatiableBoss` | Boss | - | TheInsatiable | - |

#### Act 3 Glory
| Encounter | RT | Tags | Enemies (slot) and setup | Draws |
|---|---|---|---|---|
| `DevotedSculptorWeak` | Monster(weak) | - | DevotedSculptor | - |
| `ScrollsOfBitingWeak` | Monster(weak) | Scrolls | 3x ScrollOfBiting, `n=NextInt(3)`, `StarterMoveIdx=(n,n+1,n+2)%3` | 1 |
| `TurretOperatorWeak` | Monster(weak) | - | LivingShield, TurretOperator | - |
| `AxebotsNormal` | Monster | - | Axebot (`front`); Slots [front] | - |
| `ConstructMenagerieNormal` | Monster | - | PunchConstruct, CubexConstruct, CubexConstruct | - |
| `FabricatorNormal` | Monster | - | Fabricator (`fabricator`); Slots [bot1, bot2, fabricator, bot3, bot4] | - |
| `FrogKnightNormal` | Monster | - | FrogKnight | - |
| `GlobeHeadNormal` | Monster | - | GlobeHead | - |
| `OwlMagistrateNormal` | Monster | - | OwlMagistrate | - |
| `ScrollsOfBitingNormal` | Monster | Scrolls | 4x ScrollOfBiting; `n=NextInt(3)`: idx (n, n+1, n+2)%3, 4th idx=2 | 1 |
| `SlimedBerserkerNormal` | Monster | - | SlimedBerserker | - |
| `TheLostAndForgottenNormal` | Monster | - | TheLost, TheForgotten | - |
| `KnightsElite` | Elite | Knights | FlailKnight@first, SpectralKnight@second, MagiKnight@third (no `Slots` list -> creation order) | - |
| `MechaKnightElite` | Elite | - | MechaKnight | - |
| `SoulNexusElite` | Elite | - | SoulNexus | - |
| `AeonglassBoss` | Boss | - | Aeonglass | - |
| `QueenBoss` | Boss | - | TorchHeadAmalgam@amalgam, Queen@queen; Slots [amalgam, queen] (Amalgam acts first; Queen binds to it in `AfterAddedToRoom`) | - |
| `TestSubjectBoss` | Boss | - | TestSubject | - |

#### Event-driven encounters (real combats, not in act pools)
| Encounter | Event | RT | Enemies / setup | Draws |
|---|---|---|---|---|
| `BattlewornDummyEventV1/V2/V3Encounter` | BattlewornDummy | Monster, no rewards | BattleFriendV1/V2/V3 (HP 75/150/300, time limit 3); base class `BattlewornDummyEventEncounter` (abstract) stores `RanOutOfTime` | - |
| `DenseVegetationEventEncounter` | DenseVegetation | Monster | 4x Wriggler (`StartStunned=false`) in slots wriggler1..4 | - |
| `FakeMerchantEventEncounter` | FakeMerchant | Monster, gold 300 | FakeMerchantMonster@merchant | - |
| `MysteriousKnightEventEncounter` | TheLanternKey | Monster | MysteriousKnight | - |
| `PunchOffEventEncounter` | PunchOff | Monster | PunchConstruct(`StartsWithFastPunch`, hp reduction `NextInt(2,10)`) then PunchConstruct (`NextInt(2,10)`) | 2 |
| `TheArchitectEventEncounter` | TheArchitect | Monster | Architect (9999 HP dummy) | - |

Encounter-level gameplay hooks beyond composition: none other than `CalculateGoldProportion` (GremlinMerc), `SaveCustomState`/`LoadCustomState` (BattlewornDummy `RanOutOfTime`), and `OnCreatureSpawned` (records spawned monsters for `SpawnedEnemies`, used for the default gold proportion `1 - escaped/spawned`).
Cosmetic-only members to ignore: `HasScene`, `CustomBgm`, `AmbientSfx`, camera scaling/offset, `ExtraAssetPaths`, `BossNodePath`, `HasCustomBackground`.

--------------------------------------------------------------------------------------------------------------------------------

## 5. Mock / test / deprecated / non-pool entities (skip or treat specially)

Skip entirely (never reachable in a real run; keep only as optional test fixtures):
- Mock encounters (`IsMock => true`): `MockArtifactEncounter`, `MockAttackAndSummonEncounter`, `MockBossEncounter`, `MockEliteEncounter`, `MockMonsterEncounter`, `MockNoRewardsEncounter`, `MockPlatingEncounter`, `MockTwoMonsterEncounter` (C/Models/Encounters/Mocks/).
- Mock monsters (`IsMock`): `MockArtifactMonster`(9999 HP, Artifact 1), `MockAttackAndSummonMinionMonster`(10 HP, attack 1 + summon BigDummy minion), `MockAttackMonster`(9999, attack 1), `MockIntangibleMonster`(9999, Intangible 2 then nothing),
  `MockPlatingMonster`(9999, `PlatingAmount`), `MockReattachMonster`(1 HP, ReattachPower 1) (C/Models/Monsters/Mocks/). Useful as RL/unit-test fixtures only.
- Test monsters used only by mocks/tests: `BigDummy`(9999 HP, `NOTHING` Hid), `OneHpMonster`(1), `TenHpMonster`(10), `MultiAttackMoveMonster`(999, `POKE` 1x5), `SingleAttackMoveMonster`(999, `POKE` 1). No encounter references the last four.
- `DeprecatedMonster` (HP 0, `STUB`), `DeprecatedEncounter` (empty, RoomType Monster), `DeprecatedAct`: save-compat placeholders (`SaveUtil.*OrDeprecated`), never fought. Rooms generation filters `DeprecatedEncounter`.
- `TheAdversaryMkOne/Two/Three`: complete monsters (HP 100/200/300; Artifact 0/1/2; 3-cycle SMASH|BASH|CRASH -> BEAM|FLAME_BEAM -> BARRAGE[Str+2/3/4]) but **referenced by no encounter** (only the model registry) - unreleased content; skip.
- `TunnelerNormal` (Tunneler + Chomper(`ScreamFirst`), tags Burrower+Chomper): a valid encounter class that **no act pool lists** (Hive lists only `TunnelerWeak`) - unreachable in v0.111.0 (flag; include only if you want it for completeness).
- `BattlewornDummyEventEncounter` is an abstract base.

Not enemies (player-side models of class `MonsterModel`): `Osty`, `Byrdpip`, `PaelsLegion` (section 3.6).

Event-only but real: BattleFriendV1-3, FakeMerchantMonster, MysteriousKnight, Architect, DenseVegetation/PunchOff encounters (section 4.2). They need run/event context (rewards, gold) that a combat-only env can omit.
Non-gameplay: Bestiary / compendium helpers (`GenerateBestiaryMoveList`, `ShouldShowMoveInBestiary`, `BestiaryMonsterMove`), `MonsterModel.CreateVisuals/GenerateAnimator/SetupSkins/*Sfx`, `HpBarSizeReduction`, `ShouldFadeAfterDeath` etc.

--------------------------------------------------------------------------------------------------------------------------------

## 6. Uncertainties and open questions (please resolve before bit-exact validation)

1. **Initial stream state.** `monster_ai` and `niche` are run-scoped and carry state from every earlier floor (and `niche` is shared with unrelated users). A combat-only env needs an API to start from `(seed words | counter)` or a captured `SerializableRng` (xoshiro state0..3 is directly restorable, Rng.cs `LoadFromSerializable`). Encounter composition RNG depends on `TotalFloor` and the run seed, which a combat-only env must be handed (or the composition given directly).
2. **LINQ float sum.** `RandomBranchState.GetNextState` sums f32 weights with `Enumerable.Sum`; believed to accumulate in `double` (cast to f32 at the end). Only observable for non-dyadic weights (TwoTailedRat 1/12, Fogmog 0.4/0.6). Verify with a captured trace (data/ in the repo may already contain traces) before relying on it.
3. **HashSet enumeration order** for `Enumerable.Range(min,n).ToHashSet()` followed by `ExceptWith` is assumed ascending-with-gaps (no re-insertions); this is .NET's insertion-order enumeration for a never-resized-after-removal set. Same for `Fabricator`'s `HashSet<MonsterModel>` option sets (declaration order Zapbot, Stabbot; Guardbot, Noisebot).
4. **Power-side details are referenced, not specified here**: Asleep/Slumber/Plow/Shriek/Burrowed/Imbalanced/Ravenous/Flutter/Illusion/Reattach/Adaptable/SteamEruption/Stock/Infested/Surprise/Hatch/Sandpit/CrabRage/Surrounded/Skittish/PersonalHive/Territorial/EscapeArtist/Soar/HardToKill/Slippery/etc. They hold part of each boss/elite's behaviour; the Powers spec must cover their hooks (`AfterDamageReceived`, `AfterSideTurnEnd`, `AfterDeath`, `ShouldStopCombatFromEnding`, `ShouldAllowHitting`, `ShouldCreatureBeRemovedFromCombatAfterDeath`, `ShouldClearBlock`, `AfterBlockBroken`).
5. **AsleepPower / SlumberPower / HatchPower `Decrement`**: verified that `PowerCmd.ModifyAmount` removes a non-`AllowNegative` power once `Amount<=0` (`PowerModel.ShouldRemoveDueToAmount`, C/Models/PowerModel.cs:486), so `HasPower<AsleepPower>()`/`SlumberPower` turns false after the 3rd end-of-enemy-turn tick and the branch yields SLASH/ROLL_OUT; the finer power semantics belong to the Powers spec.
6. **Move execution targets**: monsters hit `CombatState.PlayerCreatures` (players only); how `DamageCmd.Attack(...).FromMonster(this)` picks targets / redirects to pets (Osty `DieForYou`) is in the Damage/Commands spec.
7. **`SpawnedThisTurn` edge**: a monster spawned during the *player* turn acts in the next enemy turn (flag cleared at the side switch) - verified from `SwitchSides`/`OnSideSwitch`; a monster spawned during the *enemy* turn does not (not in the `Enemies.ToList()` snapshot) - both rely on the snapshot taken at `ExecuteEnemyTurn` start (C/Combat/CombatManager.cs:1402-1430).
8. **Multiplayer** (HP scaling `hp * players * scaling(encounter, act)`, per-player ThieveryPower/Sandpit/Dampen targeting, `CurseOfKnowledge` per player, KnowledgeDemon heal x players, Siphon x players) is intentionally ignored; the single-player behaviour of those multipliers is x1.
9. `Rng.NextFloat` for the weighted branch uses a 53-bit double draw; the other `Rng` float helpers (`MegaRandom.NextFloat`, 24-bit) are not used by monsters - if the port reuses a generic `next_f32` helper make sure it is the double-based one.
10. `ThievingHopper` card theft (picks from Draw+Discard cards with `DeckVersion != null`, removes the deck copy, returns it as a reward on the Hopper's death), `GremlinMerc` gold theft/return and `TestSubject` kill counter mutate **run-level** state (deck, gold, progress). For a combat-only env define explicit behaviour (e.g. track but do not persist).
11. Intent damage preview depends on the *local player* (`LocalContext.GetMe`); in single player this is the only player. The preview uses `ValueProp.Move` through `Hook.ModifyDamage` with all modifier types.
12. `Creature.SetUniqueMonsterHpValue` runs on `CreateCreature`, i.e. **before** `CombatManager.AddCreature/SetUpForCombat`; HP of earlier enemies is therefore already final when later ones roll, but `AfterAddedToRoom` HP edits (Decimillipede, PunchConstruct event) happen later and are not visible to the uniqueness rule.

--------------------------------------------------------------------------------------------------------------------------------

## Appendix A. Exact ascension-scaled constants (auto-extracted from the decompiled sources)

Format: `Class: HP range A0 [| A8+ range]`; `A8:` lists non-HP constants keyed on `ToughEnemies`; `A9:` lists constants keyed on `DeadlyEnemies`; each entry is `Name base→ascended`.
`[inline X]` = a local variable inside a method. "HP x .. y" for TestSubject = `FirstFormHp` (see its A8 line). Min==Max means fixed HP (still one Niche draw).

- `Aeonglass`: HP 512 | A8+ 535; **A9**: EbbDamage 22→26, EyeLasersDamage 11→12, IncreasingIntensityBaseStrength 3→4, WitherAmount 1→2
- `Architect`: HP 9999
- `AssassinRubyRaider`: HP 18-23 | A8+ 19-24; **A9**: KillshotDamage 10→11
- `AxeRubyRaider`: HP 20-22 | A8+ 21-23; **A9**: SwingDamage 5→6, SwingBlock 5→6, BigSwingDamage 12→13
- `Axebot`: HP 70-78 | A8+ 76-86 + RespawnMaxHpBonus; **A9**: BootUpBlock 10→15, OneTwoDamage 10→11, BootUpStrGain 3→4, HammerUppercutDamage 14→18
- `BattleFriendV1`: HP 75
- `BattleFriendV2`: HP 150
- `BattleFriendV3`: HP 300
- `BigDummy`: HP 9999
- `BowlbugEgg`: HP 21-22 | A8+ 23-24; **A9**: BiteDamage 7→8, ProtectBlock 7→8
- `BowlbugNectar`: HP 35-38 | A8+ 36-39; **A9**: BuffStrengthGain 15→16
- `BowlbugRock`: HP 45-48 | A8+ 46-49; **A9**: HeadbuttDamage 15→16
- `BowlbugSilk`: HP 40-43 | A8+ 41-44; **A9**: ThrashDamage 4→5
- `BruteRubyRaider`: HP 30-33 | A8+ 31-34; **A9**: BeatDamage 7→8
- `BygoneEffigy`: HP 127 | A8+ 132; **A9**: SlashDamage 13→15
- `Byrdonis`: HP 81-84 | A8+ 90; **A9**: PeckDamage 3→4, PeckRepeat 3→3, SwoopDamage 17→19
- `Byrdpip`: HP 9999
- `CalcifiedCultist`: HP 38-41 | A8+ 39-42; **A9**: DarkStrikeDamage 9→11
- `CeremonialBeast`: HP 252 | A8+ 262; **A9**: PlowAmount 150→160, PlowDamage 18→20, StompDamage 15→17, CrushDamage 17→19, CrushStrength 3→4
- `Chomper`: HP 60-64 | A8+ 63-67; **A9**: ClampDamage 8→9
- `CorpseSlug`: HP 25-27 | A8+ 27-29; **A9**: GlompDamage 8→9, RavenousStr 4→5
- `CrossbowRubyRaider`: HP 18-21 | A8+ 19-22; **A9**: FireDamage 14→16
- `Crusher`: HP 209 | A8+ 219; **A9**: ThrashDamage 12→14, EnlargingStrikeDamage 4→4, BugStingDamage 6→7, AdaptStrengthGain 2→3, GuardedStrikeDamage 12→14
- `CubexConstruct`: HP 65 | A8+ 70; **A9**: BlastDamage 7→8, ExpelDamage 5→6
- `DampCultist`: HP 51-53 | A8+ 52-54; **A9**: DarkStrikeDamage 1→3, IncantationAmount 5→6
- `DecimillipedeSegment`: HP 40-46 | A8+ 46-52; **A9**: WritheDamage 5→6, ConstrictDamage 8→9, BulkDamage 6→7
- `DecimillipedeSegmentBack`: (inherits / no ascension data)
- `DecimillipedeSegmentFront`: (inherits / no ascension data)
- `DecimillipedeSegmentMiddle`: (inherits / no ascension data)
- `DeprecatedMonster`: HP 0
- `DevotedSculptor`: HP 162 | A8+ 172; **A9**: SavageDamage 12→15
- `Entomancer`: HP 145 | A8+ 165; **A9**: SpearMoveDamage 18→20, BeesRepeat 7→8, BeesDamage 3→3
- `Exoskeleton`: HP 24-28 | A8+ 26-30; **A9**: SkitterRepeats 3→4, MandiblesDamage 8→9
- `EyeWithTeeth`: HP 6
- `Fabricator`: HP 150 | A8+ 155; **A9**: FabricatingStrikeDamage 18→21, DisintegrateDamage 11→13
- `FakeMerchantMonster`: HP 165 | A8+ 175; **A9**: SwipeDamage 13→15, ThrowRelicDamage 9→10
- `FatGremlin`: HP 13-17 | A8+ 14-18
- `FlailKnight`: HP 101 | A8+ 108; **A9**: FlailDamage 9→10, RamDamage 15→17
- `Flyconid`: HP 47-49 | A8+ 51-53; **A9**: SmashDamage 11→12, SporeDamage 8→9
- `Fogmog`: HP 74 | A8+ 78; **A9**: SwipeDamage 8→9, HeadbuttDamage 14→16
- `FossilStalker`: HP 51-53 | A8+ 54-56; **A9**: TackleDamage 9→11, LatchDamage 12→14, LashDamage 3→4
- `FrogKnight`: HP 191 | A8+ 199; **A8**: PlatingAmount 15→19; **A9**: StrikeDownEvilDamage 21→23, TongueLashDamage 13→14, BeetleChargeDamage 35→40
- `FuzzyWurmCrawler`: HP 55-57 | A8+ 58-59; **A9**: AcidGoopDamage 4→6
- `GasBomb`: HP 7 | A8+ 8; **A9**: ExplodeDamage 8→9
- `GlobeHead`: HP 148 | A8+ 158; **A9**: ThunderStrikeDamage 6→7, ShockingSlapDamage 13→14, GalvanicBurstDamage 16→17, GalvanicPowerAmount 6→8
- `GremlinMerc`: HP 47-49 | A8+ 51-53; **A8**: GimmeDamage 7→8, DoubleSmashDamage 6→7, HeheDamage 8→9
- `Guardbot`: HP 16-20 | A8+ 17-21
- `HauntedShip`: HP 63 | A8+ 67; **A9**: SwipeDamage 13→14, StompDamage 4→5
- `HunterKiller`: HP 121 | A8+ 126; **A9**: BiteDamage 17→19, PunctureDamage 7→8
- `InfestedPrism`: HP 161 | A8+ 171; **A8**: PulsateBlock 20→22; **A9**: JabDamage 15→17, VitalSparkAmount 2→3, PulsateDamage 8→10, RadiateDamage 11→13, RadiateBlock 11→13, WhirlwindDamage 5→6
- `Inklet`: HP 11-17 | A8+ 12-18; **A9**: JabDamage 3→4, WhirlwindDamage 2→3, PiercingGazeDamage 10→11
- `KinFollower`: HP 58-59 | A8+ 62-63; **A9**: QuickSlashDamage 5→5, BoomerangDamage 2→2, DanceStrength 2→3
- `KinPriest`: HP 190 | A8+ 199; **A9**: OrbOfFrailtyDamage 8→9, OrbOfWeaknessDamage 8→9, BeamDamage 3→3, RitualStrength 2→3
- `KnowledgeDemon`: HP 379 | A8+ 399; **A9**: SlapDamage 17→18, PonderDamage 11→13, KnowledgeOverwhelmingDamage 8→9, PonderStrength 2→3
- `LagavulinMatriarch`: HP 222 | A8+ 233; **A8**: Slash2Block 12→14; **A9**: SlashDamage 19→21, Slash2Damage 12→14, DisembowelDamage 9→10
- `LeafSlimeM`: HP 32-35 | A8+ 33-36; **A9**: ClumpDamage 8→9
- `LeafSlimeS`: HP 11-15 | A8+ 12-16; **A9**: TackleDamage 3→4
- `LivingFog`: HP 80 | A8+ 82; **A9**: AdvancedGasDamage 8→9, BloatDamage 5→6, SuperGasBlastDamage 8→9
- `LivingShield`: HP 55 | A8+ 65; **A9**: SmashDamage 16→18
- `LouseProgenitor`: HP 134-136 | A8+ 138-141; **A8**: CurlBlock 14→18; **A9**: WebDamage 9→10, PounceDamage 14→16, GrowStrength 5→7
- `MagiKnight`: HP 82 | A8+ 89; **A8**: PowerShieldBlock 5→9; **A9**: PowerShieldDamage 6→7, SpearDamage 10→11, BombDamage 35→40
- `Mawler`: HP 72 | A8+ 76; **A9**: RipAndTearDamage 14→16, ClawDamage 4→5
- `MechaKnight`: HP 300 | A8+ 320; **A9**: ChargeDamage 25→30, FlamethrowerDamage 8→12, HeavyCleaveDamage 35→40
- `MockArtifactMonster`: HP 9999
- `MockAttackAndSummonMinionMonster`: HP 10
- `MockAttackMonster`: HP 9999
- `MockIntangibleMonster`: HP 9999
- `MockPlatingMonster`: HP 9999
- `MockReattachMonster`: HP 1
- `MultiAttackMoveMonster`: HP 999
- `MysteriousKnight`: (inherits / no ascension data)
- `Myte`: HP 61-67 | A8+ 64-69; **A9**: BiteDamage 13→15, SuckDamage 4→6, SuckStrength 2→3
- `Nibbit`: HP 42-46 | A8+ 44-48; **A8**: SliceBlock 5→6; **A9**: ButtDamage 12→13, SliceDamage 6→7, HissStrengthGain 2→3
- `Noisebot`: HP 18-23 | A8+ 19-24
- `OneHpMonster`: HP 1
- `Osty`: HP 1
- `Ovicopter`: HP 124-130 | A8+ 126-132; **A9**: SmashDamage 16→17, TenderizerDamage 7→8, NutritionalPasteStrengthAmount 3→4
- `OwlMagistrate`: HP 231 | A8+ 247; **A9**: VerdictDamage 33→36, ScrutinyDamage 16→17, PeckAssaultDamage 4→4
- `PaelsLegion`: HP 9999
- `Parafright`: HP 21; **A9**: SlamDamage 16→17
- `PhantasmalGardener`: HP 26-31 | A8+ 27-32; **A8**: SkittishAmount 6→7; **A9**: BiteDamage 5→5, LashDamage 7→7, FlailRepeat 3→3, EnlargeStr 2→3
- `PhrogParasite`: HP 61-64 | A8+ 66-68; **A9**: LashDamage 4→5
- `PunchConstruct`: HP 55 | A8+ 60; **A9**: StrongPunchDamage 14→16, FastPunchDamage 5→6
- `Queen`: HP 400 | A8+ 419; **A9**: OffWithYourHeadDamage 3→4, ExecutionDamage 15→18, [inline strengthAmount] 1→1
- `Rocket`: HP 199 | A8+ 209; **A9**: TargetingReticleDamage 3→4, PrecisionBeamDamage 18→20, LaserDamage 31→35, ChargeUpStrengthGain 2→3
- `ScrollOfBiting`: HP 30-37 | A8+ 33-39; **A9**: ChompDamage 14→16, ChewDamage 5→6
- `Seapunk`: HP 44-46 | A8+ 47-49; **A8**: BubbleBlock 7→8; **A9**: SeaKickDamage 11→13, BubbleStr 1→2
- `SewerClam`: HP 56 | A8+ 58; **A8**: [inline valueIfAscension] 8→9; **A9**: JetDamage 10→11
- `ShrinkerBeetle`: HP 38-40 | A8+ 40-42; **A9**: ChompDamage 7→8, StompDamage 13→14
- `SingleAttackMoveMonster`: HP 999
- `SkulkingColony`: HP 75 | A8+ 80; **A9**: InertiaDamage 9→11, ZoomDamage 14→16, PiercingStabsDamage 7→8, InertiaStrengthGain 2→4
- `SlimedBerserker`: HP 261 | A8+ 281; **A9**: PummelingDamage 4→5, SmotherDamage 30→33
- `SlitheringStrangler`: HP 53-55 | A8+ 54-56; **A9**: ThwackDamage 7→8, LashDamage 12→13
- `SludgeSpinner`: HP 37-39 | A8+ 41-42; **A9**: OilSprayDamage 8→9, SlamDamage 11→12, RageDamage 6→7
- `SlumberingBeetle`: HP 86 | A8+ 89; **A8**: PlatingAmount 15→18; **A9**: RolloutDamage 16→18
- `SnappingJaxfruit`: HP 31-33 | A8+ 34-36; **A9**: EnergyDamage 3→4
- `SneakyGremlin`: HP 10-14 | A8+ 11-15; **A9**: TackleDamage 9→10
- `SoulFysh`: HP 211 | A8+ 221; **A9**: DeGasDamage 16→18, ScreamDamage 13→15, GazeDamage 7→8
- `SoulNexus`: HP 234 | A8+ 254; **A9**: SoulBurnDamage 29→31, MaelstromDamage 6→7, MaelstromRepeat 4→4, DrainLifeDamage 18→19
- `SpectralKnight`: HP 93 | A8+ 97; **A9**: SoulSlashDamage 15→17, SoulFlameDamage 3→4
- `SpinyToad`: HP 116-119 | A8+ 121-124; **A9**: LashDamage 17→19, ExplosionDamage 23→25
- `Stabbot`: HP 18-23 | A8+ 19-24; **A9**: StabDamage 11→12
- `TenHpMonster`: HP 10
- `TerrorEel`: HP 140 | A8+ 150; **A8**: ShriekAmount 70→75; **A9**: CrashDamage 16→18, ThrashDamage 3→4
- `TestSubject`: HP FirstFormHp .. FirstFormHp; **A8**: FirstFormHp 100→111, SecondFormHp 200→212, ThirdFormHp 300→313; **A9**: EnrageAmount 2→3, BiteDamage 20→22, SkullBashDamage 14→16, MultiClawDamage 10→11, Phase3LacerateDamage 10→11, BurningGrowlBurnCount 3→5, BurningGrowlStrengthGain 2→3
- `TheAdversaryMkOne`: HP 100
- `TheAdversaryMkThree`: HP 300
- `TheAdversaryMkTwo`: HP 200
- `TheForgotten`: HP 106 | A8+ 111; **A9**: DebilitatingSmogDexStealAmount 2→2, [inline valueIfAscension] 13→15
- `TheInsatiable`: HP 321 | A8+ 341; **A9**: ThrashDamage 8→9, BiteDamage 28→31, SalivateStrength 2→3
- `TheLost`: HP 93 | A8+ 99; **A9**: EyeLasersDamage 4→5, DebilitatingSmogStrengthStealAmount 2→2
- `TheObscura`: HP 123 | A8+ 129; **A9**: PiercingGazeDamage 10→11, HardeningStrikeDamage 6→7, HardeningStrikeBlock 6→7
- `ThievingHopper`: HP 79 | A8+ 84; **A9**: TheftDamage 17→19, HatTrickDamage 21→23, NabDamage 14→16
- `Toadpole`: HP 21-25 | A8+ 22-26; **A9**: SpikeSpitDamage 3→4, WhirlDamage 7→8
- `TorchHeadAmalgam`: HP 199 | A8+ 211; **A9**: StrongTackleDamage 26→32, TackleDamage 18→22, WeakTackleDamage 14→16, SoulBeamDamage 8→8
- `ToughEgg`: HP 14-18 | A8+ 15-19; **A8**: HatchlingMinHp 19→20, HatchlingMaxHp 22→23; **A9**: NibbleDamage 4→5
- `TrackerRubyRaider`: HP 21-25 | A8+ 22-26; **A9**: HoundsDamage 1→1, HoundsRepeat 8→9
- `Tunneler`: HP 87 | A8+ 92; **A8**: BlockGain 32→37; **A9**: BiteDamage 13→15, BelowDamage 23→26
- `TurretOperator`: HP 41 | A8+ 51; **A9**: FireDamage 3→4
- `TwigSlimeM`: HP 26-28 | A8+ 27-29; **A9**: ClumpDamage 11→12
- `TwigSlimeS`: HP 7-11 | A8+ 8-12; **A9**: TackleDamage 4→5
- `TwoTailedRat`: HP 17-21 | A8+ 18-22; **A9**: ScratchDamage 8→9, DiseaseBiteDamage 6→7
- `Vantom`: HP 173 | A8+ 183; **A8**: SlipperyAmt 8→9; **A9**: InkBlotDamage 7→8, InkyLanceDamage 6→7, DismemberDamage 26→30
- `VineShambler`: HP 61 | A8+ 64; **A9**: GraspingVinesDamage 8→9, SwipeDamage 6→7, ChompDamage 16→18
- `WaterfallGiant`: HP 240 | A8+ 250; **A8**: SiphonHeal 10→15; **A9**: PressurizeAmount 15→20, StompDamage 15→16, RamDamage 10→11, PressureUpDamage 13→14, BasePressureGunDamage 20→23
- `Wriggler`: HP 17-21 | A8+ 18-22; **A9**: BiteDamage 6→7
- `Zapbot`: HP 18-23 | A8+ 19-24; **A9**: ZapDamage 14→15
