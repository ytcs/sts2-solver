---
name: sts2-ironclad-act3
description: Use when entering or planning Act 3 (Glory) as the Ironclad, including the A10 second boss: elite win rate by HP, ancient and relic values. Deviations from the general strategy only.
---

# Ironclad Act 3 (deviations from `sts2-strategy`, `sts2-ironclad`)
Pools: `sts2-acts`; bosses, sweeps, target decks: `encounters.md`.

## Numbers `[sim]`
- Elites vs a baseline deck ~+1.7 win points/HP: 10% at 45, 31% at 60, 57% at 75, 66% at 80; Act 2-clear deck 83-98%. Bosses ~0-7% at 80 HP; two back to back ~950 enemy HP.
- Ancient Tanx: Claws (5 Strikes + 1 Defend into Maul) +31 on elites vs War Hammer +5, Iron Club +10.
- Jeweled Mask (Vakuu): +47 on Glory elites (Demon Form free T1); picks a RANDOM Power from draw pile, a second Power dilutes it (-22): keep Powers to Demon Form.
- Regal Pillow (+15 HP per rest), Meal Ticket (+15 per shop) are HP engines `[hyp]`. Test: `routes --hp` with the extra heals.

## To test `[hyp]`
- HP + potions through two bosses (Pantograph at each): `eval --boss` for boss 2 at boss 1's expected exit HP.
- A deck doing 100+ damage/turn in reach: `eval --boss` of candidates.
- Act 3 boss prediction vs outcome: `python -m agent.improve review` Brier per fight.
