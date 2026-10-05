---
name: sts2-deckbuilding
description: Use at every card reward, shop, rest site, relic, upgrade, removal or transform: the pick protocol (smooth greedy numbers first, then the judgment pass for what the numbers cannot see), the five-bucket audit, what each fight asks, plan and switch, macro choices, the override log.
---

# Deck building: numbers first, then judgment where the numbers are blind

**Adopted pick rule `[sim]`:** price every option on the SMOOTH objective (win rate averaged over start HP x1 / 1.5 / 2 / 3 of the real HP), pick the best, never skip unless every option scores below the current deck. It beat "best win rate now, skip when no gain" 0.70 vs 0.31 (Lagavulin) and 0.18 vs 0.00 (Aeonglass); lookahead variants +20-30%, not significant at 6 sequences (`evidence.md`). It is greedy, additive and combat-only; sections 2-6 are the judgment layer. `[expert]` (JapaneseExport, youtube IqNkmsA0PLo): picks that look bad now are made for a plan.

## 1. Protocol (every card reward, shop item, relic, upgrade, removal, transform)
1. `brief`: deck, five-bucket line and gaps (`data/card_buckets_ironclad.json`), relics, potions, known boss, elites left.
2. Numbers. Card reward: `reward [--attempts N] [--hp full]` (every option and skip against the boss smooth, elites left and next act, with the buckets each card fills; a card with no simulator id is "not evaluated": use `eval`). Else `eval --smooth --boss|--elites|--next --v "name|add=ID|remove=ID|upgrade=ID|relics_add=ID"` (removals, upgrades, relics, potions, transforms are variants).
3. Judgment pass: walk section 2. Each item that changes the choice is an OVERRIDE; the default is the numbers.
4. Act in one call: `a <i> -- "<bucket line>; gap; best option +x (smooth), chosen; override: <number> or none"` (`<i>` = option index on the screen, or `a ~<card name>`).

Under 2 se (~0.05 at 96 attempts) is a tie: break it by bucket gap and plan fit. Value is not additive (Blood Wall -5 -> +10 once Inferno was in): re-price after every pick.

## 2. Blind spots of the numbers (rule | test | status)
1. **Enablers and payoffs dead until the partner arrives** (Barricade, Demon Form with few hits, Feel No Pain without exhaust, Dark Embrace, Corruption). Take the enabler only if the bundle gain is large AND >= 2 payoffs are in the deck or likely met (known shop / ancient rares, elite rewards) AND the interim cost is survivable; else skip. Test: `--v "enabler|add=E" --v "bundle|add=E,P1,P2" --v "payoff alone|add=P1,P2"`; `route` the next fights with the enabler as a dead card. `[hyp]`
2. **Offer availability is the bottleneck:** best 20-card deck of any cards 0.62 vs best online deck 0.25. Buy access (elites, shops with gold kept for Act 2-3, ancients / Neow, card-choice events, boss potions): a +4 pick is worth less than a route adding an elite or shop before the boss. Test: `route` variants. `[sim]`
3. **Rares and relics are targets, not increments.** Build the target deck (section 4) and find the 2-3 cards that make it exist (Demon Form +12/+9 beat Tear Asunder +22 elites, +0 boss). Test: boss-alone `eval`. `[played]` run 3: boss pool ~0%, a +5 pick did not save the run.
4. **Interim survival.** The boss smooth score ignores the hallways and elites before the payoff; a "worse now" pick is valid only if it still reaches the boss gate HP. Test: `route` the next fights, `eval --boss --hp <arrival>`. `[hyp]`
5. **Draw dependence.** Prefer picks that survive a bad draw (acceleration, cheap cards, redundancy) over plans needing a specific draw. Test: plan failed after turn 1-2 draws: switch (section 5). `[hyp]`
6. **Engines the network may undervalue** (exhaust, Primal Force + Demon Form, 5-7 energy; high energy itself showed no policy gap `[sim]`). Test: theory says high ceiling but `eval` says ~0: more attempts, one fight with `adv 20`, `eval` variants adding Strength / Block / energy. `[hyp]`
7. **Deck size and consistency.** `remove=` variants price only the average; a 3-card combo may want a smaller deck. Test: combo assembly per fight in `review`. `[hyp]`
8. **Pickup and economy effects** (gold, max HP, pickup relics, events, shop prices, potion slots, Neow trades, curses taken for relics) are not in `eval` (curses: `add=CURSE_ID`). Test: price in HP / gold, compare with the combat delta. `[code]`
9. **Fight-specific rules the bucket view hides** (Slippery, Artifact, Thorns, Hard to Kill, Withers: `sts2-mechanics`). Test: boss-alone `eval`, never the pool average. `[code]`
10. **Time horizon.** Score carry-over (scaling, energy, draw, rares, relics, boss potions, upgrades). Once the current boss is above ~85% (or after the Act 1 boss) later horizons decide: +3 now and +12 later beats +8 now and -3 later. Test: `reward` next-act columns, `eval --future`. `[hyp]`

## 3. The five buckets
- FD front-loaded damage, SD scaling damage (Strength, Inferno, Rampage), FB front-loaded block, SB scaling block (Dexterity, Feel No Pain, Metallicize), ACC acceleration (energy, draw, cycling; Barricade belongs here: it saves block, it does not add it). A card can sit in several; one in none is a candidate to skip.
- Write the five-number line after every pick (`brief` prints it). Prefer the pick that raises the lowest bucket an upcoming fight asks about, unless the numbers show a large gain elsewhere. `[played]` FD 12, SD 3, FB 8, SB 0, ACC 3 (run 4) lost the long fights.

## 4. What each fight asks and the target deck
- Order: the act's boss, the elites that can still appear (`sts2-acts` bag), then hallways. Per fight: front-damage need, scaling need (long fight, enrage), block pattern (huge hit every N turns vs many small), conditions. Sources: `encounters.md`, `sts2-mechanics`, the enemy schedule (pass turns with a huge-HP player, read the intents).
- Target decks per later boss live in `encounters.md`; check every reward, shop and event against them first.
- Three horizons: current boss and elites left, next act, final act (A10: two bosses). `reward` prints the first two, `eval --future` all.

## 5. Plan and switch in a fight `[hyp]`
State the plan before each boss and elite (what wins it, which bucket, which condition) in the `-- why`. After turn 1-2, if the opening draw killed it, switch to the plan that uses the hand (powers bottom-decked: play 0-cost attacks to enable the three-attack relic). Solver for the micro, plan for the macro. Test: `review` shows whether the plan held.

## 6. Macro choices
- **Rest or smith:** `eval` at the HP after the rest vs now with the upgrade (Lagavulin: 42% at 44 HP vs 86% at 68 HP; best smith +8 `[sim]`): rest unless the HP gate is cleared. Smith the card that carries the plan (`upgrade=` variants).
- **Route:** `sts2-pathing`. **Relics and Neow:** score by the bucket filled and the route enabled. **Potions** beat cards in the same shop against hard bosses (`encounters.md`). Keep gold and potion slots for later acts once the current act is solved.

## 7. The override log
Log each override in the `-- why` as `override: <blind-spot number>` with the predicted payoff. After a run (`python -m agent.improve review`): held -> keep and tighten the test; failed -> narrow or delete with numbers; promote `[hyp]` to `[sim]` / `[played]` in the character or act skill. Open `[hyp]`: the bundle test (blind spot 1) vs smooth greedy over many sequences, and whether the elite-frequency picker can replace sim calls at pick time (`evidence.md`). Change one thing per run.
