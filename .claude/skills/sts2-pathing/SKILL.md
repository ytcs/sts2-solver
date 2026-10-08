---
name: sts2-pathing
description: Use at every Neow / ancient choice and before every map click: floors as a scarce resource, route enumeration, scoring with route and eval, option-by-route table, the gate that binds, drawing the route, re-planning each node.
---

# Pathing and ancient / Neow (all characters and acts)
Neow/ancient option and path are one decision; a wrong path cannot be undone.

## Floors are a budget
- Rooms: monster (small HP, card, gold), elite (large HP, relic, extra rare with White Star), rest (heal 30% max HP or smith), shop (only with gold for it), unknown (event/fight/shop/treasure), treasure (relic), boss.
- Score a route: rewards (cards, relics, gold, upgrades, removals) vs HP cost + death risk, ending with HP + deck at the boss. The pre-boss rest sets boss HP.
- Binding limits: HP, gold, deck slots, potion slots. Find the binding one; spend the others.

## Procedure (every act, at the ancient)
1. `m` (whole map visible at Neow/ancient). Keep 3-5 paths that differ in kind: safe, balanced, elite-heavy, shop-first, rest-late.
2. `routes` (whole map) and/or `route <tokens> --hp <HP> --act <Act> [--exclude seen]` per path. Fights + rests only: price by hand card rewards, gold vs shops, relics, unknowns.
3. Option-by-route table: score each Neow/ancient option on each route. Ranking never flips -> best option then its best route; flips -> choose the pair. Combat-changing options via `eval`; route-changing (gold, healing, boss-reward relics) by hand.
4. Commit the pair; `draw` the route before the first click.
5. Re-plan each node with HP, gold, deck, encounters seen (bag: `sts2-acts`); write changes in the why.
6. Before every click: read every option, gold vs target shop, HP vs elite (`eval` at current HP before an elite/boss click below 60% HP; needs `!`), next rest. Never chain map choices. `routes` at every fork (harness enforces).

## The gate that binds `[sim]`
- Bosses shown on the map (`m`): plan vs that boss (`eval --boss`; `--all` whole pool). A10 final act: two bosses in sequence; plan HP + potions for both.
- Work back from the boss: `eval --boss --hp 34/45/60/80`. Steep in HP = HP-gated: rests, avoid late costly elites. Flat and low = damage-gated: spend HP on elites, pick damage. Re-test every few picks.

## Forks and elites
- Forks are option value: prefer a lane keeping a fork at the next 2-3 nodes over a locked corridor unless the corridor ends clearly better; commit late. `[hyp]` Test: did a kept fork change the route (`review`)?
- Elite rewards `[code]` (`CardRarityOdds.cs`, A10): rare per card 1.5% normal, 5% elite, 4.5% shop, plus offset from -5%, +0.5% per card rolled, reset on rare (cap +40%); uncommon 37/40/37%; boss reward 100% rare. Elite = guaranteed relic + ~3x rare odds + more gold.

## Rules of thumb `[hyp]` (test: `routes` + `eval --boss --hp <arrival>` over candidate routes; outcome in `review`)
- Rest before a boss or planned elite chain when HP binds; smith when HP is spare. Rest between two elites.
- Shop only if gold buys something `eval`/removal value justifies; else unknown or a card fight.
- Vs a damage-gated boss pool an elite's relic + card is cheap HP.
- Neow/ancient: the option moving boss-pool win or the route's reward budget most; gold is worth the shops on the route; a curse = one slot (`add=CURSE_ID`).

## Forced lanes `[hyp]`
- A chain of single-child nodes is a commitment: before entering, price the closing elite at arrival HP after the chain and any HP-costing event (`routes` or `route`). Elite < ~60% win at expected arrival -> other lane or skip the HP event.
- An HP-costing event is priced in the HP the next elite sees.

## Risk budget `[hyp]`
- Act-boss survival is a constraint, not the goal: while `routes` shows extra elites at >= ~0.9 boss win, take them. Act boss saturated (`eval --boss --smooth` >= 0.95): the next-act pool is short; surplus HP buys it.
- Never two elites without a rest between; second elite mean arrival < ~50% max HP = missing rest. Read q10 arrival HP, not the boss-win column alone.
- Test: next-act column after each elite relic/rare vs boss-win points given up; an elite costing 0.05-0.10 of a saturated boss win must buy more in the next-act pool.
