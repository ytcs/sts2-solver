---
name: sts2-mechanics
description: Use before fighting an unfamiliar enemy or when a power, relic or card behaves unexpectedly: verified mechanics (Slippery, Hard to Kill, Burrowed, Artifact, Pen Nib, Inferno/Rupture triggers and more).
---

# STS2 mechanics (verified; add only what code or play confirmed)

## Enemy powers
- **Slippery N** (Vantom, boss): every unblocked hit deals 1 damage and removes one stack. The first N damaging hits are almost free whatever their size: strip stacks with many cheap hits (0-cost cards, Thunderclap, Inferno triggers) and keep Bludgeon-class hits for after. `[code]` `[played]`
- **Hard to Kill N** (Exoskeletons): damage per hit is capped at N. Use many small hits and area damage; big hits are wasted. `[code]` `[played]`
- **Burrowed** (Tunneler): it gains a lot of block; breaking the block stuns it, cancelling its next attack. A potion plus an Inferno trigger broke 28 block in one turn. `[code]` `[played]`
- **Artifact N**: absorbs the next N debuffs (my Vulnerable). Do not count on Vulnerable against it. `[played]`
- Phrog Parasite (Act 1 elite) dies into four Wrigglers that are stunned on their first turn; they and the Infection cards they add reward area damage. `[played]`
- Vantom (Act 1 boss): a 30+ damage hit about every 4th turn, buff turns between; plan block for the big one. `[played]`

## My side
- **Inferno+** triggers on every HP loss on my own turn (start-of-turn loss, Bloodletting, Brand, Infection damage at end of turn): each trigger is a separate hit on all enemies. Strong against Slippery, Hard to Kill and groups. `[played]`
- **Pen Nib**: every 10th Attack played deals double; the counter persists across fights (`relics`). Card displays show doubled numbers when the next attack is the 10th. Aim it at the biggest single hit that is not capped. `[played]`
- **Armaments+** upgrades the whole hand for the rest of the combat; play it before cards that profit (Battle Trance+, Bash+). Cost 1 energy; compare with the turn's alternative. `[hyp]`
- **Tangled**: my attacks cost 1 more; **Frail**: block reduced; **Shrink**: my damage reduced. They show in the displayed numbers.
- Shop **Meal Ticket** heals 15 on entering a shop; entering a new act heals (13 -> 66 of 80). `[played]`
- Boss and elite reward screens can offer extra rare card sets (White Star). `[played]`

## Events
- **Slippery Bridge** (Act 1+, after floor 6) `[code]` (`SlipperyBridge.cs`): "Overcome" removes the card SHOWN in its hover tip, not a random one. The first shown card is random among non-Basic removable cards (never a Strike, Defend or Bash). "Hold On" costs HP (3, then 4, 5, ...) and re-rolls the shown card to a different type; the re-roll can draw any removable card except ones already skipped, Basics included. So: price removing each card with `eval remove=ID`, and hold on until the shown card is a Strike/Defend or otherwise cheap to lose, if the escalating HP is affordable. `[played]` I removed Tremble (-5.1 pts vs Lagavulin) on my first reading of this event; a Strike removal was +2.3.
