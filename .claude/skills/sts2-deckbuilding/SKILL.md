---
name: sts2-deckbuilding
description: Use at every card reward, shop, rest site, relic, upgrade, removal or transform: the pick protocol (smooth greedy numbers first, then the judgment pass for what the numbers cannot see), the five-bucket audit, what each fight asks, plan and switch, macro choices, the decision record.
---

# Deck building: numbers are one input; judgment weighs what they cannot see
Smooth greedy (best smooth score; skip only if every option is worse) beat "best win now, skip on no gain" 0.70 vs 0.31, 0.18 vs 0.00 `[sim]` (`evidence.md`). Greedy, additive, combat-only: weigh sections 2-6.

## 1. Protocol (reward, shop, relic, upgrade, removal, transform)
1. `brief`: buckets + gaps + bottleneck line (cards left at end of turn: no energy -> energy/cost cuts; with energy -> removal, or a combo piece needing draw/retain; `[hyp]` test: flagged bottleneck vs later deaths in baseline rows). Name the weakest upcoming fight (fights won >= 0.95 do not decide), what it asks (`sts2-acts/encounters.md`), the plan cards.
2. Numbers: `reward [--attempts N] [--hp full]` on card rewards, else `eval --smooth --boss|--elites|--next --v ...`. Read need-weighted gain + weakest-fight column.
3. Judgment: section-3 bar, section 2.
4. `a <i> -- buckets: ...; weakest: ...; numbers: <best, chosen gain, bar status>; judgment: ...` (harness refuses without `reward` or a field).
- < 2 se = tie: break by bucket gap + plan; re-price after every pick. Smooth saturated (x1.5 >= 0.9): `eval --boss --hp <arrival> --attempts 192+` `[sim]`.
- Multi-pick screens: price bundles + leave-one-out at real HP. `[hyp]` test: bundle vs skip-all.

## 2. Blind spots of the numbers
1. Enablers (Barricade, Demon Form, Feel No Pain, Dark Embrace, Corruption): only with large bundle gain, >= 2 payoffs likely, survivable interim. `[hyp]` test: enabler/bundle/payoff variants.
2. Offers are the bottleneck (best 20-card deck 0.62 vs online 0.25): buy access (elites, shop gold, ancients, card events, boss potions) over a +4 pick. `[sim]`
3. Rares/relics: find the 2-3 target-deck cards. `[hyp]` test: boss-alone `eval`.
4. "Worse now" only if the boss-gate HP is still reached. `[hyp]` test: `route`, `eval --boss --hp <arrival>`.
5. Prefer picks surviving a bad draw. `[hyp]`
6. Engines the network may undervalue (exhaust, Strength stacking): more attempts, `adv 20`. Energy itself is no gap `[sim]`.
7. Density `[sim]` (28-card act-2 deck vs Kaiser Crab): remove 8 starters +0.20, 5 fillers +0.05. (a) past ~20 cards skip ties; (b) `rmcalc` every few picks; (c) removals before marginal cards. `[hyp]` test: 22- vs 28-card runs.
8. Economy (gold, max HP, slots, curses) is not in `eval`: price in HP/gold. `[code]`
9. Enchants: `--v "x|enchant=ID:ENCH"`. Elites at arrival HP (`reward --hp <q50>`; full HP saturates ~0.97). HP-cliff bosses: re-price with `--seed 1`. `[sim]` Rules buckets hide (Slippery, Artifact, Thorns, Hard to Kill): boss-alone `eval`. `[code]`
10. Boss > ~85% or after act 1: later horizons decide (`reward` next-act column, `eval --future`). `[hyp]`

## 3. The five buckets
- FD front damage, SD scaling damage, FB front block, SB scaling block, ACC acceleration.
- Section-3 bar: a bucket is solved when fights ahead ask no more of it. A card only into solved buckets needs boss smooth > max(3 se, 0.05) or +0.10 on the weakest fight; tie -> skip. `[hyp]` test: solved-bucket picks vs later density + next-act column.

## 4. What each fight asks; target deck
Order: act boss, elites in the bag, hallways (`encounters.md`). Horizons: current boss + elites, next act, final act (two bosses).

## 5. Plan and switch `[hyp]`
State the plan before each boss/elite; switch after T1-2 if the draw killed it. Test: `review`.

## 6. Macro choices
- Smith by default. Rest only if the heal changes the act more than the upgrade changes the run: `routes --hp <after rest>` vs `routes`, plus `eval --smooth --boss --next --v "up|upgrade=ID"`. Later rest before the boss: smith now. `[hyp]`
- Potions beat cards in the same shop vs hard bosses `[sim]`. Act solved: keep gold and slots for later acts.

## 7. Decision record
`judgment:` names the deciding rules. After a run: held -> tighten test; failed -> narrow/delete; measured -> `[sim]`. One change per run.
