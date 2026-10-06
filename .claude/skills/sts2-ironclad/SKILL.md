---
name: sts2-ironclad
description: Use when the character is Ironclad (at character select and at every Ironclad card or relic choice): only what differs from the general strategy: A10 start facts, HP-loss triggers, card values, conditional picks.
---

# Ironclad (deviations from `sts2-strategy`)

## Starting facts `[code]`
- A10 start: 64/80 HP, 99 gold, Burning Blood (+6 HP after each fight), 11 cards (5 Strike, 4 Defend, Bash, Ascender's Bane). The starter deck wins Act 1 weak and regular fights and loses the first elite (Bygone Effigy, predicted win 2%) `[sim]`.
- Burning Blood makes HP cheap between fights: spend it for relics and cards. Test: `eval --hp` at the arrival HP of the elite route vs the safe route. `[hyp]`
- Ascender's Bane and Greed are Eternal (cannot be removed); curses (Greed from Cursed Pearl, Clumsy) are dead slots.

## Cards
- HP-loss triggers: Inferno+, Bloodletting, Brand, Rupture, Hemokinesis, Breakthrough (`sts2-mechanics`). Relax (Pael's Horn) +30 vs Act 2 elites `[hyp]`; test: `eval --elites` with Relax added.
- Act 1 boss pool from a weak deck (win 12-16%; 256 attempts, se ~1.5) `[sim]`: Dismantle +13.4, Hemokinesis +12.5, Perfected Strike +11.3, Setup Strike +7.8, Twin Strike +6.9, Infernal Blade +5.3; Flame Barrier, Drum of Battle, Vicious, Blood Wall, Cinder ~0. Damage beats block for bosses; verify with boss-alone `eval`.
- Perfected Strike counts every card named Strike (Twin Strike, Setup Strike): the starter deck is its engine `[hyp]`. Test: `add=PERFECTED_STRIKE` with and without `remove=` Strikes.
- True Grit exhausts a RANDOM card; True Grit+ lets me CHOOSE it. With curses or statuses in the deck or added by enemies (Clumsy, Toxic, Beckon, Wound), the choice is worth much more than the +2 block: True Grit+ is a priority smith target there, and price it with `upgrade=TRUE_GRIT` against the fights that add statuses. `[hyp]` the size of the gain: test `eval --v "up|upgrade=TRUE_GRIT"` vs encounters that add statuses.
- Second Wind `[sim]` (384 attempts, 80 HP): +3.9 vs Aeonglass (Wither junk is exhausted for block), -4.2 vs The Insatiable, -1.8 vs Glory elites. Pick only when Aeonglass is the gate or the deck carries many status / junk cards.
- Pael's Tooth: removed cards return upgraded, one per combat, ~5 fights of thinning `[hyp]` (source unrecorded; check the relic text).

## To test `[hyp]`
- Smith vs rest before a boss by HP and boss: `eval` at the HP after the rest vs now with the upgrade.
- HP cost of a typical Act 1 elite for a starter-plus-few-cards deck and the break-even: `route` over elite vs non-elite lanes.
