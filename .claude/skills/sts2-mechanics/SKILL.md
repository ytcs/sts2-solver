---
name: sts2-mechanics
description: Use before fighting an unfamiliar enemy, at any event after floor 1, or when a power, relic or card behaves unexpectedly: verified mechanics (Slippery, Hard to Kill, Burrowed, Artifact, Pen Nib, Inferno/Rupture triggers, Slippery Bridge and more).
---

# STS2 mechanics (add only what code or the simulator confirmed)

## Enemies `[code]`
- Slippery N: each unblocked hit deals 1 and removes a stack: cheap hits first. Hard to Kill N: damage per hit capped at N. Burrowed: breaking the block stuns. Artifact N: absorbs N debuffs.
- Living Fog: Smoggy (one Skill per turn), Gas Bombs explode for 9.
- `[hyp]` Phrog Parasite -> four Wrigglers + Infections; Paper Cuts costs max HP. Test: decomp.

## Your side
- Inferno+ hits all enemies on every HP loss on your turn. `[hyp]` test: decomp.
- Pen Nib / Nunchaku counters carry across fights; the solver ignores that: in a safe AUTO fight steer to 9 before an elite/boss. `[hyp]` test: `relics` across fights, win with counter at 9.
- Armaments+ upgrades the hand: play it first. `[hyp]`
- Petrified Toad: a 15-damage rock each fight; price a potion filling the last slot as its use minus the rocks displaced (`eval --v "rock|potions=..."`).
- Orrery / multi-set rewards: open + skip loses nothing until Proceed; price bundles. `[expert]`

## Events
- Slippery Bridge `[code]`: Overcome removes the shown card; Hold On costs 3, 4, 5 ... HP and re-rolls. Price each card `eval --v "x|remove=ID"`.
- Catalog `data/events.json`; `price` prices catalogued events; else judgment + note the gap.
