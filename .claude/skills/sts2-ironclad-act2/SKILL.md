---
name: sts2-ironclad-act2
description: Use when entering or planning Act 2 (Hive) as the Ironclad: hallway and elite HP cost, rest rule, The Insatiable deck, ancient and relic values. Deviations from the general strategy only.
---

# Ironclad Act 2 (deviations from `sts2-strategy`, `sts2-ironclad`)

Pools: `sts2-acts`; Insatiable row, target deck, potion numbers: `encounters.md`.

## HP economy `[played]` / `[sim]`
- Hallways cost 20-40 HP (predicted 10-30; Act 1: 5-15), runs 3-4. Hive elites: 42-67% at 66 HP (mid deck), 94-99% at ~50% HP cost (Act 1 end deck), 99% at full HP (run 4).
- Rest before every elite: 84% at 43 HP vs 99% at 67 HP (run 3). Test: `eval --elites --hp <now>` vs `<after rest>`.
- The Insatiable: ~30-50% for an Act 1 end deck; potions decide it (`encounters.md`); `hold` them for it.

## Items
- Run 4 beat The Insatiable at 97% (80 HP) with Primal Force x2, Demon Form (elite reward: +33.6 vs 7.8% base), Bag of Preparation +9.8, Bronze Scales +10.9 (3 Thorns), Kusarigama, Pael's Legion.
- Ancient Tezcatara (run 3): Yummy Cookie (upgrade 4) +14 on elites; Biiig Hug loses to the Soot cards it adds; Seal of Gold drains shop gold.
- Book of Five Rings heals 20 when a 5th card is added (full heal at 10 HP): with low HP take cheap cards `[played]`.
- Colorful Philosophers (3 cards of another class): best +5 +- 3.8, under 2 se `[sim]`.
- Darv ancient vs Kaiser Crab (96 attempts): Ectoplasm +0.118 smooth, Philosopher's Stone +0.104, Snecko Eye +0.095; Hive elites 0.87 / 0.67 / 0.90 (Entomancer 0.61 / 0.19 / 0.85). Ectoplasm ends gold: spend it first `[sim]`.

## To test `[hyp]`
- Another class's pool holds a card worth >= +10 on the Act 2 boss: scan every card with `eval ... add=ID`.
- Smith vs rest before the Act 2 boss, per boss: `eval` with and without the upgrade at arrival HP.
