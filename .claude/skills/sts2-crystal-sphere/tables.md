# Crystal Sphere tables (`[sim]`, `agent/crystal_sphere.py`)
Gold-eq: relic 208.5, potion 50 / rare 100, card 50/75/150, gold 10/30, curse -100. Placement matches the C# (no retry in 328k games); posterior sampler calibrated within 2 se (2500 games). Re-run: `bench --n 6 --games 20000 --policies tile_row,blind_opt,greedy,setplan` (~1 h, 16 cores).

| policy (20k paired games) | n=3 value | P(curse) | n=6 value | P(curse) |
|---|---|---|---|---|
| tile_row (3x3 tiling) | 79.0 | 0.17 | 262.8 | 0.41 |
| blind_opt (best fixed list) | 103.3 | 0.21 | 281.0 | 0.44 |
| rules (hand play) | 108.7 | 0.13 | 219.0 | 0.19 |
| greedy (posterior next click) | 158.8 | 0.15 | 347.4 | 0.27 |
| setplan (advisor; replan each click) | 158.6 | 0.15 | 368.6 | 0.25 |

se 0.6-0.9. Curse penalty -300 while planning: n=6 P(curse) 0.27 -> 0.22 for < 1 gold; -1000 costs 66.

Option economics: base V3 160, V6 369; Payment beats Uncover under cards x2, curse -250, relic 400; under cheap items only while Debt < ~230.

Prior Big-click value by centre: ring y/x = 1, 9 (x/y 3-7) 25-33; middle 16-20; edges 11-18; corner triangles 0-13. P(click reveals curse): 0 in corners, 5.5-6.7% on ring rows, 3.0-3.7% middle.
