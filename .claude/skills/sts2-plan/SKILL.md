---
name: sts2-plan
description: Core run planning for the operator, loaded every session: the win condition (the known final pair), the target deck, HP / potion / gold / elite budgets, build vs complete phase, and how card value changes over the run. Write the plan at every act start and after every boss; every macro decision says what it changes in the plan.
---

# Run plan (deviations: character and act skills)

## The plan (`note` the plan and `target` the deck at each act start and after each boss)
- Win condition: the known final pair (A10: two act-3 bosses, shown on the act-3 map; before that the act-3 boss pool). Target deck: the engine and payoffs that beat both, reasoned from each boss's gimmicks (`sts2-acts/encounters.md`) and measured with `eval`; `target` records it as adds / removes / upgrades vs the current deck.
- Answers [expert: user]: for each fight ahead (act boss, elites left, the pair or the act-3 pool), per gimmick in `encounters.md`: the deck's answer (cards / relics / potions) or none; the missing answers are the gaps. A gap the numbers miss (floored wins, noisy prices) still steers picks, shops and routes; check a disagreement with `eval --enc ID --v`.
- Gaps, measured: `eval --boss` (this act, current deck) and `eval --pool Glory:boss --hp full --target` (the pair pool, or the pair once known, on the target deck); an option's effect on the run = its variant on the target deck.
- Budgets: HP into the next binding fight (elite / boss), potions for the pair, gold for relics / removals, elites left worth taking.
- Phase: build (the win condition is not in the deck) or complete (it is: skip by default, remove, upgrade the engine).

## Rules
- P1 [sim] The final pair decides the run; deck strength vs it at full HP decides the pair: expert decks 0.8-1.0, bot decks ~0.3 (E88, E84). Test: outcome: pair deck strength at arrival vs pair result, ledger.
- P2 [sim] A card is worth the binding fights it still meets: front-damage commons pay at act-1 elites / boss and cost at act-3 bosses (dilution); scaling / engine cards keep their value (E85, E95). Test: instrument: `eval --pool Glory:boss --hp full --target --v "x|add=ID"` before a pick.
- P3 [expert] Phase switch: build until the win condition is in, then complete: skip almost everything, remove, upgrade the engine. Test: outcome: deck size and pair strength at the pair, runs that switched vs not.
- P4 [sim] Price HP at the next binding fight: 20-35% of HP saved in a hallway reaches it; boss slope ~0.005-0.009 win per HP (E85, E132). Test: instrument: `restcalc` / `eval --hp` at the binding fight.
- P5 [sim] Elites pay through the relic and rare odds, not the card; take them while the act boss stays safe (E85, E132). Test: outcome: elites per act vs act clear and pair strength, ledger.
- P6 [code] Potions at the final pair: the search spends what boss 2 does not need; carry potions into act 3; every throw is operator-confirmed (`potion yes|no`) (E87, E89). Test: outcome: potions at the pair, boss-1 wins, ledger.
- P10 [expert: user] Hold a potion only for what it measurably adds (`eval --boss --potions` vs held); else spend it where a fight would cost HP. Full slots: price an offered potion as a swap against each held one. Fixed-value potions (Block) gain nothing by waiting: spend at the first full-value use; heal potions when the heal is uncapped or the slot is needed (E108, E130). Test: outcome: HP lost at elites / bosses with potions held (review readout).
- P7 [sim] Relics are worth at least removals per gold; the shop's best relic vs the final pair beats a marginal card (E131). Test: instrument: `eval --boss --v "r|relics_add=ID"`.
- P8 [hyp] `target` the deck at every act start, after every boss and after any rare / relic that changes the engine: the package (engine, payoffs, the answers `encounters.md` asks of the pair or act-3 pool; `prior` as a community check) that beats the pair; act-2/3 picks, shops and smiths serve it. Per-pick numbers without a target do not build the deck (E134, E145). Test: outcome: `target` set per act vs pair strength at arrival, ledger.
- P9 [hyp] Price an HP-changing option (event, rest, elite) at the HP it reaches the next binding fight with (rests and Burning Blood between; never add Pantograph by hand), not at current HP. Test: instrument: `eval --hp <arrival>` per option.
