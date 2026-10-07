# Slay the Spire 2 bugs (game code, v0.111.0)

Bugs in the game's own code, found while building a bit-exact simulator of it. Each entry gives the cause in the decompiled code and a way to reproduce it. They cannot be fixed from our side; the simulator only guards against them.

## 1. Infinite loop: Pillage + Hellraiser + Velvet Choker

**Effect:** the game hangs mid-turn (soft-lock); the fight can never end.

**Reproduce**
1. Ironclad with Velvet Choker, Hellraiser in play, and Pillage in hand.
2. Have the draw and discard piles contain only Strike-tagged cards (Strike, Setup Strike); everything else exhausted or in hand.
3. Play 6 cards this turn, then play Pillage (or play Pillage when its auto-played Strikes bring the count to 6).

**What happens:** Pillage draws a Strike. Hellraiser auto-plays it, but the Choker refuses the 7th play, so `CardCmd.AutoPlay` moves it to the discard pile unplayed. Pillage drew an Attack, so it draws again; the empty draw pile reshuffles the same Strike back in; repeat forever.

**Cause**
- `Pillage.OnPlay`: `do { Draw } while (drawn is Attack && hand < MaxCardsInHand)`. A refused auto-play never fills the hand.
- `VelvetChoker.ShouldPlay` refuses at 6 plays a turn, and `AfterCardPlayed` counts auto-plays too.
- `CardPileCmd.ShuffleIfNecessary` reshuffles whenever the draw pile is empty, with no per-draw limit.
- The only guard, `HellraiserPower._infiniteAutoPlayCap = 9`, applies only against enemies that show infinite HP.

**Suggested fix:** stop Pillage's loop when a drawn card leaves the hand without being played, or cap draws per card play.

<details><summary>Observed state (simulator, training fight vs KNIGHTS_ELITE)</summary>

Ironclad, 24-card deck: Strike x3, Defend x3, Defend+, Bash, Armaments x2, Abundance, Infernal Blade, Entropy x2, Poor Sleep, Flame Barrier+ x2, Apparition, Hellraiser+ x2, Drum of Battle, Setup Strike+, Eternal Armor, Ascender's Bane. Relics include Velvet Choker. Turn 6, 4/130 HP. Entropy transformed the Drum of Battle into Pillage at turn start. Choker at 3 plays; hand: Pillage, Flame Barrier+; draw: Strike, Strike; discard: Setup Strike+; 19 cards exhausted. Pillage's three auto-played Strikes are plays 4-6; the loop starts on the next draw. Full dump: git history of this file (2026-10-07).
</details>

## 2. Random move selection can throw (float rounding)

**Effect:** an exception during an enemy's move roll; rare, about one roll in millions.

**Reproduce:** any monster whose move uses a `RandomBranchState` with non-integer or several weights. It happens when the RNG draw falls just below the weight total.

**Cause:** `RandomBranchState.GetNextState` does this:
- `max = States.Sum(weights)`: `Enumerable.Sum` over floats accumulates in double, then casts to float;
- `num = rng.NextFloat(max)`;
- it subtracts each weight from `num` in float and returns at `num <= 0f`.

Sequential float subtraction can leave a tiny positive remainder after the last branch, so the loop falls through to `throw new InvalidOperationException("No valid state found in RandomBranchState ...")`.

**Suggested fix:** return the last branch with positive weight after the loop, or do the walk in double.

**Found:** the simulator, ported faithfully, hit it during a 70k-fight training collection (2026-10-07). It now returns the last positive-weight branch instead (`crates/sts2sim/src/engine/monster.rs`).
