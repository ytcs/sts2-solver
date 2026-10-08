---
name: sts2-ironclad-act1
description: Use when entering or planning Act 1 (Overgrowth or Underdocks) as the Ironclad: pick values, boss HP gates, routing numbers. Deviations from the general strategy only.
---

# Ironclad Act 1 (deviations from `sts2-strategy`, `sts2-ironclad`)
Pools: `sts2-acts`; bosses: `encounters.md`.

## Picks (256 attempts, elite or boss pool) `[sim]`
- Inferno+ +25, Anger +17..+25, Setup Strike +21, Dismantle +13, Perfected Strike +11 (weak deck) / HP -5 (strong deck), Rampage HP -17. Blood Wall -5 -> +10 once Inferno is in.
- Built deck vs Lagavulin: Breakthrough, Rupture, Vicious, Molten Fist, Cinder, Body Slam, second Anger <= 0 (Cinder +10 vs Waterfall Giant). Inflame +42, Inflame+Tremble +73 vs Lagavulin alone while the elite pool showed ~0: evaluate the known boss alone.
- Primal Force (Hefty Tablet): Act 1 elites 22 -> 99%, Waterfall 0 -> 22%, second copy +18 on Hive elites. Vs Waterfall: Shrug +18, Taunt +15, Radiant Tincture +22 (single use), Centennial Puzzle +10.5.
- Hellraiser is playable (powers sync).

## HP gates `[sim]`
- Weak deck: damage-gated; boss win 18% at 49 HP, 24% at 80.
- Inferno+/Anger/Rampage/Blood Wall deck: HP-gated; Overgrowth boss pool 96/66/28/19% at 80/60/45/34 HP; Vantom 0.46 at 60, 0.11 at 45. Re-test every few picks: `eval --boss --hp 34/45/60/80`.
- Build for the weakest boss of the pool from the first reward (Waterfall Giant 45% at 79/80, se 2.8). `[hyp]` Test: boss-alone `eval` per boss every few picks.
- Pantograph (+25 HP at each boss) makes arrival HP cheap; without it each elite costs a rest. `[hyp]` Test: `routes` elite lanes with/without +25.

## Routing `[hyp]`
- Before a forced lane, count rests after its last elite (two elites + a regular for one rest is too few); `eval --boss --hp <arrival>`.
- Spoils Map (+600 gold at Act 2 treasure, one dead slot): gold alone moves no Act 2/3 boss; only with Act 2 shop targets. Test: `eval --next` of the basket.

## To test `[hyp]`
- Ancient choice vs the real map: option-by-route table (`sts2-pathing`).
- Elite count maximizing boss readiness: `route` variants, `eval --boss` at arrival HP.
- What beats Waterfall Giant: variants vs `WATERFALL_GIANT_BOSS` only.
- Strength Potion on the burst turn vs boss turn 1: `hindsight`.
