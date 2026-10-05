---
name: sts2-acts
description: Use when planning routes or fights: act structure, encounter pools (weak/regular/elite/boss per act), how the game draws encounters (bag and narrowing), the A10 double boss; encounters.md lists what each boss and elite asks of a deck, target decks and potion numbers.
---

# STS2 acts and encounter pools

## How encounters are assigned `[code]` (ActModel.GenerateRooms, RoomSet.cs)
- At act start the game pre-rolls ordered lists: monsters (first `NumberOfWeakEncounters` from the weak pool, the rest from the regular pool), 15 elites, the boss, the ancient. The Nth monster room you ENTER gets `list[N]`, the Nth elite the Nth elite: difficulty follows fights taken, not the node. The lists are hidden; do not read them.
- Each kind draws from a bag refilled with the whole pool when empty, avoiding tags shared with the previous entry (`AddWithoutRepeatingTags`). After n met of a pool of P, the next is among the pool minus the last n mod P met; on a fresh bag (n mod P == 0, e.g. the 4th elite of 3) everything but the one just met is possible. The harness applies this in `eval` and `route` (`macro.narrow`); tracking seen encounters is observed information.
- Three elites per act, so a fourth is a repeat. Cost a route node by its slot: monster rooms beyond the weak allowance are regular-pool fights; an elite costs the elite-pool average minus those met.
- Rooms per act (excluding boss and ancient): Overgrowth 15, Underdocks 15, Hive 14, Glory 13. Weak fights: 3, 3, 2, 2.
- Ascension 10 adds a second boss ("Double Boss mode (Ascension 10+)" in the act code) `[code]`.

## Pools `[code]` (solver ids carry `_WEAK`, `_NORMAL`, `_ELITE`, `_BOSS`)
- Act 1 Overgrowth: weak Fuzzy Wurm Crawler, Nibbits, Shrinker Beetle, Slimes; elites Bygone Effigy, Byrdonis, Phrog Parasite; bosses Vantom, Ceremonial Beast, The Kin.
- Act 1 Underdocks: weak Corpse Slugs, Seapunk, Sludge Spinner, Toadpoles; elites Phantasmal Gardeners, Skulking Colony, Terror Eel; bosses Waterfall Giant, Soul Fysh, Lagavulin Matriarch.
- Act 2 Hive: weak Bowlbugs, Exoskeletons, Thieving Hopper, Tunneler; elites Decimillipede, Entomancer, Infested Prisms; bosses The Insatiable, Knowledge Demon, Kaiser Crab.
- Act 3 Glory: weak Devoted Sculptor, Scrolls of Biting, Turret Operator; elites Knights, Mecha Knight, Soul Nexus; bosses Queen, Test Subject, Aeonglass.
- Regular pools are the act's remaining `_NORMAL` encounters (`GenerateAllEncounters` in `MegaCrit.Sts2.Core.Models.Acts/<Act>.cs`).

## Open `[hyp]`
- How the game picks the Act 1 variant (Underdocks vs Overgrowth).
