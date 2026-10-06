---
name: sts2-ironclad-act3
description: Use when entering or planning Act 3 (Glory) as the Ironclad, including the A10 second boss: elite win rate by HP, ancient and relic values. Deviations from the general strategy only.
---

# Ironclad Act 3 (deviations from `sts2-strategy`, `sts2-ironclad`)

Pools: `sts2-acts`; boss rows, sweeps, target decks: `encounters.md`.

## Numbers `[sim]`
- Elites vs a baseline deck: ~+1.7 win points per HP: 10% at 45 HP, 31% at 60, 57% at 75, 66% at 80; an Act 2-clear deck 83-98%. Bosses ~0-7% at 80 HP; two back to back are ~950 HP of enemy.
- Ancient Tanx: Claws (5 Strikes + 1 Defend into Maul) +31 on elites vs War Hammer +5, Iron Club +10.
- Jeweled Mask (Vakuu): +47 on Glory elites (Demon Form lands free on turn 1); it picks a RANDOM Power from the draw pile, so a second Power (Inflame) dilutes it (-22): keep Powers to Demon Form.
- Regal Pillow (+15 HP per rest) and Meal Ticket (+15 per shop) are HP engines `[hyp]`; test: `routes --hp` with the extra heal at each rest / shop on the route.

## To test `[hyp]`
- HP and potions through two consecutive bosses (Pantograph heals at each): `eval --boss` for boss 2 at boss 1's expected exit HP.
- Whether a deck doing 100+ damage per turn exists in reach: `eval --boss` of candidate decks.
- Distribution shift: the solver's fight-start prediction vs outcome for Act 3 bosses (`python -m agent.improve review`, Brier per fight).
