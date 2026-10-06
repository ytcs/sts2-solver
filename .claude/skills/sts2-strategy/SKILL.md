---
name: sts2-strategy
description: Use at the start of a run and when a decision is not covered by pathing or deckbuilding: the per-decision loop and the general optimization targets shared by every character and act (pricing, combat, potions, events), each with its test.
---

# General strategy

## The loop (every decision)
1. Invoke the decision's skill: map / Neow `sts2-pathing`; card, shop, rest, relic, upgrade `sts2-deckbuilding`; fight `adv` / `combat` (`sts2-harness`), `sts2-mechanics` for unknowns.
2. Name the bounding facts and the plan for the next two acts (`sts2-deckbuilding` section 4): known boss, elites that can still appear, HP, gold, rests and shops ahead.
3. Price the options against the known boss and next threats (picks: smooth greedy, never skip unless every option is worse than the current deck: `sts2-deckbuilding`).
4. Act with a `-- why` containing the numbers; after the run `python -m agent.improve review`, then update the book at the most specific level.

## Targets: rule | test | status
- **Maximize the smooth boss score of the known boss, subject to arriving at each fight above its HP gate.** Test: `reward`; `eval --smooth` with `--boss`, `--elites` or `--next`. `eval` omits the deck slot, rewards to come, gold, rest actions, route interplay: price those by hand. The regular pool saturates at 100%; there only HP lost differs. `[sim]` 0.70 vs 0.31 (`evidence.md`).
- **Find the gate that binds (HP or damage); re-test every few picks.** Test: `eval --boss --hp 34/45/60/80` (`sts2-pathing`). `[sim]`
- **Decide auto or manual per encounter.** The harness prints `DRIVE: AUTO|MANUAL (why)` when a fight starts: manual for elites, bosses, encounters listed in `data/drive_manual.json` (the solver has shown a gap there) and fights predicted below 0.90 win or with a 90th-percentile loss of 40%+ of my HP; `combat` / `turn` refuse there without `!`. Manual = `adv` every decision, read the options and their values, then `a <i>`; follow the solver unless I see what it misses (a mechanic, the macro plan) and write that in the `-- why`: each disagreement is a candidate solver gap for the review. Add an encounter to the list after a fight in the worst 10% of its prediction whose play-by-play shows a solver mistake; remove it when a review shows the solver plays it well. `[hyp]` Test: per-encounter PIT and HP lost, manual vs auto, over runs.
- **Manual fights are planned over two or three turns, not one.** The search plays to the end of this turn and a value network guesses the rest, so its ties and near-ties hide plans that need the next turns. Each turn, read the `eN plan` lines (next moves, what they add or debuff) and the `draw` line (what the next hand can hold), then pick the line for this turn and the next: kill or block before a big turn, keep a card or energy (Ice Cream) for it, strip a debuff source first. Write the plan in the `-- why`; follow `adv` where the plan has no view. `[hyp]` Test: `agent.hindsight` on manual fights, with plan-based turns marked in the `-- why`: HP lost vs the solver's line on the same states.
- **Combat: play the `adv` line; deviate only for what the solver cannot weigh (a potion kept for the boss, a route consequence) and write why.** Test: override tally in `review`, `agent.hindsight` on costly fights. `[played]` manual lines vs Vantom cost ~25 HP more than the solver's (one fight, no se).
- **Read power descriptions and relic counters before big hits** (`sts2-mechanics`, `relics`). `[code]`
- **Potions: keep the strongest for the act's boss unless the solver predicts a clear loss without it** (automatic in comfortable fights: `sts2-harness`). Test: the potion as an `eval` variant (values `[sim]` in `encounters.md`); fights lost with it held. Hold rule `[hyp]`.
- **Events: a relic or large resource over a small heal; a curse costs one slot.** Test: `eval --v` with `add=CURSE_ID` vs the relic variant. `[hyp]`
- **Check gold against shop prices before routing to a shop** (`brief`, `m`). `[played]`
- **On a reward screen take relics before gold (and read what a relic changes before any pickup): Bowler Hat gives +25% to gold gained after it.** Test: gold before and after the pickup (`s`), 30 -> 37 with the hat first. `[played]`
