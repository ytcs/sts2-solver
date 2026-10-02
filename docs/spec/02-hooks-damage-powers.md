# 02 - Hook System, Damage Pipeline, Block/HP, Powers

Spec for a bit-exact Rust re-implementation of STS2 v0.111.0 combat (single-player only).
Source root (read-only): `decomp/MegaCrit/Sts2/Core/` (abbreviated `Core/`).
Citations are `file:line` relative to `Core/`. Line numbers refer to the decompiled files as shipped
in this repo. "Not verified" / "UNCERTAIN" flags mark things inferred rather than read.

Everything async/await, VFX, SFX, animation, waits (`Cmd.CustomScaledWait`), history/stat tracking,
multiplayer scaling, and `LocalContext.NetId` checks is ignored. In single-player `NetId` is always
present, so the `if (!netId.HasValue) return;` guards in `Hook.AfterDeath/BeforeSideTurnStart/...`
never trigger. `await` is "run to completion, in order" (see section 9 for the one pause-semantics
caveat).

---------------------------------------------------------------------------------------------------

## 0. Arithmetic conventions (read first)

* **All damage/block/power-amount math is C# `decimal`** (96-bit integer mantissa, scale 0-28,
  base-10, results rounded to 28 significant digits when a product overflows). Not float. Multipliers
  seen in the content: 1.5, 0.75, 0.5, 2, 0.7 (`(100-30)/100`), `1 + 0.1*n`, 0.3 -- exactly
  representable in decimal, NOT in binary. Chains of several fractional multipliers add decimal places
  (0.75 x 1.5 x 0.7 x 1.2 ...). To be bit-exact use a 96-bit/28-digit decimal type with
  System.Decimal semantics (`rust_decimal` matches); a custom fixed-point type needs scale >= 1e-18 and
  i128 range. Do not use f64 (e.g. 0.7 and 1.1 truncate differently).
* **Truncation is `(int)` cast = toward zero** (not floor). Negative values only occur for Strength/
  Dexterity style amounts and are cast the same way.
* **Where decimals become ints** (complete list, nothing else rounds):
  | Site | Rule |
  |---|---|
  | `Creature.DamageBlockInternal` Core/Entities/Creatures/Creature.cs:431-436 | block lost = `(int)min(Block, amount)`; returned "blocked" value stays fractional |
  | `Creature.LoseHpInternal` :446-458 | `num = (int)clamp(amount, 0, 999_999_999)`; kill flag uses the *un-truncated* `amount >= CurrentHp` |
  | `Creature.GainBlockInternal` :460-467 | `Block = (int)min(Block + amount, 999_999_999)` |
  | `Creature.LoseBlockInternal` :469-476 | `Block = (int)max(Block - amount, 0)` |
  | `Creature.SetCurrentHpInternal` :489-492 | `CurrentHp = (int)min(amount, MaxHp)` |
  | `Creature.SetMaxHpInternal` :494-502 | `MaxHp = min((int)amount, 999_999_999)`; `CurrentHp = min(CurrentHp, MaxHp)` |
  | `PowerModel.ApplyInternal` :572-581 | `SetAmount((int)amount)` (skipped entirely if amount == 0) |
  | `PowerCmd.ModifyAmount` Core/Commands/PowerCmd.cs:240 | `newAmount = Amount + (int)modifiedOffset` |
  | `PowerModel.SetAmount` :550-561 | clamp to +-999_999_999 |
  | `DamageResult` fields | `(int)blockedDamage` at CreatureCmd.cs:298/307; `UnblockedDamage/OverkillDamage` are ints |
  | `Hook.ModifyHpLost` change detection | `decimal.Truncate(before) != decimal.Truncate(after)` (Hook.cs:1735 etc.) |
  | `Hook.ModifyEnergyGain/ModifyGoldGained/ModifyHandDraw` change detection | `(int)a != (int)b` (Hook.cs:1622,1642,1700) |
  There is **no rounding inside `Hook.ModifyDamage` or `Hook.ModifyBlock`**; only a final
  `Math.Max(0m, x)`.
* Hook "modifier lists" (which models changed the value) gate side effects of the `AfterModifying*`
  hooks (Buffer decrement, Artifact decrement, relic flashes, Intangible flash...). The gating
  predicates differ per hook (`!= 0`, `!= 1`, truncated-differs, `!=`). They must be replicated
  exactly because they decide whether stateful hooks (Buffer, Artifact) fire.
* HP/Block/power Amount are `int` (i32); `MaxHp`/`CurrentHp` clamped >= 0 (setter throws on
  negative, so code paths never produce negatives).

---------------------------------------------------------------------------------------------------

## 1. Listener model: who receives hooks, and in what order

### 1.1 The three iterators

1. **`CombatState.IterateHookListeners()`** Core/Combat/CombatState.cs:411-494. The canonical list
   (call it `L_combat`).
2. **`RunState.IterateHookListeners(combatState?)`** Core/Runs/RunState.cs:545-598 (`L_run`).
3. **`Hook.IterateCombatHookListeners(combatState)`** Core/Hooks/Hook.cs:53-63 (the "guarded"
   iterator): yields nothing if `CombatManager.IsOverOrEnding && !IsStarting` **at the moment the
   dispatch begins** (checked once, not per listener); otherwise yields `L_combat`.

Each `Hook.*` dispatcher uses exactly one of them. In the table of section 2 the "Disp" column is:
* **G** = guarded `IterateCombatHookListeners` (silent no-op once combat is over/ending),
* **C** = unguarded `combatState.IterateHookListeners()` (fires even while combat is ending; the
  source comments explain each is part of the kill/death/combat-end sequence),
* **R** = `runState.IterateHookListeners(combatState)` = `L_run`.

#### `L_combat` construction (CombatState.cs:411-494)

```
list = []
for creature in allies ++ enemies:                    # allies first, then enemies, each in list order
    list += creature.Powers                           # in creature._powers order (= application order)
    if creature.Player == null:                       # a monster (incl. Osty / pets, which are Monster creatures)
        list += creature.Monster
    else:                                             # a player creature
        if !player.IsActiveForHooks: continue         # dead player: no relics/potions/orbs/cards (its powers were already added above, but are dropped by Contains)
        for relic in player.Relics:  if !relic.IsMelted: list += relic        # acquisition order
        for potion in player.PotionSlots: if potion != null: list += potion   # slot order
        if player.PlayerCombatState == null: continue
        list += player.PlayerCombatState.OrbQueue.Orbs                        # queue order
        for pile in [Hand, DrawPile, DiscardPile, ExhaustPile, PlayPile]:     # PlayerCombatState.AllPiles, PlayerCombatState.cs:70-78
            for card in pile.Cards:                                            # pile list order (draw pile index 0 = top, CardPile.cs:167)
                list += card
                if card.Affliction != null:   list += card.Affliction          # affliction BEFORE enchantment
                if card.Enchantment != null:  list += card.Enchantment
list += Modifiers ; list += BadgeModels ; list += MultiplayerScalingModel(if any)
# --- snapshot ends; lazily yield with liveness filter ---
for item in list: if Contains(item): yield item
for item in ModHelper.IterateAllCombatStateSubscribers(this): yield item   # mods only; ignore
```

Notes:
* The creature's **powers are added before the player's own relics/potions/orbs/cards** (a creature's
  powers come first). Because the player creature is `allies[0]` (and Osty, a pet Creature with
  `Player == null`, comes later in `allies`), player powers precede player relics, then potions,
  orbs, hand cards, draw cards, discard, exhaust, play pile; then Osty's powers + Osty's Monster
  model; then enemies in `enemies` order (slot order, `SortEnemiesBySlotName` CombatState.cs:496).
* `IsActiveForHooks` (Core/Entities/Players/Player.cs:112, :276, :442, :863): `= Creature.IsAlive`
  at init/revive, set `false` by `DeactivateHooks()` when the player dies (after `AfterDeath`).
  A dead player's **powers are still listed** in the snapshot but filtered by `Contains` (below).
* Monster `Contains` is `monster.Creature.CombatState != null`. A dead enemy that is still attached
  (not yet removed, or removal prevented e.g. by `ShouldCreatureBeRemovedFromCombatAfterDeath`
  returning false) keeps receiving hooks for its Monster model and any powers it still owns.
* `MonsterModel`s listed: all monsters (and Osty). Monster overrides are few
  (AfterDeath x10, BeforeDeath, AfterCurrentHpChanged, AfterDamageReceived, AfterCardGeneratedForCombat,
  AfterCardChangedPilesLate); enumerated in 2.5.
* `ShouldReceiveCombatHooks` (AbstractModel.cs:68; Card: `Pile?.IsCombatPile`, Enchantment: card's)
  is **never consulted by any iterator** (grep: only the property definitions exist). Cards in all
  five combat piles always receive every hook; a card's own override must check `Pile` itself
  (see Melancholy.cs:35-47, Guilty.cs:45-56).

#### `Contains(item)` liveness filter (CombatState.cs:550-601)

Evaluated **when each item is about to be yielded** (the underlying list was snapshotted when
enumeration started):

| Model | Alive iff |
|---|---|
| PowerModel | `power.Owner.CombatState != null && (owner.Player?.IsActiveForHooks ?? true)` -- **does NOT check the power is still in `Owner.Powers`** |
| RelicModel / PotionModel / CardModel / OrbModel | `!HasBeenRemovedFromState && Owner.IsActiveForHooks` (Orb: `!HasBeenRemovedFromState`) |
| Affliction / Enchantment | `HasCard && !Card.HasBeenRemovedFromState && Card.Owner.IsActiveForHooks` |
| MonsterModel | `Creature.CombatState != null` |
| Modifier / Badge / MultiplayerScaling | always |

Consequences a faithful sim must reproduce:
1. **A power removed during a dispatch pass still receives the remainder of that pass** (it is in
   the snapshot and the filter does not look at the creature's power list). A power removed *before*
   the pass begins is not in the snapshot.
2. **Models added during a pass are not visited by that pass** (snapshot). Each tier
   (Early / normal / Late) and each separate `foreach` in a dispatcher is a *new* pass with a
   *new* snapshot, so a model added by tier-1 *is* visited by tier-2.
3. A card that left play (removed from state) mid-pass is skipped; a card that merely moved piles
   is still visited (it is in the snapshot at its old position).
4. A creature that is removed from combat (`CombatState` set to null by `RemoveCreature(unattach: true)`)
   drops out of every subsequent snapshot step: both its Monster model and its powers fail `Contains`
   (powers check `Owner.CombatState != null`). Exception: in `KillWithout`, `state.RemoveCreature` is skipped
   while `monster.IsPerformingMove` (CreatureCmd.cs:557-560), so a monster that dies during its own move
   stays attached (and keeps listening) until that move completes.

#### `L_run` construction (RunState.cs:545-598)

```
for player in run.Players: if player.IsActiveForHooks:
    for card in player.Deck.Cards: list += card ; if card.Enchantment: list += card.Enchantment   # DECK copies
if childCombatState == null:                       # out of combat
    for player ...: relics (non-melted), Potions ; then Modifiers, BadgeModels, MultiplayerScalingModel
yield [filtered by RunState.Contains] ++ ModHelper.IterateAllRunStateSubscribers
if childCombatState != null: yield* childCombatState.IterateHookListeners()      # = L_combat, after the deck cards
```
So an **R-dispatched hook inside combat visits: deck-copy cards (+their enchantments) first, then all
of `L_combat`**. Deck cards are *different instances* from the combat clones
(`Player.PopulateCombatState` Core/Entities/Players/Player.cs:806-815 clones each deck card;
`CardModel.DeckVersion` links them). Relics/potions/modifiers are NOT in the run segment when a
combat state is supplied (they appear once, inside `L_combat`). RunState.Contains throws on
Power/Monster/Orb types, which is fine because those only appear via the combat segment.
Deck-card overrides of R-dispatched combat hooks: grep found **none** among Cards/Enchantments
(deck-version overrides exist only for out-of-combat hooks, e.g. Guilty.AfterCombatEnd guards on
`Pile.Type == Deck`; SpoilsMap map hooks). Sim can ignore deck-copy listening for combat unless
a card is later found overriding an R hook without a pile guard. UNCERTAIN only to the extent that
this grep covered `override` of: ModifyDamage*/ModifyHpLost*/Before/AfterDamageReceived/
AfterCurrentHpChanged/BeforeDeath/AfterDeath/ShouldDie/AfterModifyingDamageAmount/AfterCardChangedPiles/
BeforeCombatStart/AfterCombatEnd/AfterCombatVictory/Potion hooks. Of those only Guilty
(AfterCombatEnd), Melancholy (AfterDeath), SovereignBlade (AfterCardChangedPiles; filters
`card != this`) appear; Hoarder (modifier) overrides AfterCardChangedPiles.

### 1.2 Guarded vs. unguarded, and the dynamic "combat ending" predicate

`CombatManager.IsEnding` (Core/Combat/CombatManager.cs:~200, `IsCombatEnding` :417-436) is a
**live predicate, not a flag**:
```
is_combat_ending = in_progress && ( pending_loss
                   || ( !any(enemy in state.Enemies: enemy.IsAlive && enemy.IsPrimaryEnemy)
                        && !Hook.ShouldStopCombatFromEnding(state) ) )
IsOverOrEnding = IsEnding || !IsInProgress
```
It flips true the instant the last primary enemy's HP reaches 0 (HP only; the creature need not yet be
"killed"/removed). `IsPrimaryEnemy` = enemy-side and has no power with `OwnerIsSecondaryEnemy`
(Creature.cs:253-279). Effects that bail on `IsOverOrEnding`/`IsEnding`:
* every **G** hook dispatch (becomes a no-op if the check is true when the dispatch *starts*),
* `PowerCmd.Apply` / `ModifyAmount` (PowerCmd.cs:73, 107, 221) -> return null/0,
* `CreatureCmd.GainBlock` (CreatureCmd.cs:667) returns 0; `CreatureCmd.LoseBlock` :712 no-op,
* `CreatureCmd.Heal` of a non-player (:737),
* `AttackCommand.Execute` start (AttackCommand.cs:533) and per-hit "no valid targets" break.
`CheckWinCondition` (CombatManager.cs:1387-1400) is the only place combat actually ends (called at
safe points: after each enemy move :1426, after turn-end phases :1557/1576/1588, after
EndEnemyTurnInternal :1086, in StartTurn :827/:844). Teardown therefore happens after the current
card/move completes.

### 1.3 Dispatch shape (every dispatcher)

```
for model in <iterator>:            # snapshot semantics as in 1.1
    await model.Hook(args)
    model.InvokeExecutionFinished()   # event only; ignore
```
* **Notification hooks**: call every listener in order; no early exit.
* **Tiered hooks** (`X`, `XEarly`, `XLate`, `XVeryEarly`): *separate full passes*, in the order
  listed in the table; pass k+1 starts only after pass k finished for all listeners.
* Value-threading hooks pass the running value to the next listener (order matters when the
  operations do not commute; see 3.5 for the cases where they do not).

---------------------------------------------------------------------------------------------------

## 2. Hook catalogue

Legend. **Disp**: G/C/R as in 1.1 (corrected where the generator output was ambiguous). **Impl by**:
every hook is a `virtual` on `AbstractModel` (Core/Models/AbstractModel.cs), so any model class can
override it; the column lists who actually does in this version (counts from grep over `Models/`;
P=powers, Rl=relics, C=cards, E=enchantments, M=monsters, Po=potions, O=orbs, A=afflictions).
**Aggregation**: NOTIFY (no value), ADD (`v += h(v)` and record if `!= 0`), MUL (`v *= h(v)` and
record if `!= 1`), THREAD (`v = h(v)`), AND/OR (bool short-circuit), TRY (bool + out value; threaded),
MUT (void; mutates its argument). Hook.cs line = dispatcher.

### 2.1 Combat notification hooks (all return `Task`)

| Hook (Hook.cs line) | Disp | Params | Tiers (pass order) | Fired when / notes |
|---|---|---|---|---|
| BeforeAttack :80 | G | `AttackCommand cmd` | 1 | Once per `AttackCommand.Execute` before hit count is modified (AttackCommand.cs:549) and by `AttackContext.CreateAsync` |
| AfterAttack :92 | G | `PlayerChoiceContext, AttackCommand` | 1 | Once after all hits (AttackCommand.cs:672; AttackContext dispose). Vigor, Gigantification, PainfulStabs, Skittish, Suck |
| BeforeBlockGained :131 | G | `Creature, decimal amount, ValueProp, CardModel? src` | 1 | `CreatureCmd.GainBlock` start, with RAW amount (before modifiers) |
| AfterBlockGained :143 | G | same | 1 | After block applied, with the *modified* amount; fires even if modified amount is 0 (CreatureCmd.cs:696). Juggernaut, BeaconOfHope |
| AfterBlockBroken :107 | C | `PlayerChoiceContext, Creature target, Creature? breaker` | 1 | After a damage call finishes (per damage result with `WasBlockBroken`, CreatureCmd.cs:397-400) or `LoseBlock` taking block to 0 (:716-720). Burrowed |
| AfterBlockCleared :119 | G | `Creature` | 1 | `CombatManager.StartTurn` for every creature starting its side's turn, **unconditionally** (even if Barricade/Blur prevented the clear, and on player turn 1 where no clear happens) CombatManager.cs:762-770 |
| AfterPreventingBlockClear :1044 | G | `AbstractModel preventer, Creature` | 1 | Only the preventer is called (only if still in the listener set). Blur |
| BeforeCardAutoPlayed :155 | G | `CardModel, Creature? target, AutoPlayType` | 1 | Before an auto-play |
| BeforeCardPlayed :263 | G | `CardPlay` | 1 | Before a card resolves (22 powers, PenNib...) |
| AfterCardPlayed :278 | **C** | `PlayerChoiceContext, CardPlay` | `AfterCardPlayed`, then `AfterCardPlayedLate` | Unguarded: completes resolution of the killing card. 29 powers |
| AfterCardChangedPiles :167 | R | `CardModel, PileType oldPile, AbstractModel? clonedBy` | `AfterCardChangedPiles`, then `AfterCardChangedPilesLate` | New pile = `card.Pile` |
| AfterCardDiscarded :186 | G | `PlayerChoiceContext, CardModel` | 1 | |
| AfterCardDrawn :202 | G | `PlayerChoiceContext, CardModel, bool fromHandDraw` | `AfterCardDrawnEarly` (Hellraiser), then `AfterCardDrawn` | Guarded: silent once the last enemy died |
| AfterCardEnteredCombat :223 | G | `CardModel` | 1 | Card put into a combat pile |
| AfterCardGeneratedForCombat :251 | G | `CardModel, Player? creator` | 1 | Player-generated (not enemy-applied status) |
| AfterCardExhausted :237 | G | `PlayerChoiceContext, CardModel, bool causedByEthereal` | 1 | |
| AfterCombatEnd :328 | R | `CombatRoom` | 1 | `EndCombatInternal` (CombatManager.cs:1315) |
| AfterCombatVictory :352 | R | `CombatRoom` | `AfterCombatVictoryEarly`, then `AfterCombatVictory` | After `Player.AfterCombatEnd` (powers wiped, block zeroed) :1322-1326 |
| BeforeCombatStart :311 | R | none | `BeforeCombatStart`, `BeforeCombatStartLate` | CombatManager.cs:594 |
| AfterCreatureAddedToCombat :374 | C | `Creature` | 1 | `CreatureCmd.Add` (summon) after `AfterCreatureAdded` |
| AfterCurrentHpChanged :386 | R | `Creature, decimal delta` | 1 | After damage with `UnblockedDamage > 0` (delta = -unblocked, CreatureCmd.cs:401-404), in `Kill` (:533), `Heal` (raw requested amount, only if `amount > 0`, :803-806), `SetCurrentHp`. NOT fired by HP changes inside `SetMaxHp` clamp |
| BeforeDamageReceived :415 | R | `PlayerChoiceContext, Creature target, decimal amount, ValueProp, Creature? dealer, CardModel?` | 1 | After ModifyDamage, **before** block is applied; fires even if amount is 0 (Thorns lives here) |
| AfterDamageGiven :401 | **C** | `PlayerChoiceContext, Creature? dealer, DamageResult, ValueProp, Creature target, CardModel?` | 1 | Per damage *result*, even if 0 or fully blocked. 8 powers (Envenom, Imbalanced, SicEm...) |
| AfterDamageReceived :429 | R | `PlayerChoiceContext, Creature target, DamageResult, ValueProp, Creature? dealer, CardModel?` | `AfterDamageReceived`, `AfterDamageReceivedLate` | **Skipped for a result that killed the target** (`WasTargetKilled && IsDead`, CreatureCmd.cs:413-420) |
| BeforeDeath :450 | R | `Creature` | 1 | Fires even if death will be prevented (CreatureCmd.cs:535) |
| AfterDeath :462 | R | `PlayerChoiceContext, Creature, bool wasRemovalPrevented, float animLen` | 1 | Fired both when the creature really died (`false`) and when `ShouldDie` prevented it (`true`) |
| AfterPreventingDeath :1056 | R | `Creature` | 1 | Only the preventer, if still a listener. Heals (Fairy/Lizard Tail) |
| AfterDiedToDoom :496 | C | `IReadOnlyList<Creature>` | 1 | After Doom kills |
| AfterEnergyReset :515 | G | `Player` | `AfterEnergyReset`, `AfterEnergyResetLate` | In `SetupPlayerTurn` |
| AfterEnergySpent :532 | G | `CardModel, int` | 1 | |
| BeforeFlush :544 | G | `PlayerChoiceContext, Player` | `BeforeFlush`, `BeforeFlushLate` | Player-turn-end, hand flush |
| AfterFlush :572 | G | `PlayerChoiceContext, Player, flushedCards, retainedCards` | 1 | |
| AfterForge :586 | G | `decimal, Player forger, AbstractModel? src` | 1 | |
| BeforeHandDraw :600 | G | `Player, PlayerChoiceContext, ICombatState` | `BeforeHandDraw`, `BeforeHandDrawLate` | |
| AfterHandEmptied :623 | G | `PlayerChoiceContext, Player` | 1 | |
| AfterModifyingBlockAmount :661 | G | `decimal modified, CardModel?, CardPlay?` | 1 | Only for models in the `modifiers` list from `ModifyBlock` (iterates `L` and tests `modifiers.Contains`) |
| AfterModifyingCardPlayCount :676 | G | `CardModel` | 1 | same "only modifiers" pattern |
| AfterModifyingDamageAmount :706 | R | `CardModel?` | 1 | only models in `ModifyDamage` modifier list (Intangible/HardToKill/Slow flash...) |
| AfterModifyingEnergyGain :721 | G | `()` | 1 | only models whose `ModifyEnergyGain` changed the int value |
| AfterModifyingHandDraw :751 | G | `()` | 1 | only `ModifyHandDraw(+Late)` modifiers |
| AfterModifyingHpLostBeforeOsty :766 / AfterModifyingHpLostAfterOsty :781 | R | `()` | 1 each | only models recorded by `ModifyHpLost` for that phase; HardenedShell flash / Intangible flash / Buffer decrement / Tungsten flash |
| AfterModifyingOrbPassiveTriggerCount :796 | G | `(OrbModel)` | 1 | |
| AfterModifyingPowerAmountGiven :811 | G | `(PowerModel modifiedPower)` | 1 | only given-modifiers (SneckoSkull/UnsettlingLamp) |
| AfterModifyingPowerAmountReceived :826 | G | `(PowerModel modifiedPower)` | 1 | only received-modifiers; **Artifact decrements here** |
| AfterOrbChanneled :856 | G | `(ctx, Player, OrbModel)` | 1 | |
| AfterOrbEvoked :870 | G | `(ctx, OrbModel, IEnumerable<Creature> targets)` | 1 | |
| AfterOstyRevived :882 | G | `(Creature osty)` | 1 | |
| AfterPreventingDraw :1068 | G | `()` (called on the single `modifier` returned by `Hook.ShouldDraw`, only if it is still a listener) | 1 | NoDraw |
| AfterPlayerTurnStart :894 | G | `PlayerChoiceContext, Player` | `...Early`, `AfterPlayerTurnStart`, `...Late` | Inside `SetupPlayerTurn` (CombatManager.cs:923) |
| AfterAutoPrePlayPhaseEntered :940 | G | `HookPlayerChoiceContext, Player` | `...Early`, normal, `...Late` | |
| AfterAutoPostPlayPhaseEntered :922 | G | `HookPlayerChoiceContext, Player` | 1 | Start of player turn end (CombatManager.cs:1542) |
| BeforePowerAmountChanged :1020 | G | `PowerModel, decimal amount, Creature target, Creature? applier, CardModel?` | 1 | RAW amount (before Artifact/modifiers) |
| AfterPowerAmountChanged :1032 | G | `PlayerChoiceContext, PowerModel, decimal amount, Creature? applier, CardModel?` | 1 | Only if the *modified* amount != 0 (new-power path) / `(int)modifiedOffset != 0` (existing) |
| BeforePotionUsed :996 / AfterPotionUsed :1008 / AfterPotionProcured :984 / AfterPotionDiscarded :972 | R | `PotionModel, Creature? target` | 1 | |
| AfterShuffle :1142 | G | `PlayerChoiceContext, Player` | 1 | |
| BeforeSideTurnStart :1156 | G | `PlayerChoiceContext, CombatSide, IReadOnlyList<Creature> participants, ICombatState` | 1 | Right after `BeforeTurnStart` snapshots `AmountOnTurnStart` (CombatManager.cs:719-721) |
| AfterSideTurnStart :1175 | G | `CombatSide, participants, ICombatState` | `AfterSideTurnStart`, `AfterSideTurnStartLate` | After block clear and `SetupPlayerTurn` (CombatManager.cs:783). Poison, Plating, Blur... |
| BeforeSideTurnEnd :1244 | G | `PlayerChoiceContext, CombatSide, IEnumerable<Creature>` | `BeforeSideTurnEndVeryEarly`, `...Early`, `BeforeSideTurnEnd` | Player: CombatManager.cs:1556 (before DoTurnEnd + flush). Enemy: :1699 (after all monsters acted). Plating(Early block), Regen(Early heal), Doom(normal) |
| AfterSideTurnEnd :1279 | G | `PlayerChoiceContext, CombatSide, participants` | `AfterSideTurnEnd`, then (after awaiting all) `AfterSideTurnEndLate` | Player: after flush (:1779). Enemy: after `EndOfTurnCleanup` (:1704). All duration ticking lives here |
| AfterStarsGained :1192 / AfterStarsSpent :1204 | G | `(int amount, Player gainer/spender)` | 1 | |
| AfterSummon :1218 | G | `(ctx, Player summoner, decimal amount)` | 1 | |
| AfterTakingExtraTurn :1232 | G | `(Player)` | 1 | MP/Ambergris |
| AfterTargetingBlockedVfx, AfterAddToDeckPrevented, AfterModifyingCardPlayResultLocation | (no `Hook` wrapper) | | | Called directly (NTargetManager.cs:607 UI-only; CardPileCmd.cs:460; CardModel.cs:1885) |

Run/map/shop/reward hooks (R, out of combat scope; listed for completeness):
AfterActEntered, BeforeCardRemoved, BeforeCombatRewardOffered, AfterGoldGained, AfterItemPurchased,
AfterMapGenerated, AfterRestSiteHeal/Smith, AfterRewardTaken, BeforeRoomEntered, AfterRoomEntered,
AfterModifyingCardRewardOptions/GoldGained/Rewards, ModifyCardBeingAddedToDeck(+Late),
ModifyCardRewardAlternatives/CreationOptions(+Late)/UpgradeOdds, TryModifyCardRewardOptions(+Late),
ModifyGoldGained, ModifyExtraRestSiteHealText, ModifyGeneratedMap(+Late), ModifyMerchant*,
ModifyNextEvent, ModifyOddsIncreaseForUnrolledRoomType, ModifyRestSiteHealAmount, TryModifyRestSite*,
ModifyRewards(+Late), ModifyUnknownMapPointRoomTypes, ShouldAddToDeck, ShouldAllowAncient,
ShouldAllowMerchantCardRemoval, ShouldAllowSelectingMoreCardRewards, ShouldDisableRemainingRestSiteOptions,
ShouldGenerateTreasure, ShouldProceedToNextMapPoint, ShouldRefillMerchantEntry, ShouldForcePotionReward,
ShouldAllowFreeTravel. (Hook.cs:68-1128 and :1357-1500, :1634-1680, :1791-1861, :1944-2007,
:2069-2135, :2154-2180, :2262-2275, :2324-2335, :2401-2441, :2480-2501.)

### 2.2 Combat modify hooks (value-returning)

All combat modify hooks that take only `ICombatState` are **G**; the damage/HP-loss ones are **R**.

| Hook (Hook.cs) | Disp | Per-listener method (signature) | Aggregation | Result post-processing / "modifiers" list rule |
|---|---|---|---|---|
| ModifyDamage :1495 (+Internal :2519) | R | see 3.2 | enchantment add, enchant mul, then pass A ADD, pass B MUL, pass C CAP | `Math.Max(0, v)`; list = non-zero adders, non-1 multipliers, and lowering cappers |
| ModifyDamageAdditive | | `decimal ModifyDamageAdditive(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource, CardPlay? cardPlay)` default 0 | ADD | `amount` passed = running total |
| ModifyDamageMultiplicative | | same sig, default 1 | MUL | |
| ModifyDamageCap | | `decimal ModifyDamageCap(Creature? target, ValueProp, Creature? dealer, CardModel?, CardPlay?)` default `decimal.MaxValue` | MIN-with-ratchet | `if cap < best { best = cap; if v > cap { v = cap; record } }` |
| ModifyHpLost :1725 | R | `ModifyHpLostBeforeOsty / ...BeforeOstyLate / ...AfterOsty / ...AfterOstyLate (Creature target, decimal amount, ValueProp, Creature? dealer, CardModel?)` default `amount` | THREAD, 4 passes in that order (phases by flags) | record if `Truncate(before) != Truncate(after)` |
| ModifyUnblockedDamageTarget :2056 | **C** | `Creature ModifyUnblockedDamageTarget(Creature target, decimal amount, ValueProp, Creature? dealer)` | THREAD | Only DieForYouPower (Osty redirect) |
| ModifyBlock :1322 | G | `ModifyBlockAdditive(Creature target, decimal block, ValueProp, CardModel?, CardPlay?)`, `ModifyBlockMultiplicative(...)` | enchant add, enchant mul, pass ADD, pass MUL | `Math.Max(0, v)`; list rule as damage |
| ModifyAttackHitCount :1309 | G | `int ModifyAttackHitCount(AttackCommand, int hitCount)` | THREAD (int) | **No model in `Models/` overrides it** |
| ModifyCardPlayCount :1384 | G | `int ModifyCardPlayCount(CardModel, Creature? target, int playCount)` | THREAD (int) | record if changed |
| ModifyCardPlayResultLocation :1403 | G | `CardLocation (CardModel, bool isAutoPlay, ResourceInfo, CardLocation)` | THREAD | record if changed |
| ModifyEnergyCostInCombat :1582 | G | `TryModifyEnergyCostInCombat(CardModel, decimal original, out decimal modified)` then `...Late` | TRY, threaded (`modifiedCost` fed forward, even when the method returns false it must set out=original) | skipped if `original < 0` (X-cost etc.) |
| ModifyStarCost :2023 | G | `TryModifyStarCost(...)` | TRY | same `< 0` skip |
| ModifyKeywordsInCombat :1603 | **C** | `bool TryModifyKeywordsInCombat(CardModel, ISet<CardKeyword>)` | MUT | |
| ModifyEnergyGain :1614 | G | `decimal (Player, decimal)` | THREAD | record if `(int)` changed |
| ModifyMaxEnergy :1778 | G | `decimal (Player, decimal)` | THREAD | |
| ModifyHandDraw :1692 | G | `decimal ModifyHandDraw(Player, decimal)` then `ModifyHandDrawLate` | THREAD x2 passes | `(int)`-changed |
| ModifyOrbPassiveTriggerCount :1863 | G | `int ModifyOrbPassiveTriggerCounts(OrbModel, int)` | THREAD | |
| ModifyOrbValue :1882 | G | `decimal ModifyOrbValue(OrbModel, decimal)` (Focus) | THREAD | |
| ModifyPowerAmountGiven :1896 | G | `ModifyPowerAmountGivenAdditive(PowerModel, Creature giver, decimal, Creature? target, CardModel?)`, `...Multiplicative(...)` | pass ADD, pass MUL | (no floor here; see 6.3) |
| ModifyPowerAmountReceived :1925 | G | `bool TryModifyPowerAmountReceived(PowerModel power, Creature target, decimal amount, Creature? applier, out decimal modified)` | TRY threaded; each `true` adds to modifiers | Artifact, RuinedHelmet |
| ModifyShuffleOrder :2012 | G (exempt during IsStarting) | `void (Player, List<CardModel>, bool isInitialShuffle)` | MUT | |
| ModifySummonAmount :2040 | G | `decimal (Player, decimal, AbstractModel? src)` | THREAD | |
| ModifyXValue :2079 | G | `int (CardModel, int)` | THREAD | |

### 2.3 Combat "Should" hooks (bool)

| Hook (Hook.cs) | Disp | Method | Aggregation | Notes |
|---|---|---|---|---|
| ShouldAllowHitting :2139 | G | `bool (Creature)` default true | AND, short-circuit | Used by `Creature.IsHittable` / `CanReceivePowers` (Creature.cs:286-323). NB under guard: **if combat is ending every creature is "hittable" (loop yields nothing)** |
| ShouldAllowTargeting :2184 | G | `bool (Creature)` | AND, returns preventer | UI/targeting |
| ShouldAfflict :2109 | G | `bool (CardModel, AfflictionModel)` | AND | |
| ShouldClearBlock :2201 | G | `bool (Creature)` | AND, returns preventer | Barricade, Blur, Burrowed |
| ShouldDie :2237 | R | `ShouldDie(Creature)` pass 1, then `ShouldDieLate(Creature)` pass 2 | AND, first `false` wins -> that model is the `preventer` | Fairy in a Bottle (ShouldDie), Lizard Tail (ShouldDieLate), mock powers |
| ShouldCreatureBeRemovedFromCombatAfterDeath :2222 | C | `bool (Creature)` | AND | |
| ShouldPowerBeRemovedOnDeath :2503 | C | `bool (PowerModel)` | AND | Illusion |
| ShouldDraw :2277 | G | `bool (Player, bool fromHandDraw)` | AND, returns modifier | NoDraw |
| ShouldEtherealTrigger :2294 / ShouldFlush :2309 | G | `bool` | AND | RetainHand, WellLaidPlans |
| ShouldGainStars :2339 | G | `bool (decimal, Player)` | AND | |
| ShouldPayExcessEnergyCostWithStars :2354 | G | `bool (Player)` default false | OR | |
| ShouldPlay :2369 | G | `bool (CardModel, AutoPlayType)` | AND, returns preventer | |
| ShouldPlayerResetEnergy :2386 | G | `bool (Player)` | AND | |
| ShouldProcurePotion :2416 | R | `bool (PotionModel, Player)` | AND | |
| ShouldStopCombatFromEnding :2450 | C | `bool ()` default false | OR | Adaptable, Infested, SteamEruption, Stock, Surprise (5 powers) |
| ShouldTakeExtraTurn :2465 | G | `bool (Player)` default false | OR | Ambergris |

### 2.4 Non-`Hook` virtuals on the base classes

* **PowerModel** (Core/Models/PowerModel.cs): `Type` (Buff/Debuff/None, abstract), `StackType`
  (None/Counter/Single, abstract), `InstanceType` (None default / Instanced / InstancedPerApplier),
  `AllowNegative` (default false), `IsVisibleInternal` (default true; only AmbergrisPower is false),
  `ShouldScaleInMultiplayer` (MP only; ignore), `OwnerIsSecondaryEnemy`, `DisplayAmount`,
  `BeforeApplied(Creature target, decimal amount, Creature? applier, CardModel?)` :616,
  `AfterApplied(Creature? applier, CardModel?)` :627, `AfterRemoved(Creature oldOwner)` :636,
  `ShouldPowerBeRemovedAfterOwnerDeath()` :645 (default true), `ShouldOwnerDeathTriggerFatal()` :654
  (default true; false for Minion/Reattach; queried by Feed/HandOfGreed/TheHunt/BookRepairKnife),
  `GetScaledAmountForMultiplayer` (MP), `InitInternalData()` (per-instance state), runtime fields
  `Amount`, `AmountOnTurnStart`, `SkipNextDurationTick`, `Owner`, `Applier`, `Target` (almost always
  null: the only writers are SwipePower.cs:71 and SurprisePower.cs:31), `DynamicVars`.
* **CardModel** hook-ish virtuals: `OnPlay(choiceContext, cardPlay)` (CardModel.cs:1644),
  `OnTurnEndInHand` :1678, `OnUpgrade`, `AfterCreated`, `AfterTransformedFrom/To`,
  `GetResultLocationForCardPlay` :2084, `CanonicalKeywords/Tags/Vars`. Cards also override ~25
  AbstractModel hooks (AfterCardEnteredCombat x5, AfterCardDrawn x3, ShouldPlay x2, BeforeHandDraw x2,
  AfterCardPlayed(+Late), AfterCardExhausted x2, AfterAutoPostPlayPhaseEntered x2, BeforeSideTurnEnd,
  BeforeCardPlayed, AfterDeath, AfterCombatEnd, AfterAttack, AfterCardChangedPiles ...).
* **EnchantmentModel** (Core/Models/EnchantmentModel.cs:406-456):
  `decimal EnchantBlockAdditive(decimal)` (0), `EnchantBlockMultiplicative(decimal)` (1),
  `decimal EnchantDamageAdditive(decimal original, ValueProp)` (0),
  `EnchantDamageMultiplicative(decimal original, ValueProp)` (1), `int EnchantPlayCount(int)`,
  `OnPlay`, `OnEnchant`, `RecalculateValues`, `CanEnchant*`. These are *not* AbstractModel hooks;
  they are called directly by `Hook.ModifyDamage/ModifyBlock` on `cardSource.Enchantment` **before**
  the listener loops. Enchantments additionally override AfterCardPlayed x3, BeforeFlush,
  AfterCardDrawn, ModifyShuffleOrder, AfterAutoPrePlayPhaseEntered.
* **RelicModel**: `AfterObtained`, `AfterRemoved`, `IsAllowed*`, `IsMelted`, `IsUsedUp/Status`; everything
  else is AbstractModel hooks (84 relics override AfterObtained; 40 AfterCombatEnd; 29 AfterSideTurnStart;
  27 AfterCardPlayed; 21 BeforeSideTurnStart; 19 BeforeCombatStart; 12 ModifyMaxEnergy; ...).
* **PotionModel**: `OnUse(choiceContext, Creature? target)` :361; `OnUseWrapper` :297 =
  `RemoveBeforeUse(); BeforePotionUsed; OnUse; AfterPotionUsed` (potion leaves the slot **first**).
  Fairy in a Bottle is `PotionUsage.Automatic` and acts through `ShouldDie` + `AfterPreventingDeath`.
* **MonsterModel**: `AfterAddedToRoom`, `BeforeRemovedFromRoom`, `GenerateMoveStateMachine`,
  `OnDieToDoom`; overrides of AbstractModel hooks listed in 1.1.
* **OrbModel**: `Passive(choiceContext, Creature? target)`, `Evoke(choiceContext)`,
  `BeforeTurnEndOrbTrigger`, `AfterTurnStartOrbTrigger` (called by `OrbQueue`, not by `Hook`).
* **AfflictionModel**: `OnPlay`, `AfterApplied`, `BeforeRemoved`, `CanAfflict*`.
* **ModifierModel**: no combat virtuals beyond AbstractModel hooks (Hoarder overrides
  AfterCardChangedPiles, ShouldAllowMerchantCardRemoval).

### 2.5 Which models override damage/HP-pipeline hooks (this version, from grep)

* ModifyDamageAdditive: relics FakeStrikeDummy, MiniatureCannon, MysticLighter, StrikeDummy;
  powers Calcify, Accuracy, Leadership, OneForAll, PhantomBlades, Vigor, Strength, Tainted.
* ModifyDamageMultiplicative: relics PenNib, VitruvianMinion, UndyingSigil; powers Covered, Colossus,
  Intercept, Conqueror, Flanking, Lethality, Knockdown, Guarded, Flutter, Shrink, Soar, Hang,
  Gigantification, Slow, Tracking, Surrounded, Vulnerable, Tank, Weak, DoubleDamage (+Mock).
* ModifyDamageCap: Intangible, HardToKill.
* ModifyBlockAdditive: Dexterity, Fasten. ModifyBlockMultiplicative: Frail, NoBlock, Unmovable,
  Shadowmeld, MultiplayerScalingModel (MP), relics VitruvianMinion, PaelsLegion, Vambrace.
* ModifyHpLostBeforeOstyLate: HardenedShell. ModifyHpLostAfterOsty: Intangible, Slippery, relics
  TungstenRod, BeatingRemnant. ModifyHpLostAfterOstyLate: Buffer, relic TheBoot.
  (`ModifyHpLostBeforeOsty` itself: no overrides.)
* ModifyUnblockedDamageTarget: DieForYou. ShouldDie: potion FairyInABottle (+mock powers);
  ShouldDieLate: relic LizardTail. TryModifyPowerAmountReceived: Artifact, relic RuinedHelmet.
  ModifyPowerAmountGiven: relics SneckoSkull (add), UnsettlingLamp (mul).
* BeforeDamageReceived: Thorns only. AfterDamageReceived: 14 powers, 6 relics, 1 monster.

---------------------------------------------------------------------------------------------------

## 3. Damage pipeline (card base value -> HP lost)

### 3.1 From a card to `CreatureCmd.Damage`

A damaging card calls `DamageCmd.Attack(DynamicVars.Damage.BaseValue).FromCard(card, cardPlay)
.Targeting(t | TargetingAllOpponents | TargetingRandomOpponents).WithHitCount(n).Execute(ctx)`
(e.g. Models/Cards/StrikeIronclad.cs). The base value passed is the **raw dynamic-var BaseValue**
(upgrade deltas already applied to the var); the enchantment is **not** pre-applied (it is applied
inside `Hook.ModifyDamage`, so do not apply it twice). `AttackCommand` default props =
`ValueProp.Move` (`.Unpowered()` ORs in Unpowered); props are *not* taken from `DamageVar.Props`
in the execute path (AttackCommand.cs:127, :349-353, :669). Calculated damage
(`DamageCmd.Attack(CalculatedDamageVar)`) evaluates `base + extra * multiplierCalc(card, target)`
**per hit** with `target = singleTarget` (null for multi-target) (CalculatedVar.cs; AttackCommand.cs:669),
as a decimal; multiplier calcs return decimals. `DamageCmd.cs` claims "special logic to ensure that
intermediate values are floored at the appropriate time" for `CalculatedDamageVar`, but none is visible in
`CalculatedVar.Calculate` or `AttackCommand`; each card's multiplier lambda must be read (open question 12).

`AttackCommand.Execute` (AttackCommand.cs:526-673), game-logic only:
```
if combat.IsOverOrEnding(and live) or attacker is null/dead: return
await Hook.BeforeAttack(cmd)                                  # G
hits = Hook.ModifyAttackHitCount(cmd, _hitCount)              # G, THREAD int (no overrides in Models)
i = 0
while i < hits:                                               # loop bound is the *modified* count, evaluated once
    if attacker.IsDead: break
    valid = possibleTargets.where(IsAlive)                    # recomputed EVERY hit; IsAlive, NOT IsHittable
    if valid is empty: break
        # possibleTargets: single -> [target]; all-opponents: attacker.Side's opposite creatures
        #   (monster attackers: state.PlayerCreatures only -- Osty/pets are not targeted by AoE)
    if random:  (no dup option: filter out creatures already hit in this command's results)
                target = Rng.CombatTargets.NextItem(valid)    # consumes the CombatTargets RNG stream once per hit
    else:       target = (valid.Count == 1) ? valid[0] : null
    results += await CreatureCmd.Damage(ctx, targets = target? [target] : valid,
                       amount = damagePerHit | calc.Calculate(target), props, dealer=attacker, cardSource, cardPlay)
    i++
CombatManager.History.CreatureAttacked(...)                   # ignore
await Hook.AfterAttack(ctx, cmd)                              # G
```
Notes: an AoE hit passes the whole `valid` list into **one** `CreatureCmd.Damage` call
(per-target pipeline in sequence, post-hooks batched; see 3.3). A single-target attack whose target
died mid-attack stops when `valid` becomes empty. `AttackContext` (AttackContext.cs) is the
manual-loop variant used by some cards (BeforeAttack at creation, `AddHit`, AfterAttack at dispose).

### 3.2 `Hook.ModifyDamage(runState, combatState, target, dealer, damage, props, cardSource, cardPlay, type=All, previewMode=None)` (Hook.cs:1495-1577, :2519-2566)

```
v = damage
if cardSource?.Enchantment != null:
    v += enchantment.EnchantDamageAdditive(v, props)        # skipped if Additive flag not set (always set for real damage)
    v *= enchantment.EnchantDamageMultiplicative(v, props)
# (the MultiCreatureTargeting branch :1510-1571 only runs when target == null && preview mode; UI preview only)
v = ModifyDamageInternal(...)
return max(0, v)                                            # no rounding

ModifyDamageInternal:
  A) for model in L_run: d = model.ModifyDamageAdditive(target, v, props, dealer, cardSource, cardPlay); v += d; if d != 0: mods += model
  M) for model in L_run: m = model.ModifyDamageMultiplicative(target, v, props, dealer, cardSource, cardPlay); v *= m; if m != 1: mods += model
  C) best = decimal.Max
     for model in L_run: c = model.ModifyDamageCap(target, props, dealer, cardSource, cardPlay)
         if c < best: best = c; if v > c: v = c; mods += model
```
Stage order is therefore: **enchant add -> enchant mul -> all additive -> all multiplicative -> caps -> floor 0**.
Per stage the listeners are in `L_run` order (deck-copy cards first, then `L_combat` order of 1.1).
Additive and multiplicative stages are commutative with respect to the final value
(exact decimal), so listener order only matters for the *modifiers list*, for stateful models,
and for the amount arguments they see (almost never used). Caps: the min is order independent.

Real `CreatureCmd.Damage` always passes `ModifyDamageHookType.All` and `CardPreviewMode.None`
(CreatureCmd.cs:282).

Typical contributors (all gated by the checks shown; "powered" = `props.Move && !props.Unpowered`
i.e. `ValuePropExtensions.IsPoweredAttack`, ValuePropExtensions.cs:5-12):
* **Strength** (additive): `Owner == dealer && powered ? +Amount : 0` (StrengthPower.cs). Amount may
  be negative. Osty's attacks do not get the owner's Strength.
* **Vigor** (additive): `+Amount` when `Owner == dealer && powered` with an AttackCommand-binding
  quirk (6.6 / section 10).
* **Vulnerable** (mult): `target == Owner && powered` -> `1.5` (DynamicVar `DamageIncrease`), modified
  by dealer's PaperPhrog relic / Cruelty power (own `ModifyVulnerableMultiplier`) and target's
  Debilitate (VulnerablePower.cs). **Weak** (mult): `dealer == Owner && powered` -> `0.75`, modified
  by *target's* PaperKrane relic and dealer's Debilitate (WeakPower.cs).
* **Pen Nib** (mult x2): powered, `cardSource != null`, dealer is owner/Osty, per-card flag
  (PenNib.cs:106-134). Stateless inside the hook (state set in BeforeCardPlayed, cleared in
  AfterCardPlayed), so AoE does not consume it per target.
* **Intangible** (cap 1 for `target == Owner`, any props), **HardToKill** (cap Amount).
* **Shrink** (x0.7 for dealer == Owner), **Slow** (x(1 + 0.1*SlowAmount) on target == Owner),
  **Tank** (x1.5 target), **DoubleDamage** (x2 if dealer == Owner or in Owner.Pets, powered,
  `cardSource != null`), plus the rest listed in 2.5 (not individually read: Accuracy, Leadership,
  OneForAll, PhantomBlades, Tainted, Colossus, Conqueror, Covered, Flanking, Flutter, Gigantification,
  Guarded, Hang, Intercept, Knockdown, Lethality, Soar, Surrounded, Tracking, relics Miniature Cannon,
  MysticLighter, StrikeDummy, VitruvianMinion, UndyingSigil; **flag: read each before implementing**).
* Because `Unpowered` damage (Thorns, Constrict, orbs, relic damage) fails the "powered" checks, it
  gets no Strength/Vulnerable/Weak/Slow/Tank/Shrink, but it **still gets the Intangible/HardToKill
  caps** (their checks look only at `target == Owner`). Poison/HP-loss (`Unblockable|Unpowered`,
  dealer null) likewise.

### 3.3 `CreatureCmd.Damage` (Core/Commands/CreatureCmd.cs:258-433)

Signature family: `Damage(choiceContext, targets, amount (decimal), props, dealer?, cardSource?, cardPlay?)`;
all overloads funnel here (:99-243). Pseudocode (game-logic only):

```
if targets == null: return []
if dealer != null && dealer.IsDead: return [empty DamageResult per target]          # dead dealer => no-op
results = []
for originalTarget in targets:                                   # AoE: sequential per target
    if originalTarget.IsDead: continue
    modified = Hook.ModifyDamage(..., originalTarget, dealer, amount, props, cardSource, cardPlay, All, None, out mods)   # 3.2
    await Hook.AfterModifyingDamageAmount(runState, combatState, cardSource, mods)   # R, only models in mods
    await Hook.BeforeDamageReceived(ctx, runState, combatState, originalTarget, modified, props, dealer, cardSource)  # R  (Thorns here)
    blockOwner = originalTarget.PetOwner?.Creature ?? originalTarget   # PET QUIRK: Osty is damaged against its OWNER's block
    blocked   = blockOwner.DamageBlockInternal(modified, props)         # 4.2: 0 if Unblockable, else min(Block, modified); Block -= (int)blocked
    unblocked = Hook.ModifyHpLost(.., originalTarget, max(modified - blocked, 0), props, dealer, cardSource, phases=BeforeOsty, out mods)
    await Hook.AfterModifyingHpLostBeforeOsty(.., mods)
    hpTarget  = (combatState == null) ? originalTarget : Hook.ModifyUnblockedDamageTarget(combatState, originalTarget, unblocked, props, dealer)   # C, THREAD
    unblocked = Hook.ModifyHpLost(.., hpTarget, unblocked, props, dealer, cardSource, phases=AfterOsty, out mods)
    await Hook.AfterModifyingHpLostAfterOsty(.., mods)
    res = hpTarget.LoseHpInternal(unblocked, props)                     # 5.1  -> DamageResult{Unblocked,Overkill,WasTargetKilled}
    wasBlockBroken = originalTarget.Block <= 0 && blocked > 0           # reads originalTarget's block (== blockOwner's unless pet)
    wasFullyBlocked = !props.Unblockable && (blocked > 0 || originalTarget.Block > 0) && (int)unblocked == 0
    damageResults = [res]
    if originalTarget == hpTarget:
        res.BlockedDamage = (int)blocked; res.WasBlockBroken = wasBlockBroken; res.WasFullyBlocked = wasFullyBlocked
    else:                                                               # redirected (Osty took the hit)
        overkillToOriginal = Hook.ModifyHpLost(.., originalTarget, res.OverkillDamage, props, .., phases=AfterOsty, out mods)
        await Hook.AfterModifyingHpLostAfterOsty(.., mods)
        r2 = overkillToOriginal > 0 ? originalTarget.LoseHpInternal(overkillToOriginal, props) : new DamageResult(originalTarget, props)
        r2.BlockedDamage = (int)blocked; r2.WasBlockBroken = ...; r2.WasFullyBlocked = ...
        damageResults += r2
    (VFX/history only in the per-result loop :312-390)
    results += damageResults

# ---- after ALL targets resolved ----
killed = []
for r in results:
    t = r.Receiver
    if r.WasBlockBroken:           await Hook.AfterBlockBroken(t.CombatState, ctx, t, dealer)      # C
    if r.UnblockedDamage > 0:      await Hook.AfterCurrentHpChanged(.., t, -r.UnblockedDamage)      # R
    if combatState != null:        await Hook.AfterDamageGiven(ctx, combatState, dealer, r, props, t, cardSource)   # C  (always, even 0/blocked)
    if !r.WasTargetKilled || !t.IsDead:  await Hook.AfterDamageReceived(ctx, .., t, r, props, dealer, cardSource)  # R, tiers normal then Late
    else: killed += t
await Kill(killed)                                                                            # 5.3; AFTER all post-hooks
return results
```
Key properties:
* **Everything before the post-hook loop runs for all targets first** (AoE: all targets lose HP
  before any `AfterDamageGiven/Received` fires). `AfterDamageReceived` is **not** fired for results
  that killed the target; those creatures are `Kill`ed afterwards, in `results` order.
* The retaliation from Thorns (`BeforeDamageReceived`, 6.7) is a nested complete `CreatureCmd.Damage`.
* `wasFullyBlocked` with `(int)unblocked == 0`: a fractional unblocked 0.x counts as fully blocked.
* `AfterDamageGiven`/`AfterBlockBroken` are dispatched with `combatState.IterateHookListeners()`
  directly (unguarded).
* The `killed` check `WasTargetKilled && IsDead` is evaluated when the loop reaches the result; an
  earlier listener in the same loop could have healed it (edge).

### 3.4 The HP-lost phases in detail (Hook.ModifyHpLost, Hook.cs:1725-1773)

```
BeforeOsty phase:   pass1 ModifyHpLostBeforeOsty   (no overrides)
                    pass2 ModifyHpLostBeforeOstyLate  (HardenedShell)
   -> hpTarget redirect (ModifyUnblockedDamageTarget: DieForYou)
AfterOsty phase:    pass3 ModifyHpLostAfterOsty       (Intangible, Slippery, TungstenRod, BeatingRemnant, + order of 1.1)
                    pass4 ModifyHpLostAfterOstyLate   (Buffer, TheBoot)
```
Each pass is `v = h(target, v, props, dealer, cardSource)` in listener order; a listener is recorded
iff `Truncate(v_before) != Truncate(v_after)`, and only recorded listeners get the matching
`AfterModifyingHpLost*` hook (flash, Buffer decrement). Note `ModifyHpLost` is **invoked even when
the unblocked amount is 0** (e.g. fully blocked): listeners see 0. Examples of the exact semantics:
* Intangible: `target==Owner && amount >= 1 -> 1` else unchanged (fractional <1 untouched).
* Slippery: same clamp to 1; its decrement happens in `AfterDamageReceived` when `UnblockedDamage >= 1`.
* Tungsten Rod: `max(0, amount - 1)` for `target == Owner.Creature` (applies to amount 0 too, no change).
* Beating Remnant: `min(amount, 20 - damageReceivedThisTurn)`; tally updated in AfterDamageReceived.
* HardenedShell (BeforeOstyLate): `amount==0 ? amount : min(amount, Amount - damageReceivedThisTurn)`,
  tally reset in `BeforeSideTurnStart`.
* Buffer (AfterOstyLate): `target==Owner -> 0` (for any amount). Because the "changed" test uses
  truncation, Buffer is consumed only when `Truncate(amount) >= 1`; an amount like 0.5 -> 0 is
  *not* recorded (no decrement).
* The Boot (AfterOstyLate): dealer is the relic owner or their Osty, target is NOT the owner,
  powered, `1 <= amount < 5 -> 5`.
* **Intangible interplay**: Intangible's cap (3.2) already limits the damage to 1 *before* block, so the
  unblocked amount entering the HP-loss passes is <= 1 and the AfterOsty clamp is a no-op except on
  an Osty redirect (`target` is then Osty; the clamp checks `target == Owner`). With Buffer:
  10 damage, Intangible, Buffer, no block: cap -> 1; AfterOsty Intangible 1 -> 1; AfterOstyLate Buffer
  1 -> 0 (truncation changed => recorded => Buffer decremented). With 1 block: unblocked 0, Buffer not
  consumed.
* **Order-dependence worked example (HP-loss passes)**: Tungsten Rod (`max(0, a - 1)`) and Beating
  Remnant (`min(a, 20 - receivedThisTurn)`) are both player relics in the same AfterOsty pass, visited in
  relic acquisition order. 25 unblocked HP lost: Remnant first -> `min(25,20)=20`, then Rod -> 19; Rod
  first -> 24, then Remnant -> 20. (Powers always precede relics, so a power in this pass such as Slippery
  runs before both.)

### 3.5 Where listener ORDER is observable (non-commutative cases)

1. HP-loss phases (3.4): `min(x, c)` vs `x-1` floors (Tungsten Rod vs Beating Remnant), clamp-to-1 on
   an Osty redirect, Hardened Shell `min`, do not commute.
2. Damage cap vs. everything: cap is last, fully separated from add/mul (no order issue).
3. Stateful hooks: Thorns (BeforeDamageReceived) retaliation order across multiple thorns owners;
   Artifact; Buffer; Slippery; HardenedShell tallies.
4. Death prevention: `ShouldDie` AND short-circuits at the first `false`; pass 1 beats pass 2, so
   **Fairy in a Bottle (ShouldDie, potion) always beats Lizard Tail (ShouldDieLate)**; among
   same-pass preventers, first in `L` order wins (relics before potions on the same player).
5. Power application `TryModifyPowerAmountReceived` (threaded amount; Artifact sets 0 and returns true,
   later listeners still run and Artifact-like listeners could "re-block" a 0).
6. Anything that spends a shared resource (Pen Nib flag, card play count modifiers).

### 3.6 Worked examples (exact)

* Strike (6) + Strength 3, target Vulnerable, dealer Weak: `((6+3) * 1.5) * 0.75 = 10.125`; no
  rounding yet; Block 5 absorbs `min(5, 10.125)=5`, `Block -= 5`; unblocked `5.125`; HP lost
  `(int)5.125 = 5`.
* Same with block 11: blocked `10.125`, **`Block -= (int)10.125 = 10` -> Block 1 left**; unblocked
  `max(10.125-10.125, 0)=0`; `wasFullyBlocked` (blocked > 0 and `(int)0 == 0`).
* Block 10, damage 7.5: Block drops by `(int)7.5=7` -> 3 left (not 2.5/2).
* Intangible target, Strike 6: cap 1 in ModifyDamage (before block), then if Block>=1 it absorbs the 1;
  otherwise AfterOsty clamp keeps 1.

---------------------------------------------------------------------------------------------------

## 4. Block

### 4.1 Gaining block: `CreatureCmd.GainBlock(creature, amount (BlockVar or decimal), props, cardPlay?, fast)` (CreatureCmd.cs:665-698)
```
if CombatManager.IsOverOrEnding: return 0           # dynamic, see 1.2
if creature.IsDead: return 0
await Hook.BeforeBlockGained(state, creature, amount(raw), props, cardPlay?.Card)           # G
v = Hook.ModifyBlock(state, creature, amount, props, card, cardPlay, out mods)               # below
v = max(v, 0)
await Hook.AfterModifyingBlockAmount(state, v, card, cardPlay, mods)                          # only mods
if v > 0: creature.GainBlockInternal(v)  # Block = (int)min(Block + v, 999_999_999)          (truncation of v!)
await Hook.AfterBlockGained(state, creature, v, props, card)                                  # fires even if v == 0
return v
```
`Hook.ModifyBlock` (Hook.cs:1322-1352): enchant add, enchant mul (`cardSource.Enchantment`), then pass
ADD (`ModifyBlockAdditive`: Dexterity, Fasten), then pass MUL (`ModifyBlockMultiplicative`: Frail 0.75,
NoBlock, Unmovable, Shadowmeld, relics), `max(0, v)`. No rounding until `GainBlockInternal` truncates
the fractional result. So 5 block + Dex 0... Frail: `5 * 0.75 = 3.75 -> 3`. Dexterity
(`DexterityPower.cs`): `cardSource != null ? cardSource.Owner.Creature == Owner : target == Owner`, and
`IsPoweredCardOrMonsterMoveBlock` (= Move && !Unpowered). Frail: `target == Owner && powered`.
Block props (`BlockProps`): card=Move, cardUnpowered=Unpowered|Move, monsterMove=Move,
nonCardUnpowered=Unpowered (Plating, relics) so those do not get Dexterity/Frail.
`CardModel` previews call `ModifyBlock`/`ModifyDamage` too, but gameplay paths do not.

### 4.2 Absorbing damage: `Creature.DamageBlockInternal(amount, props)` (Creature.cs:431-436)
`blocked = props.Unblockable ? 0 : min(Block, amount)` (decimal), `Block -= (int)blocked`, returns the
**decimal** `blocked`. Block never goes below 0 (setter throws on negative; `min` guarantees).

### 4.3 Losing block directly: `CreatureCmd.LoseBlock(ctx, target, amount, remover)` (:710-722)
No-op if combat over/ending, target dead, or amount <= 0. `LoseBlockInternal`; if `block > 0 && Block <= 0`
-> `AfterBlockBroken(target, remover)` (dispatcher is C, i.e. unguarded; but the `LoseBlock` itself is a
no-op once combat is over/ending).

### 4.4 Clearing block
`Creature.AfterTurnStart(side)` (Creature.cs:686-697) -> `ClearBlock()` :723-733:
`if Hook.ShouldClearBlock(state, creature, out preventer): Block = 0 else AfterPreventingBlockClear(preventer)`.
It is called in `CombatManager.StartTurn` for every creature on the side whose turn is starting
(:753-761), i.e. **a creature's block expires at the START of its own side's next turn** (player block
persists through the enemy turn; enemy block is cleared at the start of the enemy turn).
Exception: player's first turn (`PlayerCombatState.TurnNumber == 1`) skips the clear (:688-695).
`Player.AfterCombatEnd` sets block to 0 and removes all powers (Player.cs:~855-860).
`Hook.AfterBlockCleared` is then fired unconditionally for each creature (:762-770).

### 4.5 Order inside StartTurn (CombatManager.cs:688-851), relevant to ticks
```
creatures_starting = (side's creatures)            # extra-turn handling ignored (single player)
for c in creatures_starting: c.BeforeTurnStart(side)           # power.AmountOnTurnStart = power.Amount for all its powers (Creature.cs:678-684)
Hook.BeforeSideTurnStart(side, creatures_starting)             # G (HardenedShell/BeatingRemnant reset, Plating round-1 block)
(player side: each enemy.PrepareForNextTurn -> roll move; not in this spec)
for c in creatures_starting: await c.AfterTurnStart(side)      # block clear
for c in creatures_starting: Hook.AfterBlockCleared(c)
(player side) SetupPlayerTurn per player: ShouldPlayerResetEnergy/AfterEnergyReset(+Late), BeforeHandDraw(+Late),
              ModifyHandDraw/AfterModifyingHandDraw, draw, AfterPlayerTurnStart(Early/normal/Late)   # :877-925
Hook.AfterSideTurnStart(side, creatures_starting)              # Poison, Plating, Blur, Reflect... (+Late pass)
(player side) orbs AfterTurnStart; AfterAutoPrePlayPhaseEntered(Early/normal/Late); CheckWinCondition
(enemy side) enemies act in `Enemies` order; CheckWinCondition after each; EndEnemyTurn
```
`AmountOnTurnStart` (used by DrawCardsNextTurn, SummonNextTurn, HelloWorld) is the amount at
`BeforeTurnStart`, so powers applied after that point in the same turn start do not count.

---------------------------------------------------------------------------------------------------

## 5. HP, healing, death

### 5.1 `Creature.LoseHpInternal(amount, props)` (Creature.cs:446-458)
```
killed   = CurrentHp > 0 && amount >= (decimal)CurrentHp          # un-truncated compare
before   = CurrentHp
n        = (int)clamp(amount, 0, 999_999_999)                      # truncation
CurrentHp = max(CurrentHp - n, 0)
return DamageResult{ Unblocked = before - CurrentHp,
                     WasTargetKilled = killed,
                     Overkill = killed ? max(n - before, 0) : 0 }
```
Note `killed` can be true with `n < CurrentHp`? No: `amount >= CurrentHp` and `n = trunc(amount) >= CurrentHp`
since CurrentHp is an integer. Fine. `props` only recorded. HP can never go below 0.

### 5.2 Heal / set HP / max HP
* `Heal(creature, amount)` (CreatureCmd.cs:735-807): no-op for non-players when `IsEnding`; `HealInternal`
  -> `SetCurrentHpInternal(CurrentHp + amount)` = `(int)min(CurrentHp + amount, MaxHp)` (truncation of
  fractional heals, e.g. Fairy: `max(MaxHp*0.3, 1)`: 75 -> 22.5 -> 22; Lizard Tail `max(1, MaxHp*0.5)`).
  **Healing a dead creature revives it** (`HealInternal` :478-487: if `isDead && !IsDead` ->
  `Player?.ActivateHooks()`, `Revived` event). `AfterCurrentHpChanged(creature, amount)` fires only if
  `amount > 0` and a combat state exists, with the **requested** amount not the actual healed.
  There is no "modify heal" hook in combat (`ModifyRestSiteHealAmount` is rest-site only).
* `SetCurrentHp` (:814-831): `SetCurrentHpInternal`; if value changed -> `AfterCurrentHpChanged(delta)`; if
  `IsDead` afterwards -> `Kill(creature)`.
* `GainMaxHp` (:838-851): `SetMaxHp(MaxHp + amount)`, then `Heal(creature, MaxHp delta)`.
  `LoseMaxHp(ctx, creature, amount, isFromCard)` (:863-880): `newMax = MaxHp - amount`; if
  `newMax < CurrentHp` runs a **full `Damage` call** for `CurrentHp - newMax` with props
  `Unblockable|Unpowered(|Move if isFromCard)`, dealer null, no card -> hooks apply (Intangible cap 1,
  Buffer, Tungsten...). Then `SetMaxHp(max(1, newMax))`.
* `SetMaxHp(creature, amount)` (:891-901): `SetMaxHpInternal(max(0, amount))`; `MaxHp <= 0` => `Kill`.
  Raising MaxHp does not raise CurrentHp by itself; lowering clamps CurrentHp (no hook).
* Monster HP at spawn: `SetUniqueMonsterHpValue` (Creature.cs:372-384) picks a value in
  `[MinInitialHp, MaxInitialHp]` not already used by another living enemy on the side (uses
  `RunState.Rng.Niche`); out of scope here but it consumes RNG (CombatState.cs:241).

### 5.3 Death: `CreatureCmd.Kill` / `KillWithoutCheckingWinCondition` (CreatureCmd.cs:445-603)
```
Kill(creatures, force=false):
    for c in creatures: KillWithout(c, force)
    if run over (all players dead): LoseCombat   # all players dead => pending loss
    else if combat in progress: for dead player creatures on player side during player turn: EndTurn

KillWithout(c, force, recursion=0):
    if c has no combat state (and not player) or not live: return
    hp = c.CurrentHp
    if hp > 0:  c.LoseHpInternal(hp, Unblockable|Unpowered); Hook.AfterCurrentHpChanged(c, -hp)     # direct kill: NO damage hooks
    Hook.BeforeDeath(c)                                                                                # R, even if prevented
    if force || c.MaxHp <= 0 || Hook.ShouldDie(c, out preventer):                                       # R; pass1 ShouldDie, pass2 ShouldDieLate
        remove = state != null && Hook.ShouldCreatureBeRemovedFromCombatAfterDeath(state, c)           # C, AND
        Hook.AfterDeath(c, wasRemovalPrevented=false)                                                   # R
        teammates = alive teammates of c
        if remove && c.Side==Enemy && c in state.Enemies:
            CombatManager.RemoveCreature(c); if !monster.IsPerformingMove: state.RemoveCreature(c)     # unattach: CombatState = null
        isPrimary = c.IsPrimaryEnemy                       # computed BEFORE its powers are stripped
        removedPowers = c.RemoveAllPowersAfterDeath()       # keeps p if !p.ShouldPowerBeRemovedAfterOwnerDeath() || !Hook.ShouldPowerBeRemovedOnDeath(p)
        for p in removedPowers: await p.AfterRemoved(c)     # NOT PowerCmd.Remove: no amount hooks
        if c.Side==Enemy:  if isPrimary && teammates.nonEmpty && all(teammates are secondary): Kill(teammates)   # minions die with the last primary
        elif c.IsPlayer:   clear orb queue; if Osty alive: Kill(Osty, force); player.DeactivateHooks(); CombatManager.HandlePlayerDeath
    else:                                                  # death prevented
        if recursion >= 10: throw
        Hook.AfterDeath(c, wasRemovalPrevented=true); Hook.AfterPreventingDeath(preventer, c)      # preventer heals
        if c.IsDead: KillWithout(c, force, recursion+1)
```
Notes:
* Dead creatures stay in `state.Enemies` until removed (`ShouldCreatureBeRemovedFromCombatAfterDeath`;
  DieForYou on Osty keeps it, returns false for `creature == Owner`), so `IsAlive` is the liveness test
  everywhere (HP > 0). Dead-but-attached creatures stay in `L_combat` through their Monster model and
  retained powers.
* Player death removes the player from hooks only after `AfterDeath` (`DeactivateHooks`), so
  AfterDeath hooks still see player relics.
* Doom's kill (`DoomPower.DoomKill` DoomPower.cs:40-53) calls `Kill` directly: Intangible/Buffer are
  bypassed, but `ShouldDie` preventers (Fairy, Lizard Tail) still apply.
* `Escape` (:609-633): `RemoveAllPowersInternalExcept()` (no `AfterRemoved` callbacks), remove from
  CombatManager, `CreatureEscaped` (kept in `EscapedCreatures`, removed from `Enemies`).
* `CreatureCmd.Add` (:53-82): `AddCreature` -> `CombatManager.AddCreature` (SetUpForCombat, slot sort)
  -> `AfterCreatureAdded` (AfterAddedToRoom; if player side's turn, `RollMove`) -> if current side is not
  Enemy and creature is monster: `PrepareForNextTurn(rollNewMove: false)` -> `Hook.AfterCreatureAddedToCombat`.
* `Stun(creature, nextMoveId)` (:922-956): monster-only; replaces current move with a `STUNNED` MoveState
  (follow-up = given id or the last logged state id), `MustPerformOnceBeforeTransitioning = true`
  (Creature.StunInternal :525-545). Move-machine details belong to the monster spec.
* `Creature.Reset` / `Player.AfterCombatEnd`: `RemoveAllPowersInternalExcept()` (no hooks), Block=0.

---------------------------------------------------------------------------------------------------

## 6. Powers

### 6.1 Data model (PowerModel.cs)
* Canonical instance from `ModelDb.Power<T>()`; applied powers are mutable clones
  (`ToMutable()` :563-570). Fields: `Amount` (int), `AmountOnTurnStart`, `SkipNextDurationTick`,
  `Owner`, `Applier`, `Target`, `DynamicVars`, per-instance `InternalData`.
* `Type`: Buff/Debuff (abstract). `GetTypeForAmount(a)` (:468-479): a Counter+AllowNegative power with
  `a < 0` is a **Debuff**; a non-AllowNegative Debuff with `a < 0` is a **Buff**; otherwise `Type`.
* `StackType` (None/Counter/Single) is **display/classification only**: `PowerCmd` never reads it.
  A Single power (Barricade, Corruption, ...) re-applied still adds to `Amount`; only `GetTypeForAmount`
  uses `StackType == Counter` (with `AllowNegative`) and `InvokePowerModified` for UI events.
  47 powers declare Single, 217 Counter (rest None/dynamic, e.g. Shrink flips with `Amount < 0`).
* `InstanceType` (stacking key, PowerCmd.cs:169-178):
  `Instanced` -> never stacks (always a new instance; ~dozens, e.g. Automation, Covered, Guarded, MagicBomb,
  Sandpit, TheBomb); `InstancedPerApplier` -> stacks into an existing instance with the same `Id` **and**
  `Applier == applier` (Oblivion, Strangle); `None` (default) -> one instance per `Id`
  (`target.GetPower(id)` = first match). `Creature.ApplyPowerInternal` throws if a `None` power would be
  duplicated by type (Creature.cs:606-618).
* **Order**: `Creature._powers` is a `List` in application order; `Remove` deletes in place, a later
  re-application appends at the end. **This order is the hook order for that creature's powers** (1.1).
* `ShouldRemoveDueToAmount()` (:486-497): `AllowNegative ? Amount == 0 : Amount <= 0`.
* `IsVisible` -> `Target == null || IsMe(Target) || Target.IsEnemy` then `IsVisibleInternal`; since
  `Target` is almost always null, `IsVisible == IsVisibleInternal` (only AmbergrisPower is invisible).
  Artifact only blocks *visible* debuffs.

### 6.2 `PowerCmd.Apply<T>(ctx, targets, amount, applier, cardSource, silent)` (PowerCmd.cs:36-163)
Per target, sequentially:
```
Apply<T>(target, amount, applier, card):
    if CombatManager.IsEnding: return null
    if !target.CanReceivePowers: return null              # = target.CombatState != null && Hook.ShouldAllowHitting(state, target); NOT an IsDead check
    canonical = ModelDb.Power<T>()
    existing  = FindExistingInstanceForStacking(canonical, target, applier)         # 6.1
    if existing == null:
        power = canonical.ToMutable(); Apply(power, target, amount, applier, card)   # new-instance path
    else if ModifyAmount(existing, amount, applier, card, silent) == 0: power = null  # resulting amount 0 (removed) OR combat ending
    return power as T                                    # NB: new-instance path returns the object even if nothing was attached (amount zeroed)

Apply(power, target, amount, applier, card, silent):            # new-instance path
    if IsEnding || amount == 0 || !target.CanReceivePowers: return
    if (existing = FindExistingInstanceForStacking(power...)) != null: ModifyAmount(existing, amount, applier, card); return
    power.Applier = applier
    Hook.BeforePowerAmountChanged(power, amount /*RAW*/, target, applier, card)             # G, every listener
    v = amount
    if applier != null && state.ContainsCreature(applier): v = Hook.ModifyPowerAmountGiven(state, power, applier, v, target, card, out givenMods)   # ADD pass then MUL pass
    v = Hook.ModifyPowerAmountReceived(state, power, target, v, applier, out recvMods)      # TRY-threaded (Artifact...)
    (multiplayer scaling skipped in single-player)
    await power.BeforeApplied(target, v, applier, card)                                      # TemporaryStrength applies the Strength here, VoidForm
    if target.CanReceivePowers:                                                              # re-checked
        power.ApplyInternal(target, v, silent)      # if v != 0: Owner=target; Amount=(int)v; target._powers.Add(power)   (v == 0 -> NOT attached)
        if target.Side == Player && power.Type == Debuff: power.SkipNextDurationTick = true   # only on this new-instance path
        if givenMods != null: Hook.AfterModifyingPowerAmountGiven(givenMods, power)
        Hook.AfterModifyingPowerAmountReceived(recvMods, power)                              # Artifact decrements HERE, even when v == 0
        if v != 0: await power.AfterApplied(applier, card); Hook.AfterPowerAmountChanged(power, v, applier, card)   # G
```
Because `AfterApplied` runs before `Hook.AfterPowerAmountChanged`, the new power is already in the
creature's list/snapshot for that dispatch (it hears about its own application).

`ModifyAmount(ctx, power, offset, applier, card, silent)` (:219-275) is the stacking/decrement path
(also `Decrement(power)` = `ModifyAmount(-1, null, null)`):
```
if IsEnding: return 0
owner = power.Owner; if owner.CombatState == null: return 0
Hook.BeforePowerAmountChanged(power, offset, owner, applier, card)
v = offset
if applier != null && state.ContainsCreature(applier): v = Hook.ModifyPowerAmountGiven(power, applier, v, owner, card)
v = Hook.ModifyPowerAmountReceived(power, owner, v, applier)       # NB: also applied to negative offsets/decrements
newAmount = power.Amount + (int)v ; power.SetAmount(newAmount)       # clamp +-999,999,999
Hook.AfterModifyingPowerAmountGiven/Received
if (int)v != 0: Hook.AfterPowerAmountChanged(power, v, applier, card)
if power.ShouldRemoveDueToAmount(): PowerCmd.Remove(power)           # RemoveInternal (list removal) then power.AfterRemoved(owner)
return newAmount
```
`SkipNextDurationTick` is **not** set when stacking onto an existing power.
`PowerCmd.Remove` (:291-299): `RemoveInternal()` (fires `Removed`, removes from `_powers`) then
`power.AfterRemoved(owner)`; no `Before/AfterPowerAmountChanged` hooks, no amount change.
`TickDownDuration` (:194-204): `if SkipNextDurationTick { SkipNextDurationTick = false } else Decrement(power)`.

### 6.3 Modifier hooks on power amounts
* `ModifyPowerAmountGiven`: additive pass (SneckoSkull) then multiplicative pass (UnsettlingLamp); result is
  a decimal and is truncated at `(int)` in `ApplyInternal`/`ModifyAmount`. Only if `applier` is still in
  combat.
* `ModifyPowerAmountReceived`: **Artifact** (ArtifactPower.cs): matches if `target == Owner`,
  `canonicalPower.GetTypeForAmount(currentAmount) == Debuff`, and `power.IsVisible`; sets amount to 0 and
  returns true (so it is recorded as a modifier); `AfterModifyingPowerAmountReceived` -> `PowerCmd.Decrement(Artifact)`.
  Consequences: (a) it blocks *negative Strength/Dexterity/Focus changes* (AllowNegative Counter with a < 0)
  and applications of debuffs, but **not** decrements of ordinary debuffs (non-AllowNegative debuff with
  a<0 is a Buff, so Weak ticking -1 passes through); (b) it consumes one charge per blocked application, even
  if an earlier listener already zeroed the amount (it re-tests `GetTypeForAmount(0)` => still Debuff);
  (c) `Apply<T>` still returns a non-null object and `BeforeApplied` still runs when blocked.
  RuinedHelmet is the other `TryModify` (not read; flag).

### 6.4 Duration / tick-down timing

All timed expiry is driven by hooks; there is no generic "turn end tick".
Key facts (side = `CombatSide` of the turn that just ended; `participants` = creatures that took that turn):

| Power | Hook | Condition | Action |
|---|---|---|---|
| Vulnerable, Weak, Frail | AfterSideTurnEnd | `side == Enemy` (any owner: enemy-owned and player-owned both tick at end of the ENEMY turn) | `TickDownDuration` (skip-once flag honoured) |
| Intangible, NoBlock, Colossus | AfterSideTurnEnd | `side == Enemy` | `Decrement` (no skip flag) |
| DoubleDamage, Conqueror, Debilitate, RetainHand, Hatch, Shrink(not infinite), Asleep, Slumber | AfterSideTurnEnd | `participants.Contains(Owner)` (end of the OWNER's side turn) | `Decrement` (Asleep/Slumber then wake) |
| Burst, Duplication, Rage, Flanking, Knockdown, NoDraw, NoEnergyGain, BorrowedTime, SicEm, Strangle, Shadowmeld, Tangled, Gravity, Ringing, ... | AfterSideTurnEnd | owner's side ends | `Remove` |
| Covered, Intercept, Underworld, Tainted | AfterSideTurnEnd | `side == Enemy` | `Remove` |
| Oblivion | AfterSideTurnEnd | `side == Player` | `Remove` |
| Concoct, FlameBarrier | AfterSideTurnEnd | `Owner.Side != side` | `Remove` |
| Temporary Strength/Dexterity/Focus | AfterSideTurnEnd | owner's side ends | `Remove` self, then apply opposite sign |
| Ritual | AfterSideTurnEnd | owner's side ends | Strength +Amount; skipped once if `WasJustAppliedByEnemy` |
| Poison | AfterSideTurnStart | owner's side starts | `for i in 0..min(Amount, 1 + sum(opponents' Accelerant)): Damage(Amount, Unblockable|Unpowered, dealer=null); if owner alive: Decrement` (PoisonPower.cs:55-81) |
| Doom | BeforeSideTurnEnd(side != Player) and AfterSideTurnEnd(side != Enemy); only the FIRST doomed creature on the owner's side runs `DoomKill` for all doomed creatures of that side | doomed = `Owner.CurrentHp <= Amount` (DoomPower.cs:144-147); skipped if combat over/ending or owner dead | `DoomKill` (Kill each, then AfterDiedToDoom) |
| Regen | BeforeSideTurnEndEarly | owner's side ends | `Heal(Amount); Decrement` |
| Plating | AfterSideTurnStart (decrement; enemies: `Decrement var`), BeforeSideTurnEndEarly (gain Amount block, Unpowered) | owner side | see PlatingPower.cs:41-83 (first-round conditions) |
| Blur | ShouldClearBlock false; AfterSideTurnStart | owner side starts | `Decrement` |
| Barricade | ShouldClearBlock false for owner | permanent | |

`SkipNextDurationTick` is set only by `PowerCmd.Apply` new-instance path when
`target.Side == Player && power.Type == Debuff` (PowerCmd.cs:148-151). Net effect: a debuff placed on
the *player* skips the very next end-of-enemy-turn tick (so it lasts through the player's next turn),
whether applied by an enemy or by the player (self-inflicted Weak etc. also skip one tick). A debuff
placed on an enemy ticks at the end of the same round's enemy turn (so Vulnerable 1 applied by a card
is removed at the end of that round's enemy turn; Vulnerable 2 -> 1). Stacking onto an existing
instance never sets the flag.

`AmountOnTurnStart`: snapshot at `BeforeTurnStart` (4.5).

### 6.5 Power application targets and ordering inside AoE
`PowerCmd.Apply<T>(targets: IEnumerable<Creature>)` iterates targets in the given order calling the
single-target path; each target runs the whole hook sequence before the next.

### 6.6 Vigor (verify-and-replicate quirk, VigorPower.cs)
* `BeforeAttack(cmd)`: if `cmd.Attacker == Owner`, powered, `commandToModify == null`, and the cmd's
  source is a card (or none) -> `commandToModify = cmd; amountWhenAttackStarted = Amount`.
* `ModifyDamageAdditive`: `Owner == dealer && powered`; if `commandToModify != null && cardSource != null &&
  cardSource != commandToModify.ModelSource -> 0`; `commandToModify.Attacker != dealer -> 0`; else `+Amount`.
* `AfterAttack(cmd)`: `if cmd == commandToModify: ModifyAmount(-amountWhenAttackStarted)` -> normally
  reaches 0 and the power is removed. **`commandToModify` is never cleared.** If Vigor remains after
  the attack (gained mid-attack), the stale command makes later damage from a *different card* get `+0`
  and never consume. Damage outside any `AttackCommand` (relics/potions with powered props and
  `cardSource == null`) gets the full bonus without consuming it when `commandToModify == null`.
  Replicate verbatim; per-power instance state.

### 6.7 Thorns (ThornsPower.cs:17-24)
`BeforeDamageReceived`: `target == Owner && dealer != null && (props.IsPoweredAttack() || cardSource is Omnislice)`
-> `CreatureCmd.Damage(ctx, dealer, Amount, Unpowered|SkipHurtAnim, dealer=Owner, null, null)`. Fires
before block, once per hit per target, even if the hit will be fully blocked or 0. The nested
call runs the full pipeline with `dealer = Thorns owner` (so Intangible/HardToKill on the original
dealer cap it, Strength does not apply).

---------------------------------------------------------------------------------------------------

## 7. Death/combat-end flow summary and when hooks stop

1. Last primary enemy reaches HP 0 -> `IsEnding` becomes true immediately; remaining G hooks of the
   current effect become silent; the in-flight `CreatureCmd.Damage` still runs its C/R post hooks
   (AfterBlockBroken, AfterDamageGiven, AfterDamageReceived (not for the dead), AfterCurrentHpChanged,
   Kill -> BeforeDeath/ShouldDie/AfterDeath). Card resolution continues (`AfterCardPlayed` is C),
   but further card effects that call `GainBlock`/`PowerCmd`/guarded hooks do nothing.
2. `CheckWinCondition` at the next safe point -> `EndCombatInternal`: revive dead players with 1 HP,
   `AfterCombatEnd` (R), `Player.AfterCombatEnd` (powers wiped via `RemoveAllPowersInternalExcept` with no
   callbacks, orbs/pile cleanup, block 0), `AfterCombatVictory` (+Early) (CombatManager.cs:1297-1360).
3. Player death -> `LoseCombat` sets `PendingLoss`; `IsEnding` true; processed at `CheckWinCondition`.

---------------------------------------------------------------------------------------------------

## 8. Subtle things a faithful simulator must replicate (checklist)

1. **Listener order**: allies then enemies; per creature powers-first (apply order), then Monster (or
   relics -> potions -> orbs -> Hand -> Draw -> Discard -> Exhaust -> Play piles, each card followed by
   affliction then enchantment). R-hooks prepend deck-copy cards.
2. **Snapshot semantics**: removed-mid-pass powers still get the rest of the pass; models added mid-pass
   do not; each Early/normal/Late tier is a fresh snapshot.
3. **Tier structure**: BeforeSideTurnEnd has 3 tiers (VeryEarly, Early, normal); AfterSideTurnEnd 2;
   AfterPlayerTurnStart / AfterAutoPrePlayPhaseEntered 3; AfterSideTurnStart, AfterCardPlayed,
   AfterCardChangedPiles, AfterDamageReceived, AfterEnergyReset, BeforeFlush, BeforeHandDraw,
   BeforeCombatStart, AfterCombatVictory, ModifyHandDraw, ModifyEnergyCost, ShouldDie, ModifyHpLost (4 passes)
   each have 2.
4. **Guarded vs unguarded dispatch** and the live `IsEnding` predicate (1.2). Without this, post-kill
   effects (draws, power application, block gain) are wrong.
5. **No rounding in ModifyDamage/ModifyBlock**; truncation at `Block -= (int)`, `LoseHpInternal`,
   `GainBlockInternal`, `ApplyInternal`, `ModifyAmount`. Block can be depleted by `(int)blocked` while
   the decimal `blocked` still prevents HP loss (Section 3.6).
6. **Intangible is a damage CAP applied before block** (to all props incl. Unblockable and Unpowered
   non-Move) **and** a HP-loss clamp after block. Cap runs after multiplicative stages.
7. **Buffer** is consumed only when `Truncate(amount_before) != Truncate(0)`, i.e. unblocked HP loss >= 1
   reaches AfterOstyLate (so blocked/0.x damage never consumes it); it is always the last HP-loss pass.
   Order-sensitive pairs among the AfterOsty relics are Tungsten Rod vs Beating Remnant (3.4).
8. **Osty (pet) damage**: damage to a pet is first absorbed by the **owner's** Block (CreatureCmd.cs:285),
   but `wasBlockBroken = originalTarget.Block <= 0 && blocked > 0` reads the pet's OWN `Block`
   (always 0), so a hit on Osty is reported "block broken" whenever any owner block was consumed, even
   if the owner still has block left (and `AfterBlockBroken` then fires for Osty). `wasFullyBlocked`
   uses `blocked > 0` so it works from owner block. DieForYou redirects *powered* unblocked damage from the owner to Osty (`Osty.IsAlive`);
   Osty overkill is re-run through `ModifyHpLost(AfterOsty)` against the owner.
9. **Block-expiry timing**: cleared at the start of the owner's own side turn, skipped on player turn 1,
   `AfterBlockCleared` always fires.
10. **Duration ticks**: Weak/Vuln/Frail tick at end of the *enemy* turn for every owner, with the
    "skip once on newly applied player-side debuff" flag; Intangible/NoBlock/Colossus tick at end of enemy turn
    without skip; others tick at end of their owner's side.
11. **Artifact** consumes a charge per blocked application even if the amount was already 0, blocks
    negative Strength/Dex/Focus (AllowNegative), not debuff decrements.
12. **`Apply<T>` returns non-null when blocked/zeroed in the new-instance path**, null when stacking
    results in 0 (removed) or when combat is ending. `BeforePowerAmountChanged` sees the raw amount;
    `BeforeApplied` runs even if amount was zeroed; `AfterApplied`/`AfterPowerAmountChanged` only if the
    modified amount != 0.
13. **`StackType` is cosmetic**; stacking is `InstanceType`-driven; Single powers still accumulate Amount.
14. **Poison**: triggers at AfterSideTurnStart of the owner's side (after block clear and, for the
    player, after draw/energy), damage = *current* Amount each iteration (Unblockable|Unpowered, dealer
    null, so Strength/Vuln don't apply but Intangible cap does), decrement only if still alive,
    iteration count `min(Amount, 1 + sum Accelerant of opponents)`.
15. **AoE damage**: all targets resolved before any post-hook; kills batched at the end; AfterDamageReceived
    skipped for killed results; `Kill` bypasses damage hooks (Doom).
16. **Death prevention order**: ShouldDie pass 1 (potion Fairy) before ShouldDieLate pass 2 (Lizard
    Tail); preventer heals via `AfterPreventingDeath`; `BeforeDeath` and `AfterDeath(true)` still fire on
    prevention; Fairy/Lizard heal amounts are truncated decimals.
17. **Dead-but-attached creatures** remain listeners and in `Enemies`; liveness is HP > 0 everywhere;
    `IsHittable` = alive && `ShouldAllowHitting` (guarded: true if combat ending); AttackCommand target
    filtering uses `IsAlive` only.
18. **Random targeting** consumes `RunState.Rng.CombatTargets` once per hit via `NextItem(validTargets)`
    (valid = alive opponents recomputed each hit; with `allowDuplicates=false` previously-hit creatures
    filtered); needs the exact RNG algorithm from the RNG spec.
19. **Vigor** stale `commandToModify` quirk (6.6).
19b. **Dead dealer mid-call**: `CreatureCmd.Damage` tests `dealer.IsDead` only on entry (CreatureCmd.cs:264).
    If Thorns (BeforeDamageReceived) kills the attacker, the hit itself still lands (block, HP loss,
    post-hooks); only the next hit of the AttackCommand breaks (`Attacker.IsDead`).
19c. **Fractional power amounts**: `ApplyInternal` tests the decimal `amount != 0` but stores `(int)amount`;
    a modified amount with |v| < 1 would attach a power with Amount 0 that is never removed
    (`ModifyAmount`'s removal check only runs on the existing-power path). Only reachable if a Given
    multiplier yields fractions.
19d. **Artifact vs AllowNegative debuffs**: besides Strength/Dex/Focus loss, finite Shrink (Counter +
    AllowNegative) is a Debuff for `a < 0`, so Artifact eats Shrink's own `Decrement` tick.
20. **Enchantment** damage/block modifiers are applied inside `Hook.ModifyDamage/ModifyBlock` before the
    listener loops, not at card creation.
21. **Calculated damage** is evaluated per hit with that hit's single target (null for AoE/multi).
22. **ModifyHpLost is called with 0**, hooks such as HardenedShell short-circuit on 0 explicitly; others
    return unchanged. Recorded-modifier gating uses truncation; Tungsten Rod flashes only if truncation
    changed.
23. **ShouldAllowHitting also gates `CanReceivePowers`**, so DieForYou (`creature.IsAlive`) etc. affect
    power application, and under a guarded dispatch while combat is ending everything is "hittable".
24. **Mutation of the creature list while iterating**: `AoE` uses the list captured at call time;
    `AttackCommand` recomputes alive targets per hit.
25. **Heal on a dead creature revives** (HealInternal) and reactivates hooks; `AfterCurrentHpChanged` heal
    delta is the requested amount, not the effective heal.
26. **LoseMaxHp** runs a full Damage pipeline (hooks/Intangible/Buffer can eat it) before shrinking max.
27. **PowerCmd.Remove** and `RemoveAllPowers*` never fire `AfterPowerAmountChanged`; death/escape strip
    powers via `RemoveInternal` + `AfterRemoved` (death) or silently (escape/reset/combat end).
28. **Debuff check in Artifact uses current amount** (`GetTypeForAmount`), so "negative Weak" cannot
    exist but "negative Strength" is a debuff.

---------------------------------------------------------------------------------------------------

## 9. Pause / async semantics (assumption)

Several dispatchers use `HookPlayerChoiceContext.AssignTaskAndWaitForPauseOrCompletion` and
`Task.WhenAll` (BeforeSideTurnStart, BeforeSideTurnEnd (3 tiers), AfterSideTurnEnd (2 tiers), BeforeFlush
(2 tiers), AfterDeath, AfterDiedToDoom, AfterAutoPre/PostPlayPhase): a listener that blocks on a
player choice lets the next listener *start*, and completion is awaited at the end of the tier. With no
mid-hook player choice this is observationally identical to sequential in-order execution; the sim
should execute them sequentially and treat any hook that needs a choice (card selection for
Mayhem/Stratagem-style effects) as a decision point per the engine's choice model. UNCERTAIN: exact
interleaving when two listeners in the same tier both raise choices (not needed for no-choice combats).

---------------------------------------------------------------------------------------------------

## 10. Open questions / uncertainties

1. Many damage/block/power modifiers were not individually read (list in 3.2/2.5). Each needs its own
   audit; the hook *framework* is fully specified here.
2. `ModifyAttackHitCount` has no overrides in `Models/` (grep); mods or future content only. Multi-hit
   counts come from `WithHitCount(n)` / `IncrementHitsInternal`. `hits` is a decimal compared with `i`.
3. `DoomPower.GetDoomedCreatures(CombatSide)` overload and the player-side behaviour were only skimmed
   (the HP rule `CurrentHp <= Amount` was read).
4. `PotionModel.RemoveBeforeUse` + Fairy in a Bottle: the potion leaves the slot before `OnUse`; the
   `AfterPreventingDeath` Contains-check then sees a removed potion only if Fairy's own call path
   re-checks; `Hook.AfterPreventingDeath` checks `L.Contains(preventer)` *before* calling
   (Hook.cs:1058), at which point the potion is still in its slot, so it fires. Not tested.
5. `Hook.ModifyUnblockedDamageTarget` is dispatched without a `props`-based guard in the caller; only
   DieForYou overrides it, gated on powered attacks and `Osty.IsAlive`. Combat-start wiring of
   DieForYou / Osty creation (`OstyCmd.cs:61`, `PlayerCmd.AddPet`) belongs to the Necrobinder spec.
6. `IsPrimaryEnemy/IsSecondaryEnemy` depend on `OwnerIsSecondaryEnemy` overrides (not enumerated).
7. `CardModel.Pile`, `Card.Owner` liveness semantics (`HasBeenRemovedFromState`) when cards are created
   mid-pass: assumed added to the end of the next snapshot only.
8. Deck-copy cards (`Deck.Cards`) are R-listeners during combat: no relevant overrides found (see 1.1), but
   any future card overriding e.g. `ModifyDamageAdditive` without a pile check would double-count.
9. `Creature.Powers` ordering when a creature dies and is revived (RemoveAllPowersAfterDeath keeps a
   subset) assumed list-stable.
10. Random targeting RNG details (`Rng.CombatTargets.NextItem`) and `Rng.Shuffle` belong to the RNG spec.
12. Per-card `CalculatedDamageVar` multiplier lambdas and any flooring they perform (see 3.1) were not read.
11. Multiplayer-only branches (`GetScaledAmountForMultiplayer`, MultiplayerScalingModel, extra turns,
    `Players.Count > 1`) are deliberately omitted.

---------------------------------------------------------------------------------------------------

## Appendix A. Complete hook-virtual index on `AbstractModel` (generated mechanically from AbstractModel.cs)

Every `Before*/After*/Modify*/Should*/TryModify*` virtual, with exact signature and AbstractModel.cs line.
Return `Task` = notification (no-op default); `decimal/int` = value hook (default returns its input /
0 for additive / 1 for multiplicative / `MaxValue` for cap); `bool Should*` default true except where noted
in 2.3; `bool TryModify*` default false with `out` = input. `ctx` = `PlayerChoiceContext`.

| Line | Ret | Name | Parameters |
|---|---|---|---|
| 208 | Task | AfterActEntered |  |
| 217 | Task | AfterAddToDeckPrevented | CardModel card |
| 228 | Task | BeforeAttack | AttackCommand command |
| 240 | Task | AfterAttack | ctx, AttackCommand command |
| 253 | Task | AfterAutoPostPlayPhaseEntered | ctx, Player player |
| 267 | Task | AfterAutoPrePlayPhaseEnteredEarly | ctx, Player player |
| 280 | Task | AfterAutoPrePlayPhaseEntered | ctx, Player player |
| 294 | Task | AfterAutoPrePlayPhaseEnteredLate | ctx, Player player |
| 304 | Task | AfterBlockCleared | Creature creature |
| 317 | Task | BeforeBlockGained | Creature creature, decimal amount, ValueProp props, CardModel? cardSource |
| 330 | Task | AfterBlockGained | Creature creature, decimal amount, ValueProp props, CardModel? cardSource |
| 347 | Task | AfterBlockBroken | ctx, Creature target, Creature? breaker |
| 359 | Task | AfterCardChangedPiles | CardModel card, PileType oldPileType, AbstractModel? clonedBy |
| 373 | Task | AfterCardChangedPilesLate | CardModel card, PileType oldPileType, AbstractModel? clonedBy |
| 384 | Task | AfterCardDiscarded | ctx, CardModel card |
| 398 | Task | AfterCardDrawnEarly | ctx, CardModel card, bool fromHandDraw |
| 410 | Task | AfterCardDrawn | ctx, CardModel card, bool fromHandDraw |
| 420 | Task | AfterCardEnteredCombat | CardModel card |
| 435 | Task | AfterCardGeneratedForCombat | CardModel card, Player? creator |
| 447 | Task | AfterCardExhausted | ctx, CardModel card, bool causedByEthereal |
| 459 | Task | BeforeCardAutoPlayed | CardModel card, Creature? target, AutoPlayType type |
| 468 | Task | BeforeCardPlayed | CardPlay cardPlay |
| 477 | Task | AfterCardPlayed | ctx, CardPlay cardPlay |
| 488 | Task | AfterCardPlayedLate | ctx, CardPlay cardPlay |
| 498 | Task | BeforeCombatStart |  |
| 510 | Task | BeforeCombatStartLate |  |
| 520 | Task | AfterCombatEnd | CombatRoom room |
| 532 | Task | BeforeCombatRewardOffered | RewardsSet rewards, CombatRoom room |
| 545 | Task | AfterCombatVictoryEarly | CombatRoom room |
| 556 | Task | AfterCombatVictory | CombatRoom room |
| 565 | Task | AfterCreatureAddedToCombat | Creature creature |
| 578 | Task | AfterCurrentHpChanged | Creature creature, decimal delta |
| 593 | Task | AfterDamageGiven | ctx, Creature? dealer, DamageResult result, ValueProp props, Creature target, CardModel? cardSource |
| 608 | Task | BeforeDamageReceived | ctx, Creature target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource |
| 623 | Task | AfterDamageReceived | ctx, Creature target, DamageResult result, ValueProp props, Creature? dealer, CardModel? cardSource |
| 640 | Task | AfterDamageReceivedLate | ctx, Creature target, DamageResult result, ValueProp props, Creature? dealer, CardModel? cardSource |
| 650 | Task | BeforeDeath | Creature creature |
| 668 | Task | AfterDeath | ctx, Creature creature, bool wasRemovalPrevented, float deathAnimLength |
| 679 | Task | AfterDiedToDoom | ctx, IReadOnlyList<Creature> creatures |
| 689 | Task | AfterEnergyReset | Player player |
| 701 | Task | AfterEnergyResetLate | Player player |
| 712 | Task | AfterEnergySpent | CardModel card, int amount |
| 721 | Task | BeforeCardRemoved | CardModel card |
| 732 | Task | BeforeFlush | ctx, Player player |
| 745 | Task | BeforeFlushLate | ctx, Player player |
| 758 | Task | AfterFlush | ctx, Player player, IReadOnlyCollection<CardModel> flushedCards, IReadOnlyCollection<CardModel> retainedCards |
| 767 | Task | AfterGoldGained | Player player |
| 779 | Task | BeforeHandDraw | Player player, ctx, ICombatState combatState |
| 793 | Task | BeforeHandDrawLate | Player player, ctx, ICombatState combatState |
| 804 | Task | AfterHandEmptied | ctx, Player player |
| 815 | Task | AfterItemPurchased | Player player, MerchantEntry itemPurchased, int goldSpent |
| 825 | Task | AfterMapGenerated | ActMap map, int actIndex |
| 842 | Task | AfterModifyingBlockAmount | decimal modifiedAmount, CardModel? cardSource, CardPlay? cardPlay |
| 851 | Task | AfterModifyingCardPlayCount | CardModel card |
| 861 | Task | AfterModifyingCardPlayResultLocation | CardModel card, CardLocation cardLocation |
| 870 | Task | AfterModifyingOrbPassiveTriggerCount | OrbModel orb |
| 878 | Task | AfterModifyingCardRewardOptions |  |
| 886 | Task | AfterModifyingDamageAmount | CardModel? cardSource |
| 894 | Task | AfterModifyingEnergyGain |  |
| 904 | Task | AfterModifyingGoldGained | Player player, decimal amount |
| 913 | Task | AfterModifyingHandDraw |  |
| 922 | Task | AfterPreventingDraw |  |
| 930 | Task | AfterModifyingHpLostBeforeOsty |  |
| 938 | Task | AfterModifyingHpLostAfterOsty |  |
| 948 | Task | AfterModifyingPowerAmountReceived | PowerModel power |
| 958 | Task | AfterModifyingPowerAmountGiven | PowerModel power |
| 966 | Task | AfterModifyingRewards |  |
| 977 | Task | AfterOrbChanneled | ctx, Player player, OrbModel orb |
| 988 | Task | AfterOrbEvoked | ctx, OrbModel orb, IEnumerable<Creature> targets |
| 997 | Task | AfterOstyRevived | Creature osty |
| 1008 | Task | BeforePotionUsed | PotionModel potion, Creature? target |
| 1019 | Task | AfterPotionUsed | PotionModel potion, Creature? target |
| 1029 | Task | AfterPotionDiscarded | PotionModel potion |
| 1039 | Task | AfterPotionProcured | PotionModel potion |
| 1057 | Task | BeforePowerAmountChanged | PowerModel power, decimal amount, Creature target, Creature? applier, CardModel? cardSource |
| 1077 | Task | AfterPowerAmountChanged | ctx, PowerModel power, decimal amount, Creature? applier, CardModel? cardSource |
| 1088 | Task | AfterPreventingBlockClear | AbstractModel preventer, Creature creature |
| 1097 | Task | AfterPreventingDeath | Creature creature |
| 1112 | Task | AfterRestSiteHeal | Player player, bool isMimicked |
| 1121 | Task | AfterRestSiteSmith | Player player |
| 1131 | Task | AfterRewardTaken | Player player, Reward reward |
| 1140 | Task | BeforeRoomEntered | AbstractRoom room |
| 1153 | Task | AfterRoomEntered | AbstractRoom room |
| 1164 | Task | AfterShuffle | ctx, Player shuffler |
| 1175 | Task | AfterStarsSpent | int amount, Player spender |
| 1186 | Task | AfterStarsGained | int amount, Player gainer |
| 1198 | Task | AfterForge | decimal amount, Player forger, AbstractModel? source |
| 1210 | Task | AfterSummon | ctx, Player summoner, decimal amount |
| 1220 | Task | AfterTakingExtraTurn | Player player |
| 1230 | Task | AfterTargetingBlockedVfx | Creature blocker |
| 1247 | Task | BeforeSideTurnStart | ctx, CombatSide side, IReadOnlyList<Creature> participants, ICombatState combatState |
| 1268 | Task | AfterSideTurnStart | CombatSide side, IReadOnlyList<Creature> participants, ICombatState combatState |
| 1290 | Task | AfterSideTurnStartLate | CombatSide side, IReadOnlyList<Creature> participants, ICombatState combatState |
| 1306 | Task | AfterPlayerTurnStartEarly | ctx, Player player |
| 1320 | Task | AfterPlayerTurnStart | ctx, Player player |
| 1336 | Task | AfterPlayerTurnStartLate | ctx, Player player |
| 1354 | Task | BeforeSideTurnEndVeryEarly | ctx, CombatSide side, IEnumerable<Creature> participants |
| 1372 | Task | BeforeSideTurnEndEarly | ctx, CombatSide side, IEnumerable<Creature> participants |
| 1388 | Task | BeforeSideTurnEnd | ctx, CombatSide side, IEnumerable<Creature> participants |
| 1406 | Task | AfterSideTurnEnd | ctx, CombatSide side, IEnumerable<Creature> participants |
| 1425 | Task | AfterSideTurnEndLate | ctx, CombatSide side, IEnumerable<Creature> participants |
| 1436 | int | ModifyAttackHitCount | AttackCommand attack, int hitCount |
| 1459 | decimal | ModifyBlockAdditive | Creature target, decimal block, ValueProp props, CardModel? cardSource, CardPlay? cardPlay |
| 1482 | decimal | ModifyBlockMultiplicative | Creature target, decimal block, ValueProp props, CardModel? cardSource, CardPlay? cardPlay |
| 1495 | int | ModifyCardPlayCount | CardModel card, Creature? target, int playCount |
| 1512 | CardLocation | ModifyCardPlayResultLocation | CardModel card, bool isAutoPlay, ResourceInfo resources, CardLocation cardLocation |
| 1524 | int | ModifyOrbPassiveTriggerCounts | OrbModel orb, int triggerCount |
| 1536 | CardCreationOptions | ModifyCardRewardCreationOptions | Player player, CardCreationOptions options |
| 1549 | CardCreationOptions | ModifyCardRewardCreationOptionsLate | Player player, CardCreationOptions options |
| 1561 | decimal | ModifyCardRewardUpgradeOdds | Player player, CardModel card, decimal odds |
| 1579 | decimal | ModifyDamageAdditive | Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource, CardPlay? cardPlay |
| 1595 | decimal | ModifyDamageCap | Creature? target, ValueProp props, Creature? dealer, CardModel? cardSource, CardPlay? cardPlay |
| 1613 | decimal | ModifyDamageMultiplicative | Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource, CardPlay? cardPlay |
| 1624 | decimal | ModifyEnergyGain | Player player, decimal amount |
| 1635 | decimal | ModifyGoldGained | Player player, decimal amount |
| 1646 | ActMap | ModifyGeneratedMap | IRunState runState, ActMap map, int actIndex |
| 1660 | ActMap | ModifyGeneratedMapLate | IRunState runState, ActMap map, int actIndex |
| 1671 | decimal | ModifyHandDraw | Player player, decimal count |
| 1683 | decimal | ModifyHandDrawLate | Player player, decimal count |
| 1702 | decimal | ModifyHpLostBeforeOsty | Creature target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource |
| 1722 | decimal | ModifyHpLostBeforeOstyLate | Creature target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource |
| 1741 | decimal | ModifyHpLostAfterOsty | Creature target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource |
| 1761 | decimal | ModifyHpLostAfterOstyLate | Creature target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource |
| 1771 | decimal | ModifyMaxEnergy | Player player, decimal amount |
| 1783 | IEnumerable<CardModel> | ModifyMerchantCardPool | Player player, IEnumerable<CardModel> options |
| 1794 | CardRarity | ModifyMerchantCardRarity | Player player, CardRarity rarity |
| 1807 | void | ModifyMerchantCardCreationResults | Player player, List<CardCreationResult> cards |
| 1818 | decimal | ModifyMerchantPrice | Player player, MerchantEntry entry, decimal cost |
| 1829 | decimal | ModifyOrbValue | OrbModel orb, decimal value |
| 1845 | decimal | ModifyPowerAmountGivenAdditive | PowerModel power, Creature giver, decimal amount, Creature? target, CardModel? cardSource |
| 1861 | decimal | ModifyPowerAmountGivenMultiplicative | PowerModel power, Creature giver, decimal amount, Creature? target, CardModel? cardSource |
| 1872 | decimal | ModifyRestSiteHealAmount | Creature creature, decimal amount |
| 1887 | void | ModifyShuffleOrder | Player player, List<CardModel> cards, bool isInitialShuffle |
| 1902 | decimal | ModifySummonAmount | Player summoner, decimal amount, AbstractModel? source |
| 1915 | Creature | ModifyUnblockedDamageTarget | Creature target, decimal amount, ValueProp props, Creature? dealer |
| 1925 | EventModel | ModifyNextEvent | EventModel currentEvent |
| 1935 | IReadOnlySet<RoomType> | ModifyUnknownMapPointRoomTypes | IReadOnlySet<RoomType> roomTypes |
| 1947 | float | ModifyOddsIncreaseForUnrolledRoomType | RoomType roomType, float oddsIncrease |
| 1958 | int | ModifyXValue | CardModel card, int originalValue |
| 1970 | bool | TryModifyCardBeingAddedToDeck | CardModel card, out CardModel? newCard |
| 1984 | bool | TryModifyCardBeingAddedToDeckLate | CardModel card, out CardModel? newCard |
| 1998 | bool | TryModifyCardRewardAlternatives | Player player, CardReward cardReward, List<CardRewardAlternative> alternatives |
| 2009 | bool | TryModifyCardRewardOptions | Player player, List<CardCreationResult> cardRewardOptions, CardCreationOptions creationOptions |
| 2021 | bool | TryModifyCardRewardOptionsLate | Player player, List<CardCreationResult> cardRewardOptions, CardCreationOptions creationOptions |
| 2034 | bool | TryModifyEnergyCostInCombat | CardModel card, decimal originalCost, out decimal modifiedCost |
| 2049 | bool | TryModifyEnergyCostInCombatLate | CardModel card, decimal originalCost, out decimal modifiedCost |
| 2066 | bool | TryModifyKeywordsInCombat | CardModel card, ISet<CardKeyword> keywords |
| 2078 | bool | TryModifyStarCost | CardModel card, decimal originalCost, out decimal modifiedCost |
| 2093 | bool | TryModifyPowerAmountReceived | PowerModel canonicalPower, Creature target, decimal amount, Creature? applier, out decimal modifiedAmount |
| 2105 | bool | TryModifyRestSiteOptions | Player player, ICollection<RestSiteOption> options |
| 2122 | bool | TryModifyRestSiteHealRewards | Player player, List<Reward> rewards, bool isMimicked |
| 2140 | bool | TryModifyRewards | Player player, List<Reward> rewards, AbstractRoom? room |
| 2155 | bool | TryModifyRewardsLate | Player player, List<Reward> rewards, AbstractRoom? room |
| 2167 | IReadOnlyList<LocString> | ModifyExtraRestSiteHealText | Player player, IReadOnlyList<LocString> currentExtraText |
| 2177 | bool | ShouldAddToDeck | CardModel card |
| 2188 | bool | ShouldAfflict | CardModel card, AfflictionModel affliction |
| 2200 | bool | ShouldAllowAncient | Player player, AncientEventModel ancient |
| 2211 | bool | ShouldAllowHitting | Creature creature |
| 2221 | bool | ShouldAllowTargeting | Creature target |
| 2232 | bool | ShouldAllowSelectingMoreCardRewards | Player player, CardReward cardReward |
| 2242 | bool | ShouldClearBlock | Creature creature |
| 2252 | bool | ShouldDie | Creature creature |
| 2264 | bool | ShouldDieLate | Creature creature |
| 2273 | bool | ShouldDisableRemainingRestSiteOptions | Player player |
| 2284 | bool | ShouldDraw | Player player, bool fromHandDraw |
| 2294 | bool | ShouldEtherealTrigger | CardModel card |
| 2304 | bool | ShouldFlush | Player player |
| 2315 | bool | ShouldGainStars | decimal amount, Player player |
| 2325 | bool | ShouldGenerateTreasure | Player player |
| 2336 | bool | ShouldPayExcessEnergyCostWithStars | Player player |
| 2347 | bool | ShouldPlay | CardModel card, AutoPlayType autoPlayType |
| 2357 | bool | ShouldPlayerResetEnergy | Player player |
| 2366 | bool | ShouldProceedToNextMapPoint |  |
| 2377 | bool | ShouldProcurePotion | PotionModel potion, Player player |
| 2387 | bool | ShouldPowerBeRemovedOnDeath | PowerModel power |
| 2398 | bool | ShouldRefillMerchantEntry | MerchantEntry entry, Player player |
| 2408 | bool | ShouldAllowMerchantCardRemoval | Player player |
| 2419 | bool | ShouldCreatureBeRemovedFromCombatAfterDeath | Creature creature |
| 2429 | bool | ShouldStopCombatFromEnding |  |
| 2439 | bool | ShouldTakeExtraTurn | Player player |
| 2449 | bool | ShouldForcePotionReward | Player player, RoomType roomType |
| 2457 | bool | ShouldAllowFreeTravel |  |
