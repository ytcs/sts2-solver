---
name: sts2-crystal-sphere
description: Use when the event is Crystal Sphere (options "Uncover Future" / "Payment Plan", screen kind CRYSTAL_SPHERE): which option to take, and which cell and tool to click on the fog grid.
---

# Crystal Sphere (Act 2+ event): option, cell, tool

Units: gold-equivalents (gold-eq). Every `[sim]` comes from `python agent/crystal_sphere.py bench` (20000 games per row, paired).

## Facts `[code]` (CrystalSphereMinigame / CrystalSphereItem / CrystalSphere / NCrystalSphereScreen)
- Offered only in Act 2+ and when every player holds >= 100 gold. **Uncover Future**: pay 50 + rand(1..49) = 51-99 gold (printed on the button), 3 divinations. **Payment Plan**: no gold, a **Debt** curse (unplayable; at end of turn in hand you lose min(10, gold)), 6 divinations.
- Grid 11x11, `a 0 <x> <y>`, x = column, y = row. 6 cells at each corner start clear. 15 items are placed one by one in this order, each on a uniformly random free spot, touching allowed: relic 4x4, potion 1x3 (wide x tall) x2, rare potion 2x2, cards (common, uncommon, rare) 2x2, curse 2x2, gold 1x1 x5, gold 2x1 x2.
- Big tool (default, `a 1`) clears the 3x3 block around the cell (clipped at the edge), Small (`a 2`) one cell; each click costs one divination. An item pays when its LAST cell is clear. Rewards come on a rewards screen at the end: relic (random rarity 50/33/17 %), potion, card pick 1 of 3 at that rarity (skip allowed), gold 10 / 30. The curse (Doubt: unplayable, 1 Weak at end of turn in hand) joins the deck the moment it is revealed.
- No early exit: Proceed appears only after the last divination. The bridge accepts a Big click centred on an already clear cell (a mouse cannot); Small on a clear cell is refused.
- The grid shows only the KIND under a clear cell (R P C X g), not which part. Never use hidden positions.

## Values `[code]` prices, `[hyp]` worth
Revealed item = its shop price: relic 208 (175/225/275 at 50/33/17 %), potions 50 / 100 (rare), cards 50 / 75 / 150, gold 10 / 30, Doubt -100 (a removal, 75-100 gold). Debt: -30 when a shop is the next stop or on the way (the removal replaces a Strike removal), else -130 (removal price plus ~10 gold per fight until removed); more the longer it stays. `[hyp]` test: re-run `bench` with your own `VALUES`.

## 1. Whether to play and which option
- **Play whenever offered (gold >= 100).** Uncover Future nets +61..+109 at price 99..51; Payment Plan nets +340 (shop ahead) / +240 (no shop) `[sim]`. Not playing is never best.
- **Payment Plan over Uncover Future**: its 6 divinations are worth ~347 vs ~159 for 3 (advisor play), so it wins even at Debt 250 (break-even Debt ~260: 347 - Debt vs 159 - the 51-99 gold) and keeps your 51-99 gold. Take Uncover Future only if the Debt cannot be tolerated (deck already clogged with curses, gold under ~30 so it cannot buy a removal, an energy-light deck with no exhaust). Test: price the Debt with `eval --v "x|add=DEBT"` when in doubt. `[sim]` + `[hyp]`
- Route rule: Payment Plan with a shop next (or within ~3 floors) costs about nothing; with none ahead, remove it at the first shop, and spend gold there on the removal before cards.

## 2. Which click: run the advisor, do not improvise
Paste the bridge's grid text and run (read-only, ~5-30 s, never touches the game):
`python agent/crystal_sphere.py advise <<'EOF'` ... grid text ... `EOF`  (add `--n N` if the divinations line is missing).
It samples placements consistent with the grid and prints the best click, the planned set, expected value and curse probability per click. Play its recommended click, re-run after every click. `[sim]` policy "setplan" below.
Why not by hand `[sim]`: the literal hand rules (finish a visible partial item by centring on its visible cell, else the best fresh ring click, keep 2 cells from an X) score only 109 (n=3) / 219 (n=6): worse than the optimised blind list at n=6. The advisor also uses the negative information (which cleared areas are empty).

## 3. Tool and cell rules (what the advisor does, and the fallback)
- **Tool: Big, always.** Small was never chosen by the posterior policy in 20000 games per n, and never beat the best Big click in 1600 random mid-game states `[sim]`; "greedy_big" = "greedy" to the last digit. Small only wins if every 3x3 block that covers a missing cell of a wanted item also covers the curse's last hidden cell: a border cell whose inward neighbour is that cell (potion 1x3 against the edge). `[code]` geometry, `[hyp]` otherwise.
- **Opening (blind), first click `a 0 4 1`** (E 32.6; equals 6 1, 4 9, 6 9 by symmetry; columns x=1 / x=9 at y=4..6 are 4-8 lower). Reason `[code]` + `[sim]`: the relic is placed first and sits in the middle, so everything else is pushed to the outer ring; the middle (5,5) is worth 16, the ring rows y=1 / y=9 and columns x=1 / x=9 are worth 25-33, the four corner triangles (x+y<=3 and mirrors) 0-12. Never click inside a corner triangle.
- **After the opening: finish what shows.** A visible 'C', 'P' of a 2x2, or 'g' part: a Big click centred ON a visible cell of it clears every 2x2 / 2x1 / 1x1 that contains that cell (all lie within one cell). A 'P' pair in a column (1x3): centre on the middle cell or the cell next to the hidden end. Offset the centre by one so the block also covers fresh fog (the advisor does this).
- **Relic ('R')**: needs >= 4 Big blocks (4x4 = corners 3 apart). Chase it only with >= 4 divinations left and a visible R patch; otherwise ignore it. The advisor starts the chase by itself (n=6: relic in 20 % of games vs 0 % for greedy, +20 gold) `[sim]`.
- **Curse ('X')**: a visible X cell means the curse lies within 1 cell of it, so never click within 2 cells (Chebyshev) of an X unless the advisor says P(curse)=0. A first click has 3-7 % curse risk wherever it lands. Planning with a curse penalty of -300 instead of -100 cuts P(curse) 0.266 -> 0.217 (n=6) at no value loss; -1000 costs 66 gold for 0.098. `[sim]` Use the advisor's `Pc` column: take a click only if its value beats the best Pc=0 click by more than 3 x Pc x 100.
- **Wasted clicks**: every divination must be spent. If nothing worth taking remains, click a fogged cell far from any X (a block that clears only fog is free, one that reveals the curse costs 100).

## 4. Numbers `[sim]` (20000 games per row unless marked; gold-eq; P(curse) = reveal probability)
| policy | n=3 value | P(curse) | n=6 value | P(curse) |
|---|---|---|---|---|
| naive tile (1,1),(4,1),(7,1),(1,4),(4,4),(7,4) | 79.0 ± 0.6 | 0.17 | 262.8 ± 0.8 | 0.41 |
| best fixed (blind) list | 103.3 ± 0.7 | 0.21 | 281.0 ± 0.9 | 0.44 |
| posterior greedy (advisor, one click ahead) | 158.8 ± 0.6 | 0.15 | 347.4 ± 0.7 | 0.27 |
| posterior plan (advisor, set of the remaining clicks) | 158.6 ± 0.6 | 0.15 | not measured | - |
Adaptive beats blind by +55 (n=3) and +66 (n=6, greedy); 2-step lookahead adds nothing at n=3 (160.3 ± 1.0, 8000 games). Blind tiling lets the curse out in 17 % / 41 % of games.

## 5. Re-run and update
`python agent/crystal_sphere.py bench --n 6 --games 20000 --policies tile_row,blind_opt,greedy,setplan` (about 1 h per policy at n=6), `... opening --n 6`. Change values in `VALUES` / `DEBT_COST` (top of the file). Open `[hyp]`: the worth of each item (shop price proxy), the Debt cost, and 4-click relic chases beyond what `plan_set` finds.
