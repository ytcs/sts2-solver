---
name: sts2-ironclad
description: Use when the character is Ironclad (at character select and at every Ironclad card or relic choice): only what differs from the general strategy: A10 start facts, HP-loss triggers, card values, conditional picks.
---

# Ironclad (deviations from `sts2-strategy`)

## Start `[code]`
- A10: 64/80 HP, 99 gold, Burning Blood (+6 HP after each fight), 11 cards (5 Strike, 4 Defend, Bash, Ascender's Bane). Starter wins Act 1 weak/regular fights, loses the first elite (Bygone Effigy 2%) `[sim]`.
- Burning Blood makes HP cheap between fights: spend it on relics/cards. `[hyp]` Test: `eval --hp` at elite-route vs safe-route arrival HP.
- Ascender's Bane and Greed are Eternal (unremovable); curses are dead slots.

## Cards
- HP-loss triggers: Inferno+, Bloodletting, Brand, Rupture, Hemokinesis, Breakthrough (`sts2-mechanics`).
- Act 1 boss pool from a weak deck (win 12-16%, 256 attempts, se ~1.5) `[sim]`: Dismantle +13.4, Hemokinesis +12.5, Perfected Strike +11.3, Setup Strike +7.8, Twin Strike +6.9, Infernal Blade +5.3; Flame Barrier, Drum of Battle, Vicious, Blood Wall, Cinder ~0. Damage beats block vs bosses.
- Second Wind `[sim]` (384 attempts, 80 HP): +3.9 vs Aeonglass, -4.2 vs Insatiable, -1.8 vs Glory elites: only when Aeonglass is the gate or many status/junk cards.
- Relax (Pael's Horn) +30 vs Act 2 elites `[hyp]`. Test: `eval --elites` with Relax.
- Perfected Strike counts every "Strike" card: the starter is its engine `[hyp]`. Test: `add=PERFECTED_STRIKE` with/without `remove=` Strikes.
- True Grit exhausts a random card, True Grit+ a chosen one: with curses/statuses in deck or added by enemies (Clumsy, Toxic, Beckon, Wound) True Grit+ is a priority smith `[hyp]`. Test: `eval --v "up|upgrade=TRUE_GRIT"` vs status-adding encounters.
- Pael's Tooth: removed cards return upgraded, one per combat, ~5 fights of thinning `[hyp]`. Test: relic text.

## To test `[hyp]`
- Smith vs rest before a boss by HP and boss: `eval` at HP after rest vs now with the upgrade.
- HP cost of a typical Act 1 elite for starter+few cards, and break-even: `route` elite vs non-elite lanes.
