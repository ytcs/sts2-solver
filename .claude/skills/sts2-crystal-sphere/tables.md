# Crystal Sphere: tables (all `[sim]` from `agent/crystal_sphere.py`, values in gold-eq: relic 208.5, potion 50 / rare 100, card 50 / 75 / 150, gold 10 / 30, curse -100)

Placement was checked against the scalar re-implementation of the C# code (retry quirk never triggered: 0 of 328000 simulated games needed a second placement pass). The posterior sampler was calibrated: predicted vs realised value of a third click over 2500 games agrees within 2 se (differences -1.8 ± 1.0 .. +1.1 ± 1.0).

## Policies (20000 paired games per row, the same 20000 placements for every policy; * = 8000 games)
Policies: `tile_row` naive 3x3 tiling, centres (1,1),(4,1),(7,1),(1,4),(4,4),(7,4) in that order; `tile_centre` the same lattice, centre first; `blind_opt` the best FIXED click list (greedy + swap search on 6000 prior samples: n=3 `(6,1),(3,1),(3,4)`; n=6 `(5,1),(9,4),(9,7),(6,7),(6,4),(8,1)`); `greedy` posterior, best expected value of the next click, Big or Small; `greedy_big` the same with Big only; `rules` the hand rules in the skill played literally; `setplan` posterior open-loop plan of all remaining clicks (greedy and shaped seeds, swap polish), play its best member, replan; `look2` exact two-step expectimax (top 14 first clicks); `hybrid` setplan while >= 3 clicks remain, look2 at 2, greedy at 1.

| policy | n | games | mean value ± se | P(curse) ± se | relics | potions | card rewards | gold | paired vs tile_row |
|---|---|---|---|---|---|---|---|---|---|
| tile_row | 3 | 20000 | 79.0 ± 0.6 | 0.169 ± 0.003 | 0.00 | 0.31 | 0.51 | 26 | 0 |
| tile_centre | 3 | 20000 | 74.3 ± 0.6 | 0.133 ± 0.002 | 0.00 | 0.44 | 0.42 | 21 | -4.7 ± 0.9 |
| blind_opt | 3 | 20000 | 103.3 ± 0.7 | 0.211 ± 0.003 | 0.00 | 0.58 | 0.63 | 27 | +24.2 ± 0.5 |
| rules | 3 | 20000 | 108.7 ± 0.6 | 0.126 ± 0.002 | 0.00 | 0.58 | 0.60 | 27 | +29.7 ± 0.7 |
| greedy | 3 | 20000 | 158.8 ± 0.6 | 0.146 ± 0.003 | 0.00 | 0.82 | 0.99 | 26 | +79.8 ± 0.7 |
| greedy_big | 3 | 20000 | 158.8 ± 0.6 | 0.146 ± 0.003 | 0.00 | 0.82 | 0.99 | 26 | +79.8 ± 0.7 |
| setplan | 3 | 20000 | 158.6 ± 0.6 | 0.148 ± 0.003 | 0.00 | 0.82 | 0.99 | 26 | +79.5 ± 0.7 |
| look2* | 3 | 8000 | 160.3 ± 1.0 | 0.131 ± 0.004 | 0.00 | 0.84 | 1.00 | 24 | +79.8 ± 1.4 |
| hybrid* | 3 | 8000 | 160.1 ± 1.0 | 0.142 ± 0.004 | 0.00 | 0.82 | 1.01 | 26 | +79.6 ± 1.1 |
| tile_row | 6 | 20000 | 262.8 ± 0.8 | 0.413 ± 0.003 | 0.28 | 1.18 | 1.26 | 50 | 0 |
| tile_centre | 6 | 20000 | 247.9 ± 0.9 | 0.384 ± 0.003 | 0.22 | 1.24 | 1.19 | 49 | -14.9 ± 1.2 |
| blind_opt | 6 | 20000 | 281.0 ± 0.9 | 0.441 ± 0.004 | 0.28 | 1.38 | 1.35 | 52 | +18.2 ± 1.2 |
| rules | 6 | 20000 | 219.0 ± 0.7 | 0.185 ± 0.003 | 0.00 | 1.22 | 1.21 | 46 | -43.8 ± 1.1 |
| greedy | 6 | 20000 | 347.4 ± 0.7 | 0.268 ± 0.003 | 0.00 | 1.87 | 2.10 | 53 | +84.6 ± 1.0 |
| greedy_big | 6 | 20000 | 347.4 ± 0.7 | 0.268 ± 0.003 | 0.00 | 1.87 | 2.10 | 53 | +84.6 ± 1.0 |
| **setplan** | 6 | 20000 | **368.6 ± 0.7** | 0.249 ± 0.003 | 0.20 | 1.74 | 1.98 | 50 | +105.9 ± 1.0 |
| look2* | 6 | 8000 | 361.9 ± 1.1 | 0.247 ± 0.005 | 0.02 | 1.93 | 2.16 | 51 | +97.7 ± 1.7 |
| hybrid* | 6 | 8000 | 361.6 ± 1.2 | 0.248 ± 0.005 | 0.12 | 1.79 | 2.04 | 52 | +97.4 ± 1.6 |

Paired, n=6, same 8000 games: look2 +13.5 ± 1.5 and hybrid +13.2 ± 1.2 over greedy; setplan +21.2 over greedy (20000 games). At n=3 every adaptive policy sits at 159-160 (differences below 1.5 ± 1.4): the one-click posterior greedy is enough.
Items revealed per game (reveal rate, n=6 setplan): relic 0.20, each potion 0.54 / rare 0.66, each card 0.66, small gold 0.48, big gold 0.44, curse 0.25. Blind list n=6: relic 0.28, cards 0.45, curse 0.44. n=3 greedy: cards 0.33, potions 0.25 / 0.32, curse 0.15.

## Curse penalty while planning (greedy posterior, 3000 games, scored with curse = -100)
| planning penalty | n=3 value | n=3 P(curse) | n=6 value | n=6 P(curse) |
|---|---|---|---|---|
| 0 | 157.6 ± 1.6 | 0.156 | 349.7 ± 1.8 | 0.294 |
| -100 | 156.2 ± 1.6 | 0.151 | 348.3 ± 1.8 | 0.266 |
| -300 | 155.3 ± 1.6 | 0.138 | 348.0 ± 1.7 | 0.217 |
| -1000 | 99.7 ± 1.3 | 0.038 | 281.6 ± 1.7 | 0.098 |
Avoiding the curse costs at most ~1 gold of value up to a penalty of 300 (P(curse) -9 % relative at n=3, -18 % at n=6 versus -100) and a lot beyond; most curse reveals are unforced (a click into fog with no hint carries 3-7 %).

## Option economics (best policy: n=3 160, n=6 369; net of the cost; `agent/crystal_sphere.py` VALUES)
| value table | V3 | V6 | Uncover Future net at price 51 / 75 / 99 | Payment Plan net at Debt 30 / 130 / 250 | better |
|---|---|---|---|---|---|
| base | 160 | 369 | 109 / 85 / 61 | 339 / 239 / 119 | Payment Plan |
| cards x2 | 252 | 549 | 201 / 177 / 153 | 519 / 419 / 299 | Payment Plan |
| cheap relic / potions / cards (150; 30 / 60; 40 / 60 / 120) | 119 | 273 | 68 / 44 / 20 | 243 / 143 / 23 | Payment Plan unless Debt > ~230 |
| curse -250 | 139 | 331 | 88 / 64 / 40 | 301 / 201 / 81 | Payment Plan |
| relic 400 | 160 | 407 | 109 / 85 / 61 | 377 / 277 / 157 | Payment Plan |
Break-even Debt (Payment Plan = Uncover Future) = V6 - V3 + price = 369 - 160 + 51..99 = 260-308 (base table).

## Prior value of ONE Big click, by centre (gold-eq, no clicks yet; 20000 prior samples; x = column, y = row; Small is worth 0.4-0.8 anywhere)
```
      x=0   1    2    3    4    5    6    7    8    9   10
y= 0  0.0  0.8  3.2 11.8 17.7 16.9 17.6 11.4  3.2  0.8  0.0
y= 1  0.8  3.3 13.3 26.4 32.6 31.5 32.4 25.9 13.2  3.2  0.8
y= 2  3.1 13.0 25.2 28.7 25.0 24.2 24.9 28.1 24.7 13.2  3.2
y= 3 11.1 24.9 28.3 23.3 20.5 20.1 20.8 23.0 27.7 25.8 12.0
y= 4 18.2 29.8 24.0 20.5 18.1 17.2 18.1 20.1 24.0 30.7 19.0
y= 5 17.8 28.3 22.2 19.6 17.3 16.3 17.3 19.8 22.9 28.4 17.6
y= 6 18.4 29.4 23.5 20.3 17.8 17.2 18.4 21.1 24.1 30.0 19.1
y= 7 11.2 24.9 28.2 23.7 21.0 19.7 20.5 23.3 27.6 24.9 11.8
y= 8  3.1 13.3 25.5 29.3 25.7 23.8 24.5 28.4 24.7 12.6  3.1
y= 9  0.8  3.2 13.4 26.6 32.4 30.5 31.8 26.4 13.4  3.2  0.8
y=10  0.0  0.8  3.2 11.6 17.5 16.3 17.3 11.6  3.2  0.8  0.0
```
P(this click reveals the curse), percent: 0 in the corner triangles; 2.0-2.3 at the edge ends (3,0), (2,1), (10,3); 4.0-4.3 along y=0 / y=10 (x=4..6); 5.5-6.7 on the ring rows y=1 / y=9 (x=3..7) and columns x=1 / x=9 (y=4..6); 3.0-3.7 in the middle. The ring is where the cards, potions and gold are (the relic, placed first, takes the middle: it covers (5,5) with probability 40 %, an edge cell like (5,0) with 5 %), so the best clicks are also the riskiest (value 32.6 at 6.5 % curse vs 16.3 at 3.0 % in the middle).
