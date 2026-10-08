# STS2 game-code bugs (v0.111.0)

Found while porting the game bit-exactly. Not fixable from our side; the simulator guards against each.

## 1. Soft-lock: Pillage + Hellraiser + Velvet Choker
**Effect:** the game hangs mid-turn; the fight never ends.
**Reproduce:** Ironclad, Velvet Choker, Hellraiser in play, Pillage in hand; draw + discard hold only Strike-tagged cards; play 6 cards, then Pillage (or let its auto-played Strikes reach 6).
**Loop:** Pillage draws a Strike -> Hellraiser auto-plays it -> Choker refuses the 7th play -> `CardCmd.AutoPlay` discards it unplayed -> Pillage drew an Attack, draws again -> empty draw pile reshuffles the same Strike -> repeat.
**Cause:** `Pillage.OnPlay` loops `do { Draw } while (drawn is Attack && hand < MaxCardsInHand)` and a refused auto-play never fills the hand; `VelvetChoker` refuses at 6 plays and counts auto-plays; `CardPileCmd.ShuffleIfNecessary` has no per-draw limit; `HellraiserPower._infiniteAutoPlayCap = 9` applies only vs infinite-HP enemies.
**Fix:** stop the loop when a drawn card leaves the hand unplayed, or cap draws per play.
**Simulator:** `engine/budget.rs` cuts the step (`ov::LOOP`) and scores it as a loss (evidence E6). Observed in a training fight vs KNIGHTS_ELITE (full dump: git history of this file).

## 2. Random move roll can throw (float rounding)
**Effect:** exception in an enemy's move roll, ~1 in millions.
**Cause:** `RandomBranchState.GetNextState`: `max = States.Sum(weights)` (double accumulate, cast to float), `num = rng.NextFloat(max)`, then subtract each weight in float and return at `num <= 0f`; a tiny positive remainder falls through to `throw new InvalidOperationException("No valid state found ...")`.
**Reproduce:** any `RandomBranchState` with non-integer or several weights when the draw lands just below the total.
**Fix:** return the last positive-weight branch after the loop, or walk in double.
**Simulator:** returns the last positive-weight branch (`crates/sts2sim/src/engine/monster.rs`).
