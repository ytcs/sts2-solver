---
name: sts2-ironclad-act3
description: Use when entering or planning Act 3 (Glory) as the Ironclad, including the A10 second boss: the final pair as the target, elites, ancient and relic values, HP and potions through both bosses. Deviations only.
---

# Ironclad Act 3 (deviations from `sts2-plan`, `sts2-ironclad`)
- A3-1 [sim] The pair is the run and the second boss is the wall (E89): price every act-3 card, relic, removal and smith against both known bosses at full HP (D3). Test: instrument: `eval --boss` vs each boss.
- A3-2 [code] The search spends potions at boss 1 only where they beat their boss-2 value (P6); arrive with potions (E89). Test: outcome: potions at the pair, ledger.
- A3-3 [sim] Glory elites only when the pair is already likely: each costs act-3 HP (E87, E132, E145). Test: instrument: `routes` + `eval --boss` at arrival HP.
- A3-4 [hyp] Tanx: Claws over War Hammer for elites; Jeweled Mask plays a random Power (a second Power can hurt); Regal Pillow / Meal Ticket are HP engines (E133). Test: instrument: `eval --v "r|relics_add=ID"`.
