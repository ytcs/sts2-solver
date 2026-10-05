---
name: sts2-deckbuilding
description: Use at every card reward, shop, rest site, relic, upgrade, removal or transform: the pick protocol (smooth greedy numbers first, then the judgment pass for what the numbers cannot see), the five-bucket audit, what each fight asks, horizon and plan rules, the override log.
---

# Deck building = numbers first, then judgment where the numbers are blind

**Adopted pick rule `[sim]`:** price every option with the solver on the SMOOTH objective (win rate averaged over start HP x1 / 1.5 / 2 / 3 of the real HP), pick the best, never skip unless every option scores below the current deck. It beat the old "best win rate now, skip when no gain" rule 0.70 vs 0.31 (Lagavulin) and 0.18 vs 0.00 (Aeonglass), because against a hard fight every single card scores 0 at real HP and the old rule skipped everything. Lookahead variants (mock completion, sampled futures) were +20-30% but not significant at 6 sequences; a myopic beam was worse (`evidence.md`).

That rule is a greedy, additive, combat-only number. This skill is what a person adds on top. The expert-play observation behind it `[expert]` (JapaneseExport, youtube IqNkmsA0PLo): picks that look bad now ("it is basically a curse right now, trust me") are made for a plan; the skill is knowing when that is right.

## 1. Protocol (every card reward, shop item, relic, upgrade, removal, transform)
1. `brief` (deck, bucket line and gaps, relics, known boss, elites left). 
2. Numbers: on a card reward `reward` (one table: boss smooth, boss at full, elites left, next act, buckets filled). Anything else: `eval --smooth --boss|--elites|--next --v "name|add=ID|remove=ID|upgrade=ID|relics_add=ID"`. Removals, upgrades, relics, potions and transforms are variants too.
3. Judgment pass: walk the blind-spot list in section 2. Each item that changes the choice is an OVERRIDE; none is the default (take the numbers).
4. Act in one call with the reason: `a ~card N -- "<bucket line>; gap; best option +x (smooth), chosen option; override: <blind spot> or none"`.

Reading the numbers: a difference under 2 standard errors (~0.05 at 96 attempts) is noise, so within noise the tie-break is the bucket gap and plan fit, not the sign of the difference. Value is not additive (Blood Wall -5 -> +10 once Inferno was in the deck): re-price options after every pick.

## 2. What the numbers cannot see (the blind-spot list; each has a test)
1. **Enablers and payoffs that are dead until the partner arrives** (Barricade, Demon Form with few hits, Feel No Pain without exhaust, Dark Embrace, Corruption, a scaling card in a deck without multi-hit). Myopic rules score them <= 0; experts take them for the plan. Test: price the bundle, `--v "enabler|add=E" --v "bundle|add=E,P1,P2" --v "payoff alone|add=P1,P2"`. Take the enabler only if the bundle gain is large AND at least two payoffs are in the deck, or likely to be met (rares in a known shop/ancient, elite rewards) AND the interim cost is survivable (`route` the next fights with the enabler as a dead card). Otherwise skip it. Write which partners you are counting on.
2. **Offer availability is the bottleneck, not choice quality.** The best 20-card deck of any cards beats the best online deck 0.62 vs 0.25 (`evidence.md`). So buy access to power: elites (higher rare odds, always a relic), shops (keep gold for Act 2-3 shops), ancients/Neow, events with card choices, potions for the boss. A pick that is +4 is worth less than a route that adds an elite or shop before the boss.
3. **Rares and relics are targets, not increments.** When the boss pool is at ~0% a +5 pick will not save the run (run 3). Build the target deck for the boss (section 4) and look for the 2-3 cards that make it exist: Demon Form +12/+9 beat marginal elite-HP picks (Tear Asunder +22 elites, +0 boss) when the boss is the gate.
4. **Interim survival.** The boss smooth score ignores the hallway fights and elites before the payoff. A "worse now" pick is only valid if `route` still arrives at the boss gate HP; compute it, do not assume it.
5. **Draw dependence and robustness.** The average hides plans that need a specific draw (three attacks in a turn, a power on turn 1). Prefer picks that survive a bad draw (acceleration, cheap cards, redundancy) over one more condition; if a plan fails after turn 1-2, switch plans (section 5).
6. **Engines the network may undervalue.** Novel synergy decks (exhaust engine, Primal Force + Demon Form, 5-7 energy) are a small share of the training mix. If the theory says high ceiling but `eval` says ~0, run `eval` with more attempts, check one fight by hand with `adv 20`, and look at which resource binds (`eval` variants adding Strength / Block / energy) before dismissing it. High energy itself showed no policy gap `[sim]`.
7. **Deck size and consistency.** Removals and thin decks raise how often the plan assembles; `remove=` variants price it, but only on the average. A deck with a 3-card combo wants a smaller deck than the numbers' one-card-at-a-time view suggests.
8. **Pickup and economy effects.** Gold, max HP, relics that act on pickup, events, shop prices, potion slots, Neow trades and curses accepted for relics are not in `eval` as costs or gains (curses can be added as `add=CURSE_ID`, the rest by hand). Price them in HP/gold terms and compare with the combat delta.
9. **Fight-specific rules the sim already knows but the bucket view hides**: Slippery, Artifact (debuffs blocked), Thorns (hitting it hurts), Hard to Kill, Withers (see `sts2-mechanics`). A card can be strong in general and zero against the boss: the boss-alone `eval` is the check, never the pool average.
10. **Time horizon.** Score carry-over: scaling, energy, draw, rares, relics, potions for later bosses, upgrades of the carry cards. Once the current boss is above ~85% (or after the Act 1 boss) the later horizons decide: +3 now and +12 later beats +8 now and -3 later.

## 3. The five buckets (the audit that tells you what to ask of the numbers)
- **Front-loaded damage** (kills quickly), **scaling damage** (Strength, Inferno, Rampage), **front-loaded block**, **scaling block** (Dexterity, Feel No Pain, Metallicize), **acceleration** (energy, draw, cycling; Barricade belongs here: it saves block, it does not add it). A card can sit in several; one in none is a candidate to skip.
- Write the five-number line after every pick (`brief` prints it with the gaps from `data/card_buckets_ironclad.json`). Do not over-index: run 4's deck had strong front damage and Strength but no scaling block and lost the long fights. Prefer the pick that raises the lowest bucket a known upcoming fight asks about, unless the numbers show a large gain elsewhere. Tags propose; `eval` decides.

## 4. What each fight asks and the target deck
- For the act's boss first (known from the map), then the elites that can still appear (`sts2-pathing` bag logic), then hallways: front-loaded damage need, scaling need (long fight, enrage), block pattern (one huge hit every N turns, many small hits), and conditions. Source: `sts2-acts`, `encounters.md`, `sts2-mechanics`, the enemy schedule (pass turns with a huge-HP player and read the intents).
- Keep a written target deck per later boss in the act skill (The Insatiable: Strength scaling + burst + acceleration, Demon Form + Bludgeon + Tremble + Offering = 60% vs 2%; Aeonglass/Queen: high sustained damage plus HP carry; for Aeonglass the best found deck is a self-damage Strength engine, Inferno + Rupture + Demon Form + Fight Me + Spite + Stone Armor + Crimson Mantle, not an exhaust deck). Every reward, shop and event is checked against that list first.
- Evaluate at three horizons (current boss and elites left, next act's pools, final act; at A10 the last act has two bosses in a row): `reward` prints the first two, `eval --future` all.

## 5. Plan and switch in a fight
State the plan before each boss and elite (what wins it, which bucket, which condition). After turn 1-2 ask whether the opening draw killed it; if so switch to the plan that uses the hand (powers bottom-decked: play the 0-cost attacks to enable the three-attack relic instead of blocking). A solver line that contradicts the plan is data: trust the solver for the micro, change the plan for the macro.

## 6. Macro choices
- **Rest or smith:** `eval` at the HP after the rest vs now with the upgrade (Lagavulin at 44 HP 42% vs at 68 HP 86%, best smith +8): rest unless the HP gate is cleared. Smith the card that carries the plan (`upgrade=` variants).
- **Route:** work back from the boss's HP gate (`eval --hp` at 34/45/60/80), arrive above it with the buckets filled (`sts2-pathing`). **Relics and Neow:** score by the bucket they fill and the route they enable. **Potions** against hard bosses beat cards in the same shop (Powdered Demise +14, Flex +11, Block Potion +12).
- Keep gold and potion slots for later acts when the current act is solved.

## 7. Making it learn (the override log)
- Every override from section 2 is logged in the `-- why` as `override: <number of the blind spot>` with the predicted payoff. After a run (`python -m agent.improve review`), check each override against what happened: held -> keep and tighten the test; failed -> narrow or delete it, with the numbers. Rules that predicted well are promoted from `[hyp]` to `[sim]` / `[played]` in the character or act skill.
- Open hypotheses to settle with data, not argument: the value of the bundle test (blind spot 1) against smooth greedy over many sequences (`evidence.md`, round 2), and whether the elite-frequency picker can replace sim calls at pick time.
- Change one thing per run when possible, so the effect can be read. Evidence and numbers behind every rule: `evidence.md`.
