---
name: sts2-pathing
description: How to plan the map route and choose the Neow / ancient option in Slay the Spire 2 as one co-optimized decision: floors as a scarce resource, candidate-route enumeration, scoring with route/eval, drawing the route, option-by-route tables, re-planning per node. Load at every Neow / ancient and before every map click.
---

# Pathing and ancient / Neow (shared by all characters and acts)

Each act has a fixed number of floors; every floor is a resource, and a route is a purchase of which rooms to spend them on. A wrong path cannot be undone, and the Neow / ancient option changes what the best path is. Deliberate here longer than anywhere else in the run: this is where a run is won before the first fight.

## Floors are a budget
- A route is a sequence of room types: monster (regular fight: small HP cost, a card, gold), elite (large HP cost, relic + extra rare with White Star, strong card), rest (heal 30% max HP, or smith, or a Dream-Catcher-style bonus), shop (gold -> cards / relics / removal; only worth entering with the gold for it), unknown (event, fight, shop or treasure: variance), treasure (relic), boss.
- Score a route by what it buys against what it costs: rewards (cards, relics, gold, upgrades, removals) versus HP and risk of death, ending with the HP and deck I bring to the boss. Elites are the main source of power and the main source of risk; rests and the pre-boss rest decide how much HP the boss sees. Count how many fights, elites, rests, shops and unknowns each candidate has, and where the rests fall relative to the elites.
- Limits that bind: HP (heals are rests only; Burning Blood-type relics add a little per fight), gold (shops need it), deck slots, potion slots. Know which one is binding for this route and spend the others freely.

## Procedure (every act, at the ancient)
1. Read the whole map (`m`; it is visible at the Neow / ancient screen). List the start nodes and enumerate the distinct paths (the graph is small: `m` prints the children of each node). Keep the 3-5 paths that differ in kind: safe, balanced, elite-heavy, shop-first, rest-late.
2. For each candidate, write its room sequence and run `route <tokens> --hp <HP> --act <Act> [--exclude seen]` for the fights (win probability per node, HP after each, product). `route` prices fights and rests only; price the rest by hand: card rewards (counts of picks), gold vs shop positions (can I afford the shop when I get there?), relics from elites / treasure, unknowns as a mix.
3. Option-by-route table: score every Neow / ancient option on each candidate route. If the ranking of options never flips across routes, choose the best option, then the best route for it. If it flips, choose the pair. Price each option with `eval` where it changes combat (a relic, a transform, a card) and by hand where it changes the route (gold -> shop value, healing -> rest value, Lava-Rock-style boss rewards -> only worth it if the boss is likely to be reached with HP).
4. Commit the pair, then `draw` the chosen route on the in-game map before the first click, so the plan is visible and a later deviation is a decision, not drift.
5. Re-plan at every node: after each room, compare the remaining paths from here with updated HP, gold, deck and the encounters already seen (the bag does not repeat them until it empties). Change the plan only for a reason that I write in the `-- why`.
6. Before every click: read every option on the map screen, check gold against the shop I am routing to, HP against the elite (a click onto an elite or boss below 60% HP needs `!`: only after `eval` at the current HP), and where the next rest is. Never chain map choices.

## Rules of thumb `[hyp]` unless marked
- Rest before a boss or a planned elite chain when HP binds; smith when HP is spare. Two consecutive elites need a rest between them.
- A shop is worth the floor only if gold covers something `eval` or removal value justifies; otherwise take the unknown or the fight that rewards a card. `[played]`: entered a shop with 40 gold and bought nothing.
- Elites for the boss: against a boss pool that is damage-gated, an elite's relic + card is cheap HP (`sts2-strategy`); verify with `eval` at the HP I would arrive with.
- Neow / ancient: take the option that moves the boss-pool win rate or the route's reward budget the most; a large gold sum is only as good as the shops on the chosen route; a curse is one deck slot, price it.

Character and act files record the per-act evidence (which route shapes worked, option-by-route results).
