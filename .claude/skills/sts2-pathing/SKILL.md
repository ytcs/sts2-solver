---
name: sts2-pathing
description: Use at every Neow / ancient choice and before every map click: floors as a scarce resource, route enumeration, scoring with route and eval, option-by-route table, the gate that binds, drawing the route, re-planning each node.
---

# Pathing and ancient / Neow (shared by all characters and acts)

The Neow / ancient option changes which path is best: decide the pair. A wrong path cannot be undone.

## Floors are a budget
- Rooms: monster (small HP cost, card, gold), elite (large HP cost, relic, extra rare with White Star), rest (heal 30% max HP, or smith), shop (enter only with the gold for it), unknown (event, fight, shop or treasure), treasure (relic), boss.
- Score a route by rewards (cards, relics, gold, upgrades, removals) against HP cost and death risk, ending with the HP and deck brought to the boss. Count fights, elites, rests, shops, unknowns and where the rests fall relative to the elites; the pre-boss rest sets the HP the boss sees.
- Binding limits: HP (heals are rests; Burning Blood-type relics add a little per fight), gold, deck slots, potion slots. Find the binding one; spend the others freely.

## Procedure (every act, at the ancient)
1. Read the whole map (`m`; fully visible at the Neow / ancient screen; prints each node's children). Keep the 3-5 paths that differ in kind: safe, balanced, elite-heavy, shop-first, rest-late.
2. For each, write the room sequence and run `route <tokens> --hp <HP> --act <Act> [--exclude seen]` (win probability per node, HP after each). `route` prices fights and rests only; price by hand the card rewards (number of picks), gold vs shop positions, relics from elites / treasure, unknowns as a mix.
3. **Option-by-route table:** score every Neow / ancient option on each candidate route. If the option ranking never flips across routes, take the best option, then its best route; if it flips, choose the pair. Price an option with `eval` where it changes combat (relic, transform, card), by hand where it changes the route (gold -> shop value, healing -> rest value, Lava-Rock-style boss rewards only if the boss is likely reached with HP).
4. Commit the pair, then `draw` the route before the first click.
5. Re-plan at every node with updated HP, gold, deck and the encounters already seen (bag: `sts2-acts`); write the reason for any change in the `-- why`.
6. Before every click: read every option, check gold against the target shop, HP against the elite (`eval` at the current HP before a click onto an elite or boss below 60% HP, which needs `!`), and where the next rest is. Never chain map choices (`sts2-harness`).

## The gate that binds `[sim]`
- Bosses are shown on the map (`m`: `boss: <row> <ID> [+ <ID>]`): plan against that boss only (`eval --boss`; `eval --pool <Act>:boss` and `route` narrow to it, `--all` for the whole pool). At A10 the final act has two bosses in sequence: plan HP and potions for both. Elite and regular narrowing: `sts2-acts`.
- Work back from the boss: `eval --boss --hp 34/45/60/80`. Win rate climbing steeply with HP = HP-gated: plan rests, avoid costly elites late. Flat and low = damage-gated: spend HP on elites for relics and cards, pick damage. Re-test every few picks (`sts2-ironclad-act1`: weak deck damage-gated, strong deck HP-gated).

## Forks and elites
- **Forks are option value** `[hyp]`: a node with two or more children keeps options open for later information (HP, gold, shop stock, rewards). Prefer a lane that keeps a fork at the next 2-3 nodes over a locked corridor, unless the corridor ends somewhere clearly better; commit as late as possible. Test: after a run, did a kept fork change the route (`review`)?
- **Elite rewards `[code]`** (`CardRarityOdds.cs`; A10 Scarcity): rare chance per card 1.5% in normal fights, **5% in elites**, 4.5% in shops, plus a rarity offset that starts at -5%, grows +0.5% per card rolled, resets after a rare (cap +40%); uncommon 37% / 40% / 37%; boss reward 100% rare. An elite always gives a relic and more gold: ~3x a monster's rare odds plus a guaranteed relic. Weigh elites as rewards against their HP cost and the rests that pay it.

## Rules of thumb `[hyp]` (test: `route` plus `eval --boss --hp <arrival HP>` over candidate routes; compare with the outcome in `review`)
- Rest before a boss or a planned elite chain when HP binds; smith when HP is spare. Two consecutive elites need a rest between them.
- A shop is worth the floor only if gold covers something `eval` or removal value justifies; else take the unknown or the fight that rewards a card.
- Against a damage-gated boss pool an elite's relic + card is cheap HP.
- Neow / ancient: take the option that moves the boss-pool win rate or the route's reward budget most; a large gold sum is only as good as the shops on the route; a curse is one slot (`add=CURSE_ID`).

## Forced lanes `[hyp]`
- A chain of single-child nodes is a commitment. Before the click that enters it, price the elite that ends it at the HP you will arrive with, after the HP the chain and any HP-costing event take: `python -m agent routes` (survival over the whole map, per option and per elite requirement) or `route` over the chain. An elite below ~60% win at the expected arrival HP means another lane, or skipping the HP-costing event.
- An event that costs HP is priced in the HP the next elite sees, not only in what it gives.

## Risk budget `[hyp]`
- Rule: survival of the act boss is a constraint, not the goal. While `python -m agent routes` shows the route with the extra elites at >= ~0.9 boss win, take the elites (relic + rare odds + gold); once the act boss is saturated (`eval --boss --smooth` >= 0.95) the next-act pool is what is short, and surplus HP is the currency that buys it.
- Guard: never plan two elites with no rest between them, whatever the boss-win column says: `routes` prints each elite's arrival HP (alive, mean, q10) under every representative route; the second elite's mean below ~50% of max HP means a rest is missing. The boss-win column hides variance: read the q10 arrival HP.
- Test: `reward` / `eval --next --smooth` next-act column after each elite relic and rare vs the boss-win points the route gave up; an elite that cost 0.05-0.10 of a saturated boss win must have bought more than that in the next-act pool.
- Status: unmeasured; `routes` prices only the survival side (rewards are counts).
