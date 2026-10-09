---
name: sts2-crystal-sphere
description: Use when the event is Crystal Sphere (options "Uncover Future" / "Payment Plan", screen kind CRYSTAL_SPHERE): which option to take, and which cell and tool to click on the fog grid.
---

# Crystal Sphere (Act 2+ event): option, cell, tool
Units: gold-equivalents (`VALUES` in `agent/crystal_sphere.py`: shop-price proxy `[hyp]`). `[sim]` = `python agent/crystal_sphere.py bench` (`tables.md`).

## Facts `[code]` (CrystalSphereMinigame, CrystalSphereItem, Debt, Doubt)
- Act 2+, gold >= 100. Uncover Future: 51-99 gold, 3 divinations. Payment Plan: Debt curse (lose min(10, gold) per turn it ends in hand), 6 divinations.
- Grid 11x11, `a 0 <x> <y>`. Corner triangles (6 cells each) start clear and empty. 15 items: relic 4x4, potion 1x3 x2, rare potion 2x2, three cards 2x2, curse (Doubt) 2x2, gold 1x1 x5, 2x1 x2.
- Big tool (`a 1`, default) clears 3x3; Small (`a 2`) one cell; one divination each. An item pays when fully clear; Doubt enters the deck when revealed. No early exit. Grid shows only the kind under clear cells.

## 1. Option `[sim]`
Payment Plan: +339 with a shop ahead, +239 without (Debt `[hyp]` -30 / -130). Uncover Future nets +61..+109. Uncover only with several curses already, or gold < ~30 and no shop ahead.

## 2. Click
Every click: `python agent/crystal_sphere.py advise <<'EOF'` + `s` text + `EOF` (`--n N` if divinations left are missing). Play RECOMMENDED. Hand play is worse `[sim]` (219 vs 369 at n=6).

## 3. Fallback rules `[sim]`
- Big always (Small never beat Big in 1600 states).
- Open `a 0 4 1` (or mirrors); ring rows/columns 1 and 9 score 25-33, middle 16-20, corner triangles 0-13: never.
- Big centred on a visible cell of a small item clears all its placements.
- Relic only with >= 4 divinations and R showing.
- No click within 2 cells of an X unless the advisor shows Pc=0.
