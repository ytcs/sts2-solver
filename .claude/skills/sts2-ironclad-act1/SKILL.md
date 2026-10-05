---
name: sts2-ironclad-act1
description: Use when entering or planning Act 1 as the Ironclad: deviations and run evidence for Overgrowth / Underdocks, per-boss prep (Vantom, Waterfall Giant, Lagavulin Matriarch), HP-gate numbers; run logs are in runs.md.
---

# Ironclad Act 1 (additions to `sts2-strategy`, `sts2-ironclad`)

Pools and room counts: `sts2-acts`.

## Distilled from the runs (details: `runs.md`)
- **Which gate binds changes with the deck.** A weak deck (starter + few cards) loses bosses for lack of damage at any HP (boss win 18% at 49 HP, 24% at 80). A deck with Inferno+/Anger/Rampage/Blood Wall is HP-gated: Overgrowth boss pool 96% at 80 HP, 66% at 60, 28% at 45, 19% at 34 (Vantom alone 0.46 at 60, 0.11 at 45) `[sim]`. Re-run `eval --hp 34/45/60/80` against the known boss every few picks; once HP-gated, plan rests (not elites) to arrive above the gate.
- **Boss-specific eval beats pool eval.** Against Lagavulin Matriarch alone, Inflame +42 and Inflame+Tremble +73 points while the elite pool showed ~0 for both `[sim]`: always evaluate against the known boss.
- **Strong early picks `[sim]`** (elite or boss pool, 256 attempts): Inferno+ +25, Anger +17..+25, Setup Strike +21, Dismantle +13, Perfected Strike +11 (weak deck) / HP -5 (strong deck), Rampage -17 HP. Blood Wall flips from -5 to +10 once Inferno is in the deck; Breakthrough, Rupture, Vicious, Molten Fist, Cinder, Body Slam, a second Anger were <= 0 against Lagavulin once the deck was built.
- **HP path of the lost Vantom run:** two elites and a regular fight between one rest and the boss left 10 HP before the final rest. A forced lane without rests near the end is a route error (plan back from the boss's HP gate).
- **Avoid Hellraiser** until the simulator is fixed: it desyncs Slippery (rust 7 vs game 6), so the solver's advice was void in the boss fight `[played]`.
- Pantograph (+25 HP at each boss) makes boss arrival HP cheap; without it every elite must be paid for with a rest `[played]`.

## To test `[hyp]`
- Ancient choice against the real Act 1 map: option-by-route table.
- Elite count that maximises boss readiness for a given early deck (this run: 2 elites, both won, relic + rare card each).
- A Waterfall Giant plan: what beats it (eval variants vs `WATERFALL_GIANT_BOSS` only).
