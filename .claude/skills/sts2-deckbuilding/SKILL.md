---
name: sts2-deckbuilding
description: Use at every card reward, shop, rest site, relic, upgrade, removal or transform: the five-bucket deck audit, what each boss and elite asks, the gap-then-eval pick procedure, plan switching, and the per-pick ledger.
---

# Deck building as an audit, not a feeling

Why: picks made on intuition cannot be compared run to run. Every pick below is the same four steps, and each step leaves a number in the `-- why` and in the run record, so a later review can ask which rule predicted the outcome. The five-bucket model and the "answer the fight's questions" idea are from an expert player's analysis `[expert]` (JapaneseExport, "The hidden win conditions that most players miss", youtube IqNkmsA0PLo, 10 min): he reads decks in five buckets and found a run-losing mistake by asking what the boss demands of the deck.

## 1. The five buckets (classify every card and relic in the deck)
- **Front-loaded damage:** kills things quickly (hallway fights, a boss's early window).
- **Scaling damage:** more damage per turn as the fight goes on (Strength, Inferno, poison-like, Rampage, Rolling Boulder).
- **Front-loaded block:** blocks now (Defend, Blood Wall, Body Slam as a finisher).
- **Scaling block:** block that grows or repeats (Dexterity, Feel No Pain, Metallicize, Barricade-with-block).
- **Acceleration:** energy, draw, cycling, and things that make the other four happen on time (Pyre, Bag of Preparation, Shrug, Pommel; Barricade belongs here: it saves block from turns it is not needed, it does not add block).
A card can sit in several buckets; a card that sits in none is a candidate to skip. Write the deck's count per bucket (a line of five numbers) after every pick and note the empty ones.

## 2. The questions ahead (what each fight asks)
For the act's boss first (it is known: `m`), then the elites that can still appear (`macro.narrow`), then hallways and events. For each: front-loaded damage need (does it threaten early, or give free turns?), scaling need (long fight, enrage, Strength?), block need (one huge hit every N turns, many small hits, multi-hit?), and any condition (Slippery, Hard to Kill, Artifact, stun). Source: `sts2-acts`, `sts2-mechanics`, the first turns of the fight, and `eval` against that encounter alone. Record the answers in the act's skill as a table (encounter -> what it asks).

## 3. Pick procedure (card reward, shop, relic, upgrade, removal)
1. List the unanswered questions (bucket gaps against the boss/elites ahead).
2. Shortlist the options that fill a gap; skipping is an option with its own cost (a diluted deck).
3. `eval` the shortlist against the **known boss alone** and the elites that can still appear, at the HP I expect to arrive with, 256 attempts; the gain must clear ~2 SE; otherwise skip. Remember a card's value depends on the deck (Blood Wall -5 -> +10 once Inferno was in the deck).
4. Check conditions: does the plan need a specific draw (three attacks in a turn, a power early)? Prefer picks that make the plan robust to a bad draw (acceleration, cheap cards, redundancy) over picks that add one more condition.
5. Write the ledger line: `bucket line; gap; options with eval deltas (win, HP); choice`.

## 4. Plan and switch
- State the fight plan before the boss and each elite (what wins it, in which bucket it lives, what condition it needs).
- During the fight, after turn 1-2, ask whether the opening draws killed the plan. If so, switch to the plan that uses what is in hand (the expert's example: with the powers bottom-decked, play the 0-cost attacks to enable a three-attack relic instead of blocking).
- A solver line that contradicts the plan is data: trust the solver for the micro, change the plan for the macro.

## 5. Macro audit at every rest, route and relic choice
- **Rest or smith:** compute `eval` at the HP after rest versus the HP now with the upgrade (Lagavulin at 44 HP 42% vs at 68 HP 86%, best smith +8): rest unless the HP gate is already cleared.
- **Route:** work back from the boss's HP gate (`eval --hp` at 34/45/60/80), then pick the path that arrives above it with the buckets filled (`sts2-pathing`).
- **Relics and Neow:** score by the bucket they fill and by the route they enable.

## 6. Making it iterative
- Each run's `events.jsonl` holds every eval (spec and result) and every action with its `-- why`. After a run: for each pick, compare the eval gain to what happened in the fights it was meant to fix (`python -m agent.improve review`); mark each rule that held or failed in the character/act skill with `[played]`.
- Keep two lists in the character skill: rules that predicted well (promote to `[sim]`/`[played]`) and rules that failed (delete or revise with the numbers).
- Change one thing per run when possible (a pick threshold, a route shape) so the effect can be read.

## Lessons from run 3 (reached Act 3, died to the first Act 3 boss) `[played]`
- Early game is where power is added: Act 1 picks (Inferno, Anger, Setup Strike, Inflame+Tremble vs Lagavulin) moved win rates by 20-70 points; by Act 2-3 almost every offered card was below the 2 SE bar (12 skips in Act 2), and the deck stalled. When a boss pool is at ~0%, a +5 point pick will not save the run: look for outliers (rares, relics, potions, shops, transforms, upgrades) and build the target deck for the boss, not incremental picks; consider taking rare picks (Demon Form +12/+9) over marginal elite-HP picks (Tear Asunder +22 elites, +0 boss) when the boss is the gate.
- Pre-check the next act's boss on the Act N boss reward: it is on the map only after entering the act, but the pool is known (`sts2-acts`): evaluate against all three bosses.
- Potions and consumables are cheap power: against hard bosses Powdered Demise (+14), Flex (+11) and Block Potion (+12) beat every card in the same shop.
