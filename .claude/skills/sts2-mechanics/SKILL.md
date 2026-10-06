---
name: sts2-mechanics
description: Use before fighting an unfamiliar enemy, at any event after floor 1, or when a power, relic or card behaves unexpectedly: verified mechanics (Slippery, Hard to Kill, Burrowed, Artifact, Pen Nib, Inferno/Rupture triggers, Slippery Bridge and more).
---

# STS2 mechanics (verified; add only what code or the simulator confirmed)

## Enemy powers
- **Slippery N** (Vantom): every unblocked hit deals 1 and removes one stack, so the first N hits are nearly free whatever their size: strip stacks with many cheap hits (0-cost cards, Thunderclap, Inferno triggers), keep Bludgeon-class hits for after. `[code]`
- **Hard to Kill N** (Exoskeletons): damage per hit is capped at N; use many small hits and area damage. `[code]`
- **Burrowed** (Tunneler): gains a lot of block; breaking it stuns and cancels the next attack. `[code]`
- **Artifact N**: absorbs the next N debuffs (my Vulnerable). `[code]`
- Phrog Parasite dies into four Wrigglers, stunned on their first turn; they and the Infection cards they add reward area damage. `[hyp]` Test: the decompiled source or the sim port.

## My side
- **Inferno+** triggers on every HP loss on my own turn (start-of-turn loss, Bloodletting, Brand, Infection damage at end of turn); each trigger hits all enemies, strong against Slippery, Hard to Kill and groups. `[hyp]` Test: the decompiled source or the sim port.
- **Pen Nib**: every 10th Attack deals double; the counter persists across fights (`relics`); card text shows doubled numbers when the next attack is the 10th. Aim it at the biggest uncapped hit. `[hyp]` Test: `relics` at the end of one fight and the start of the next.
- **Counter relics carry across fights; the solver does not plan for that** (Pen Nib: the 10th Attack doubles; Nunchaku: every 10th Attack gives +1 energy; read the counter with `relics`). The search plays one fight and treats the counter after it as worthless. In a safe fight (AUTO, small HP risk, Burning Blood refunds a little), steer the counter so it sits just below the trigger entering the next elite or boss (Pen Nib at 9: the first big hit doubles; Nunchaku at 9: +1 energy on turn 1). Play extra attacks, or hold them, or stall a turn when the HP cost is small. Drive the last turn of the hallway fight by hand (`adv`, then `a <i>`) after reading `relics`. `[hyp]` Test: boss / elite win with the counter set to arrive at 9 vs not, plus the HP the stall cost (review, over runs).
- **Armaments+** upgrades the whole hand for the combat (cost 1): play it before cards that profit (Battle Trance+, Bash+). `[hyp]` Test: `adv` with and without it on turns with 2+ upgradable cards.
- **Tangled**: my attacks cost 1 more; **Frail**: block reduced; **Shrink**: my damage reduced; all show in displayed numbers.
- **Petrified Toad**: each combat start procures a Potion-Shaped Rock (15 damage) into a free slot only; an unthrown rock stays and takes a slot. Throw the rock every fight it is in the belt. A potion that would fill the last slot is a calculation, not a rule: it is worth the best single use of that potion (the fight where it helps most) minus the rocks it displaces (one 15-damage throw in every fight it waits through, until thrown). Price both belts on the fights ahead: `eval --boss` / `--elites` at the expected HP with `--v "rock|potions=<belt>" --v "new|potions=<belt>,<ID>"` (Toad procures the rock into a free slot in the simulator too).
- **Living Fog** (Underdocks regular, 82 HP): its first attack applies Smoggy (after a Skill is played, every other Skill is unplayable for the rest of that turn: one Skill per turn), then it summons Gas Bombs (8 HP minions that explode for 9). Area damage (Inferno) answers the bombs. `[code]`
- **Orrery** (and any rewards screen with several card sets): every set is on the screen at once, and opening a set then skipping returns to the list without losing it; nothing is spent until Proceed. Rule: read every set before taking any, price the offers as bundles at the real HP (`sts2-deckbuilding`, multi-pick screens): the best card of each set together and leave-one-out bundles vs the deck without them; take the best bundle. Single-card ties can add up, so do not skip every set on single-card numbers. Book of Five Rings counts every card taken. `[expert]` (the user)
- **Paper Cuts N** (Scroll of Biting): unblocked hits from it also cost max HP. `[hyp]` Test: max HP before and after a fight with them (`s`); the decompiled source.
- Shop **Meal Ticket** heals 15 on entering a shop. Entering a new act heals. Boss and elite rewards can offer extra rare sets (White Star). `[hyp]` Test: HP before and after (`s`); the decompiled source.

## Events
- **Slippery Bridge** (Act 1+, after floor 6) `[code]` `SlipperyBridge.cs`: "Overcome" removes the card SHOWN in its hover tip. The first shown card is random among non-Basic removable cards (never Strike, Defend, Bash). "Hold On" costs HP (3, then 4, 5, ...) and re-rolls the shown card to a different type; the re-roll can draw any removable card except ones already skipped, Basics included. Rule: price each shown card with `eval --v "x|remove=ID"` and hold on until it is a Strike / Defend or otherwise cheap, if the HP is affordable. `[sim]` Tremble removal -5.1 vs Lagavulin, Strike removal +2.3.
