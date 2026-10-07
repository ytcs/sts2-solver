# Soft-locks found in Slay the Spire 2 (v0.111.0)

Real game situations where an action never finishes and the fight cannot end. Each one was found by the simulator's loop guard (`crates/sts2sim/src/engine/budget.rs`) during search and confirmed against the decompiled game code. New ones are added as they are discovered.

## 1. Pillage + Hellraiser + Velvet Choker (found 2026-10-07)

**Ingredients**
- **Pillage** (Ironclad attack): draw cards until you draw a non-Attack.
- **Hellraiser** (Ironclad power): Strike cards are played automatically when drawn.
- **Velvet Choker** (relic): you cannot play more than 6 cards per turn.
- A draw pile and discard pile holding only Strike cards (everything else exhausted or in hand).

**What happens**
1. Pillage draws a Strike, and Hellraiser auto-plays it. Each auto-play counts toward the Choker.
2. Once 6 cards have been played this turn, the Choker refuses every further play. A refused auto-play moves the Strike to the discard pile without playing it.
3. Pillage drew an Attack and the hand isn't full, so it draws again.
4. The draw pile is empty, so the discard pile (that same Strike) is reshuffled into it, and the Strike is drawn again. Back to step 2, forever.

No damage is dealt, the hand never fills, and the turn never ends.

**Why the game doesn't stop it.** The game's only guard against endless auto-play is in Hellraiser itself (`HellraiserPower._infiniteAutoPlayCap = 9`), and it only applies when every enemy shows infinite HP. Reshuffles have no per-draw limit.

**Game code** (decompiled):
- `Pillage.OnPlay`: `do { Draw } while (drawn is Attack && hand < MaxCardsInHand)`.
- `HellraiserPower.AfterCardDrawnEarly` calls `CardCmd.AutoPlay`.
- `CardCmd.AutoPlay`: when `Hook.ShouldPlay` refuses, the card goes to the discard via `MoveToResultPileWithoutPlaying`.
- `VelvetChoker.ShouldPlay` refuses at 6 plays this turn, and `AfterCardPlayed` counts auto-plays too.
- `CardPileCmd.ShuffleIfNecessary` reshuffles whenever the draw pile is empty.

**Where it was found:** an Ironclad training fight against the Knights elite, at 2-4 HP on turn 5-6. The Pillage was generated during the fight (the deck had none). Search play-outs ran into the loop and hung the training-data collection for hours until the loop guard was added.

### State at the start of the loop

**Scenario:** `target/exit/pool_r2_rest.json` #10351 (`m3_53_30022`): Ironclad, A10, floor 47, KNIGHTS_ELITE, HP 38/130, 3 max energy.

**Deck (24 cards, no enchantments):**

| card | count |
|---|---|
| Strike | 3 |
| Defend | 3 |
| Defend+ | 1 |
| Bash | 1 |
| Armaments | 2 |
| Abundance | 1 |
| Infernal Blade | 1 |
| Entropy | 2 |
| Poor Sleep | 1 |
| Flame Barrier+ | 2 |
| Apparition | 1 |
| Hellraiser+ | 2 |
| Drum of Battle | 1 |
| Setup Strike+ | 1 |
| Eternal Armor | 1 |
| Ascender's Bane | 1 |

- **Relics:** Burning Blood, Pael's Eye, Blessed Antler, Ice Cream, Velvet Choker, Fake Blood Vial, Art of War, Paper Phrog.
- **Potions:** Swift Potion (slot 0), Stable Serum (slot 1).

**Which run:** job seed 101,208,629, which is 101 x 1,000,003 + 10351 + 25 x 7919.
- Searched alone: `FastSearch` with `models/solver_h128.pt`, M=5, K=32, roots=64, bf16, on the build after the loop-as-loss change.
- One search play-out reached the loop.
- The state was dumped with the scratch debug build (`loopdbg.rs`, not committed). It also records which effect created each generated card.
- Of 64 such seeds run one at a time, 6 reached the loop: k = 25, 27, 38, 39, 46, 49.

**The card played:** Pillage. Entropy's power made it at a turn start, by transforming the Drum of Battle.

**Just before Pillage is played** (player turn 6, in a search play-out):

| | |
|---|---|
| Player | 4/130 HP, 0 block, 8 energy |
| Potions | none (both used earlier) |
| Velvet Choker | 3 plays counted this turn |
| Player powers | Hellraiser 2 (both Hellraiser+ played), Strength 10, Demon Form 4, Setup Strike 6, Plating 6, Entropy 1 |
| Debuffs on the player | Hex 2 (every card carries Hexed 2, which makes it Ethereal), Dampen 1 (cast by the Magi Knight) |
| Flail Knight | 75/108 HP, Strength 3 |
| Spectral Knight | 87/97 HP |
| Magi Knight | 18/89 HP |

Dampen temporarily downgrades the upgraded cards while its caster lives, so the + marks are missing in this snapshot.

| pile | cards |
|---|---|
| Hand (2) | Pillage (generated), Flame Barrier+ (shown downgraded) |
| Draw (2) | Strike, Strike |
| Discard (1) | Setup Strike+ (shown downgraded) |
| Exhaust (19) | Dazed x3 (from Blessed Antler), Defend x2, Defend+ x2 (one upgraded during the fight), Entropy, Fiend Fire (from Infernal Blade), Strike, Poor Sleep, Abundance, Bash, Apparition, Armaments, Infernal Blade, Ascender's Bane, Second Wind (Entropy transformed an Armaments), Flame Barrier+ |

Not in any pile:
- the power cards already played: Entropy, Hellraiser+ x2, Eternal Armor;
- the two transformed cards: the Drum of Battle (now Pillage) and one Armaments (now Second Wind).

**Pillage resolves:**
1. Pillage hits the Magi Knight.
2. Hellraiser auto-plays the three Strike-tagged cards as Pillage draws them: Strike, Strike, then a reshuffled Strike. The Choker counts them as plays 4-6.
3. The third Strike kills the Magi Knight. Its Dampen ends, and the upgrades come back.
4. The next draw is refused.

**The moment the loop begins** (the first auto-play the Choker refuses):

| | |
|---|---|
| Player | 4/130 HP, 7 energy, Velvet Choker at 6 |
| Player powers | Hellraiser 2, Strength 10, Demon Form 4, Setup Strike 6, Plating 6, Entropy 1, Hex 2 |
| Flail Knight | 59/108 HP, Strength 3 |
| Spectral Knight | 71/97 HP |
| Magi Knight | dead |
| Hand | Flame Barrier+, and the Strike just drawn (refused, about to go to the discard) |
| Draw | Setup Strike+ |
| Discard | Strike |
| Play | Pillage (still resolving) |
| Exhaust | unchanged (19 cards) |

From here the three Strike-tagged cards (Strike, Strike, Setup Strike+) cycle forever: draw, refused, discarded, reshuffled. Nothing else changes.

**Handling:** the search scores a loop as a loss (it would freeze the real game), so the solver never plays into it. Tests: `crates/sts2sim/tests/loop_guard.rs` (the simulator loops like the game), `crates/sts2env/tests/loop_loss.rs` (the search scores it as a loss and avoids it).
