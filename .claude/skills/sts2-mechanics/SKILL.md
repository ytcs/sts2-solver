---
name: sts2-mechanics
description: Use before fighting an unfamiliar enemy, at any event after floor 1, or when a power, relic or card behaves unexpectedly: verified mechanics (Slippery, Hard to Kill, Burrowed, Artifact, Pen Nib, Inferno/Rupture triggers, Slippery Bridge and more).
---

# STS2 mechanics (verified; add only what code or play confirmed)

## Enemy powers
- **Slippery N** (Vantom): every unblocked hit deals 1 and removes one stack, so the first N hits are nearly free whatever their size: strip stacks with many cheap hits (0-cost cards, Thunderclap, Inferno triggers), keep Bludgeon-class hits for after. `[code]` `[played]`
- **Hard to Kill N** (Exoskeletons): damage per hit is capped at N; use many small hits and area damage. `[code]` `[played]`
- **Burrowed** (Tunneler): gains a lot of block; breaking it stuns and cancels the next attack (a potion plus an Inferno trigger broke 28 block in one turn). `[code]` `[played]`
- **Artifact N**: absorbs the next N debuffs (my Vulnerable). `[played]`
- Phrog Parasite dies into four Wrigglers, stunned on their first turn; they and the Infection cards they add reward area damage. `[played]`

## My side
- **Inferno+** triggers on every HP loss on my own turn (start-of-turn loss, Bloodletting, Brand, Infection damage at end of turn); each trigger hits all enemies, strong against Slippery, Hard to Kill and groups. `[played]`
- **Pen Nib**: every 10th Attack deals double; the counter persists across fights (`relics`); card text shows doubled numbers when the next attack is the 10th. Aim it at the biggest uncapped hit. `[played]`
- **Armaments+** upgrades the whole hand for the combat (cost 1): play it before cards that profit (Battle Trance+, Bash+). `[hyp]` Test: `adv` with and without it on turns with 2+ upgradable cards.
- **Tangled**: my attacks cost 1 more; **Frail**: block reduced; **Shrink**: my damage reduced; all show in displayed numbers.
- Shop **Meal Ticket** heals 15 on entering a shop. Entering a new act heals (13 -> 66 of 80). Boss and elite rewards can offer extra rare sets (White Star). `[played]`

## Events
- **Slippery Bridge** (Act 1+, after floor 6) `[code]` `SlipperyBridge.cs`: "Overcome" removes the card SHOWN in its hover tip. The first shown card is random among non-Basic removable cards (never Strike, Defend, Bash). "Hold On" costs HP (3, then 4, 5, ...) and re-rolls the shown card to a different type; the re-roll can draw any removable card except ones already skipped, Basics included. Rule: price each shown card with `eval --v "x|remove=ID"` and hold on until it is a Strike / Defend or otherwise cheap, if the HP is affordable. `[sim]` Tremble removal -5.1 vs Lagavulin, Strike removal +2.3.
