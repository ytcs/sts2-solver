---
name: sts2-ironclad-act2
description: Use when entering or planning Act 2 (Hive) as the Ironclad: hallway and elite HP cost, rest rule, The Insatiable deck, ancient and relic values. Deviations from the general strategy only.
---

# Ironclad Act 2 (deviations from `sts2-strategy`, `sts2-ironclad`)
Pools: `sts2-acts`; Insatiable, target deck, potions: `encounters.md`.

## HP economy `[sim]`
- Hive elites: 42-67% at 66 HP (mid deck); 94-99% at ~50% HP cost (Act 1-end deck); 99% at full HP.
- Rest before every elite: 84% at 43 HP vs 99% at 67. Test: `eval --elites --hp <now>` vs `<after rest>`.
- The Insatiable: ~30-50% for an Act 1-end deck; potions decide it: `potion aside` them.
- Hallways cost 20-40 HP, above prediction (10-30; Act 1 5-15) `[hyp]`. Test: `review` loss pct on Act 2 hallways.

## Items
- Ancient Tezcatara: Yummy Cookie (upgrade 4) +14 on elites; Biiig Hug loses to its Soot cards; Seal of Gold drains shop gold. `[sim]`
- Colorful Philosophers (3 off-class cards): best +5 +- 3.8, < 2 se `[sim]`.
- Darv's Ectoplasm stops all gold gain: spend gold first; price vs shops left.
- Insatiable target deck (97% at 80 HP): Primal Force x2 + Demon Form, Bag of Preparation, Bronze Scales, Kusarigama, Pael's Legion `[hyp]`. Test: `eval --boss --hp 80`.
- Book of Five Rings heals 20 on the 5th card added: at low HP take cheap cards `[hyp]`. Test: HP before/after (`s`).

## To test `[hyp]`
- An off-class card worth >= +10 on the Act 2 boss: scan `eval ... add=ID`.
- Smith vs rest before the Act 2 boss, per boss: `eval` with/without the upgrade at arrival HP.
