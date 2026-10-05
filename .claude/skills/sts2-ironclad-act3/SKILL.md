---
name: sts2-ironclad-act3
description: Use when entering or planning Act 3 (Glory) as the Ironclad, including the A10 second boss: elite and boss notes and deviations from the general strategy. No evidence yet.
---

# Ironclad Act 3 (additions to `sts2-strategy`, `sts2-ironclad`)

Pools: `sts2-acts` (2 weak fights, 13 rooms, two bosses at A10).

## Run 3 (died to Aeonglass, floor 48) `[played]` / `[sim]`
- Glory is another step up: baseline deck beats the elites 11-66% depending on HP (about +1.7 points of win rate per HP: 10% at 45 HP, 31% at 60, 57% at 75, 66% at 80), bosses 3% (Aeonglass 7%, Queen 0%). Two bosses back to back is ~950 HP of enemy: plan for HP carry (Pantograph-type relics), potions for both, and a deck that does 100+ damage a turn.
- Ancient Tanx: Claws (5 Strikes + 1 Defend into Maul) +31 on elites vs War Hammer +5, Iron Club +10.
- Regal Pillow (+15 HP per rest) and Meal Ticket (+15 at shops) are HP engines worth buying: with Pillow a rest was +37 HP.
- The deck had stopped improving by Act 3 (22 cards, ~half starter/dead): see `sts2-deckbuilding` on acceleration of power early.

## Run 4 (bosses Aeonglass + Test Subject, died at Aeonglass, floor 48) `[played]` / `[sim]`
- Act 3 bosses are a wall for this solver+deck family: both ~0-7% at 80 HP; no single card (88 tested) or relic (215 tested) moved either by more than +4; bundles of six strong cards 3-4% on Aeonglass. Elites are fine (83-98%).
- Jeweled Mask (Vakuu): +47 on Glory elites because Demon Form lands in hand free on turn 1; it picks a RANDOM Power from the draw pile, so a second Power (Inflame -22) dilutes it: keep Powers to Demon Form.
- Test Subject has three forms (111 -> 212 -> 313 HP) and respawns; Aeonglass 535 HP, Artifact 3, Wither every 6 cards played; Queen is the other possible second boss (419 HP + Torch Head Amalgam).
- Tinker Time (event): custom card; Book of Five Rings heals 20 on the 5th added card: at 10 HP this was a full heal.

## To test `[hyp]`
- Whether the solver plays the Act 3 bosses well from decks it has not seen (distribution shift): compare its fight prediction with the outcome.
- How to carry HP and potions through two consecutive bosses (Pantograph heals at each boss).
