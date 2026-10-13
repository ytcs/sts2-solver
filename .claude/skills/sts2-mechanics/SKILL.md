---
name: sts2-mechanics
description: Use before fighting an unfamiliar enemy, at any event after floor 1, or when a power, relic or card behaves unexpectedly: verified mechanics (Slippery, Hard to Kill, Burrowed, Artifact, Pen Nib, Inferno/Rupture triggers, Slippery Bridge and more).
---

# STS2 mechanics (add only what code or the simulator confirmed)

## Enemies
- M1 [code] Slippery N: each unblocked hit deals 1 and removes a stack: cheap hits first. Hard to Kill N: damage per hit capped at N. Burrowed: breaking the block stuns. Artifact N: absorbs N debuffs.
- M2 [code] Living Fog: Smoggy (one Skill per turn), Gas Bombs explode for 9.
- M3 [hyp] Phrog Parasite -> four Wrigglers + Infections; Paper Cuts costs max HP. Test: instrument: decomp.

## Your side
- M4 [hyp] Inferno+ hits all enemies on every HP loss on your turn. Test: instrument: decomp.
- M5 [hyp] Pen Nib / Nunchaku counters carry across fights and the solver ignores that: in a safe AUTO fight steer to 9 before an elite / boss. Test: outcome: `relics` counters at elites / bosses.
- M6 [hyp] Armaments+ upgrades the hand: play it first. Test: instrument: decomp.
- M7 [code] Petrified Toad: a 15-damage rock each fight; price a potion filling the last slot as its use minus the rocks displaced (`eval --v "rock|potions=..."`).
- M8 [code] Toy Box (ToyBox.cs:100-114, RelicCmd.cs:89): the earliest-taken wax relic melts after every 3rd fight; pickup effects survive. Take pickup-effect relics first (Strawberry, War Paint, Pear, Mango, Whetstone), the best lasting relic last.
- M9 [code] Reward cards upgrade by act (A10: 0 / 12.5% / 25% of non-rare cards in acts 1 / 2 / 3); Molten / Toxic / Frozen Egg, Lava Lamp (no unblocked damage taken) and Silver Crucible (first 3 rewards) upgrade offers of any rarity (`docs/research/game_code.md` C2).

## Events
- M10 [code] Slippery Bridge: Overcome removes the shown card; Hold On costs 3, 4, 5 ... HP and re-rolls. Price each card `eval --v "x|remove=ID"`.
- M12 [code] Foul Potion (FoulPotion.cs): 12 damage to everyone in combat, or thrown at a Merchant for 100 gold; `price` does not see gold.
- M11 [code] Catalog `data/events.json`; `price` prices catalogued events; otherwise judgment, cite `new:`.
