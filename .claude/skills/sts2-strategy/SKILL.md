---
name: sts2-strategy
description: General Slay the Spire 2 strategy shared by every character and act: how to judge fights, price every choice with eval, boss-first deck building, route and rest decisions, potions, HP as a resource. Character and act skills only add to or deviate from this.
---

# General strategy

## Principles
- **Price every choice with `eval`** (cards, relics, upgrades, removals, shop buys) against the elite, boss and regular pools of the act, then decide; intuition alone is not enough. Put the numbers in the `-- why`. Use `--hp full` and 256 attempts; take a gain only if it clears ~2 SE (about 3 points of win rate) on a pool that matters, else skip. The regular pool saturates at 100% early, so its differences show only as HP lost. `eval` does not price the deck slot, rewards still to come, gold, rest actions or route interplay: price those by hand.
- **Boss-first.** The boss pool decides the early acts: bosses are won by damage output, not HP (boss win rate moved 18% -> 24% from 49 to 80 HP `[sim]`, Ironclad Act 1), while elites are won at 94-100% even at ~46 HP. So HP spent on an elite for a relic and a card is cheap, and card picks are judged on boss and elite pools. Re-run the boss pool after every pick: the weakest boss is the one to build for.
- **Combat:** ask `adv` at non-trivial decisions and play its line; deviate only for what the solver cannot weigh (a potion kept for the boss, a route consequence) and say why. My manual lines against Vantom cost ~25 HP more than the solver's `[played]`. If every line loses the same HP, thinking cannot help: that is a macro matter. Read power descriptions and relic counters before big hits (`sts2-mechanics`).
- **Route and Neow / ancient:** the most important macro decision; each floor is a scarce resource. Co-optimize the option and the path with the procedure in `sts2-pathing` (enumerate paths, `route`, option-by-route table, `draw` the route, re-plan per node). Check gold against shop prices before routing to a shop. Act-transition heals (to ~80%) and shop heals belong in the HP budget.
- **Cards:** threshold-and-skip; cover the known weakness; rares deserve a longer evaluation than the next pool; a marginal common is a skip; removal and thin decks matter when the deck cycles on a few cards.
- **Potions:** keep the strongest for the act's boss unless the solver sees a clear loss without it.
- **Events:** take a relic or large resource over a small heal; a curse costs one slot, price it against the gain.

## Measured `[sim]` / `[played]`
- Shakedown run: Act 1 cleared, died at Act 2 floor 24 after entering an elite at 37/80 by a chained map click: never chain map choices.
