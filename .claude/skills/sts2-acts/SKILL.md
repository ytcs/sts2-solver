---
name: sts2-acts
description: Use when planning routes or fights: act structure, encounter pools (weak/regular/elite/boss per act), how the game draws encounters (bag and narrowing), the A10 double boss; encounters.md lists what each boss and elite asks of a deck, target decks and potion numbers.
---

# STS2 acts and encounter pools

## Encounter assignment `[code]` (ActModel.GenerateRooms, RoomSet.cs)
- Act start pre-rolls hidden lists (monsters: weak first, then regular; elites; boss; ancient). The Nth monster/elite room entered gets list[N]. Never read the lists.
- Bag per kind, refilled with the whole pool, avoiding the previous entry's tags. After n met of pool P the next is in the pool minus the last n mod P met. `eval`/`route` apply it.
- Three elites per pool. Rooms (excl. boss, ancient): Overgrowth 15, Underdocks 15, Hive 14, Glory 13; weak fights 3/3/2/2. A10: second boss.

## Pools `[code]` (ids `_WEAK`, `_NORMAL`, `_ELITE`, `_BOSS`)
- A1 Overgrowth: weak Fuzzy Wurm Crawler, Nibbits, Shrinker Beetle, Slimes; elites Bygone Effigy, Byrdonis, Phrog Parasite; bosses Vantom, Ceremonial Beast, The Kin.
- A1 Underdocks: weak Corpse Slugs, Seapunk, Sludge Spinner, Toadpoles; elites Phantasmal Gardeners, Skulking Colony, Terror Eel; bosses Waterfall Giant, Soul Fysh, Lagavulin Matriarch.
- A2 Hive: weak Bowlbugs, Exoskeletons, Thieving Hopper, Tunneler; elites Decimillipede, Entomancer, Infested Prisms; bosses The Insatiable, Knowledge Demon, Kaiser Crab.
- A3 Glory: weak Devoted Sculptor, Scrolls of Biting, Turret Operator; elites Knights, Mecha Knight, Soul Nexus; bosses Queen, Test Subject, Aeonglass.
- Regular: the act's other `_NORMAL` encounters.
- `[hyp]` how act 1 is chosen (Underdocks vs Overgrowth). Test: decomp.
