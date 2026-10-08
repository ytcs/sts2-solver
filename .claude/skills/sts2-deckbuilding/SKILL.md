---
name: sts2-deckbuilding
description: Use at every card reward, shop, rest site, relic, upgrade, removal or transform: the pick protocol (smooth greedy numbers first, then the judgment pass for what the numbers cannot see), the five-bucket audit, what each fight asks, plan and switch, macro choices, the decision record.
---

# Deck building: numbers are one input; judgment weighs what they cannot see
Smooth greedy (best smooth score, never skip unless all options are worse) beat "best win now, skip on no gain" 0.70 vs 0.31 (Lagavulin), 0.18 vs 0.00 (Aeonglass) `[sim]` (`evidence.md`). It is greedy, additive, combat-only: weigh with sections 2-6. `[expert]` picks that look bad now serve a plan.

## 1. Protocol (card reward, shop item, relic, upgrade, removal, transform)
1. Buckets + fights: `brief` (five-bucket line, gaps). Name the WEAKEST upcoming fight (boss or lowest-win elite; `reward` prints it), what it asks (`sts2-acts/encounters.md`, `sts2-mechanics`), the answering bucket, the target deck's plan cards (section 4). A fight won >= ~0.95 does not decide.
2. Numbers: card reward `reward [--attempts N] [--hp full]` (options + skip vs boss smooth, elites left, next act; buckets; weakest fight; need-weighted gain; "not evaluated" -> `eval`). Else `eval --smooth --boss|--elites|--next --v ...`. Read need-weighted gain + weakest-fight column, not the mean.
3. Judgment: the `section-3 bar` lines, section 2, buckets: answers the weakest fight or a future problem? plan card or filler? synergy?
4. Act in one call: `a <i> -- buckets: <line, open>; weakest: <fight, asks>; numbers: <best +x, chosen gain, bar status>; judgment: <plan fit, density, future problem, synergy>`. Harness refuses without this screen's `reward` or any field.
- `pickplan` (~3 min): tau* take-threshold for offers left before the dangerous fight (pools/odds `[code]`, rho/slots `[hyp]`); falls with runway (K=0: take any positive); skip only near-zero picks.
- One `reward` per screen; `pickplan` per act + after shops; extra evals only for options within 2 se.
- < 2 se (~0.05 at 96 attempts) = tie: break by bucket gap + plan. Re-price after every pick (not additive).
- Smooth saturated (x1.5+ >= ~0.9): decide on `eval --boss --hp <arrival> --attempts 192+`. `[sim]`
- Multi-pick screens (Orrery, multi-set, shop): price bundles + leave-one-out at real HP; never skip all on single-card ties. `[hyp]` Test: bundle vs skip-all.

## 2. Blind spots of the numbers
1. Enablers dead until partners (Barricade, Demon Form, Feel No Pain, Dark Embrace, Corruption): take only if bundle gain large AND >= 2 payoffs owned/likely AND interim survivable. Test: enabler / bundle / payoffs variants; `route` with it dead. `[hyp]`
2. Offers are the bottleneck (best 20-card deck 0.62 vs online 0.25): buy access (elites, shops with gold for Acts 2-3, ancients, card events, boss potions) over a +4 pick. `[sim]`
3. Rares/relics are targets: find the 2-3 cards that make the target deck. Test: boss-alone `eval`. `[hyp]`
4. Interim survival: "worse now" only if boss-gate HP still reached. Test: `route`, `eval --boss --hp <arrival>`. `[hyp]`
5. Prefer picks surviving a bad draw (acceleration, cheap, redundancy). `[hyp]`
6. Engines the network may undervalue (exhaust, Primal Force + Demon Form, 5-7 energy; energy itself no gap `[sim]`): more attempts, `adv 20`, Strength/Block/energy variants. `[hyp]`
7. Density `[sim]` (28-card Act 2 deck vs Kaiser Crab, se ~0.013): remove 8 starters +0.20, 5 fillers +0.05; bucket counts showed no gap. (a) past ~20 cards skip picks within 2 se of skip; (b) every few picks `rmcalc` + bundle `remove=` (one ~+0.03, eight +0.20); (c) removals before marginal cards; (d) key cards need low dilution. `[hyp]` "skip ties past ~20 cards" as pick rule (test: 22- vs 28-card runs).
8. Economy effects (gold, max HP, pickup relics, events, potion slots, Neow trades, curses for relics) not in `eval` (curse: `add=CURSE_ID`): price in HP/gold. `[code]`
9a. Enchants priced as cards: `--v "x|enchant=FLAME_BARRIER:IMBUED"` (ids `sts2.names()`). HP-cliff bosses (Kaiser Crab): re-price close pairs with `--seed 1` or 2x attempts. `[sim]`
9b. Price elites at arrival HP (`reward --hp <q50 arrival from routes>`, `eval --elites --hp`); full HP saturates ~0.97. `[sim]`
9. Fight rules buckets hide (Slippery, Artifact, Thorns, Hard to Kill, Withers): boss-alone `eval`. `[code]`
10. Horizon: carry-over (scaling, energy, draw, rares, relics, boss potions, upgrades); boss > ~85% or after Act 1: later horizons decide (`reward` next-act column, `eval --future`). `[hyp]`

## 3. The five buckets
- FD front damage, SD scaling damage, FB front block, SB scaling block, ACC acceleration (energy, draw, cycling, Barricade). In none = skip candidate.
- Section-3 bar: bucket solved when fights ahead ask no more of it (at arrival HP). A card only into solved buckets needs boss smooth > max(3 se, 0.05) or +0.10 on the weakest fight; tie -> skip. Spend picks on open buckets, a coming weakness, or an engine piece (2.1/3/6/10) even if worse now; name the future problem in the why. `[hyp]` Test: solved-bucket picks vs later density + next-act column.
- Prefer raising the lowest bucket an upcoming fight asks about. `[hyp]`

## 4. What each fight asks; target deck
- Order: act boss, elites still possible (bag), hallways. Per fight: front damage, scaling, block pattern, conditions (`encounters.md`, intents).
- Target decks: `encounters.md`; check every reward/shop/event.
- Horizons: current boss + elites left, next act, final act (A10 two bosses). `reward` first two, `eval --future` all.

## 5. Plan and switch `[hyp]`
- State the plan before each boss/elite in the why; after T1-2 switch if the draw killed it. Test: `review`.

## 6. Macro choices
- Rest or smith: smith default. Rest only if the heal changes the rest of the act more than the best upgrade changes the run: `routes --hp <after rest>` vs `routes` now, and `eval --smooth --boss --next --v "up|upgrade=ID"` (next-act column = long term). Never a one-fight `eval --boss` at current HP. Later rest before the boss: smith now. Smith the plan card. `[hyp]`
- Relics/Neow: bucket filled + route enabled. Potions beat cards in the same shop vs hard bosses. Once the act is solved keep gold and potion slots for later acts.

## 7. Decision record
- `judgment:` names the deciding rules/blind spots + expected payoff. After a run (`review`, `evals/judgments.jsonl`): held -> tighten test; failed -> narrow/delete; `[hyp]` -> `[sim]` when measured. One change per run.
