---
name: sts2-pathing
description: Use at every Neow / ancient choice and before every map click: floors as a scarce resource, route enumeration, scoring with route and eval, option-by-route table, the gate that binds, drawing the route, re-planning each node.
---

# Pathing and ancient / Neow (all characters and acts)
Neow/ancient option and path are one decision. Binding limits: HP, gold, deck slots, potion slots: find the binding one, spend the others.

## Procedure (every act, at the ancient)
1. `m`; keep 3-5 paths that differ in kind (safe, elite-heavy, shop-first, rest-late).
2. `routes` and/or `route <tokens> --hp <HP> --act <Act>` per path (fights + rests; price rewards, gold, unknowns by hand).
3. Option-by-route table: ranking never flips -> best option then route; flips -> choose the pair. Combat options via `eval`.
4. `draw` the route before the first click; re-plan each node (bag: `sts2-acts`).
5. Before every click: gold vs shop, HP vs elite (`!` below 60%), next rest. `routes` at every fork.

## The gate that binds `[sim]`
Work back from the shown boss(es): `eval --boss --hp 34/45/60/80`. Steep = HP-gated (rests, avoid late elites); flat and low = damage-gated (elites, damage picks). A10 final act: two bosses.

## Rules `[hyp]` (test: `routes` + `eval --boss --hp <arrival>`; outcome in `review`)
- Prefer lanes that keep a fork in the next 2-3 nodes; commit late.
- Elite = relic + ~3x rare odds `[code]` (`docs/research/game_code.md` C2): take extra elites while `routes` shows boss win >= ~0.9; never two without a rest between (second arrival < ~50% = missing rest); read q10 arrival HP.
- Rest before a boss or elite chain when HP binds; else smith.
- Shop only when gold buys something priced worth it.
- Forced lane: price its closing elite at arrival HP after the chain and HP events; < ~60% win -> other lane.
- Neow/ancient: the option moving boss-pool win or route budget most; a curse = one slot.
