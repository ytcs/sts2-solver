---
name: sts2-ironclad-act2
description: Use when entering or planning Act 2 (Hive) as the Ironclad: hallway and elite HP cost, rest rule, The Insatiable deck, ancient and relic values. Deviations from the general strategy only.
---

# Ironclad Act 2 (deviations from `sts2-strategy`, `sts2-ironclad`)

Pools: `sts2-acts`; Insatiable row, target deck, potion numbers: `encounters.md`.

## HP economy `[sim]`
- Hallways cost 20-40 HP, above the prediction (10-30; Act 1: 5-15) `[hyp]`; test: `review` loss pct on Act 2 hallways. Hive elites: 42-67% at 66 HP (mid deck), 94-99% at ~50% HP cost (Act 1 end deck), 99% at full HP.
- Rest before every elite: 84% at 43 HP vs 99% at 67 HP. Test: `eval --elites --hp <now>` vs `<after rest>`.
- The Insatiable: ~30-50% for an Act 1 end deck; potions decide it (`encounters.md`); `potion aside` them for it.

## Items
- Target deck for The Insatiable (97% at 80 HP): Primal Force x2 + Demon Form, Bag of Preparation, Bronze Scales (3 Thorns), Kusarigama, Pael's Legion `[hyp]`; test: `eval --boss --hp 80` of the deck.
- Ancient Tezcatara: Yummy Cookie (upgrade 4) +14 on elites; Biiig Hug loses to the Soot cards it adds; Seal of Gold drains shop gold.
- Book of Five Rings heals 20 when a 5th card is added: with low HP take cheap cards `[hyp]`; test: HP before and after the 5th card (`s`).
- Colorful Philosophers (3 cards of another class): best +5 +- 3.8, under 2 se `[sim]`.
- Darv's Ectoplasm (energy relic) stops all gold gain: spend the gold before taking it, and price it against the shops left on the route.

## To test `[hyp]`
- Another class's pool holds a card worth >= +10 on the Act 2 boss: scan every card with `eval ... add=ID`.
- Smith vs rest before the Act 2 boss, per boss: `eval` with and without the upgrade at arrival HP.
