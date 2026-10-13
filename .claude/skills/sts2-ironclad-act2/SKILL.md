---
name: sts2-ironclad-act2
description: Use when entering or planning Act 2 (Hive) as the Ironclad: the skip default, hallway and elite HP cost, rest rule, The Insatiable and Kaiser Crab, ancient and relic values. Deviations only.
---

# Ironclad Act 2 (deviations from `sts2-plan`, `sts2-ironclad`)
- A2-1 [sim] Front-damage commons are worth ~0 at act-2 binding fights and negative later, upgraded or not (E85): skip them (D1). Test: instrument: `eval --boss --v` vs the act-2 boss and the pair.
- A2-2 [sim] Act-2 hallways cost ~8.6 HP on a win, more when started below half HP (E87, E93, E94): rest before an elite when HP binds (R3). Test: instrument: `restcalc`.
- A2-3 [hyp] The Insatiable: ~65 damage / turn for 5 turns; potions decide it (P6). Test: instrument: `eval --enc THE_INSATIABLE_BOSS`.
- A2-4 [hyp] Tezcatara: Yummy Cookie helps elites; Biiig Hug loses to its Soot cards. Darv's Ectoplasm stops gold gain: spend first. Test: instrument: `eval --elites --v`.
