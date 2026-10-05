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

## Run 20261004-215036 (Overgrowth, boss Vantom) `[played]` / `[sim]`
- Died to Vantom (floor 17) at 34/80 HP; predicted 10.6% (se 1.7) at that HP. Deck win rate vs the Overgrowth boss pool by arrival HP: 96% at 80, 66% at 60, 28% at 45, 19% at 34; Vantom alone 0.46 at 60 HP and 0.11 at 45. With a strong damage deck (Inferno+, Anger, Rampage, Blood Wall) the boss became HP-gated, the opposite of the weak-deck run (damage-gated): re-test which gate binds after every few picks.
- HP path: 62 (smith Inferno+ instead of rest) -> elite Bygone Effigy 53 -> 26 (all 3 potions used) -> rest 50 -> Phrog elite at 35 -> 23 -> Mawler (regular) 23 -> 10 -> rest 34. The last third of the lane had one rest and two elites; the forced left lane (no shops, no choice after r11) was committed at r1 without reading what its end looked like. Plan the rests against the boss's HP gate, not just the next fight.
- `[sim]` Inferno+ was +24.9 pts on the boss pool; Anger +25.5; Blood Wall flips from -5 to +10 once Inferno is in the deck (card value depends on the deck).
- Neow Phial Holster (3 potions) was fine; Morphic Grove "Group" (lose all gold, transform 2 Strikes) gave Hellraiser + One-Two Punch: Hellraiser desyncs the simulator (Slippery amount off by one after auto-played Strikes) so the solver's advice was void in the boss fight. Avoid Hellraiser until the simulator is fixed.

## To test `[hyp]`
- Ancient choice against the real Act 1 map: option-by-route table.
- Elite count that maximises boss readiness for a given early deck (this run: 2 elites, both won, relic + rare card each).
- A Waterfall Giant plan: what beats it (eval variants vs `WATERFALL_GIANT_BOSS` only).
