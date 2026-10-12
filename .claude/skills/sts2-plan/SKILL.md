---
name: sts2-plan
description: Core run planning for the operator, loaded every session: the win condition (the known final pair), the target deck, HP / potion / gold / elite budgets, build vs complete phase, and how card value changes over the run. Write the plan at every act start and after every boss; every macro decision says what it changes in the plan.
---

# Run plan (deviations: character and act skills)

## The plan (write it with `note` at each act start and after each boss)
- Win condition: the known final pair (A10: two act-3 bosses, shown on the act-3 map; before that the act-3 boss pool). Target deck: the engine and payoffs that beat both, what each boss asks (`sts2-acts/encounters.md`).
- Gaps: what the deck still lacks against the act boss now and the final pair; the number: `eval --boss` (this act) and `eval --pool Glory:boss --hp full` (the pair pool) or the pair itself once known.
- Budgets: HP into the next binding fight (elite / boss), potions for the pair, gold for relics / removals, elites left worth taking.
- Phase: build (the win condition is not in the deck) or complete (it is: skip by default, remove, upgrade the engine).

## Rules
- P1 [sim] The final pair decides the run and deck strength vs it at full HP decides the pair: expert decks 0.8-1.0, bot decks 0.24-0.33 (E88, E84). Test: outcome: pair deck strength at arrival vs pair result, ledger.
- P2 [sim] A card is worth the binding fights it still meets: front-damage commons pay at act-1 elites / boss (+1 to +3 win points each) and cost at act-3 bosses (-2 each; dilution); scaling / engine cards keep their value (Demon Form +6 at act-3 bosses) (E85, E95 256-attempt re-measure). Test: instrument: `eval --boss --v "x|add=ID"` vs the final pair before an act-2/3 pick.
- P3 [expert] Phase switch: build until the win condition is in, then complete: skip almost everything, remove, upgrade the engine (Baalorlord: "I want less cards, not more"). Test: outcome: deck size and pair strength at the pair, runs that switched vs not.
- P4 [sim] HP is priced at the next binding fight: 20-35% of HP saved in a hallway reaches it (heals and the max-HP cap absorb the rest); slope at bosses ~0.005-0.009 win per HP (E85). Test: instrument: `restcalc` / `eval --hp` at the binding fight.
- P5 [sim] Elites pay through the relic and rare odds (the card channel of one elite ~0.5-0.75 win points vs ~4 points of act-1 boss HP cost); take them while the act boss stays safe (E85). Test: outcome: elites per act vs act clear and pair strength, ledger.
- P6 [code] Potions at the final pair: the search spends what boss 2 does not need (boss-1 worth prices each kept potion by its boss-2 gain); carry potions into act 3 (E87, E89). Test: outcome: potions at the pair and boss-1 wins, ledger.
- P7 [sim] Relics are worth at least removals per gold (review section 4); the shop's best relic vs the final pair beats a marginal card. Test: instrument: `eval --boss --v "r|relics_add=ID"`.
- P8 [hyp] Re-plan after every boss and any rare / relic that changes the engine; a plan unchanged for an act is a stale plan. Test: outcome: decisions citing a stale plan vs result.
