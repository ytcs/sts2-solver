---
name: sts2-acts
description: Use in every fight and when planning fights: operator fight rules F# (drive mode, planning turns, lethal, potions), act structure, encounter pools and how the game draws them, the A10 double boss; encounters.md lists what each boss and elite asks of a deck.
---

# Fights, acts and encounter pools

## Operator fight rules
- F1 [hyp] `DRIVE: MANUAL` (`data/drive_manual.json`; elites / bosses predicted below 0.95 or q90 loss >= 25% HP; any fight below 0.90 or q90 >= 40%): `adv` each decision and follow it unless you see what it misses (write that in the why). Add an encounter after a worst-10% fight that shows a solver mistake; remove it when the review shows the search plays it well. Test: outcome: per-encounter HP lost, manual vs auto.
- F2 [hyp] Manual fights: plan 2-3 turns from `eN plan` + `draw`; write the plan in the why. Test: outcome: `agent.hindsight` on plan-marked turns.
- F3 [hyp] Check lethal before `turn !` / `combat !` (Vulnerable x1.5); read power text and relic counters before big hits. Test: outcome: missed-lethal count in the review.
- F4 [code] Potions: the search throws them at a reserve cost (`sts2-harness` Potions); at the first final boss it spends what the second does not need, at the last boss everything (E89). `potion aside` only to keep a potion for a later fight the search cannot see. Test: outcome: potions held at a death, ledger (should be 0).
- F5 [hyp] Not playing a card can be best (stranded draw, Skill vs Enrage, a wasted doubling). Test: outcome: hindsight on turns ending with an unplayed card the next turn wanted.

## Encounter assignment [code] (ActModel.GenerateRooms, RoomSet.cs)
- Act start pre-rolls hidden lists (monsters: weak first, then regular; elites; boss; ancient). The Nth monster / elite room entered gets list[N]. Never read the lists.
- Bag per kind, refilled with the whole pool, avoiding the previous entry's tags. After n met of pool P the next is in the pool minus the last n mod P met. `eval` / `routes` / `price` apply it.
- Three elites per pool. Rooms (excl. boss, ancient): Overgrowth 15, Underdocks 15, Hive 14, Glory 13; weak fights 3 / 3 / 2 / 2. A10: second boss.

## Pools [code] (ids `_WEAK`, `_NORMAL`, `_ELITE`, `_BOSS`)
- A1 Overgrowth: weak Fuzzy Wurm Crawler, Nibbits, Shrinker Beetle, Slimes; elites Bygone Effigy, Byrdonis, Phrog Parasite; bosses Vantom, Ceremonial Beast, The Kin.
- A1 Underdocks: weak Corpse Slugs, Seapunk, Sludge Spinner, Toadpoles; elites Phantasmal Gardeners, Skulking Colony, Terror Eel; bosses Waterfall Giant, Soul Fysh, Lagavulin Matriarch.
- A2 Hive: weak Bowlbugs, Exoskeletons, Thieving Hopper, Tunneler; elites Decimillipede, Entomancer, Infested Prisms; bosses The Insatiable, Knowledge Demon, Kaiser Crab.
- A3 Glory: weak Devoted Sculptor, Scrolls of Biting, Turret Operator; elites Knights, Mecha Knight, Soul Nexus; bosses Queen, Test Subject, Aeonglass.
- Regular: the act's other `_NORMAL` encounters. Act 1 variant (Overgrowth / Underdocks) is seeded ~50/50 (map library, E92).
