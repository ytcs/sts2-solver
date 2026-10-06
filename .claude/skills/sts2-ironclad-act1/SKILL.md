---
name: sts2-ironclad-act1
description: Use when entering or planning Act 1 (Overgrowth or Underdocks) as the Ironclad: pick values, boss HP gates, routing numbers. Deviations from the general strategy only.
---

# Ironclad Act 1 (deviations from `sts2-strategy`, `sts2-ironclad`)

Pools: `sts2-acts`; boss rows: `encounters.md`.

## Picks (256 attempts, elite or boss pool) `[sim]`
- Inferno+ +25, Anger +17..+25, Setup Strike +21, Dismantle +13, Perfected Strike +11 (weak deck) / HP -5 (strong deck), Rampage HP -17. Blood Wall -5 -> +10 once Inferno is in the deck.
- Breakthrough, Rupture, Vicious, Molten Fist, Cinder, Body Slam, a second Anger: <= 0 vs Lagavulin once the deck was built (Cinder +10 vs Waterfall Giant). Inflame +42, Inflame+Tremble +73 vs Lagavulin alone while the elite pool showed ~0: evaluate the known boss alone.
- Primal Force (Hefty Tablet): Act 1 elites 22 -> 99%, Waterfall 0 -> 22%, a second copy +18 on Hive elites; vs Waterfall: Shrug +18, Taunt +15, Radiant Tincture +22 (single use), Centennial Puzzle +10.5.
- Hellraiser is playable: the Slippery 7-vs-6 desync was a replay sample (6/7/8 over 40 shuffles), powers now sync.

## HP gates `[sim]`
- Weak deck (starter + few cards): damage-gated; boss win 18% at 49 HP, 24% at 80.
- Deck with Inferno+ / Anger / Rampage / Blood Wall: HP-gated; Overgrowth boss pool 96 / 66 / 28 / 19% at 80 / 60 / 45 / 34 HP, Vantom alone 0.46 at 60, 0.11 at 45. Test every few picks: `eval --boss --hp 34/45/60/80`.
- Build for the weakest boss of the pool from the first reward (Waterfall Giant: 45% at 79/80 HP, se 2.8, 320 fights). `[hyp]` Test: boss-alone `eval` for each boss of the pool every few picks.
- Pantograph (+25 HP at each boss) makes arrival HP cheap; without it every elite costs a rest. `[hyp]` Test: `routes` elite lanes at the arrival HP with and without the +25.

## Routing `[hyp]`
- Before committing a forced lane, count the rests after its last elite (two elites and a regular fight for one rest is too few) and run `eval --boss --hp <arrival HP>`.
- Spoils Map (+600 gold at Act 2 treasure, one dead slot): the gold alone moves no Act 2/3 boss; take it only with Act 2 shop targets. Test: `eval --next` of the basket the gold buys.

## To test `[hyp]`
- Ancient choice against the real map: option-by-route table (`sts2-pathing`).
- Elite count that maximises boss readiness: `route` variants, `eval --boss` at arrival HP.
- What beats Waterfall Giant: `eval` variants against `WATERFALL_GIANT_BOSS` only.
- Hold the Strength Potion for the burst turn instead of boss turn 1: compare with `hindsight`.
