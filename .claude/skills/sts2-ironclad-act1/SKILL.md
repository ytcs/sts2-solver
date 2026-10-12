---
name: sts2-ironclad-act1
description: Use when entering or planning Act 1 (Overgrowth or Underdocks) as the Ironclad: pick values at the act-1 binding fights, boss HP gates, elite access. Deviations only.
---

# Ironclad Act 1 (deviations from `sts2-plan`, `sts2-ironclad`)
- A1-1 [sim] Act-1 binding fights pay front damage: Anger / Setup Strike / Dismantle +1.3 to +2.6 win points at act-1 elites and boss, Shrug It Off ~0 at the boss (E85); take them for the act-1 boss, not for later (P2). Test: instrument: `eval --boss --v "x|add=ID"`.
- A1-2 [sim] Act-1 elites cost ~4 win points of act-1 boss HP each (13 HP x ~0.4 carried x slope 0.008) and pay through relic and rare odds (E85, P5): take them while `routes` keeps the boss >= 0.9. Test: outcome: act-1 elites vs act-2 reach, ledger.
- A1-3 [hyp] Build toward the weakest boss of the pool from the first reward; once the boss is shown, price picks against it alone. Test: instrument: boss-alone `eval` per boss.
- A1-4 [hyp] Primal Force (Hefty Tablet) makes act-1 elites near-safe (earlier model: 22 -> 99%). Test: instrument: `eval --elites --v`.
- A1-5 [hyp] Before a forced lane count the rests after its last elite; Pantograph (+25 HP at bosses) makes arrival HP cheap. Test: instrument: `eval --boss --hp <arrival>` with / without +25.
