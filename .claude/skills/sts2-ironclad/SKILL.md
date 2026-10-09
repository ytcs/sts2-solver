---
name: sts2-ironclad
description: Use when the character is Ironclad (at character select and at every Ironclad card or relic choice): only what differs from the general strategy: A10 start facts, HP-loss triggers, card values, conditional picks.
---

# Ironclad (deviations from `sts2-strategy`)
- A10 start `[code]`: 64/80 HP, 99 gold, Burning Blood (+6 HP per fight), 5 Strike, 4 Defend, Bash, Ascender's Bane (Eternal, as Greed). Starter loses the first elite (Bygone Effigy 2%) `[sim]`.
- Burning Blood makes HP cheap between fights. `[hyp]` test: `eval --hp` at elite- vs safe-route arrival.
- Act 1 boss pool from a weak deck `[sim]` (256 attempts): Dismantle +13, Hemokinesis +12, Perfected Strike +11, Setup Strike +8; block cards ~0: damage beats block vs bosses.
- Second Wind `[sim]`: +3.9 vs Aeonglass, -4.2 vs Insatiable: only when Aeonglass is the gate.
- `[hyp]` (test: `eval` variant named): Relax +30 vs act-2 elites (`--elites`); Perfected Strike scales with Strikes (`remove=STRIKE`); True Grit+ is a priority smith with statuses (`upgrade=TRUE_GRIT`).
