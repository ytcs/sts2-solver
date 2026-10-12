---
name: sts2-ironclad-act3
description: Use when entering or planning Act 3 (Glory) as the Ironclad, including the A10 second boss: the final pair as the target, elites, ancient and relic values, HP and potions through both bosses. Deviations only.
---

# Ironclad Act 3 (deviations from `sts2-plan`, `sts2-ironclad`)
- A3-1 [sim] The pair is the run: price every act-3 card, relic, removal and smith against both known bosses at full HP (D3); the first boss is won 50% when reached, the second 1 in 10 (E89): the second boss is the wall. Test: instrument: `eval --boss` vs each boss.
- A3-2 [code] The search spends potions at boss 1 only where they beat their boss-2 value (P6); arrive with potions (E89). Test: outcome: potions at the pair, ledger.
- A3-3 [hyp] Glory elites only when the pair is already likely (each costs act-3 HP, E87: act-3 elites -32 HP on a win). Test: instrument: `routes` + `eval --boss` at arrival HP.
- A3-4 [hyp] Tanx: Claws over War Hammer for elites; Jeweled Mask plays a random Power (a second Power can hurt); Regal Pillow / Meal Ticket are HP engines (earlier model). Test: instrument: `eval --v "r|relics_add=ID"`.
