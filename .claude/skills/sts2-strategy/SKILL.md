---
name: sts2-strategy
description: Use at the start of a run and when a decision is not covered by pathing or deckbuilding: the per-decision loop and the general optimization targets shared by every character and act (pricing, combat, potions, events), each with its test.
---

# General strategy

## Loop (every decision)
1. Decision's skill: map/Neow `sts2-pathing`; card/shop/rest/relic `sts2-deckbuilding`; fight `sts2-harness`; unknowns `sts2-mechanics`.
2. Bounding facts + plan for two acts (`sts2-deckbuilding` section 4): boss, elites left, HP, gold, rests/shops ahead.
3. Price by P(win run) = P(win fight) x V(next act), never the myopic fight delta.
4. Act with a `-- why` holding the numbers.

## Targets (rule; tag; test)
1. Max smooth score vs the known boss while arriving above each fight's HP gate. `[sim]` 0.70 vs 0.31 (`sts2-deckbuilding/evidence.md`). `eval` omits slots, rewards, gold, route: price those by hand.
2. Find the binding gate (HP or damage), re-test every few picks: `eval --boss --hp 34/45/60/80`. `[sim]`
3. `DRIVE: MANUAL` (elites/bosses predicted below 0.95 win or q90 loss >= 25% HP, `data/drive_manual.json`, any fight win < 0.90 or q90 loss >= 40% HP): `adv` each decision, follow it unless you see what it misses, write that in the why. Add an encounter after a worst-10% fight showing a solver mistake; remove when review shows it plays well. `[hyp]` test: per-encounter PIT + HP lost, manual vs auto.
4. Manual fights: plan 2-3 turns from `eN plan` + `draw`; write the plan in the why. `[hyp]` test: `agent.hindsight` on plan-marked turns.
5. Not playing a card can be best (stranded draw, Skill vs Enrage, wasted doubling). `[hyp]` test: hindsight on turns ending with an unplayed card the next turn wanted.
6. Check lethal before `turn !`/`combat !` (Vulnerable x1.5). `[hyp]` test: missed-lethal count in `review`.
7. Read power text and relic counters before big hits. `[code]`
8. Potions: spend where they buy most run survival; boss before a heal: max P(win), not HP; commit one at a time. `[hyp]` test: hindsight on held/spent potions; potion as `eval` variant.
9. Events: relic or large resource over a small heal; a curse costs a slot. `[hyp]` test: `eval --v` with `add=CURSE_ID`.
10. Check gold vs shop prices before routing to a shop; relics before gold on rewards (Bowler Hat). `[hyp]` test: `routes` shop pricing, gold before/after.
