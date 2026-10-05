---
name: sts2-ironclad
description: Ironclad-only additions to the general STS2 strategy: starting facts at A10, archetypes and key cards with eval evidence, HP-loss triggers. Load at character select; the act files add per-act deviations.
---

# Ironclad (additions to `sts2-strategy`)

## Starting facts `[played]`
- A10 start: 64/80 HP, 99 gold, Burning Blood (+6 HP after each fight), 11-card deck (5 Strike, 4 Defend, Bash, Ascender's Bane). Starter deck with solver play wins Act 1 weak and regular fights and loses to the first elite (Bygone Effigy, predicted win 2%) `[sim]`.
- Burning Blood makes HP cheap between fights: spend it for relics and cards (see boss-first in `sts2-strategy`).

## Cards and archetypes
- HP-loss triggers: Inferno+, Bloodletting, Brand, Rupture, Hemokinesis, Breakthrough; Demon Form and Bludgeon+ carried one Act 1 clear; Relax (Pael's Horn) was +30 points against Act 2 elites `[played]`. Trigger details: `sts2-mechanics`.
- `[sim]` Act 1 boss pool, from a weak deck (win 12-16%): Dismantle +13.4, Hemokinesis +12.5, Perfected Strike +11.3, Setup Strike +7.8, Twin Strike +6.9, Infernal Blade +5.3 points; Flame Barrier, Drum of Battle, Vicious, Blood Wall, Cinder about 0 (256 attempts, se ~1.5). Attack-and-damage cards beat block cards for bosses.
- Perfected Strike counts every card named Strike (Twin Strike, Setup Strike): the Strike-heavy starter deck is its engine `[hyp]`.
- Curses (Greed from Cursed Pearl, Clumsy) are dead slots; Ascender's Bane and Greed are Eternal (cannot be removed).

## To test `[hyp]`
- Smith versus Rest before a boss as a function of HP and the boss.
- How much HP a typical Act 1 elite costs with a starter-plus-few-cards deck, and when taking it pays.
