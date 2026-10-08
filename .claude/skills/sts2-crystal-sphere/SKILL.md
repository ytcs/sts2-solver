---
name: sts2-crystal-sphere
description: Use when the event is Crystal Sphere (options "Uncover Future" / "Payment Plan", screen kind CRYSTAL_SPHERE): which option to take, and which cell and tool to click on the fog grid.
---

# Crystal Sphere (Act 2+ event): option, cell, tool
Units: gold-equivalents. Every `[sim]` = `python agent/crystal_sphere.py bench` (paired; tables in `tables.md`). The advisor is read-only.

## Facts `[code]` (CrystalSphereMinigame, CrystalSphereItem, CrystalSphere, NCrystalSphereScreen, OneOffSynchronizer, Debt, Doubt)
- Offered Act 2+ with >= 100 gold. Uncover Future: pay 51-99 gold (on the button), 3 divinations. Payment Plan: no gold, Debt curse (unplayable; each turn it ends in hand lose min(10, gold)), 6 divinations.
- Grid 11x11, `a 0 <x> <y>` (x column, y row). The 6 cells of each corner triangle start clear, hold nothing. 15 items placed in order, each uniformly on a free spot: relic 4x4, potion 1x3 x2, rare potion 2x2, cards common/uncommon/rare 2x2, curse 2x2, gold 1x1 x5, gold 2x1 x2.
- Big tool (default, `a 1`) clears the 3x3 around the cell (clipped); Small (`a 2`) one cell; each costs one divination. An item pays when its last cell is clear. Rewards at end: relic (50/33/17%), potions, one pick of 3 per card item, gold 10/30. The curse (Doubt: unplayable, Weak 1 at end of turn in hand) enters the deck when revealed.
- No early exit (Proceed after the last divination). Big centred on a clear cell is accepted; Small on a clear cell refused.
- Grid shows only the KIND under clear cells (R P C X g). Use nothing else.

## Values (`VALUES`/`DEBT_COST` in the module) `[code]` prices, `[hyp]` worth
- Shop prices: relic 208, potion 50 / rare 100, cards 50/75/150, gold 10/30, Doubt -100.
- Debt by route `[hyp]`: shop within ~3 floors -30; no shop ahead -130; +~10 per fight beyond.

## 1. Option `[sim]`
- Always play: Uncover Future nets +109/+85/+61 at price 51/75/99; Payment Plan +339 (shop ahead) / +239 (none).
- Payment Plan unless Debt is intolerable (break-even Debt 260-308 = V6 - V3 + price); holds under 4 other value tables. Uncover Future only with several curses already, or gold < ~30 and no shop ahead.

## 2. Click: run the advisor every click
`python agent/crystal_sphere.py advise <<'EOF'` + screen text (`s`) + `EOF` (5-30 s; `--n N` if "divinations left" is missing). Prints RECOMMENDED + E and P(curse) per click. Play it; repeat after each click. Hand play is worse `[sim]` (109 / 219 at n=3 / 6).

## 3. Tool and cell rules (advisor's behaviour; fallback)
- Big always `[sim]` (Small never chosen in 40000 games, never beat best Big in 1600 states). Exception `[hyp]`: the last missing cell of a wanted item is on the border and its inward neighbour is the curse's last hidden cell: Small.
- Opening `a 0 4 1` (E 32.6; same at (6,1), (4,9), (6,9)). Ring rows y=1/9, columns x=1/9: 25-33 per click; middle 16-20; corner triangles (x+y<=3 and mirrors) 0-13: never click there. `[sim]`
- Finish what shows: Big centred ON a visible cell of a 2x2/2x1/1x1 item clears all its placements; shift one toward fog to open fresh cells. 1x3 potion: centre on its middle or one past the visible end.
- Relic (4x4) needs >= 4 Big blocks: chase only with >= 4 divinations left and R showing (n=6: relic 20% of games, +21 gold) `[sim]`.
- Curse: no click within 2 cells (Chebyshev) of an X unless the advisor shows Pc=0. Advisor plans with curse at -300: P(curse) 0.266 -> 0.217 (n=6) for < 1 gold; -1000 costs 66 `[sim]`.
- Spare clicks: fogged cell whose block holds no item, far from X.

## 4. Numbers `[sim]`
n=6 value (P(curse)): advisor plan 368.6 (0.25), posterior greedy 347.4 (0.27), best fixed list 281.0 (0.44), naive tiling 262.8 (0.41); n=3: 158.6 / 158.8 / 103.3 / 79.0. All rows: `tables.md`.

## 5. Re-run
`python agent/crystal_sphere.py bench --n 6 --games 20000 --policies tile_row,blind_opt,greedy,setplan` (~1 h, 16 cores); `opening --n 6` for the blind list. Open `[hyp]`: item worth (shop-price proxy), Debt cost by route, 4-click relic chases beyond `plan_set`.
