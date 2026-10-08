---
name: sts2-acts
description: Use when planning routes or fights: act structure, encounter pools (weak/regular/elite/boss per act), how the game draws encounters (bag and narrowing), the A10 double boss; encounters.md lists what each boss and elite asks of a deck, target decks and potion numbers.
---

# STS2 acts and encounter pools

## Encounter assignment `[code]` (ActModel.GenerateRooms, RoomSet.cs)
- Act start pre-rolls hidden ordered lists: monsters (first `NumberOfWeakEncounters` weak, rest regular), 15 elites, boss, ancient. The Nth monster/elite room ENTERED gets list[N]: difficulty follows fights taken, not the node. Never read the lists.
- Each kind draws from a bag refilled with the whole pool when empty, avoiding tags shared with the previous entry (`AddWithoutRepeatingTags`). After n met of pool P: next is in pool minus the last n mod P met; fresh bag (n mod P == 0): all but the one just met. `eval`/`route` apply it (`macro.narrow`).
- Three elites per pool: a fourth repeats. Monster rooms past the weak allowance are regular-pool fights.
- Rooms per act (excl. boss, ancient): Overgrowth 15, Underdocks 15, Hive 14, Glory 13. Weak fights: 3, 3, 2, 2.
- A10: second boss ("Double Boss mode (Ascension 10+)").

## Pools `[code]` (solver ids `_WEAK`, `_NORMAL`, `_ELITE`, `_BOSS`)
- A1 Overgrowth: weak Fuzzy Wurm Crawler, Nibbits, Shrinker Beetle, Slimes; elites Bygone Effigy, Byrdonis, Phrog Parasite; bosses Vantom, Ceremonial Beast, The Kin.
- A1 Underdocks: weak Corpse Slugs, Seapunk, Sludge Spinner, Toadpoles; elites Phantasmal Gardeners, Skulking Colony, Terror Eel; bosses Waterfall Giant, Soul Fysh, Lagavulin Matriarch.
- A2 Hive: weak Bowlbugs, Exoskeletons, Thieving Hopper, Tunneler; elites Decimillipede, Entomancer, Infested Prisms; bosses The Insatiable, Knowledge Demon, Kaiser Crab.
- A3 Glory: weak Devoted Sculptor, Scrolls of Biting, Turret Operator; elites Knights, Mecha Knight, Soul Nexus; bosses Queen, Test Subject, Aeonglass.
- Regular pools: the act's remaining `_NORMAL` encounters (`GenerateAllEncounters`, `MegaCrit.Sts2.Core.Models.Acts/<Act>.cs`).

## Open `[hyp]`
- How the game picks the Act 1 variant (Underdocks vs Overgrowth). Test: decomp.
