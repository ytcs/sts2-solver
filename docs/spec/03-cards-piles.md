# 03 - Cards, Piles, Card Play (STS2 v0.111.0 public-beta, combat only)

Scope: the pure game-logic semantics of cards, piles, the card-play pipeline, shuffling/drawing,
player choices (decision points), in-combat card generation, enchantments/afflictions/modifiers.
Everything UI/VFX/SFX/animation/network is ignored.

All paths are relative to `decomp/MegaCrit/Sts2/Core/` (read-only reference). Citations are `file:line`.
"Sim" = our Rust re-implementation. "Hook" = a virtual on `AbstractModel` dispatched by `Hook.*`.
Everything here was read from source; items I could not fully resolve are collected in section 16.

Verification performed while writing this spec (not shipped as code): the .NET `List<T>.Sort()` tie-handling
used by the reshuffle (section 7.3) was re-implemented in Python from memory of the runtime's IntroSort and
compared against the real .NET 9.0.20 runtime on ~7000 inputs (n up to 519, many duplicate keys, sorted /
reverse / all-equal / organ-pipe / median-of-3-killer inputs, heap-sort fallback exercised 8 times):
0 mismatches. The pseudocode in 7.3 is therefore verified, not just recalled.

---------------------------------------------------------------------------------------------------

## 0. Reading guide / summary of the most important facts

1. Draw pile index 0 is the TOP. `Draw` takes element 0. `Bottom` = append, `Top` = insert at 0.
   Hand/discard/exhaust/play are also plain ordered lists (insertion order matters for hooks and "first card" effects).
2. Two different shuffles exist: initial combat shuffle (deck order, NO sort) and reshuffle (discard ++ draw,
   SORTED by (Id, upgradeLevel) with .NET's unstable IntroSort, then Fisher-Yates). Both use the RUN-LEVEL
   `Rng.Shuffle` stream, whose state persists across combats (section 7).
3. Card play = spend resources (card still in hand) -> move to Play pile -> compute result location ->
   compute play count (replays) -> loop {BeforeCardPlayed, OnPlay, Enchantment.OnPlay, Affliction.OnPlay,
   AfterCardPlayed} -> move to result pile -> cleanup of "until played" costs (section 5).
4. Energy cost is a base + ordered list of local modifiers + two passes of global hook modifiers, clamped at 0 only
   at the end; a negative intermediate value skips the global hooks (section 2).
5. Hand cap is 10. Draw simply stops at 10. Any other add-to-hand at 10 silently redirects the card to the
   discard pile (same position argument) (section 6.3).
6. Every random in-card choice uses one of 5 named run-level streams (`Shuffle`, `CombatCardGeneration`,
   `CombatCardSelection`, `CombatTargets`, `CombatEnergyCosts`); the exact consumption pattern per helper is
   in section 10. `NextItem` consumes one draw even for a single-element list; `TakeRandom` shuffles the whole list.
7. The decision points are a small closed set (section 9): play-card, end-turn, choose-from-hand (with filter,
   min/max), choose-from-pile (draw/discard/exhaust), choose-1-of-<=3 (discovery), plus non-combat deck/reward
   selections. Single-forced choices auto-resolve with no decision.

---------------------------------------------------------------------------------------------------

## 1. Data model

### 1.1 Canonical vs instance
* `ModelDb.Card<T>()` is the immutable canonical model (singleton). Combat/run cards are mutable clones:
  `ToMutable()` (`Models/CardModel.cs:1188`) = `MemberwiseClone` + `DeepCloneFields` + `AfterCloned`
  (`Models/AbstractModel.cs:171-178`).
* A combat starts by cloning each DECK card into the draw pile (`Entities/Players/Player.cs:806-815`):
  `state.CloneCard(deckCard)`, `clone.DeckVersion = deckCard`. So combat cards carry the deck card's
  upgrade level, enchantment, keywords, local cost modifiers, etc. Combat-side mutations do NOT touch the deck
  card unless code does so explicitly through `DeckVersion` (only Goopy does: section 12).
* `DeepCloneFields` (`Models/CardModel.cs:1194-1217`): copies local keyword set, dynamic vars (re-based), energy
  cost object (incl. `_base`, `_capturedXValue`, ALL local modifiers), temp star cost list, enchantment, affliction.
  `AfterCloned` (`:1219-1240`) clears events, `CurrentTarget=null`, `CurrentPlayIndex=0`, `DeckVersion=null`,
  `HasBeenRemovedFromState=false`. Everything else is shallow-copied by `MemberwiseClone`:
  `_baseReplayCount`, `_exhaustOnNextPlay` (cleared by `CreateClone`), `_hasSingleTurnRetain`, `_hasSingleTurnSly`,
  `_lastStarsSpent`, `_cloneOf`, `_isDupe`, `_currentUpgradeLevel`, `FloorAddedToDeck`.

### 1.2 Per-instance state the sim must store (`Models/CardModel.cs:43-106`, `632-634`)
| Field | Meaning |
|---|---|
| id (ModelId) | `CARD.<SLUG>` (section 1.4) |
| owner | player (single-player: always the one player; `GiveToAnotherPlayer` exists for multiplayer) |
| currentUpgradeLevel (0..MaxUpgradeLevel, default Max=1) | `:765-796` |
| energy cost: `_base`, `_capturedXValue`, `_localModifiers[]` | section 2 (`Entities/Cards/CardEnergyCost.cs`) |
| star cost: `_baseStarCost`, `_temporaryStarCosts[]`, `_lastStarsSpent` | Regent only (section 2.7) |
| local keyword set | canonical keywords +added -removed (`:507-519`, `:1330-1342`) |
| dynamic vars (Damage, Block, Cards, Energy, Stars, PowerVar<T>, ...) | `BaseValue`/`EnchantedValue`/`PreviewValue` (section 11) |
| `_baseReplayCount` | permanent extra replays set by effects (default 0) |
| `_exhaustOnNextPlay` | one-shot "exhaust when played" flag (Havoc/Cinder) |
| `_hasSingleTurnRetain`, `_hasSingleTurnSly` | cleared at end of turn |
| enchantment (0..1) with `Amount`, `Status` (Normal/Disabled) + per-enchantment fields | section 12 |
| affliction (0..1) with `Amount` | section 12.4 |
| `_cloneOf`, `_isDupe` | clone/dupe lineage (section 1.3) |
| `DeckVersion` | combat card -> originating deck card (may be null) |
| `HasBeenRemovedFromState` | true after `RemoveFromState` (card left combat for good) |
| per-card counters inside specific cards | e.g. Regret `CardsInHand`, TheBall `_extraDamageFromPlays`, MadScience; see individual `Models/Cards/*.cs` (not covered here) |
| `CurrentTarget`, `CurrentPlayIndex` | transient during OnPlayWrapper (`:900-928`) |

### 1.3 Clone vs dupe
* `CreateClone()` (`:2182-2193`): only legal if the card is in a combat pile (or in no pile); clones through
  `CardScope.CloneCard` (adds to the combat state's card list), sets `_cloneOf = this`, `ExhaustOnNextPlay = false`.
  Used by Anger, Dual Wield, etc. SUBTLE: it copies live state, including `until-played` cost modifiers and
  `_capturedXValue` of the original that are still active mid-play (they are cleared only after the original
  reaches its result pile, `:2007-2010`). E.g. Anger's clone created inside Anger's OnPlay inherits a
  "free this turn / until played" modifier the original currently has.
* `CreateDupe(newOwner)` (`:2215-2226`): clone + `IsDupe=true` + `RemoveKeyword(Exhaust)`. If the card is already a dupe,
  it dupes the original (`DupeOf`). Dupes: after being played they leave combat entirely (result location None,
  `:2086-2089`) instead of going to discard/exhaust; Duplication-potion-style effects cannot dupe a dupe; an
  X-cost dupe keeps the original's captured X (clone copies `_capturedXValue`).
* `CloneOf` is only bookkeeping (e.g. Terraforming).

### 1.4 Card identity and the sort key
* `ModelId(Category, Entry)` (`Models/ModelId.cs`). For cards Category is `CARD` (type chain base == `CardModel`,
  `"CardModel"` slugified to `CARD_MODEL`, `_MODEL` suffix stripped: `Models/ModelDb.cs:520-533`, `Models/ModelId.cs:57-69`).
  Entry = `Slugify(C# class name)` (`Helpers/StringHelper.cs:95-100`): .NET `Regex.Replace(name.Trim(), @"([A-Za-z0-9]|\G(?!^))([A-Z])", "$1_$2")`,
  then `ToUpperInvariant`, whitespace runs -> `_`, strip chars not in `[A-Z0-9_]`.
  Examples: `StrikeIronclad` -> `STRIKE_IRONCLAD`, `DefendIronclad` -> `DEFEND_IRONCLAD`, `AshenStrike` -> `ASHEN_STRIKE`.
  CAUTION: this is NOT simply "underscore before every capital" for runs of 4+ capitals (the alnum branch is tried first at each position and
  consumes two characters, e.g. `ABC`->`A_B_C` but `ABCD`->`A_BC_D`). VERIFIED with the real .NET `Regex` over all 597 `class X : CardModel`
  names under `Models/Cards` (7 further mock classes in `Cards/Mocks` ignored): the naive per-capital rule and the real regex agree on every one
  (only `ExpectAFight`->`EXPECT_A_FIGHT` and `IAmInvincible`->`I_AM_INVINCIBLE` contain adjacent capitals; no card has a digit in its name).
  RECOMMENDATION: still implement the regex exactly (or ship a generated id table) instead of the naive rule, so future cards cannot diverge.
* `ModelId.CompareTo` (`Models/ModelId.cs:42-50`): `string.Compare(..., StringComparison.Ordinal)` on Category, then Entry.
  Ordinal = UTF-16 code unit order. All ids are ASCII `[A-Z0-9_]`, so a byte-wise compare is identical.
  Remember `'_'` (0x5F) sorts AFTER `'A'..'Z'` (0x41-0x5A) and after digits (0x30-0x39):
  `STRIKER` < `STRIKE_IRONCLAD`.
* `CardModel.CompareTo` (`Models/CardModel.cs:2272-2294`): same reference -> 0; null -> 1; else compare `Id`; if
  equal then compare `CurrentUpgradeLevel` ascending. NOTHING ELSE (enchantment, cost mods, affliction, keywords,
  creation order are ignored). Two different instances with the same id + upgrade level compare EQUAL; their relative
  order after `List.Sort` is decided by the algorithm in 7.3. This is the only place where "identical" cards are
  distinguishable by draw order, so the sim must track instance identity and run the same sort.

### 1.5 Enums (verbatim values; `Entities/Cards/`)
* `CardType`: None, Attack, Skill, Power, Status, Curse, Quest (`CardType.cs`).
* `CardRarity`: None, Basic, Common, Uncommon, Rare, Ancient, Event, Token, Status, Curse, Quest (`CardRarity.cs`) - numeric
  order matters (see transformation filter `rarity-2 <= 2`, i.e. Common..Rare; and `OrderBy(Rarity)` in draw-pile selection).
* `TargetType`: None, Self, AnyEnemy, AllEnemies, RandomEnemy, AnyPlayer, AnyAlly, AllAllies, TargetedNoCreature, Osty (`TargetType.cs`).
* `CardKeyword`: None, Exhaust, Ethereal, Innate, Unplayable, Retain, Sly, Eternal (`CardKeyword.cs`).
* `CardTag`: None, Strike, Defend, Minion, OstyAttack, Shiv (`CardTag.cs`). Tags are static per card (never change).
* `PileType`: None, Draw, Hand, Discard, Exhaust, Play, Deck. Combat piles = Draw, Hand, Discard, Exhaust, Play
  (`PileTypeExtensions.cs:IsCombatPile`: values 1..5). `Deck` is run-level.
* `CardPilePosition`: None, Bottom, Top, Random (`CardPilePosition.cs`).
* `AutoPlayType`: None, Default, SlyDiscard. `UnplayableReason` flags: HasUnplayableKeyword, BlockedByHook,
  BlockedByCardLogic, EnergyCostTooHigh, StarCostTooHigh, NoLivingAllies.

---------------------------------------------------------------------------------------------------

## 2. Cost system

### 2.1 Base values
* `CardEnergyCost(card, canonical, costsX)` (`Entities/Cards/CardEnergyCost.cs:82-88`): `Canonical = costsX ? 0 : canonical`;
  `_base = Canonical`. Negative canonical cost (-1) means "no cost / unplayable-by-design" (curses, statuses, Burn is -1
  but Beckon is 1). X-cost cards: `HasEnergyCostX` -> `CostsX`, `_base = 0`.
* Base can be changed only by `UpgradeBy(addend)` and `SetCustomBaseCost` (MadScience) / `ResetForDowngrade`.

### 2.2 Local modifiers (`LocalCostModifier.cs`, `CardEnergyCost.cs:175-325`)
List `_localModifiers` (insertion order is application order). Each: `{Amount, Type: Absolute|Relative, Expiration flags, IsReduceOnly}`.
```
Modify(cur):
  Absolute: reduceOnly ? min(cur, Amount) : Amount
  Relative: reduceOnly ? min(cur, cur+Amount) : cur+Amount
```
Expiration flags (`LocalCostModifierExpiration`): `EndOfCombat = 0`, `EndOfTurn = 2`, `WhenPlayed = 4` (flags; "0" means neither flag
is set, so the modifier is never removed by the two cleanups below and simply dies with the combat card).
Constructors (what each public method adds):
| method | Type | Expiration | guard |
|---|---|---|---|
| `SetUntilPlayed(c)` | Absolute | WhenPlayed | only if `c != 0 || Canonical >= 0` |
| `SetThisTurnOrUntilPlayed(c)` ("this turn" in text) | Absolute | EndOfTurn \| WhenPlayed | same guard |
| `SetThisTurn(c)` | Absolute | EndOfTurn | same guard |
| `SetThisCombat(c)` | Absolute | EndOfCombat(0) | same guard |
| `AddUntilPlayed(a)` | Relative | WhenPlayed | `a != 0` |
| `AddThisTurnOrUntilPlayed(a)` | Relative | EndOfTurn \| WhenPlayed | `a != 0` |
| `AddThisTurn(a)` | Relative | EndOfTurn | `a != 0` |
| `AddThisCombat(a)` | Relative | EndOfCombat | `a != 0` |
Each has an optional `reduceOnly`. The guard `c != 0 || Canonical >= 0` means "set to 0" on a card whose canonical
cost is negative (status/curse) is silently ignored.

Cleanup:
* `EndOfTurnCleanup()`: remove modifiers with the EndOfTurn flag (`:331-335`).
* `AfterCardPlayedCleanup()`: remove modifiers with the WhenPlayed flag (`:341-345`).
* Both are all-at-once `RemoveAll`; modifiers added later are unaffected. A "this turn or until played" modifier
  therefore disappears after the first play OR at end of turn (whichever first); a plain `SetThisTurn/AddThisTurn`
  survives plays (Pinpoint-style).

### 2.3 Current cost: `GetWithModifiers(mods)` (`CardEnergyCost.cs:94-121`)
```
fn cost_with(mods) -> int:
  n = _base
  if card.is_canonical: return n            // canonical model has no modifiers
  if _base < 0:        return n             // negative = no cost, modifiers ignored
  if CostsX:           return n             // (0)
  if mods has Local:   for m in local_modifiers (in order): n = m.modify(n)     // NO clamp between steps
  if mods has Global && card.CombatState != null (card is in a combat pile):
        n = (int) Hook.ModifyEnergyCostInCombat(card, n)       // see 2.4
  return max(0, n)
```
`CostModifiers`: Local = 2, Global = 4, All = -1 (both). Note: `GetWithModifiers` is evaluated with the card's CURRENT pile:
a card in the Deck pile has no CombatState so global modifiers are skipped.

### 2.4 Global modifiers: `Hook.ModifyEnergyCostInCombat` (`Hooks/Hook.cs:1582-1600`)
```
fn ModifyEnergyCostInCombat(card, c: decimal):
  if c < 0: return c                          // negative intermediate (from local modifiers) bypasses ALL global hooks, then clamped to 0
  for model in IterateCombatHookListeners: model.TryModifyEnergyCostInCombat(card, c, &c)      // pass 1 (in listener order)
  for model in IterateCombatHookListeners: model.TryModifyEnergyCostInCombatLate(card, c, &c)  // pass 2
  return c
```
Implementers (this build): pass 1: `SpikedGauntlets`, `CuriousPower`, `BorrowedTimePower`, `TangledPower`;
pass 2 ("Late", free-cost effects): `BrilliantScarf`, `FreePowerPower`, `FreeSkillPower`, `FreeAttackPower`,
`CorruptionPower` (Skills cost 0), `VeilpiercerPower`, `VoidFormPower`. (`grep` of `Models/**`.)
Consequence: relative +/- effects (pass 1) are applied before free-cost overrides (pass 2), independent of listener order across passes.

### 2.5 Spending, playability, X
* `GetAmountToSpend()` (`:134-141`): X-cost -> ALL of the owner's current energy; else `max(0, GetWithModifiers(All))`.
* `HasEnoughResourcesFor` (`Entities/Players/PlayerCombatState.cs:190-208`): `e = max(0, cost_with(All))`,
  `s = max(0, card.GetStarCostWithModifiers())`; if `e > Energy && Hook.ShouldPayExcessEnergyCostWithStars` then
  `s += (e-Energy)*2; e = Energy` (NO implementer in this build, so this branch never triggers - flag in 16).
  Fail reasons: `EnergyCostTooHigh` if `e > Energy`, `StarCostTooHigh` if `s > Stars`. X-cost cards always pass the energy check
  (their cost is 0) even with 0 energy.
* `SpendResources` (`Models/CardModel.cs:1807-1844`), manual play only (see 5.1): computes energy/stars, then
  `SpendEnergy`: if X -> `CapturedXValue = amount`; if `amount > 0` log history + `LoseEnergy`; ALWAYS then
  `Hook.AfterEnergySpent(card, amount)` (even for 0). `SpendStars`: `LastStarsSpent = amount`; if `> 0` lose stars and `Hook.AfterStarsSpent`.
* X value used by the card = `ResolveEnergyXValue()` = `Hook.ModifyXValue(captured)` (`:1117-1124`, `Hooks/Hook.cs:2079`);
  only implementer: `ChemicalX` relic. The captured X is fixed before the first replay; all replays use the same X.
* `GetResolved()` (`:155-162`): X -> captured value, else `max(0, cost_with(All))` (used by relics like IntimidatingHelmet - note it is evaluated at hook time).
* Free helpers: `SetToFreeThisTurn()` = `EnergyCost.SetThisTurnOrUntilPlayed(0)` + star `ThisTurn(0)`;
  `SetToFreeThisCombat()` = `SetThisCombat(0)` + star `ThisCombat(0)` (`:1267-1277`).

### 2.6 Upgrading the base cost (`CardEnergyCost.UpgradeBy`, `:351-372`)
```
UpgradeBy(a):  if CostsX or a==0: return
  old=_base; new=max(old+a,0); WasJustUpgraded=true
  if new < old: for each local Absolute modifier m: if m.Amount > new: m.Amount = new     // keeps "set to N" modifiers from exceeding the new base
  _base = new
```
(Relative modifiers are untouched.) The `WasJustUpgraded` flag is display only.

### 2.7 Star costs (Regent; `Models/CardModel.cs:410-466`, `1279-1328`, `1556-1568`)
Separate mini-system: `_baseStarCost` (canonical -1 = none), `_temporaryStarCosts` list of
`{Cost>=0, ClearsWhenTurnEnds, ClearsWhenCardIsPlayed}` (`TemporaryCardCost`: UntilPlayed -> (false,true), ThisTurn -> (true,true),
ThisCombat -> (false,false)). `CurrentStarCost` = LAST temp cost's value (a temp 0 on a card with negative base keeps the negative base)
else base. `GetStarCostWithModifiers()`: star-X -> all current stars; else if in combat pile `Hook.ModifyStarCost` (implementers: BrilliantScarf, VoidFormPower) else `CurrentStarCost`.
`UpgradeStarCostBy` removes temp costs greater than the new base. Cleared in `EndOfTurnCleanup` (ClearsWhenTurnEnds) and after play (ClearsWhenCardIsPlayed) (`:1611-1624`, `:2011-2014`).

---------------------------------------------------------------------------------------------------

## 3. Keywords, types, targeting

### 3.1 Keyword set
`Keywords` (`:526`) = local keywords (canonical + AddKeyword - RemoveKeyword) plus GLOBAL keywords computed on demand by
`Hook.ModifyKeywordsInCombat` (iterates `combatState.IterateHookListeners()` directly, `Hooks/Hook.cs:1603-1610`; only implementer: `HexPower`
adds Ethereal). Global keywords are never stored. Canonical models and cards outside combat get local only (`:1153-1181`).
Use `GetKeywordsWithSources(Local)` when the effect cares about the card's own keywords (Souls enchantment, Sculpting Strike).

| Keyword | Where it acts |
|---|---|
| Exhaust | result location after play (`GetResultLocationForCardPlay`, `:2084-2096`) and `MoveToResultPileWithoutPlaying` |
| Ethereal | at player turn end: exhausted from hand (before flush), unless `Hook.ShouldEtherealTrigger` vetoes (no implementer) (`Combat/CombatManager.cs:1599-1624`) |
| Innate | turn-1 draw: moved to top of draw pile, hand size grows (`CombatManager.cs:906-921`) |
| Retain | not flushed at turn end; `ShouldRetainThisTurn = Retain keyword OR single-turn retain flag` (`:591-601`) |
| Sly | `IsSlyThisTurn = keyword OR single-turn flag` (`:620-630`); on discard via `CardCmd.Discard*` it auto-plays (5.4) |
| Unplayable | `CanPlay` fails; auto-play of it just moves it to its result pile (`CardCmd.cs:57-61`) |
| Eternal | `IsRemovable=false` (deck removal/transform of the deck copy blocked, `:738-751`); no combat effect |
| `ExhaustOnNextPlay` flag | one-shot Exhaust; consumed (reset to false) when the result location is computed (`:2090-2093`); also cleared at end of turn (`:1613`) |

### 3.2 Card types
* Power: result location None -> after the plays it is removed from combat (not exhausted, no AfterCardExhausted) (`:2086-2089`, `:1994-1996`).
* Status/Curse/Quest: not enchantable (`EnchantmentModel.CanEnchant`, `Models/EnchantmentModel.cs` type check `type-4 <= 2`).
* `GainsBlock` virtual (Nimble filter, Osty auto-target) - per-card property.

### 3.3 TargetType and `IsValidTarget` (`:1762-1785`)
```
IsValidTarget(t):
  if t == null:  return TargetType != AnyEnemy && TargetType != AnyAlly      // every other type is played without a target
  if !t.alive:   return false
  AnyEnemy -> t.side != owner.side ;  AnyAlly -> t.side == owner.side ;  everything else -> false (a target must not be given)
```
`AnyAlly` additionally requires >= 2 living player creatures (`NoLivingAllies`) so in single-player they are never playable.
`Self/None/AllEnemies/RandomEnemy/AnyPlayer/AllAllies/TargetedNoCreature/Osty` take no target (the OnPlay logic picks creatures itself;
random picks use `Rng.CombatTargets`). `IsValidTarget` does NOT check "hittable"; enemy hittability is the OnPlay code's business.

---------------------------------------------------------------------------------------------------

## 4. Playability

`CanPlay(out reason, out preventer)` (`:1725-1755`), evaluated by `PlayCardAction` right before executing and by UIs/agents when enumerating actions:
```
if combatState == null || owner.PlayerCombatState == null: false
reason = 0
if Keywords.contains(Unplayable):                           reason |= HasUnplayableKeyword     // global keywords included
if !HasEnoughResourcesFor(card):                            reason |= Energy/StarCostTooHigh
if TargetType==AnyAlly && living players <= 1:              reason |= NoLivingAllies
if !Hook.ShouldPlay(card, AutoPlayType.None):               reason |= BlockedByHook             // first vetoing listener is `preventer`
if !IsPlayable (virtual, card logic):                       reason |= BlockedByCardLogic
playable = reason == 0
```
`CanPlayTargeting(t) = IsValidTarget(t) && CanPlay()`.
`PlayCardAction.ExecuteAction` (`GameActions/PlayCardAction.cs:62-105`) additionally requires the card to be in the HAND; if
`!CanPlay || !IsValidTarget` the action is cancelled (nothing happens). It is `GameActionType.CombatPlayPhaseOnly`.

`Hook.ShouldPlay` implementers (veto): `VelvetChoker` (6 cards/turn limit), `ChainsOfBindingPower` (one Bound card/turn), `RingingPower`
(only the first card each turn), `SmoggyPower` (Smog-afflicted cards unplayable), `SlothPower` (N cards/turn), cards `Enthralled`
(cards in hand other than itself unplayable for manual play; autoplay allowed), `Normality` (max 3 cards/turn while it is in hand).
Several read the combat history (cards played this turn), so the sim needs a per-turn play log (section 14).
`IsPlayable` card-logic overrides: `Clash` (hand all Attacks), `GrandFinale` (draw pile empty), `HighFive` (Osty alive).

---------------------------------------------------------------------------------------------------

## 5. Card-play pipeline

### 5.1 Manual play (`PlayCardAction.ExecuteAction` + `CardModel.OnPlayWrapper`)
```
PlayCardAction.Execute(card, target):
  require card in Hand; require CanPlay && IsValidTarget(target)           // else cancel, no state change
  (energySpent, starsSpent) = card.SpendResources()                          // card is STILL in Hand; see 2.5
        energyToSpend = EnergyCost.GetAmountToSpend()                       // X: all energy
        starsToSpend  = max(0, GetStarCostWithModifiers())
        SpendEnergy: CapturedXValue = amount (if X); lose energy; Hook.AfterEnergySpent(card, amount)
        SpendStars : LastStarsSpent = amount; lose stars; Hook.AfterStarsSpent
  resources = {EnergySpent=e, EnergyValue=e, StarsSpent=s, StarValue=s}
  OnPlayWrapper(card, target, isAutoPlay=false, resources)
```
`OnPlayWrapper` (`Models/CardModel.cs:1858-2019`):
```
 1  CurrentTarget = target; CurrentPlayIndex = 0
 2  manual:  CardPileCmd.AddDuringManualCardPlay(card)           // remove from Hand, append to Play pile (AddInternal), then
                                                                   // Hook.AfterCardChangedPiles(oldPile=Hand)   (CardPileCmd.cs:830-867)
    auto:    CardPileCmd.Add(card, Play, Bottom)                  // full Add semantics (5.5 / 6.2)
 3  if card.CombatState == null: return                          // combat ended
 4  resultLocation = GetResultLocationForCardPlay()              // virtual; BASE: dupe or Power -> (None); ExhaustOnNextPlay or Exhaust kw -> Exhaust
                                                                   //       (and ExhaustOnNextPlay := false!); else Discard; position Bottom; player = owner
                                                                   // overrides: ParticleWall (Discard->Hand), ShiningStrike (Discard->Draw Top),
                                                                   //            TheBall (picks target player w/ CombatTargets, Discard->Draw Random)
 5  resultLocation = Hook.ModifyCardPlayResultLocation(...)      // listener order; may change pile/position/player; then for each modifier
                                                                   //   modifier.AfterModifyingCardPlayResultLocation(card, finalLocation)  (decrement stacks etc.)
 6  playCount = (EnchantedReplayCount + 1)                       // EnchantedReplayCount = Enchantment?.EnchantPlayCount(BaseReplayCount) ?? BaseReplayCount
    playCount = Hook.ModifyCardPlayCount(card, target, playCount, out modifiers)   // sequential, listener order
    Hook.AfterModifyingCardPlayCount(card, modifiers)            // each modifier's AfterModifying... (Burst/Duplication/OneTwoPunch decrement, ThrowingAxe marks used)
 7  if owner dead: return
 8  BeginCardOrPotionEffect(owner)                               // depth++ (affects CheckForEmptyHand / death cleanup)
 9  for i in 0..playCount:
        if combat over/ending: break
        CurrentPlayIndex = i
        cardPlay = {card, player, target, ResultPile = resultLocation.pileType, resources, isAutoPlay, PlayIndex=i, PlayCount=playCount}
        Hook.BeforeCardPlayed(cardPlay)                          // all listeners (Sloth counts here)
        History.CardPlayStarted(cardPlay)                        // logged AFTER BeforeCardPlayed hooks ran (so a hook sees history without this play)
        card.OnPlay(ctx, cardPlay)                               // card logic
        if owner dead: return
        if Enchantment != null: Enchantment.OnPlay(ctx, cardPlay)
        if Affliction  != null: Affliction.OnPlay(ctx, target)
        History.CardPlayFinished(cardPlay)
        if combat in progress: Hook.AfterCardPlayed(cardPlay)    // listeners: AfterCardPlayed pass then AfterCardPlayedLate pass; NOT guarded by IsOverOrEnding
10  EndCardOrPotionEffect(owner)                                 // depth--; if owner died and depth==0: remove their cards
11  if still the same combat:
        if owner != resultLocation.player && pile != None: GiveToAnotherPlayer(...)      // multiplayer only
        if card is still in Play pile:
            None    -> CardPileCmd.RemoveFromCombat(card)                      // Powers, dupes
            Exhaust -> CardCmd.Exhaust(card)                                    // History.CardExhausted, Hook.AfterCardExhausted(causedByEthereal=false)
            else    -> CardPileCmd.Add(card, resultLocation.pileType, resultLocation.position)    // Draw/Hand/Discard, Top/Bottom/Random
    CheckForEmptyHand(owner)                                     // Hook.AfterHandEmptied if turn in progress && no card/potion effect running && hand empty
12  EnergyCost.AfterCardPlayedCleanup()                           // remove WhenPlayed local modifiers (AFTER the card has moved)
    remove temp star costs with ClearsWhenCardIsPlayed
    CurrentTarget = null; CurrentPlayIndex = 0
```
Key consequences a faithful sim must reproduce:
* Resource spend precedes the move to the Play pile; `AfterEnergySpent` listeners see the card in the hand.
* The result pile is decided BEFORE `OnPlay` (so `OnPlay` effects that exhaust/discard/move the card itself change what
  step 11 does: step 11 only acts if the card is still in the Play pile - `:1989-1990`).
* Replays: `playCount` is fixed before the loop; hooks that change replay counts later are not re-queried.
  `BeforeCardPlayed`/`AfterCardPlayed` fire once PER PLAY (not once per card).
* `Enchantment.OnPlay` runs after `OnPlay` every replay; `Affliction.OnPlay` after that.
* The card sits in the Play pile during all plays; it is a hook listener there too (Play pile is in `AllPiles`).
* `History.CardPlayStarted` is recorded after the `BeforeCardPlayed` hooks (Echo Form / Nostalgia / Feral count it).
* `AfterCardPlayed` is dispatched even if the final blow ended combat (`Hook.cs:278-295`), but the loop breaks at the next iteration.
* Hand-empty hook is suppressed while ANY card/potion effect is on the stack (`CombatManager.cs:1171-1176`,
  depth counter `:368-404`); with Havoc-style nested auto-plays the check only happens when the outermost effect finishes.
* "Until played" local cost modifiers are removed AFTER the card has been moved to its result pile and after the
  hand-empty check; cards created via `CreateClone` mid-play therefore inherit them (section 1.3).

### 5.2 Auto-play (`CardCmd.AutoPlay`, `Commands/CardCmd.cs:51-127`)
Used by Havoc/Mayhem/Imbued/Sly-discard/etc.
```
AutoPlay(card, target?, type=Default, skipXCapture=false):
  if combat over or owner dead: return
  if card has Unplayable keyword:                        MoveToResultPileWithoutPlaying; return
  if !Hook.ShouldPlay(card, type):                       MoveToResultPileWithoutPlaying; return          // (+ thought bubble)
  if TargetType == AnyEnemy:   target ??= Rng.CombatTargets.NextItem(combat.HittableEnemies)   // 1 draw if >=1 hittable enemy; none -> MoveToResultPileWithoutPlaying
  if TargetType == AnyAlly:    target ??= CombatTargets.NextItem(living other players)         // none in single-player -> without playing
  if CostsX && !skipXCapture:  CapturedXValue = owner.Energy                                    // takes ALL energy value but does NOT spend it
  if !skipXCapture:            LastStarsSpent = starX ? Stars : max(0, GetStarCostWithModifiers())
  if card.Pile == null:        CardPileCmd.Add(card, Play)
  Hook.BeforeCardAutoPlayed(card, target, type)
  resources = {EnergySpent=0, EnergyValue=card.EnergyCost.GetAmountToSpend(), StarsSpent=0, StarValue=max(0,starCost)}
  OnPlayWrapper(card, target, isAutoPlay=true, resources)      // step 2 of OnPlayWrapper uses CardPileCmd.Add(card, Play, Bottom) from WHEREVER it is
```
`MoveToResultPileWithoutPlaying(card)` (`CardCmd.cs:133-137` + `CardModel.cs:2103-2121`): FIRST `CardPileCmd.Add(card, Play)` (so a card
that was in Discard/Draw/Hand is moved to the END of the Play pile, firing pile-change hooks), THEN: dupe -> RemoveFromCombat; Exhaust
keyword or ExhaustOnNextPlay -> `CardCmd.Exhaust`; else `Add(card, Discard)` (bottom). Note: Powers go to DISCARD here, not removed.
Energy is NOT paid in auto-play; free cost does not matter. X cards autoplay with X = current energy (energy is not consumed).

### 5.3 `CardPileCmd.AutoPlayFromDrawPile(count, position, forceExhaust)` (`CardPileCmd.cs:1148-1183`)
```
cards = []
for i in 0..count:
   ShuffleIfNecessary()                                   // may reshuffle discard into draw (7.4)
   c = Bottom -> draw.last ; Top -> draw.first ; Random -> Rng.CombatCardSelection.NextItem(draw)     // null => break
   cards.push(c); CardPileCmd.Add(c, Play)                // moved to Play pile first, ALL of them
for c in cards: (if owner alive) c.ExhaustOnNextPlay = forceExhaust; CardCmd.AutoPlay(c, null)
```
### 5.4 Discard, Sly, Exhaust (`CardCmd.cs`)
* `Discard(cards)` = `DiscardAndDraw(cards, 0)` (`:172-210`): for each card in order: remember if `IsSlyThisTurn`;
  `CardPileCmd.Add(card, Discard)` (bottom); `History.CardDiscarded`; `Hook.AfterCardDiscarded`. Then optional `Draw(cardsToDraw)`.
  THEN, in the original discard order, each Sly card is `AutoPlay(card, null, SlyDiscard)`. So Sly triggers only through `CardCmd.Discard*`,
  after all discards and the draw. The end-of-turn flush uses `CardPileCmd.Add` directly and NEVER triggers Sly.
* `Exhaust(card, causedByEthereal)` (`:237-254`): no-op if combat over/ending; `CardPileCmd.Add(card, Exhaust, Bottom)`;
  `History.CardExhausted`; `Hook.AfterCardExhausted(card, causedByEthereal)`. Returns the add result.

### 5.5 Remove from combat (`CardPileCmd.RemoveFromCombat`, `:110-208`)
Card must be in a combat pile; `RemoveFromCurrentPile()`; then per card `Hook.AfterCardChangedPiles(card, oldPile, newPile=None)`;
then `card.RemoveFromState()` (`HasBeenRemovedFromState=true`; the card is dead: any later `Add` on it returns `success=false`,
`:396-408`). Used for Powers after play, dupes, dead-player cleanup.

---------------------------------------------------------------------------------------------------

## 6. Pile operations (`Entities/Cards/CardPile.cs`, `Commands/CardPileCmd.cs`)

### 6.1 Structure
`PlayerCombatState` owns 5 piles in this fixed order `{Hand, Draw, Discard, Exhaust, Play}` (`Entities/Players/PlayerCombatState.cs:70-80`);
that order is also the order cards appear in hook-listener iteration (section 13). `CardPile` = `List<CardModel>`.
Low-level: `AddInternal(card, index=-1)` appends (index -1) or inserts at index (`CardPile.cs:83-113`; throws if card already in pile);
`RemoveInternal` (`:115-139`); `MoveToBottomInternal` = remove + append; `MoveToTopInternal` = remove + insert(0) (`:142-170`).
`MaxCardsInHand = 10` (`:21`).

### 6.2 `CardPileCmd.Add(cards, pile, position, ...)` (`CardPileCmd.cs:373-545`) - THE generic move
```
Add(cards, newPile, position=Bottom):
  if newPile is combat pile && combat is ENDING: return all success=false
  VALIDATE each card: owner != null; if card.HasBeenRemovedFromState || owner dead || (in-combat && no combat state): success=false (skipped, no error)
                      deck target requires card registered in run state; combat card must be in the CombatState; previews can't be added; all same owner
  if newPile==Deck: Hook.ShouldAddToDeck veto (run-level)
  if newPile is combat pile && combat not in progress: return
  for each successful card, SEQUENTIALLY:
      target = newPile
      isFullHandAdd = newPile.Type==Hand && newPile.Count >= 10                    // evaluated per card, at that moment
      if isFullHandAdd: target = Discard pile (same owner)                         // silently redirected; same `position` argument is applied to the discard pile
      if card is in some pile (oldPile != null): card.RemoveFromCurrentPile()      // so Random index below sees the post-removal count
      else if target==Deck: Hook.ModifyCardBeingAddedToDeck (run-level)
      index = Bottom -> -1(append) | Top -> 0 | Random -> run.Rng.Shuffle.NextInt(target.Count + 1)       // NOTE: uses the SHUFFLE stream, inserts at [0..Count]
      target.AddInternal(card, index)
      if oldPile == null && target is combat pile && !isChangingOwners: Hook.AfterCardEnteredCombat(card)      // brand-new cards only
  then (visual tween skipped), then for each successful add where oldPile==null || oldPile.Type != card's new pile type:
      Hook.AfterCardChangedPiles(card, oldPileType or None)                         // NOT fired for same-type moves (e.g. Draw->Draw during a reshuffle)
  return per-card results {success, cardAdded, oldPile, targetPile}
```
Notes: `AfterCardChangedPiles` is dispatched in two passes over all listeners: `AfterCardChangedPiles` then `AfterCardChangedPilesLate`
(`Hooks/Hook.cs:167-183`). It uses `runState.IterateHookListeners(combatState)` (not the combat-guarded iterator).
Because the hand-full redirection is per-card, adding N cards to a hand with room for k fills the hand with the first k and sends the rest to the discard pile in order.

### 6.3 Draw (`DrawInternal`, `CardPileCmd.cs:1010-1069`)
```
Draw(count, player, fromHandDraw=false):
  if combat over/ending: return []
  if !Hook.ShouldDraw(player, fromHandDraw): Hook.AfterPreventingDraw(...); return []        // NoDrawPower, Fiddle(relic)
  n = ceil(count) if count > 0 else 0 ; if n == 0 return []
  room = max(0, 10 - hand.Count) ; if room == 0 return []                                      // no shuffle attempted when hand is already full
  for i in 0..n:
      if room <= 0: break ; if combat over: break
      if draw.Count + discard.Count == 0: break                                                 // "NO_DRAW"
      if hand.Count >= 10: break
      ShuffleIfNecessary()                                                                      // draw empty && discard non-empty -> Shuffle (7.4)
      if draw.Count + discard.Count == 0: break
      card = draw.first ; if card == null || hand.Count >= 10: break
      Add(card, hand)                                                                           // appended to END of hand; AfterCardChangedPiles(Draw->Hand)
      History.CardDrawn(card, fromHandDraw)
      Hook.AfterCardDrawn(card, fromHandDraw)                                                   // pass 1: AfterCardDrawnEarly for all listeners, pass 2: AfterCardDrawn for all
      card.InvokeDrawn()                                                                        // event only
      room = max(0, 10 - hand.Count)
```
Draws never lose cards (STS1-style "burn" does not exist): at 10 cards drawing just stops. Reshuffle timing = lazily at the moment a card is needed and the draw
pile is empty (so "draw 3 with 1 card left" draws 1, reshuffles, draws 2 more).
`AfterCardDrawn` hook ordering: `Hook.cs:202-220`.

### 6.4 Exhaust / discard destinations
Exhaust pile and discard pile always append (`Bottom`). No position/size limits. `Add(card, Hand)` at 10 -> discard (6.2).

### 6.5 Generated cards (`AddGeneratedCardsToCombat`, `CardPileCmd.cs:281-314`)
```
for card in cards (order):
    History.CardGenerated(card, creator)
    Add(card, pile(card.owner, newPileType), position)           // full Add semantics incl. hand-full redirect, AfterCardEnteredCombat, AfterCardChangedPiles(None->pile)
    Hook.AfterCardGeneratedForCombat(card, creator)
```
Preconditions: combat in progress; card has NO pile yet; target is a combat pile. `AddToCombatAndPreview<T>(target, pile, count, creator, position)`
(`:1214-1252`) creates `count` fresh cards via `combatState.CreateCard<T>(player)` and adds each with `AddGeneratedCardToCombat` (status cards etc.).
`CombatState.CreateCard(canonical, owner)` = `ToMutable()` + register + `AfterCreated()` (`Combat/CombatState.cs:176-181`); the only `AfterCreated` override is `SpoilsMap`.
`CardPilePosition.Random` users (Draw pile insert): BlessedAntler, BiiigHug, FuneraryMask, TeaOfDiscourtesy, SoulboundPower, PersonalHivePower, SoulFysh,
Noisebot, TheInsatiable, CaptureSpirit, Dirge, GraveWarden, Metamorphosis, Reave, GlimpseBeyond, Severance, TheBall(result).

### 6.6 Transform (`CardCmd.Transform`, `CardCmd.cs:374-535`; `Entities/Cards/CardTransformation.cs`; `Factories/CardFactory.cs:170-217`)
```
Transform(transformations[], rng):
  if combat ending: return
  for t in transformations:   original must be transformable; pile = original.Pile; idx = pile.IndexOf(original)
        replacement = t.Replacement ?? CardFactory.CreateRandomCardForTransform(original, isInCombat, rng)      // consumes rng (see below)
        original.RemoveFromCurrentPile()                                                                       // all originals removed first
  sort (pile.Type enum value, original index) ascending                                                          // so reinserts keep relative order
  for each: pile.AddInternal(replacement, idx)  [combat: also History.CardGenerated, Hook.AfterCardEnteredCombat]; 
            Hook.AfterCardChangedPiles(replacement, pile.Type[as 'old'])        // note: passed oldPile = the same pile type
            original.AfterTransformedFrom(); replacement.AfterTransformedTo()
  then for combat piles Hook.AfterCardGeneratedForCombat(replacement); original.RemoveFromState()
```
In COMBAT piles the replacement keeps the SAME INDEX in the SAME pile (hand position preserved). For the DECK pile `CardCmd.Transform` calls `pile.AddInternal(replacement)` with no index (`CardCmd.cs:443`), i.e. the replacement is APPENDED to the end of the deck; deck order is the input to the initial shuffle (7.2), so the env must take the deck list in the game's exact order (starting deck order, then acquisition order, with transformed/gained cards at the end). It is a fresh canonical clone: NOT upgraded, no enchantment, no cost mods.
`CreateRandomCardForTransform` candidates: pool = `original.Pool` unless original is Quest/Event/Ancient/Token rarity -> Colorless pool; take
`pool.GetUnlockedCards(...)` (pool array order), keep rarities Common/Uncommon/Rare unless original is Status/Curse (rarity 8/9 -> no rarity filter),
in combat also `CanBeGeneratedInCombat`, drop same Id, single-player filter; error if empty; pick `rng.NextItem(candidates)` = ONE draw (`NextInt(0,n)`).
In combat the RNG passed is `CombatCardSelection` (EntropyPower). Out of combat relics pass `Niche`.

### 6.7 Upgrade / downgrade in combat (`CardCmd.Upgrade`, `:256-300`; `CardModel.UpgradeInternal`, `:2129-2136`)
`if !IsUpgradable skip` (`level < MaxUpgradeLevel`); `UpgradeInternal` (`level++`, `OnUpgrade()`, recalc calculated vars) then `FinalizeUpgradeInternal`.
Only the combat card changes (deck copy untouched). No hooks. `OnUpgrade` implementations use `DynamicVars.X.UpgradeValueBy(+n)`,
`EnergyCost.UpgradeBy(-n)`, `AddKeyword/RemoveKeyword`, `UpgradeStarCostBy`. Downgrade (`:2149-2162`) rebuilds from the canonical model.

---------------------------------------------------------------------------------------------------

## 7. Shuffling (byte-exact)

### 7.1 RNG primitive (`Random/MegaRandom.cs`, `Random/Rng.cs`)
`MegaRandom` = xoshiro256** variant: state `s0..s3` seeded by 4x SplitMix64 (`:83-104`):
```
splitmix64(x): x += 0x9E3779B97F4A7C15; z = x; z = (z ^ (z>>30)) * 0xBF58476D1CE4E5B9; z = (z ^ (z>>27)) * 0x94D049BB133111EB; return z ^ (z>>31)
seed(s): s0=splitmix64(&s); s1=splitmix64(&s); s2=splitmix64(&s); s3=splitmix64(&s)
next_u64():  result = rotl(s1*5, 7) * 9;  t = s1<<17;  s2^=s0; s3^=s1; s1^=s2; s0^=s3; s2^=t; s3=rotl(s3,45); return result
next_double(): (next_u64() >> 11) as f64 * 2^-53      (1.1102230246251565E-16)
Next(max)         = (int)(next_double() * max)                       (max >= 1 else throws)      // MegaRandom.NextInner :296
Next(min,max)     = (int)(next_double() * (max-min)) + min
```
`Rng.NextInt(max)` = `_random.Next(max)` + `counter++` (`Rng.cs:83-90`); `Rng.NextInt(min,max)` throws if `min >= max` (`:95-103`);
`NextItem(seq)`: empty -> default WITHOUT consuming; else `NextInt(0, count)` -> `seq.ElementAt(i)` (`:289-300`) - consumes exactly 1 draw even for count 1.
The counter is only bookkeeping for saves.
Run RNG streams (`Runs/RunRngSet.cs`): one `Rng` per `RunRngType` (UpFront, Shuffle, UnknownMapPoint, CombatCardGeneration, CombatPotionGeneration,
CombatCardSelection, CombatEnergyCosts, CombatTargets, MonsterAi, Niche, CombatOrbs, TreasureRoomRelics), each `new Rng(runSeed + xxhash64_utf8(snake_case(name)), seed 0)`
(`RunRngSet.cs:CreateRng`, `Random/Rng.cs:Rng(ulong,string)`, `Helpers/StringHelper.cs:139-152`). `runSeed = xxhash64(seedString)` (or the legacy hash for "old"-prefixed seeds).
These are RUN-LEVEL: their state carries over from one combat to the next; the sim's env must be able to load/set all 4 u64 words per stream at combat start.
(Another spec covers RNG in depth; this doc only lists consumption points.)

### 7.2 Initial combat shuffle (`Entities/Players/Player.cs:806-815`, `Entities/Cards/CardPile.cs:69-74`)
```
draw_pile = [clone(c) for c in deck.cards (deck order)]                 // deck order = order cards were added to the deck
UnstableShuffle(draw_pile, Rng.Shuffle)                                  // NO sort
TestRngInjector (tests only)
Hook.ModifyShuffleOrder(player, draw_pile, isInitialShuffle=true)        // PerfectFit ignores initial shuffles
```
`UnstableShuffle` (`Extensions/ListExtensions.cs:45-62`) - Fisher-Yates from the end:
```
n = len
while n > 1:  n -= 1;  j = rng.NextInt(n + 1);  swap(list[j], list[n])       // exactly n_total-1 draws
```
### 7.3 Reshuffle (`CardPileCmd.Shuffle`, `:1076-1136`) and `StableShuffle`
```
Shuffle(player):
  if combat over/ending: return
  list = discard.cards (in order) ++ draw.cards (in order)             // draw pile is normally empty; if not, its cards are appended AFTER the discard cards
  list = StableShuffle(list, run.Rng.Shuffle)
  Hook.ModifyShuffleOrder(player, list, isInitialShuffle=false)        // PerfectFit: for each (listener order) remove its card, insert at index 0
  (debug: forced top card)
  Add(list, drawPile, Bottom)                                          // every card is removed from its old pile and appended in this order; index 0 = top
  Hook.AfterShuffle(player)                                            // only if combat not ending
```
`AfterCardChangedPiles(Discard->Draw)` fires for the previously-discarded cards only (draw-pile cards stay in the Draw type).
`StableShuffle` (`ListExtensions.cs:22-31`) is NOT stable: it copies the list, calls `List<T>.Sort()` (the default comparer = `CardModel.CompareTo`,
section 1.4) and copies the result back, THEN `UnstableShuffle`. `List<T>.Sort` is .NET's IntroSort, which is unstable and which must be replicated for equal-comparing cards.
Verified algorithm (identical to `ArraySortHelper<T>.IntrospectiveSort`, .NET 9):
```
sort(keys):  n=len; if n<2 return; intro(0, n-1, 2*(floor(log2(n))+1))
intro(lo, hi, depth):
  while hi > lo:
    size = hi-lo+1
    if size <= 16:
        if size==2: swap_if_greater(lo,hi); return
        if size==3: swap_if_greater(lo,hi-1); swap_if_greater(lo,hi); swap_if_greater(hi-1,hi); return
        insertion_sort(lo,hi); return
    if depth == 0: heap_sort(lo,hi); return
    depth -= 1
    p = pick_pivot_and_partition(lo,hi)
    intro(p+1, hi, depth)                  // recurse right
    hi = p-1                               // loop on left
swap_if_greater(i,j): if i!=j && cmp(k[i],k[j]) > 0: swap
insertion_sort(lo,hi): for i in lo..hi-1 { t=k[i+1]; j=i; while j>=lo && cmp(t,k[j])<0 { k[j+1]=k[j]; j-=1 }; k[j+1]=t }
pick_pivot_and_partition(lo,hi):
    mid = lo + ((hi-lo)>>1)
    swap_if_greater(lo,mid); swap_if_greater(lo,hi); swap_if_greater(mid,hi)
    pivot = k[mid]; swap(mid, hi-1)
    left=lo; right=hi-1
    while left < right:
        left+=1;  while cmp(k[left], pivot) < 0: left+=1
        right-=1; while cmp(pivot, k[right]) < 0: right-=1
        if left >= right: break
        swap(left,right)
    if left != hi-1: swap(left, hi-1)
    return left
heap_sort(lo,hi): n=hi-lo+1; for i in n/2 down to 1: down_heap(i,n,lo); for i in n down to 2: swap(lo,lo+i-1); down_heap(1,i-1,lo)
down_heap(i,n,lo): d=k[lo+i-1]; while i <= n/2 { c=2i; if c<n && cmp(k[lo+c-1],k[lo+c])<0: c+=1; if !(cmp(d,k[lo+c-1])<0): break; k[lo+i-1]=k[lo+c-1]; i=c }; k[lo+i-1]=d
cmp(a,b) = (Id(a) ordinal-cmp Id(b)), then upgradeLevel cmp          // 1.4
```
Note the 3-element network and the pivot swap move equal elements, so even tiny lists (e.g. exhausting 3 identical Strikes) are permuted. After the sort,
`UnstableShuffle` runs over the sorted list with `Rng.Shuffle`.
`StableShuffle` is also used by random-pick effects (SeekerStrike, StoneCracker, PowerCell, Catastrophe, BeatDown, Uproar,...): same sort-then-shuffle, then `First`/`Take(k)` (section 10).

### 7.4 `ShuffleIfNecessary` (`:1189-1198`)
`if draw.empty && !discard.empty: Shuffle()`. Called per card inside Draw and AutoPlayFromDrawPile (and by effects that read the draw pile after calling it, e.g. Havoc via 5.3).
The explicit `Shuffle` card effects (Reboot `Models/Cards/Reboot.cs:29`, BottledPotential) call `Shuffle` directly: with a NON-empty draw pile the draw cards join the sort (appended after discard cards).

### 7.5 Turn-1 draw-pile ordering and the opening hand (`Combat/CombatManager.cs:877-926`)
```
SetupPlayerTurn(player):
  energy: ResetEnergy() (to MaxEnergy via Hook.ModifyMaxEnergy) unless Hook.ShouldPlayerResetEnergy==false (IceCream: ADD max energy to current)
  Hook.AfterEnergyReset
  Hook.BeforeHandDraw
  handDraw = Hook.ModifyHandDraw(5)                      // two passes: ModifyHandDraw then ModifyHandDrawLate; casts to int per step for the "modified" bookkeeping only
  if player.TurnNumber == 1:
        bottom = [c in draw if c.Enchantment?.ShouldStartAtBottomOfDrawPile]  (Imbued)   ; for c in bottom: draw.MoveToBottom(c)       // in pile order
        innate = [c in draw if c.Keywords has Innate] \ bottom                                                                         // in CURRENT pile order
        for c in innate: draw.MoveToTop(c)                                                                                             // each goes to index 0 => innate cards end up in REVERSED relative order
        handDraw = min(10, max(handDraw, innate.len))
  Draw(handDraw, fromHandDraw=true)
  Hook.AfterPlayerTurnStart
```
Imbued: `AfterAutoPrePlayPhaseEntered` auto-plays the card on turn 1 from wherever it is (hand or bottom of the draw pile) (`Models/Enchantments/Imbued.cs`).
The first player turn has `TurnNumber == 1`; `IncrementTurnNumber` happens elsewhere (see combat-flow spec).

---------------------------------------------------------------------------------------------------

## 8. End of player turn (card parts) (`Combat/CombatManager.cs:1512-1830`)

Order for a normal end of the player's turn (single player):
1. `AutoPostPlay` phase hooks; `Hook.BeforeSideTurnEnd`.
2. `DoTurnEnd` (`:1599-1631`): orb `BeforeTurnEnd`; then over the hand IN ORDER:
   cards with `HasTurnEndInHandEffect` (Burn, Decay, Regret, Doubt, Debt, Shame, BadLuck, Infection, Toxic, Wither, Beckon,...) go to `turnEndCards`;
   `else if` the card has Ethereal (global keywords included) and `Hook.ShouldEtherealTrigger(card)` -> `etherealCards`.
   First ALL ethereal cards are exhausted (`CardCmd.Exhaust(..., causedByEthereal:true)`, which fire `AfterCardExhausted`), THEN `DoTurnEndCards`:
   for each turn-end card in order: `CardPileCmd.Add(card, Play)`, `card.OnTurnEndInHand()` (damage etc.), then `Add(card, Discard)` (or `Exhaust(causedByEthereal)` if it is also Ethereal).
   Ethereal exhaust is NOT stopped by Retain. A card with a turn-end effect is not checked for Ethereal in the first loop (`else if`) - it is exhausted at the end of its own resolution if Ethereal.
3. `CheckWinCondition`; `Hook.BeforeFlush(player)` (`BeforeFlush` pass then `BeforeFlushLate`; e.g. SlumberingEssence reduces cost of cards in hand).
4. Player-turn phase two - `FlushPlayerHand` (`:1785-1820`): `flush = Hook.ShouldFlush(player)` (veto: RunicPyramid, RetainHandPower, RingingTriangle, WellLaidPlansPower);
   for each hand card in order: if `!flush || card.ShouldRetainThisTurn` -> retain else flush. `CardPileCmd.Add(flushed, Discard)` (bottom, in hand order; fires `AfterCardChangedPiles`; does NOT trigger Sly; does not
   fire `AfterCardDiscarded`). `Hook.AfterFlush(flushed, retained)`. Then step 5.
5. `EndOfTurnCleanup` (inside `FlushPlayerHand`, i.e. right after `AfterFlush`): `PlayerCombatState.EndOfTurnCleanup()` (`PlayerCombatState.cs:268-274`) -> for every card in ALL piles (Hand, Draw, Discard, Exhaust, Play): `card.EndOfTurnCleanup()` (`CardModel.cs:1611-1624`):
   `ExhaustOnNextPlay=false`, clear single-turn Retain and Sly, drop EndOfTurn local energy modifiers, drop temp star costs with ClearsWhenTurnEnds.
   Afterwards `Hook.AfterSideTurnEnd` fires for the player side (`CombatManager.cs:1746-1760`); this is where Burst/Duplication/OneTwoPunch/Rebound-style powers remove themselves, i.e. AFTER the card cleanup above.
6. It is called AGAIN at the end of the enemy side's turn (`EndEnemyTurnInternal`, `CombatManager.cs:1696-1706`, between `BeforeSideTurnEnd` and `AfterSideTurnEnd`), so "this turn" modifiers added during
   the enemy turn also expire. Net: modifiers last until the end of whichever side's turn they were created in.
Retained cards stay in hand in their order; new turn draw appends after them. Hand size cap still applies.

---------------------------------------------------------------------------------------------------

## 9. Decision points (player choices)

Choices go through `CardSelectCmd` (`Commands/CardSelectCmd.cs`). Test/automation hook: `CardSelectCmd.Selector` (an `ICardSelector.GetSelectedCards(options, min, max)`)
short-circuits the UI/network; this is the natural place for the Sim's "agent" decision callback.

### 9.1 `CardSelectorPrefs` (`CardSelection/CardSelectorPrefs.cs`)
`(prompt, selectCount)` = `(min=max=count)`; `(prompt, min, max)`. `RequireManualConfirmation = (min >= 0 && min != max)` by default (overridable by `init`).
Optional extras (`Cancelable`, `Comparison`, `ShouldGlowGold` ...) are presentational. `max = 999999999` means "any number".
### 9.2 Auto-resolution rule (all FromX): after building the option list `L`
```
if L is empty:                                   return []          (no decision)
if !RequireManualConfirmation && |L| <= min:     return L           (all, in L order, no decision)      // i.e. "choose exactly N" with <= N candidates
else                                             DECISION: choose a subset S of L with min <= |S| <= max
```
`FromHandForUpgrade` auto-resolves if `|L| <= 1`. `FromChooseACardScreen` throws if more than 3 cards, returns null for 0.
`FromSimpleGrid/FromHand/FromCombatPile` return `[]` instantly when combat is over/ending.
### 9.3 Catalogue of decisions the combat sim must expose
| Kind | API | Options list `L` and filter | min..max | Used by (examples) |
|---|---|---|---|---|
| Play a card from hand / end turn | `PlayCardAction` / `EndPlayerTurnAction` | hand cards passing `CanPlay`, with a valid target (`IsValidTarget`) | - | core loop |
| Choose from HAND | `FromHand(prefs, filter)` | hand order (pile order), `filter` predicate | (N,N) exact or (0,k)/(0,inf) optional | BurningPact/TrueGrit/Brand/Scavenge (exhaust 1), Begone/Transfigure (transform 1), DualWield (Attack/Power), HandTrick (non-Sly Skill), Snap (non-Retain), SculptingStrike (non-ethereal), HeirloomHammer (colorless), DecisionsDecisions (playable Skill), Nightmare/ThinkingAhead/PhotonCut/Glimmer (put back N to draw TOP), Purity (0..k exhaust), Guards (0..inf), TyrannyPower/EntropyPower/ToastyMittens relic (effect-driven choices), potions Ashwater/TouchOfInsanity (cost>0) |
| Choose from hand to DISCARD | `FromHandForDiscard` (= FromHand + gold glow on Sly) | hand order | (N,N) / (0,inf) | Survivor, Acrobatics, Prepared, HiddenDaggers, DaggerThrow, ToolsOfTheTrade, GamblingChip, GamblersBrew |
| Choose from hand to UPGRADE | `FromHandForUpgrade` | hand cards with `IsUpgradable`; auto if <=1 | exactly 1 | Armaments |
| Choose from a COMBAT PILE | `FromCombatPile(pile, prefs, filter)` | pile.Cards filtered; **for the Draw pile the options are presented sorted by (Rarity, Id)** (stable `OrderBy`, `CardSelectCmd.cs:488-493`, `:501-505`) to hide draw order; Discard/Exhaust in pile order | (N,N) or (0,N) | Headbutt/Hologram/Graveblast/CosmicIndifference (discard, 1), Dredge/NeowsFury (discard -> hand), Tutor/Wish/Charge/Seance/Cleanse/SecretTechnique(Skill)/SecretWeapon(Attack)/SeekerStrike(subset of draw)/ForegoneConclusion/Stratagem (draw), potions DropletOfPrecognition/LiquidMemories |
| Choose 1 of <=3 generated cards (Discovery-style) | `FromChooseACardScreen(cards, canSkip)` | the 3 cards were already generated (RNG consumed BEFORE the decision); skip -> null | 0/1 (skip if allowed) | Discovery, Abundance, Splash, Quasar, potions Attack/Skill/Power/Colorless, relics Toolbox/HeftyTablet/MassiveScroll/LeadPaperweight, KnowledgeDemon |
| Choose 1 bundle | `FromChooseABundleScreen` | list of card lists | 1 bundle | ScrollBoxes (non-combat) |
| Simple grid | `FromSimpleGrid` / `...ForRewards` | caller list / reward card list | per prefs | ChoicesParadox, SeaGlass, SealedDeck (non-combat) |
| Deck selections (non-combat) | `FromDeckForUpgrade/Transformation/Removal/Enchantment/Generic` | deck order; Removal sorts Curses first (key -999999999) then deck index; Transform excludes Quest and non-transformable; Enchant lists deck-index order | per prefs | events/relics/rest sites - out of scope for combat but present in the engine |
Remote/replay variants exist for multiplayer; the Sim ignores them.
### 9.4 Result ordering
Selections return `IEnumerable<CardModel>`; order = the order the UI delivered. In-hand selection (`NPlayerHand.SelectCards`) returns CLICK order
(`_selectedCards` is a List; when `MaxSelect` is already reached the most recently selected card is deselected first, `Nodes/Combat/NPlayerHand.cs:1159-1166`);
pile/grid screens return a `HashSet` (insertion order, caveat: removals free slots). Auto-resolved choices return the options in list order. Order matters when the effect then does
`Add(..., Top)` per card (each goes to index 0, so the LAST processed card ends on top: PhotonCut/Glimmer put-back, Dredge/NeowsFury -> Hand append order) or discards in order.
Sim convention (recommended): the agent returns an ORDERED list; when a bot returns a set, canonicalise to the options-list order. FLAG: the real game's order is player-click order (section 16).

---------------------------------------------------------------------------------------------------

## 10. Random card generation in combat (`Factories/CardFactory.cs`)

Combat generation is UNIFORM (no rarity weights): rarity weights/odds (`PlayerOdds.CardRarity`, `RollForRarity`, `RollForUpgrade`) are only used by
`CreateForReward`/`CreateForMerchant`, which in this build are called from relics only (ArcaneScroll, HeftyTablet, MassiveScroll, LastingCandy, LeadPaperweight, Kaleidoscope, SeaGlass) and modifiers (non-combat). 

### 10.1 Filters
`FilterForCombat(cards)` (`:159-162`): `CanBeGeneratedInCombat && Rarity not in {Basic, Ancient, Event}` then `.Distinct()` (canonical singletons; order preserved).
`CanBeGeneratedInCombat == false` (cards that heal etc.): Alchemize, AscendersBane, Abundance, Disintegration, Eidolon, NotYet, Soot, MindRot, Feed, WasteAway, HiddenGem,
Transfigure, NeowsFury, FranticEscape, Royalties, TheHunt, Nightmare, Sloth, HandOfGreed (`grep "CanBeGeneratedInCombat => false" Models/Cards`).
NOTE: Token/Status/Curse rarities are NOT filtered by `FilterForCombat`; they are excluded only because callers pass character/colorless pools which don't contain them.
`FilterForPlayerCount`: single-player drops `MultiplayerOnly` cards (`:25-32`).
Pool arrays: `Models/CardPools/*CardPool.cs` (`GenerateAllCards`, alphabetical by class name; Ironclad/Silent/Defect/Necrobinder/Regent 90-91 each, Colorless 65, Curse 18,
Status 12, Token 14, Event 28, Quest 4). The generation source order is the pool's array order after `GetUnlockedCards` (`Models/CardPoolModel.cs`, `FilterThroughEpochs`
removes cards whose unlocking epoch has not been revealed - e.g. Ironclad2/5/7Epoch). **UNLOCK STATE IS THEREFORE AN INPUT TO THE SIM** (assume "everything unlocked" unless told otherwise; the candidate
list must be identical to the real run for RNG reproducibility).
### 10.2 Algorithms and RNG consumption
```
GetDistinctForCombat(player, pool, count, rng):                    // Discovery, InfernalBlade, Distraction, WhiteNoise, Abundance, Quasar, BundleOfJoy, potions, relics...
   cards = FilterForCombat(FilterForPlayerCount(pool_after_caller_where))
   L = cards.ToList(); UnstableShuffle(L, rng)    // FULL Fisher-Yates over |L|: exactly |L|-1 draws, independent of `count` (0 draws if |L|<=1)
   take first `count`; each is turned into a mutable card via CombatState.CreateCard(canonical, player) (lazy; materialised by ToList/First)
GetForCombat(player, pool, count, rng):                           // WITH replacement: Stoke, Metamorphosis, Jackpot, CalamityPower
   options = FilterForCombat(pool).ToList() then FilterForPlayerCount
   repeat count: canonical = rng.NextItem(options)  (1 draw each, possibly the same card twice); CreateCard
CreateRandomCardForTransform(...) : see 6.6 (one NextItem draw)
```
Callers pass `player.RunState.Rng.CombatCardGeneration` for all of the above (Discovery family, potions, relics, powers: HelloWorld, CreativeAi, Calamity, SpectrumShift, CallOfTheVoid; cards: see `grep GetDistinctForCombat`). Call-site `where` filters (e.g. Attack only for InfernalBlade) are applied before the shuffle, which changes `|L|` and therefore the number of draws.
Cards given by these helpers then typically get: `SetToFreeThisTurn()` (Discovery/InfernalBlade), `CardCmd.Upgrade` (Stoke+), `ApplyKeyword(Ethereal)` (CallOfTheVoid), then `AddGeneratedCardToCombat`.
### 10.3 Other random card picks (stream and algorithm)
| Stream | Effect -> algorithm |
|---|---|
| `Shuffle` | initial shuffle; reshuffle; `Add(position=Random)` index `NextInt(Count+1)`; Catastrophe/BeatDown/Uproar: `ToList().StableShuffle(Shuffle)` then take First/`Take(k)`; Stampede `NextItem` |
| `CombatCardSelection` | Cinder/TrueGrit-random/Thrash/HiddenGem/Bookmark/JeweledMask/ImprovementPower/MummifiedHand/`AutoPlayFromDrawPile(Random)`: `NextItem(list)`; Anointed/DrainPower: `TakeRandom(k)` (= full UnstableShuffle then Take); SeekerStrike/PowerCell/StoneCracker: `StableShuffle` then Take; AggressionPower: `UnstableShuffle`+Take; EntropyPower transform |
| `CombatCardGeneration` | all `GetDistinct/GetForCombat`; `AfflictionModel.PickRandomTargets` (`Models/AfflictionModel.cs:244`: filter `CanAfflict`, UnstableShuffle, trim to count) |
| `CombatTargets` | random enemy/ally: `NextItem(HittableEnemies)` for AutoPlay AnyEnemy default target, BouncingFlask, BeatDown, Juggernaut, Cacophony, Countdown, Haunt, SerpentForm, relics Kusarigama/Tingsha/..., TheBall (`NextItem(list).Player`) |
| `CombatEnergyCosts` | `NextInt(4)` (0..3): Slither enchantment on draw, ConfusedPower, SneckoOil |
| `Niche` | out-of-combat relic/event randomness (not combat) |
Mutations of a returned pile snapshot (e.g. `PileType.Draw.GetPile(Owner).Cards.Where(...).ToList()`) are taken at call time; the filtered list order = pile order (draw index 0 first) before any sort/shuffle.

---------------------------------------------------------------------------------------------------

## 11. Upgrades and DynamicVars

* Dynamic vars are per-card named numeric values (`Localization/DynamicVars/*`): `DamageVar(base, ValueProp)`, `BlockVar`, `CardsVar`, `EnergyVar`, `StarsVar`,
  `PowerVar<T>`, `HpLossVar`, `HealVar`, `GoldVar`, `RepeatVar`, `OstyDamageVar`, `ForgeVar`, `SummonVar`, `CalculatedDamage/Block` (`CalculationBase + CalculationExtra * multiplier(card, target)`),
  generic `IntVar`/`DynamicVar(name, v)`. `BaseValue` (clamped to 999999999) is the gameplay value; `IntValue = (int)BaseValue`.
* `OnUpgrade()` mutates `BaseValue` via `UpgradeValueBy(addend)` (e.g. Strike +3 damage, Bash +2 damage / +1 vulnerable) and costs/keywords; `CurrentUpgradeLevel` increments; calculated vars are re-derived (`DynamicVarSet.RecalculateForUpgradeOrEnchant`).
  `MaxUpgradeLevel` default 1; some cards 0 (Normality, curses) or >1.
* Card effects read `DynamicVars.X.BaseValue` and pass them to commands (`DamageCmd.Attack(base).FromCard(this, cardPlay)...`); powers/relics/enchantments modify them through hooks at the time of damage/block/power application
  (`PreviewValue`/`EnchantedValue` are display-only caches; the sim does not need them).
* Order of enchantment vs powers in damage/block calculation (`Hooks/Hook.cs:1322-1356, 1495-1521`):
  `v = base; if card.Enchantment: v += E.additive(v); v *= E.multiplicative(v);` THEN the normal power/relic additive pass then multiplicative pass. Both enchantment terms apply only for powered attacks (`ValueProp.Move` and not `Unpowered`).
  Block: `Hook.ModifyBlock` same pattern with `EnchantBlockAdditive/Multiplicative` and final `max(0, v)`.
* Replays/play counts: `Enchantment.EnchantPlayCount(BaseReplayCount)` adds to the base (Spiral +Times permanently, Glam +Times until used once per combat).
* `CardModel.UpdateDynamicVarPreview`/`GetDescriptionForPile` are UI only.

### 11.1 ValueProp (`ValueProps/ValueProp.cs`, `ValuePropExtensions.cs`)
Flags carried by every damage/block/HP-loss instance: `Unblockable = 2`, `Unpowered = 4`, `Move = 8`, `SkipHurtAnim = 0x10` (display).
* `Move` = the amount comes from a card play or a monster move (as opposed to a power tick / relic / environment).
* `IsPoweredAttack() = Move && !Unpowered` (also `IsPoweredCardOrMonsterMoveBlock`): only these are boosted by Strength-style additive/multiplicative modifiers and by enchantments.
* `IsCardOrMonsterMove() = Move`. `Unblockable` bypasses Block. Card damage vars are `ValueProp.Move` (normal attack) ; "pure" HP loss from cards (Corrupted, Burn/Regret) uses `Unblockable | Unpowered | Move`.
* Cards pass `DamageVar.Props`/`BlockVar.Props` into `DamageCmd.Attack(...).FromCard(this, cardPlay)` / `GainBlock(dynamicVars.Block, cardPlay)`; the details of damage resolution belong to the combat-damage spec.

---------------------------------------------------------------------------------------------------

## 12. Enchantments, Afflictions, Modifiers

### 12.1 Base behaviour (`Models/EnchantmentModel.cs`)
* One enchantment max per card; stackable only if the same type and `IsStackable` (no shipped enchantment is stackable).
  `CanEnchant`: type not Status/Curse/Quest, `CanEnchantCardType`, not an Unplayable card in the deck, no existing enchantment. `Amount` is the stack/strength value; `Status` Normal|Disabled.
* Application order: `EnchantInternal` -> `ApplyInternal` -> `ModifyCard()` = `OnEnchant()` + `RecalculateValues()` + recalc card dynamic vars (`CardCmd.Enchant`, `CardCmd.cs:536-575`).
* Enchantments are hook listeners ONLY while their card sits in a combat pile (`ShouldReceiveCombatHooks`, `CombatState.Contains`) and the owner is active.
* Hook points used by the base: `OnPlay` (inside the replay loop, after card `OnPlay`), `EnchantDamageAdditive/Multiplicative`, `EnchantBlockAdditive/Multiplicative`, `EnchantPlayCount`, `ShouldStartAtBottomOfDrawPile`, `OnEnchant`, plus any `AbstractModel` hook (AfterCardPlayed, AfterCardDrawn, BeforeFlush, ModifyShuffleOrder, AfterAutoPrePlayPhaseEntered).
* Deck enchantments are cloned into the combat card at combat start; state changes inside combat (Status=Disabled, Momentum extra damage, Glam UsedThisCombat) affect only the combat clone (so they reset next combat) - except Goopy which writes through `DeckVersion`.

### 12.2 The 23 shipped enchantments (+ test mock `MockFreeEnchantment`)
| Enchantment | Applicable cards | Effect in combat |
|---|---|---|
| Adroit | any (not curse/status) | OnPlay: gain `Amount` block via `GainBlock(DynamicVars.Block, cardPlay)` (runs through block hooks) |
| Clone | any | no logic (marker; PaelsGrowth) |
| Corrupted | Attack | damage x1.5 (powered attacks); OnPlay: owner takes 2 unblockable unpowered damage |
| DeprecatedEnchantment | - | none |
| Glam | any | replay +`Times`(1) until the first completed play iteration this combat (`UsedThisCombat` set in `AfterCardPlayed`, Status -> Disabled; `EnchantPlayCount` stops adding afterwards). The play count of the CURRENT play was already fixed before its loop, so all of its replays still happen |
| Goopy | Defend-tagged | OnEnchant adds Exhaust; `AfterCardPlayed` (fires once per replay iteration, `CardModel.cs:1963-1969`) does `Amount++` on the card AND `DeckVersion.Enchantment` (permanent), so later replays of the same play already gain more; block additive = `Amount - 1` |
| Imbued | Skill | starts at BOTTOM of draw pile (turn-1 setup); `AfterAutoPrePlayPhaseEntered` on turn <=1: auto-play the card (any pile) |
| Inky | any | OnPlay: apply Weak (1) to target / all hittable enemies for AllEnemies |
| Instinct | Attack | damage x2 (powered attacks) |
| Momentum | Attack | OnPlay: `ExtraDamage += Amount` (accumulates every play incl. replays, combat-only); damage additive `+ExtraDamage` |
| Nimble | cards with `GainsBlock` | block additive `+Amount` |
| PerfectFit | any | on RESHUFFLE only (`!isInitialShuffle`): moves its card to index 0 (top) of the shuffled list; several -> last listener processed ends on top |
| RoyallyApproved | Attack/Skill | OnEnchant adds Innate + Retain |
| Sharp | Attack | damage additive `+Amount` |
| Slither | any playable non-X | AfterCardDrawn (when drawn into hand): `EnergyCost.SetThisCombat(CombatEnergyCosts.NextInt(4))` (adds a new Absolute end-of-combat modifier per draw; last wins) |
| SlumberingEssence | any | BeforeFlush: if card in hand, `EnergyCost.AddUntilPlayed(-1)` (relative) |
| SoulsPower | cards with a LOCAL Exhaust keyword | OnEnchant removes Exhaust |
| Sown | any | OnPlay: once (Status Normal -> Disabled) gain `Amount` energy |
| Spiral | basic Strike/Defend | replay +`Times`(1) permanently |
| Steady | any | OnEnchant adds Retain |
| Swift | any | OnPlay: once (Status Normal -> Disabled) draw `Amount` |
| TezcatarasEmber | any | OnEnchant: base cost -> 0 (`UpgradeBy(-cost)`) and add Eternal; damage additive `+3` |
| Vigorous | Attack | damage additive `+Amount` while Status Normal; `AfterCardPlayed` (per replay iteration) sets Disabled, so ONLY the first iteration (replay index 0) gets the bonus; replays get none (combat clone only, re-enabled next combat) |

### 12.3 Modifiers (`Models/Modifiers/*`) - run-level game-mode flags
Combat-relevant: `Murderous` (+3 Strength to every creature entering the room and each enemy added later), `Terminal` (5 Plating to players at combat start; max-HP loss at run start),
`Hoarder` (on a card entering the DECK from no pile, adds 2 clones). Run/map/reward-only: AllStar, BigGameHunter, CharacterCards, CursedRun, DeadlyEvents, Draft, Flight, Insanity,
Midas, NightTerrors, SealedDeck, Specialized, Vintage (they generate decks/rewards or change map/rest/reward rules). Modifiers are hook listeners (end of `IterateHookListeners`).

### 12.4 Afflictions (`Models/Afflictions/*`, `Models/AfflictionModel.cs`)
Per-card flag with `Amount`, applied by powers (`CardCmd.Afflict`, `CardCmd.cs:629-668`; vetoable by `Hook.ShouldAfflict`; also `CanAfflict`: type/unplayable/stacking).
Bound (ChainsOfBinding: one Bound card playable per turn; cleared at side turn end), Ringing (RingingPower: only the first card each turn is playable), Smog (SmoggyPower: unplayable),
Hexed (HexPower: Ethereal via global keyword; clears itself if Hex gone on re-enter), Entangled (TangledPower: energy cost modification), Galvanized (GalvanicPower), Tainted (VitalSparkPower; Skills only).
Afflictions are listeners (after their card in iteration), `OnPlay(ctx, target)` runs in the replay loop after the enchantment. Their gameplay is mostly implemented in the corresponding power.

---------------------------------------------------------------------------------------------------

## 13. Hook listener order and card-relevant hooks

`ICombatState.IterateHookListeners()` (`Combat/CombatState.cs:411-479`): builds a SNAPSHOT list then yields each element only if still `Contains` (alive / not removed) at yield time:
```
for creature in allies ++ enemies:                     // allies (players, pets) first, in list order; then enemies in slot order
    creature.Powers (in order)
    if creature is not a player: creature.Monster
    else if player.IsActiveForHooks:
        relics (not melted), potions (non-empty slots), orb queue orbs,
        for pile in [Hand, Draw, Discard, Exhaust, Play]:
            for card in pile (current order): card, then card.Affliction (if any), then card.Enchantment (if any)
then Modifiers, BadgeModels, MultiplayerScalingModel
```
`Hook.IterateCombatHookListeners` yields nothing when combat is over/ending (except during combat setup); selected hooks (`AfterCardPlayed`, `AfterBlockBroken`, kill/death hooks) bypass the guard so the killing blow still resolves (`Hooks/Hook.cs:34-61`).
Because the snapshot is taken at the start of each hook dispatch, cards that move piles inside a hook still get called at their ORIGINAL position (but are skipped if `HasBeenRemovedFromState`).
Early/Late pairs: `AfterCardChangedPiles(+Late)`, `AfterCardDrawn(Early, normal)`, `AfterCardPlayed(+Late)`, `BeforeFlush(+Late)`, `AfterEnergyReset(+Late)`, `TryModifyEnergyCostInCombat(+Late)`, `ModifyHandDraw(+Late)`, `ModifyCardRewardCreationOptions(+Late)`.
Card-pipeline hooks and their call sites: `ShouldPlay` (CanPlay/AutoPlay), `AfterEnergySpent`, `AfterStarsSpent`, `BeforeCardAutoPlayed`, `ModifyCardPlayResultLocation` + `AfterModifyingCardPlayResultLocation`, `ModifyCardPlayCount` + `AfterModifyingCardPlayCount`, `BeforeCardPlayed`, `AfterCardPlayed(+Late)`,
`AfterCardChangedPiles`, `AfterCardEnteredCombat`, `AfterCardGeneratedForCombat`, `AfterCardDrawn`, `AfterCardDiscarded`, `AfterCardExhausted`, `ShouldDraw`/`AfterPreventingDraw`, `ModifyHandDraw`, `BeforeHandDraw`, `ShouldFlush`, `BeforeFlush`, `AfterFlush`, `ShouldEtherealTrigger`, `ModifyShuffleOrder`, `AfterShuffle`, `AfterHandEmptied`, `ModifyXValue`, `ModifyKeywordsInCombat`, `TryModifyEnergyCostInCombat(+Late)`, `TryModifyStarCost`.
Concrete implementers (card-pipeline subset, from `grep` over `Models/`):
* `ModifyCardPlayCount`: BurstPower (Skills), DuplicationPower (any), OneTwoPunchPower (Attacks), SignalBoostPower (Powers), TagTeamPower (enemy-applied: Attacks targeting it), EchoFormPower (first `Amount` plays each turn; reads history), ThrowingAxe (relic, once per combat); consumers decrement/remove in `AfterModifyingCardPlayCount`.
* `ModifyCardPlayResultLocation`: FeralPower (0-cost Attacks -> Hand Top), ReboundPower (Discard -> Draw Top), NostalgiaPower (Attack/Skill Discard -> Draw Top for the first `Amount` per turn), CorruptionPower (Skills -> Exhaust).
* `ShouldFlush`: RunicPyramid, RetainHandPower, RingingTriangle, WellLaidPlansPower. `ShouldDraw`: NoDrawPower, Fiddle. `ShouldPlayerResetEnergy`: IceCream. `ModifyXValue`: ChemicalX. `ModifyKeywordsInCombat`: HexPower.
* `ShouldEtherealTrigger` and `ShouldPayExcessEnergyCostWithStars`, `ModifyHandDrawLate`: declared virtuals, no implementers found in this build.

---------------------------------------------------------------------------------------------------

## 14. State the sim must keep beyond piles

* Per-turn / per-combat play history (`Combat/History/*`): entries `CardPlayStarted`, `CardPlayFinished`, `CardDrawn`, `CardDiscarded`, `CardExhausted`, `CardGenerated`, `CardAfflicted`, `EnergySpent`, `StarsModified`, plus damage/block/power entries owned by other specs.
  `entry.HappenedThisTurn(state)` = same round number AND same side AND same per-player turn number. Effects that read it: Normality, Ringing, SmoggyPower, EchoForm, Nostalgia, Feral (initial count on apply), Velvet Choker-style counters (own counters), ChainsOfBinding, etc.
* `BeginCardOrPotionEffect` depth per player (`CombatManager.cs:368-404`) - gates `AfterHandEmptied` and dead-player card cleanup.
* Player combat state: Energy, Stars, TurnNumber, 5 piles, orb queue, pets.
* The 5 run-level RNG streams used by cards (Shuffle, CombatCardGeneration, CombatCardSelection, CombatTargets, CombatEnergyCosts), each as 4 u64 words (5 streams).

---------------------------------------------------------------------------------------------------

## 15. Subtle things a faithful sim must replicate (checklist)

1. Initial shuffle has NO sort; reshuffle sorts with .NET IntroSort over `(Id ordinal, upgradeLevel)` first. Equal-comparing cards are permuted by the sort; instance identity must be tracked (7.3).
2. The shuffle RNG is a run-level stream shared with random hand/draw insertion (`Position=Random`), Catastrophe, BeatDown, Uproar, Stampede. Draws consumed by shuffles depend on pile size: n-1 per shuffle.
3. `NextItem` consumes a draw even for 1 element (0 for empty); `TakeRandom`/`GetDistinctForCombat` consume |L|-1 draws regardless of `count`; `GetForCombat` consumes `count`.
4. Innate: each innate card is moved to index 0 in pile order => reversed relative order; Imbued-type "bottom" cards are moved first and excluded from innate. Hand draw `min(10, max(5+mods, #innate))` on turn 1 only.
5. Draw stops silently at 10 cards; no discard burn; reshuffle only when a card is needed. A draw request with a full hand does not reshuffle.
6. Add-to-hand at 10 redirects to discard (position arg preserved); evaluated per card in a multi-add.
7. Cards in Play pile during their own play: they are hook listeners; `GetResultLocationForCardPlay` is evaluated BEFORE `OnPlay` and consumes `ExhaustOnNextPlay`.
8. A card that moves itself out of the Play pile during OnPlay is not moved again.
9. Energy is spent while the card is in Hand; X is captured at spend time; replays reuse X. Auto-play of an X card takes X = all current energy WITHOUT spending.
10. Local cost modifiers apply in insertion order without intermediate clamping; negative intermediate bypasses global hooks; global pass 1 then pass 2 (free effects); final `max(0, ...)`.
11. "Until played" modifiers are cleared AFTER the card has gone to its result pile; clones created mid-play inherit them.
12. `EndOfTurnCleanup` runs after the player's flush AND after the enemy turn.
13. Ethereal exhaust happens before turn-end-in-hand effects and before flush; Retain does not protect Ethereal; flush does not trigger Sly (only `CardCmd.Discard*`), which auto-plays after ALL discards/draws.
14. Failed auto-play (Unplayable/vetoed/no target) still moves the card to the Play pile, then to Exhaust/Discard-bottom (Power cards go to discard).
15. Power cards and dupes are removed from combat after the last replay (no exhaust hook); other results use `CardCmd.Exhaust`/`CardPileCmd.Add`.
16. `AfterCardPlayed` fires once per replay and for the killing blow; the replay loop still breaks when combat is over.
17. Transform keeps the position/index in the pile; replacement is fresh (not upgraded).
18. Draw-pile card choices are presented sorted by (Rarity, Id) to the player; hand/discard in pile order.
19. `AfterCardChangedPiles` is not fired for same-type moves (reshuffle draw-pile cards; Play->Play).
20. Dupes lose Exhaust keyword and disappear after play.
21. Single-player: AnyAlly cards are unplayable; `ShouldPayExcessEnergyCostWithStars` never true; no multiplayer `GiveToAnotherPlayer`.
22. Hook listener snapshot semantics and order (section 13) determine which of several same-hook effects see the previous one's result (e.g. PerfectFit ordering, `ModifyCardPlayResultLocation` chain).

---------------------------------------------------------------------------------------------------

## 16. Open questions / uncertainties

1. **Unlock state / pool contents.** `GetUnlockedCards` filters by epoch reveals (`FilterThroughEpochs`); I did not trace the epoch->cards tables. Required to reproduce Discovery-type RNG. Assume fully unlocked and record the setting.
2. **Selection result order** (9.4): real game = click order for hand selection, HashSet order for grids. The Sim convention (agent supplies an ordered list) is my recommendation, not verified behaviour of a particular outcome ordering (it only matters for put-back-on-top style effects).
3. **Card ID strings**: VERIFIED mechanically (all 597 card class names, real .NET Regex, section 1.4). Not cross-checked against the game's localization keys/save files (assumed identical since the same `ModelDb.GetId` produces both).
4. **`TargetType.AnyPlayer/TargetedNoCreature/Osty` actions in manual play**: `IsValidTarget(null)` is true for these; how an agent supplies a target for `AnyPlayer` is UI-level. Not exercised in single-player here.
5. **`ShouldPayExcessEnergyCostWithStars`/`ShouldEtherealTrigger`** have no implementers in this build (grep of `Models/`). Mods could add them; ignored.
6. **Per-card logic** (Models/Cards/*.cs, ~700 cards) is out of this spec; this spec gives the engine primitives they call. Card-specific counters (Regret, TheBall, MadScience, Normality...) must be extracted per card.
7. **`CardPileCmd.Add` for transform/AfterCardChangedPiles**: transform passes the replacement's own pile type as `oldPile` (`CardCmd.cs`), i.e. listeners see a same-type "move"; treat it as such.
8. **RunRngSet shuffle stream seeding at combat start in the Sim**: the real game continues the run stream. For a self-contained combat env, pick the 4 state words; reproducing a real run requires loading the saved `SerializableRng` (counter + 4 words) for Shuffle/CombatCardGeneration/CombatCardSelection/CombatTargets/CombatEnergyCosts.
9. **Heap-sort fallback** of the .NET sort was exercised (8 times) in my cross-check against .NET 9.0.20; the game targets `net9.0` (`decomp/*.csproj`). A different runtime major could in theory change tie behaviour (IntroSort has been stable across .NET Core 3.0-9 as far as I know).
10. **Hook-listener ties**: the sim must preserve the exact order in section 13; if the sim restructures state (e.g. SoA piles) the iteration order must still be reproducible.

---------------------------------------------------------------------------------------------------

## 17. Implementation notes (suggestions)

* Represent cards as `CardId(u32 instance)` handles into an arena with `proto: &'static CardProto` (id string, type, rarity, target, canonical cost, keywords, tags, vars, max upgrade, hooks table) and an instance struct (section 1.2). Piles = `Vec<CardId>` (draw top = index 0).
* Implement `shuffle_unstable(&mut [T], rng)` and `intro_sort_by(&mut [T], cmp)` exactly as in 7.2/7.3 (do not use Rust's `sort`/`sort_unstable`).
* Make the decision points of section 9 an explicit `Decision` enum returned by the stepper (`ChooseFromHand{min,max,filter_id,purpose}`, `ChooseFromPile{pile,min,max}`, `ChooseOne{options,skippable}`), applying the auto-resolve rule (9.2) internally so agents never see forced choices.
* Implement the play pipeline as one function mirroring 5.1 line by line, with hook dispatch through a single ordered listener snapshot builder mirroring section 13.
