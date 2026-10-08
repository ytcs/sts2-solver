---
name: sts2-mechanics
description: Use before fighting an unfamiliar enemy, at any event after floor 1, or when a power, relic or card behaves unexpectedly: verified mechanics (Slippery, Hard to Kill, Burrowed, Artifact, Pen Nib, Inferno/Rupture triggers, Slippery Bridge and more).
---

# STS2 mechanics (add only what code or the simulator confirmed)

## Enemy powers
- Slippery N (Vantom): each unblocked hit deals 1 and removes a stack: strip with many cheap hits (0-costs, Thunderclap, Inferno), big hits after. `[code]`
- Hard to Kill N (Exoskeletons): damage per hit capped at N: many small hits, area damage. `[code]`
- Burrowed (Tunneler): big block; breaking it stuns and cancels the next attack. `[code]`
- Artifact N: absorbs the next N debuffs. `[code]`
- Living Fog (Underdocks regular, 82 HP): first attack applies Smoggy (one Skill per turn), then summons Gas Bombs (8 HP, explode for 9): area damage. `[code]`
- Phrog Parasite dies into four Wrigglers (stunned first turn) + Infection cards: area damage. `[hyp]` Test: decomp or sim port.
- Paper Cuts N (Scroll of Biting): its unblocked hits also cost max HP. `[hyp]` Test: max HP before/after (`s`); decomp.

## My side
- Inferno+ triggers on every HP loss on my turn (start-of-turn loss, Bloodletting, Brand, Infection end of turn), hitting all enemies. `[hyp]` Test: decomp or sim port.
- Pen Nib: every 10th Attack doubles; counter persists across fights (`relics`); card text shows doubled numbers. Aim at the biggest uncapped hit. `[hyp]` Test: `relics` at end of one fight and start of the next.
- Counter relics carry across fights; the solver ignores that (Pen Nib, Nunchaku +1 energy every 10th Attack). In a safe AUTO fight steer the counter to 9 entering the next elite/boss: extra attacks, hold, or stall when HP cost is small; drive the last hallway turn by hand after `relics`. `[hyp]` Test: elite/boss win with counter at 9 vs not, minus stall HP.
- Armaments+ upgrades the whole hand for the combat: play before cards that profit. `[hyp]` Test: `adv` with/without on 2+ upgradable turns.
- Tangled: attacks cost +1; Frail: block reduced; Shrink: damage reduced; shown in displayed numbers.
- Petrified Toad: each combat start puts a Potion-Shaped Rock (15 damage) into a free slot; throw it every fight. A potion filling the last slot = its best single use minus the rock throws it displaces: price `eval --boss|--elites --v "rock|potions=<belt>" --v "new|potions=<belt>,<ID>"`.
- Orrery / multi-set rewards: all sets on screen; open + skip returns without loss; nothing spent until Proceed. Read every set, price bundles at real HP (`sts2-deckbuilding`). Book of Five Rings counts every card taken. `[expert]`
- Meal Ticket heals 15 on shop entry; new act heals; boss/elite rewards can add rare sets (White Star). `[hyp]` Test: HP before/after (`s`); decomp.

## Events
- Slippery Bridge (Act 1+, after floor 6) `[code]` `SlipperyBridge.cs`: Overcome removes the card shown in the hover tip (first: random non-Basic removable). Hold On costs HP (3, 4, 5, ...) and re-rolls to a different type, Basics allowed. Price each shown card `eval --v "x|remove=ID"`; hold on until a Strike/Defend or cheap card if HP allows. `[sim]` Tremble removal -5.1 vs Lagavulin, Strike +2.3.
- Event catalog: `data/events.json`; `price` prices catalogued events; uncatalogued: judgment + note the gap.
