---
name: sts2-crystal-sphere
description: Use when the event is Crystal Sphere (options "Uncover Future" / "Payment Plan", screen kind CRYSTAL_SPHERE): which option to take, and which cell and tool to click on the fog grid.
---

# Crystal Sphere (Act 2+ event): option, cell, tool

Units: gold-equivalents (gold-eq). Every `[sim]` is `python agent/crystal_sphere.py bench` (paired games, se and all rows in `tables.md`). Read-only: none of these commands touches the game.

## Facts `[code]` (CrystalSphereMinigame, CrystalSphereItem, CrystalSphere, NCrystalSphereScreen, OneOffSynchronizer, Debt, Doubt)
- Offered only in Act 2+ with >= 100 gold. **Uncover Future**: pay 50 + rand(1..49) = 51-99 gold (shown on the button), 3 divinations. **Payment Plan**: no gold, a **Debt** curse (unplayable; each turn it ends in hand you lose min(10, gold)), 6 divinations.
- Grid 11x11, `a 0 <x> <y>` (x column, y row). The 6 cells of each corner triangle start clear and hold nothing. 15 items are placed one after another in this order, each on a uniformly random free spot, touching allowed: relic 4x4, potion 1x3 (wide x tall) x2, rare potion 2x2, cards common / uncommon / rare 2x2, curse 2x2, gold 1x1 x5, gold 2x1 x2.
- Big tool (default, `a 1`) clears the 3x3 block around the clicked cell (clipped at the edge), Small (`a 2`) one cell; either costs one divination. An item pays when its LAST cell is clear. At the end: relic (rarity 50/33/17 %), potions, one card pick of 3 per card item, gold 10 / 30. The curse (Doubt: unplayable, Weak 1 at end of turn in hand) enters the deck the moment it is revealed.
- No early exit: Proceed appears after the last divination. The bridge takes a Big click centred on an already clear cell (a mouse cannot); Small on a clear cell is refused.
- The grid gives only the KIND under each clear cell (R P C X g), not which part of the item. Use nothing else.

## Values `[code]` prices, `[hyp]` worth (edit `VALUES` / `DEBT_COST` in the module)
Item = its shop price: relic 208, potion 50 / rare 100, cards 50 / 75 / 150, gold 10 / 30, Doubt -100 (a removal). **Debt depends on the route** `[hyp]`: shop next or within ~3 floors -30 (the removal replaces a Strike removal and ~10 gold per fight is lost until then); no shop ahead -130; add ~10 per fight beyond that.

## 1. Option
- **Play whenever offered**; not playing is never best: Uncover Future nets +109 / +85 / +61 at price 51 / 75 / 99 (value 160), Payment Plan nets +339 (shop ahead) / +239 (none) (value 369) `[sim]`.
- **Payment Plan over Uncover Future** in every case but an intolerable Debt: break-even Debt 260-308 (V6 - V3 + price); the Debt itself is priced above. Also keeps the 51-99 gold. Pick Uncover Future only if the deck already holds several curses, or gold is under ~30 with no shop ahead (the Debt could not be removed or paid). Holds under 4 other value tables (`tables.md`). `[sim]`

## 2. Which click: run the advisor, every click
Paste the screen text (`s`) and run `python agent/crystal_sphere.py advise <<'EOF'` ... grid text ... `EOF` (5-30 s; `--n N` if the "divinations left" line is missing). It samples placements consistent with the grid (negative information included), plans the remaining clicks, prints RECOMMENDED plus E and P(curse) per click. Play it, repeat after each click. Hand play is worse `[sim]`: finishing visible parts and otherwise taking fresh ring cells scored 109 (3) / 219 (6), below the blind list at 6.

## 3. Tool and cell rules (what the advisor does; fallback when it cannot run)
- **Tool: Big, always.** Small was never chosen by the posterior policy (greedy_big = greedy to the last digit, 40000 games) and never beat the best Big click in 1600 random mid-game states `[sim]`. Only exception `[code]` geometry, `[hyp]`: the single missing cell of a wanted item sits on the border and its inward neighbour is the curse's last hidden cell (potion 1x3 along the edge): then Small.
- **Opening: first click `a 0 4 1`** (E 32.6; same value at (6,1), (4,9), (6,9)). The relic is placed first and fills the middle, so the other items are pushed to the ring: value per click 25-33 on rows y=1 / 9 and columns x=1 / 9, 16-20 in the middle (5,5), 0-13 in the corner triangles (x+y<=3 and mirrors: never click there). Map in `tables.md`. `[sim]`
- **Finish what shows**: a visible part of a 2x2 / 2x1 / 1x1 item: Big centred ON a visible cell of it clears every placement that contains that cell (all lie within one cell); shift the centre by one toward fog so the block also opens fresh cells. A 1x3 potion (column of P): centre on its middle cell, or one cell past the visible end.
- **Relic (R, 4x4)**: needs >= 4 Big blocks (corners are 3 apart). Chase it only with >= 4 divinations left and an R patch showing; the planner does it (n=6: relic in 20 % of games, +21 gold over greedy, 20000 games) `[sim]`; with fewer clicks ignore R.
- **Curse (X)**: an X cell means the curse lies within 1 cell of it: no click within 2 cells (Chebyshev) of an X unless the advisor shows Pc=0. A blind first click risks 3-7 % anywhere. The advisor plans with the curse at -300: that cuts P(curse) 0.266 -> 0.217 (n=6) for under 1 gold of value; -1000 costs 66 gold `[sim]`.
- **Spare clicks** (all divinations must be spent): a fogged cell whose block holds no item and is far from X; a click that opens only fog is free, one that reveals the curse costs 100-300.

## 4. Numbers `[sim]` (value in gold-eq ± se; 20000 games per row; P(curse) = it is revealed)
| policy | n=3 value | P(curse) | n=6 value | P(curse) |
|---|---|---|---|---|
| naive tile (1,1),(4,1),(7,1),(1,4),(4,4),(7,4) | 79.0 ± 0.6 | 0.17 | 262.8 ± 0.8 | 0.41 |
| best fixed list (3: 6,1 3,1 3,4; 6: 5,1 9,4 9,7 6,7 6,4 8,1) | 103.3 ± 0.7 | 0.21 | 281.0 ± 0.9 | 0.44 |
| posterior greedy (next click only) | 158.8 ± 0.6 | 0.15 | 347.4 ± 0.7 | 0.27 |
| **posterior plan (advisor)** | 158.6 ± 0.6 | 0.15 | **368.6 ± 0.7** | 0.25 |
Adaptive over naive tiling: +80 (n=3), +106 (n=6); over the best blind list +55 / +88. Two-step lookahead adds nothing at n=3 and less than the plan at n=6 (361.9 ± 1.1, 8000 games). Placement never needed the C# retry (0 of 328000 games).

## 5. Re-run and update
`python agent/crystal_sphere.py bench --n 6 --games 20000 --policies tile_row,blind_opt,greedy,setplan` (setplan n=6 about 1 h on 16 cores), `opening --n 6` for the blind list. Open `[hyp]`: the worth of each item (shop price proxy), the Debt cost by route, relic chases of 4 clicks beyond `plan_set`. Revise the values when an event shows them wrong (the record goes to `runs/`).
