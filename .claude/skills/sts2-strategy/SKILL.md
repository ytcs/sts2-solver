---
name: sts2-strategy
description: Use at the start of a run and when a decision is not covered by pathing or deckbuilding: general principles shared by every character and act (eval before every choice, HP gates, combat and potion rules) and the per-decision loop.
---

# General strategy

## The loop (every decision)
1. Identify the decision type and invoke its skill: map / Neow -> `sts2-pathing`; card, shop, rest, relic, upgrade -> `sts2-deckbuilding`; fight -> `adv` / `combat` (`sts2-harness`), `sts2-mechanics` for unknowns.
2. Name the known facts that bound it (and the plan for the next two acts: `sts2-deckbuilding` section 4, three horizons): the boss (shown on the map), the elites that can still appear, HP and gold now, the rests and shops ahead.
3. Price the options with `eval` (against the known boss and the next threats), skip if nothing clears the bar.
4. Act with a `-- why` containing the numbers; after the run, update the book at the most specific level.

## Principles
- **Price every choice with `eval`** (cards, relics, upgrades, removals, shop buys) against the elite, boss and regular pools of the act, then decide; intuition alone is not enough. Put the numbers in the `-- why`. Use `--hp full` and 256 attempts; take a gain only if it clears ~2 SE (about 3 points of win rate) on a pool that matters, else skip. The regular pool saturates at 100% early, so its differences show only as HP lost. `eval` does not price the deck slot, rewards still to come, gold, rest actions or route interplay: price those by hand.
- **Find the gate that binds.** For the known boss run `eval` at several HPs (`--hp 34/45/60/80`): if win rate climbs steeply with HP the boss is HP-gated (plan rests and avoid costly elites late); if it is flat and low the deck is damage-gated (spend HP on elites for relics and cards, pick damage). The answer changes as the deck grows, so re-test every few picks.
- **Combat:** ask `adv` at non-trivial decisions and play its line; deviate only for what the solver cannot weigh (a potion kept for the boss, a route consequence) and say why. My manual lines against Vantom cost ~25 HP more than the solver's `[played]`. If every line loses the same HP, thinking cannot help: that is a macro matter. Read power descriptions and relic counters before big hits (`sts2-mechanics`).
- **Route, Neow / ancient and deck-building** are procedures with their own skills: `sts2-pathing` (co-optimize option and path, work back from the boss's HP gate) and `sts2-deckbuilding` (smooth-greedy numbers via `reward`, then the blind-spot judgment pass). Check gold against shop prices before routing to a shop.
- **Potions:** keep the strongest for the act's boss unless the solver sees a clear loss without it.
- **Events:** take a relic or large resource over a small heal; a curse costs one slot, price it against the gain.

## Measured `[sim]` / `[played]`
- Revised: an early note said bosses are won by damage, not HP; true only for a weak deck (see `sts2-ironclad-act1`).
- Shakedown run: Act 1 cleared, died at Act 2 floor 24 after entering an elite at 37/80 by a chained map click: never chain map choices.
