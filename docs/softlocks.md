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

**Handling:** the search scores a loop as a loss (it would freeze the real game), so the solver never plays into it. Tests: `crates/sts2sim/tests/loop_guard.rs`.
