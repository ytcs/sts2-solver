---
name: sts2-acts
description: Slay the Spire 2 act structure and encounter pools: room counts, how many weak fights each act has, weak/regular/elite/boss lists per act, ancients, the A10 double boss, and which information is hidden. Use when planning a route or estimating the cost of the next fight.
---

# STS2 acts and encounter pools

## How encounters are assigned `[code]` (ActModel.GenerateRooms, RoomSet.cs)
- At act start the game pre-rolls ordered lists: the monster list (first `NumberOfWeakEncounters` from the weak pool, the rest of the act's rooms from the regular pool), a list of 15 elites from the elite pool, the boss, and the ancient.
- Draws use a bag with no repeats until the bag empties, and avoid sharing tags with the previous entry when possible.
- The Nth monster room you ENTER gets `list[N]`, the Nth elite you enter gets the Nth elite: difficulty depends on how many fights you have taken, not on which node. The lists themselves are hidden; do not read them.
- Rooms per act (excluding boss and ancient): Overgrowth 15, Underdocks 15, Hive 14, Glory 13. Weak fights: Overgrowth 3, Underdocks 3, Hive 2, Glory 2.
- Ascension 10 adds a second boss (the game's act code: "Double Boss mode (Ascension 10+)"). `[code]` The user confirms A10 has double boss by default.

## Pools (solver encounter ids carry `_WEAK`, `_NORMAL`, `_ELITE`, `_BOSS`) `[code]`
| act | weak | elites | bosses |
|---|---|---|---|
| Act 1 Overgrowth | Fuzzy Wurm Crawler, Nibbits, Shrinker Beetle, Slimes | Bygone Effigy, Byrdonis, Phrog Parasite | Vantom, Ceremonial Beast, The Kin |
| Act 1 Underdocks | Corpse Slugs, Seapunk, Sludge Spinner, Toadpoles | Phantasmal Gardeners, Skulking Colony, Terror Eel | Waterfall Giant, Soul Fysh, Lagavulin Matriarch |
| Act 2 Hive | Bowlbugs, Exoskeletons, Thieving Hopper, Tunneler | Decimillipede, Entomancer, Infested Prisms | The Insatiable, Knowledge Demon, Kaiser Crab |
| Act 3 Glory | Devoted Sculptor, Scrolls of Biting, Turret Operator | Knights, Mecha Knight, Soul Nexus | Queen, Test Subject, Aeonglass |
Regular pools are the act's remaining `_NORMAL` encounters (full list: `GenerateAllEncounters` in `MegaCrit.Sts2.Core.Models.Acts/<Act>.cs` of the decompile).

Consequences:
- Each act has only three elites in its pool, so a fourth elite in an act is a repeat. `[code]`
- With few elites per pool, each elite met removes it from the next draws until the bag refills. Tracking which weak, regular and elite encounters were already seen narrows what comes next. Allowed: it is observed information.

## Observed in my own harness tests `[played]`

- The full map (every room type and every edge) is visible while the Neow / ancient choice is on screen, so route and ancient can be planned together. `[played]`

- Entering a new act (the ancient event) heals: HP went from 13/80 at the end of Act 1 to 66/80 on arriving at the Act 2 ancient. `[played]`

## Open questions `[hyp]`
- Whether the boss's identity is visible on the map before the act's fights (the map export shows only its row).
- Underdocks vs Overgrowth: how the game picks the Act 1 variant.

## Route and ancient reasoning (general rules: `sts2-strategy`)
- Option-by-route check: score every ancient option on each plausible route; if the ranking never flips, choose the best option then route; if it flips, decide the pair.
- Cost a route node by its slot: monster rooms beyond the weak allowance are regular-pool fights; elites cost by the elite pool average minus those already met.
- Routes are policies: commit to the next node only.
