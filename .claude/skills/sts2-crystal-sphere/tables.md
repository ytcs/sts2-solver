# Crystal Sphere tables (`[sim]`, `agent/crystal_sphere.py`; gold-eq: relic 208.5, potion 50 / rare 100, card 50/75/150, gold 10/30, curse -100)
Placement matches the C# code (retry never triggered in 328000 games). Posterior sampler calibrated: predicted vs realised third-click value within 2 se over 2500 games.

## Policies (20000 paired games; * = 8000)
`tile_row` 3x3 tiling (1,1),(4,1),(7,1),(1,4),(4,4),(7,4); `blind_opt` best fixed list (n=3 (6,1),(3,1),(3,4); n=6 (5,1),(9,4),(9,7),(6,7),(6,4),(8,1)); `greedy` posterior next-click; `rules` hand rules; `setplan` posterior open-loop plan, replan each click; `look2` two-step expectimax.

| policy | n | value +- se | P(curse) | relics | potions | card rewards | vs tile_row |
|---|---|---|---|---|---|---|---|
| tile_row | 3 | 79.0 +- 0.6 | 0.169 | 0.00 | 0.31 | 0.51 | 0 |
| blind_opt | 3 | 103.3 +- 0.7 | 0.211 | 0.00 | 0.58 | 0.63 | +24.2 |
| rules | 3 | 108.7 +- 0.6 | 0.126 | 0.00 | 0.58 | 0.60 | +29.7 |
| greedy | 3 | 158.8 +- 0.6 | 0.146 | 0.00 | 0.82 | 0.99 | +79.8 |
| setplan | 3 | 158.6 +- 0.6 | 0.148 | 0.00 | 0.82 | 0.99 | +79.5 |
| look2* | 3 | 160.3 +- 1.0 | 0.131 | 0.00 | 0.84 | 1.00 | +79.8 |
| tile_row | 6 | 262.8 +- 0.8 | 0.413 | 0.28 | 1.18 | 1.26 | 0 |
| blind_opt | 6 | 281.0 +- 0.9 | 0.441 | 0.28 | 1.38 | 1.35 | +18.2 |
| rules | 6 | 219.0 +- 0.7 | 0.185 | 0.00 | 1.22 | 1.21 | -43.8 |
| greedy | 6 | 347.4 +- 0.7 | 0.268 | 0.00 | 1.87 | 2.10 | +84.6 |
| setplan | 6 | 368.6 +- 0.7 | 0.249 | 0.20 | 1.74 | 1.98 | +105.9 |
| look2* | 6 | 361.9 +- 1.1 | 0.247 | 0.02 | 1.93 | 2.16 | +97.7 |
n=3: every adaptive policy 159-160 (one-click greedy enough). n=6: setplan +21.2 over greedy.

## Curse penalty while planning (greedy, 3000 games, scored at -100)
| penalty | n=3 value | P(curse) | n=6 value | P(curse) |
|---|---|---|---|---|
| 0 | 157.6 | 0.156 | 349.7 | 0.294 |
| -100 | 156.2 | 0.151 | 348.3 | 0.266 |
| -300 | 155.3 | 0.138 | 348.0 | 0.217 |
| -1000 | 99.7 | 0.038 | 281.6 | 0.098 |

## Option economics (net of cost; V3 160, V6 369)
| value table | V3 | V6 | Uncover net at 51/75/99 | Payment net at Debt 30/130/250 | better |
|---|---|---|---|---|---|
| base | 160 | 369 | 109/85/61 | 339/239/119 | Payment |
| cards x2 | 252 | 549 | 201/177/153 | 519/419/299 | Payment |
| cheap items | 119 | 273 | 68/44/20 | 243/143/23 | Payment unless Debt > ~230 |
| curse -250 | 139 | 331 | 88/64/40 | 301/201/81 | Payment |
| relic 400 | 160 | 407 | 109/85/61 | 377/277/157 | Payment |

## Prior value of one Big click by centre (no clicks yet; Small 0.4-0.8 anywhere)
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
P(click reveals curse) %: 0 in corner triangles; 2.0-2.3 at edge ends; 4.0-4.3 along y=0/10 (x=4..6); 5.5-6.7 on ring rows/columns; 3.0-3.7 middle. Relic covers (5,5) 40%, (5,0) 5%.
