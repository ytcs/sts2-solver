---
name: sts2-strategy
description: Use at the start of a run and when a decision is not covered by pathing or deckbuilding: the per-decision loop and the general optimization targets shared by every character and act (pricing, combat, potions, events), each with its test.
---

# General strategy

## Loop (every decision)
1. Invoke the decision's skill: map/Neow `sts2-pathing`; card/shop/rest/relic/upgrade `sts2-deckbuilding`; fight `adv`/`combat` (`sts2-harness`); unknowns `sts2-mechanics`.
2. Name bounding facts + plan for the next two acts (`sts2-deckbuilding` section 4): known boss, elites left, HP, gold, rests/shops ahead.
3. Price options vs the known boss and next threats (P(win run) = P(win fight) x V(next act), never the myopic fight delta).
4. Act with a `-- why` holding the numbers. After the run: `python -m agent.improve review`, update the book at the most specific level.

## Targets (rule | test | tag)
- Maximize smooth boss score of the known boss subject to arriving above each fight's HP gate. Test: `reward`; `eval --smooth --boss|--elites|--next`. `eval` omits deck slot, future rewards, gold, rests, route: price those by hand. Regular pool saturates at 100%: only HP lost differs there. `[sim]` 0.70 vs 0.31 (`sts2-deckbuilding/evidence.md`).
- Find the binding gate (HP or damage); re-test every few picks. Test: `eval --boss --hp 34/45/60/80`. `[sim]`
- Auto vs manual per fight: harness prints `DRIVE: AUTO|MANUAL (why)`. MANUAL = elites, bosses, `data/drive_manual.json` encounters, predicted win < 0.90 or q90 loss >= 40% HP; `combat`/`turn` refuse there without `!`. Manual = `adv` every decision, read values, `a <i>`; follow the solver unless I see what it misses (mechanic, macro plan) and write it in the why (= candidate solver gap). Add an encounter to the list after a worst-10% fight whose play shows a solver mistake; remove when review shows it plays well. `[hyp]` Test: per-encounter PIT + HP lost, manual vs auto.
- Manual fights: plan 2-3 turns. Search plays to this turn's end + value net; near-ties hide multi-turn plans. Read `eN plan` + `draw`, pick the line for this and next turn (kill/block before a big turn, keep a card/energy for it, strip a debuff source first); write the plan in the why; follow `adv` where the plan has no view. `[hyp]` Test: `agent.hindsight` on plan-marked turns vs solver line.
- Not playing a card can be best: a draw with nothing left to pay strands the card; a Skill vs Enrage feeds the enemy; a small attack can waste a doubling meant for a bigger card. Before the last plays, ask what remaining cards/energy do next turn. `[hyp]` Test: `agent.hindsight` on turns ending with a drawn, unplayed card the next turn wanted.
- Check lethal before `turn !`/`combat !` in a manual fight: hand damage vs each enemy HP + block (Vulnerable x1.5); a kill or must-kill -> play by hand. `[hyp]` Test: missed-lethal count in `review`.
- Combat: play the `adv` line; deviate only for what the solver cannot weigh (boss potion, route consequence), write why. `[hyp]` Test: override tally, `agent.hindsight`.
- Read power text and relic counters before big hits (`sts2-mechanics`, `relics`). `[code]`
- Potions: spend where it buys most run survival; keep the strongest for the act boss unless spending here is worth more. Commit one at a time (`sts2-harness`); keep/save are priced within the fight only, the boss need is my judgment. Boss before a heal: maximize P(win), not HP. `[hyp]` Test: `hindsight` on held/spent potions; potion as `eval` variant (`encounters.md`).
- Events: relic or large resource over a small heal; a curse costs one slot. `[hyp]` Test: `eval --v` with `add=CURSE_ID` vs the relic variant.
- Check gold vs shop prices before routing to a shop (`brief`, `m`). `[hyp]` Test: `routes` prices shops at arrival gold.
- Reward screen: relics before gold; read what a relic changes first (Bowler Hat +25% to later gold). `[hyp]` Test: gold before/after (`s`).
