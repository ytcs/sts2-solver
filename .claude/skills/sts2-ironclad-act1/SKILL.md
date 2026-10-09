---
name: sts2-ironclad-act1
description: Use when entering or planning Act 1 (Overgrowth or Underdocks) as the Ironclad: pick values, boss HP gates, routing numbers. Deviations from the general strategy only.
---

# Ironclad Act 1 (deviations from `sts2-strategy`, `sts2-ironclad`)
- Picks `[sim]` (256 attempts, elite/boss pool): Inferno+ +25, Anger +17..+25, Setup Strike +21, Dismantle +13; Blood Wall -5 -> +10 once Inferno is in. Price the known boss alone: pool averages hide it.
- Primal Force (Hefty Tablet) `[sim]`: act-1 elites 22 -> 99%, Waterfall Giant 0 -> 22%.
- HP gates `[sim]`: weak deck damage-gated (boss 18% at 49 HP, 24% at 80); Inferno/Anger deck HP-gated (Overgrowth pool 96/66/28/19% at 80/60/45/34). Re-test `eval --boss --hp 34/45/60/80`.
- `[hyp]` build for the weakest boss of the pool from the first reward; test: boss-alone `eval` per boss.
- `[hyp]` Pantograph (+25 HP at bosses) makes arrival HP cheap; test: `routes` with/without +25.
- `[hyp]` before a forced lane count rests after its last elite; test: `eval --boss --hp <arrival>`.
