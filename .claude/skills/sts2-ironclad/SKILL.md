---
name: sts2-ironclad
description: Use when the character is Ironclad (at character select and at every Ironclad card or relic choice): only what differs from the general rules: A10 start facts, HP-loss triggers, card values, conditional picks.
---

# Ironclad (deviations from level 1)
- IC1 [code] A10 start: 64 / 80 HP, 99 gold, Burning Blood (+6 HP after each fight), 5 Strike, 4 Defend, Bash, Ascender's Bane (Eternal).
- IC2 [hyp] Burning Blood makes HP cheap between fights (P4). Test: instrument: `eval --hp` at elite- vs safe-route arrival.
- IC3 [sim] Demon Form is the reference scaling card: +6 win points at act-3 bosses taken from act 1 on (E85, E95); front-damage commons (Anger, Setup Strike, Dismantle) pay only at act-1 binding fights (P2). Test: instrument: `eval --pool Glory:boss --hp full --target --v "x|add=DEMON_FORM"`.
- IC4 [sim] Inferno is a Power that slightly hurts at act-3 bosses (-1.0 +- 0.65, E95): do not count it as an engine. Test: instrument: same `eval`.
- IC5 [hyp] Self-damage Strength engine (Inferno, Rupture, Demon Form, Fight Me, Spite, Stone Armor, Crimson Mantle) is the Aeonglass answer (earlier model: 0.61). Test: instrument: `eval --enc AEONGLASS_BOSS --v` of the engine.
- IC6 [hyp] Second Wind: only when Aeonglass is the gate (earlier model: +3.9 vs Aeonglass, -4.2 vs Insatiable). Test: instrument: boss-alone `eval`.
- IC7 [hyp] True Grit+ is a priority smith with statuses; Perfected Strike scales with Strikes (conflicts with D4). Test: instrument: `eval --v "u|upgrade=TRUE_GRIT"`, `remove=STRIKE`.
