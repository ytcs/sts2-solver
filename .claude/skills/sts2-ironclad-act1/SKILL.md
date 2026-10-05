---
name: sts2-ironclad-act1
description: Ironclad Act 1 (Overgrowth / Underdocks) deviations and run evidence: boss-specific prep (Waterfall Giant, Soul Fysh, Lagavulin Matriarch), Neow and shop results. Load on entering Act 1 as the Ironclad.
---

# Ironclad Act 1 (additions to `sts2-strategy`, `sts2-ironclad`)

Pools and room counts: `sts2-acts`.

## Run 20261004-214119 (seed Y6HTLYNN5P7S, Underdocks) `[played]` / `[sim]`
- Died to Waterfall Giant at floor 17 with 79/80 HP after Pantograph. Predicted 45% (se 2.8, 320 fights): a coin flip, not a bug. Deck: 22 cards (Greed, Clumsy, Ascender's Bane; Hemokinesis, Perfected Strike, Dismantle, Twin Strike, 2 Breakthrough, Thunderclap, Pommel, Shrug, Bash+).
- Boss pool end-of-act win by boss for that deck: Soul Fysh 0.98, Lagavulin Matriarch 0.77, Waterfall Giant 0.43-0.50 (the weak link). Build for the weakest boss from the first card reward; the full boss pool is in `sts2-acts`.
- Neow Cursed Pearl (+333 gold) bought Pantograph (+25 HP at each boss, 54 -> 79 on entry), a Breakthrough, Armaments and a removal by floor 4: good value. The next shop was entered with 40 gold and bought nothing.
- Pathing: the Neow option was picked in one step without enumerating routes or drawing the map `[played]`; the route happened to give a shop at floor 4, but no option-by-route table was made. Do it per `sts2-pathing` next run.
- Strength Potion was used on the boss's first turn by the solver `[hyp: holding it for the burst turn is better]`.

## To test `[hyp]`
- Ancient choice against the real Act 1 map: option-by-route table.
- Elite count that maximises boss readiness for a given early deck (this run: 2 elites, both won, relic + rare card each).
- A Waterfall Giant plan: what beats it (eval variants vs `WATERFALL_GIANT_BOSS` only).
