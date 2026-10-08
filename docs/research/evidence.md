# Rebuild evidence log

Measurements taken while planning the rebuild (2026-10-06 onward). Each entry: question, method, result, implication.

## E1. Is the h128 outcome head a predictor of the best play we have?
- **Method:** `target/rebuild/calib_start.py`. First 600 fights of `data/train/eval.json` (5 characters, 3 acts, A10).
  - Predicted P(win) at fight start: the h128 outcome head (`1 - P(class 0)`), averaged over 8 opening shuffles.
  - Realized win rate, 8 attempts per fight, under two kinds of play:
    - greedy h128;
    - search at 3x8 with h128 as the only evaluator (depth 2).
- **Result:**

| play | win | predicted | bias | pred 0.2-0.4 → actual | 0.4-0.6 → | 0.6-0.8 → |
|---|---|---|---|---|---|---|
| greedy | 0.664 | 0.663 | -0.001 | 0.31 | 0.59 | 0.70 |
| search 3x8 | 0.728 | 0.663 | -0.065 | 0.49 | 0.73 | 0.88 |

- **Implication:** the head predicts the raw policy, not search play.
  - It underestimates search outcomes by 19-23 points in the 0.2-0.8 band, where macro decisions turn.
  - Any macro pricing built on it would be pessimistic and would mis-rank risky fights.
  - The foundation predictor must be trained on outcomes under the play it is supposed to predict, through search distillation or expert iteration.
  - The search's gain is concentrated in mid-probability fights.

## E2. Knowledge Demon at 0%: is it the deck or the solver?
- **Method:** `eval --boss --hp 70 --attempts 128` from the playtest run, at the Act 2 start: Silent with Velvet Choker, a 21-card Act 1 deck, a Flex Potion.
- **Result:**

| deck | win |
|---|---|
| deck as played | 0.00 |
| + poison engine (Noxious Fumes, Accelerant, 2x Bouncing Flask, Afterimage, Malaise; -3 Strike, -2 Defend) | 1.00 (HP lost 29%) |
| + power scaling (2x Noxious Fumes, Accelerant, Afterimage, Footwork, Well-Laid Plans, Burst, Corrosive Wave; -6 basics) | 0.98 |
| + shiv package | 0.22 |

- **What this shows:** under h128 search (5x32 via `eval`, 128 attempts, potions in the belt), the solver beat Knowledge Demon with the two hand-built decks above and never with the deck I played.
- **What it does not show:**
  - That the played deck is unwinnable: a stronger player might win with it.
  - That the shiv idea is weak: the solver may play shiv lines badly, and one hand-built package is not the archetype.
  - The results are lower bounds under this solver, not verdicts on decks or plans.
- **Implication for the design:** greedy single-pick pricing gives no signal while every option scores 0 under the solver. Plans tested as whole decks give a signal, bounded by the solver's strength.

## E3. Baseline benchmark (h128, labels from live-width search 5x32, `tools/bench.py`)

| set | n | search win | predicted | bias | pred 0.4-0.6 → actual |
|---|---|---|---|---|---|
| eval | 600 | 0.753 | 0.663 | -0.090 | 0.50 → 0.79 |
| corpus (real runs) | 192 | 0.783 | 0.764 | -0.019 | 0.47 → 0.88 |
| mix (cross-character, big belts) | 600 | 0.720 | 0.594 | -0.126 | 0.51 → 0.78 |
| tail (Act 3 elites/bosses, 28+ cards) | 400 | 0.299 | 0.155 | -0.144 | 0.49 → 0.78 |

- **Ranking (550 pairs: add / remove / upgrade a card, drop a potion; 32 paired attempts):** predicted worth difference has the same sign as the reference on 63% of pairs with |d| > 0.02. By kind: add 0.52, remove 0.52, upgrade 0.66, potion 0.83.
- **The reference is too noisy to judge card effects.** It agrees with itself on only 66% (split-half 16 vs 16 attempts, Spearman 0.35). Card effects are small (mean |Δworth| 0.04-0.09) against fight-to-fight variance.
- **Implications:**
  - The ranking benchmark needs about 256+ attempts per pair (at a cheaper search width) to have a ceiling worth measuring against.
  - Single macro choices move outcomes by a few points, so pricing them by sampling fights needs hundreds of play-outs per option. A calibrated predictor that returns the expectation directly is the only affordable way to price them.

## E4. Round 1 of expert iteration (`rl/exit.py`)
- **Setup:**
  - Data: h128 search at 3x8 played 88,600 fights, one attempt each (12 min, 122 fights/s), giving 2.33M searched decisions.
  - Pool: 60k from target/m3/train_mix, the real-run corpus train set x4, ~5.8k tail fights x2, 15k later-act fights (generator seed 47); held out from every bench set.
  - Training: 3 epochs from h128 (21 min), lr 1e-4, policy target = softmax of the options' search estimates (tau 0.02), outcome target = the fight's realized class under search play, HL-Gauss sigma 0.75.
  - Holdout loss: outcome 4.15 → 1.93, policy 2.82 → 1.18.
- **Calibration against live-width h128 search labels (`tools/bench.py score`):**

| set | bias h128 → r1 | Brier h128 → r1 | pred 0.4-0.6 → actual (r1) |
|---|---|---|---|
| eval | -0.090 → -0.017 | 0.047 → 0.030 | 0.51 → 0.62 |
| corpus | -0.019 → +0.009 | 0.032 → 0.023 | (n 4) |
| mix | -0.126 → -0.028 | 0.061 → 0.029 | 0.50 → 0.55 |
| tail | -0.144 → -0.014 | 0.079 → 0.038 | 0.51 → 0.51 |

- **What this shows:** the outcome head trained on search-played outcomes predicts search play far better on every set, including the tail. A residual -0.02 remains on eval and mix; the labels used 3x8, the bench 5x32.
- **What it does not show:**
  - Ranking quality: the strong ranking reference is not built yet.
  - Whether r1 plays as well as h128 as the search's policy: that is the `play` check.

## E5. Ranking against a strong reference (300 pairs, 256 paired attempts at 3x8 with h128)
- The reference agrees with itself (split-half, 128 vs 128 attempts) on 79.4% of signs with |d| > 0.02 (141 pairs); Spearman 0.60.

| predictor | worth sign agreement (137 pairs, \|d\|>0.02) | P(win) sign (44 significant) | Spearman worth | add / remove / upgrade / potion |
|---|---|---|---|---|
| h128 | 0.788 | 0.841 | 0.533 | 0.69 / 0.57 / 0.92 / 0.95 |
| r1 | 0.796 | 0.864 | 0.526 | 0.69 / 0.63 / 0.88 / 0.95 |

- **What this shows:** one forward pass of either predictor orders these deck changes about as well as 128 paired search play-outs do; r1 ranks no worse than h128.
- **Weak spots:** card removals (0.57-0.63) and adds (0.69).
- **What it does not show:**
  - The split-half is a lower bound on the full reference's reliability, so the gap to the truth is unknown.
  - The pairs are random variants of generated decks, not the choices a run actually offers.

## E6. Long-lived search processes slow down (collection safeguards)
- **Round 2, first attempt:** one `FastSearch.run` over 64.5k fights at 5x32 ran 6.7 h without finishing, at 7 GB of memory. It was stopped.
- **Round 2, chunked attempt:** 4,096-fight chunks in one process ran at 25-27 fights/s for 8 chunks (153-162 s each). Chunk 9 took 479 s and the watchdog (3x the median) ended the run.
- **Bisection:** the same 4,096 scenarios, run in 256-fight pieces in a fresh process, finished in 4.7 min, with every piece at 14-22 s.
- **Correction (later the same day): the process-state explanation did not hold.** A fresh process that started with chunk 9 also passed the watchdog limit (600 s), while the chunk's first 256 jobs, with their exact seeds, ran in 15 s in a fresh process. Cause unknown, still under investigation; chunking with resume plus the watchdog keeps a collection bounded meanwhile.
- **Root cause (bisection with exact seeds, each piece in a subprocess with a hard timeout):**
  - The culprit was one job: pool_r2_rest #10351, Ironclad, 24-card deck, KNIGHTS_ELITE, seed 101 x 1,000,003 + 10351.
  - Alone it hung for minutes; its 15 neighbours took 0.2-1.2 s each.
  - On a build with the engine loop guard it finishes in 1.9 s.
  - So an infinite loop inside one engine step, reached in a search play-out, hung the search. Nothing bounded a single step before the guard.
- **What loops (answered):** a real-game soft-lock, not a simulator bug and not a combo: Pillage + Hellraiser + Velvet Choker.
  - **Method:** a scratch build that dumps the state before any step that trips `ov::LOOP` and replays it with a trace of every card play and hook call. Run in a separate package directory; the shared venv was not touched.
  - **Reproduction:** on the current build the exact job seed no longer trips (0 trips in 1, 8 and 64 copies). Other seeds of the same scenario do: 12 trips over 64 seeds at 5x32. All 12 are the same cycle: Pillage in the play pile, the Choker at 6, Hellraiser on, the work budget exhausted at nesting depth 0.
  - **The state:** turn 5-6 at 2-4 HP. A Pillage created in combat (the deck has none; Infernal Blade and two Entropy are present) is played while Hellraiser's power is on. Everything else is exhausted or in hand, so the draw and discard piles hold only Strike-tagged cards (Strike, Setup Strike).
  - **The cycle:**
    - Pillage draws until it draws a non-Attack.
    - Hellraiser auto-plays each drawn Strike, which Velvet Choker counts.
    - Pillage itself counts only after it resolves, so the Strikes land until the Choker has counted 6 plays.
    - From then on every auto-play is refused, and the refused card goes to the discard unplayed.
    - Pillage saw an Attack and the hand is not full, so it draws again. The empty draw pile reshuffles the same Strike back, and so on.
    - Nothing changes: no damage, no hand growth, no end of combat.
  - **Game source:**
    - `Pillage.OnPlay`: `do { Draw } while (drawn is Attack && hand < MaxCardsInHand)`.
    - `HellraiserPower.AfterCardDrawnEarly`: `CardCmd.AutoPlay`.
    - `CardCmd.AutoPlay`: `!Hook.ShouldPlay` leads to `MoveToResultPileWithoutPlaying`, which goes to the discard.
    - `VelvetChoker.ShouldPlay`: refuses at 6 plays this turn, auto-plays included, and `AfterCardPlayed` counts them.
    - `CardPileCmd.DrawInternal`: returns the drawn card even after a hook has moved it out of the hand.
    - `CardPileCmd.ShuffleIfNecessary` and `CheckIfDrawIsPossible...`: reshuffle whenever the draw pile is empty and the discard pile is not, with no per-draw limit.
    - The game's only cap is `HellraiserPower._infiniteAutoPlayCap = 9`. It applies only when every hittable enemy shows infinite HP, and there a capped card stays in the hand, which fills it and ends Pillage. Here the game's action never completes.
  - **Simulator:** faithful step for step (`ironclad_b1.rs` Pillage, `HellraiserPower`, `VelvetChoker`, `Combat::auto_play`). The guard is what ends the step. Tests in `crates/sts2sim/tests/loop_guard.rs`:
    - with the Choker, the step trips;
    - with the Choker closing mid-chain, exactly 6 Strikes land and then it trips;
    - without the Choker, the same chain is a finite combo that wins the fight with no trip.
  - **Implication (scoring):**
    - Before: `search.rs` `terminal` reported any overflow as `OUTCOME_OVERFLOW` with score 0. At 2 HP a soft-lock line (0) beat a likely loss (-1 linear, or `u[0]`), so the search was paid for finding it.
    - Changed: in the real game, playing into it freezes the fight, so `ov::LOOP` now scores as a loss (`sts2env::looped`):
      - search play-outs get the loss value (-1 linear, `u[0]` with a worth table) and the search avoids the line (`crates/sts2env/tests/loop_loss.rs`);
      - a real fight ending this way is recorded as `OUTCOME_LOSS`;
      - `BatchEnv` episodes end as `OUTCOME_LOSS` with the loss reward;
      - counters: `BatchEnv::loops`, and the search stats `end_loop` / `fight_loops`;
      - capacity overflows stay neutral.
    - Cost: a guard false positive is now scored as a loss, so the limits must stay far above every finite chain (corpus max 174 work units vs 20,000).

## E7. Round 2 (r2, from r1)
- **Data:** about 157k search-played fights (r1's 88.6k at 3x8, plus 64.5k at the live 5x32 and 4k from the bisection), ~4.2M decisions; new look-ahead observations (S1).
- **Training:** 3 epochs, about 7 min (parallel replay).
- **Calibration (`tools/bench.py score`, both on the S1 build):**

| set | bias r1 → r2 | Brier r1 → r2 |
|---|---|---|
| eval | -0.021 → -0.005 | 0.0310 → 0.0289 |
| corpus | +0.005 → +0.005 | 0.0222 → 0.0168 |
| mix | -0.032 → -0.011 | 0.0295 → 0.0277 |
| tail | -0.017 → +0.004 | 0.0381 → 0.0341 |

- **Ranking:** unchanged (worth sign agreement 0.796 for both, Spearman 0.53-0.54).
- **S3 gate:** not fully met. Bias is within 0.02 on every set, but some deciles still miss by ~0.07 (eval 0.4-0.6: 0.51 → 0.58; mix 0.2-0.4: 0.29 → 0.36).
- **Adopted** as the predictor (`models/current.json`); h128 stays the search policy.

## E8. Large-budget search is not a better player (loop 2 pilot)
- **Method:** `tools/frontier.py`, the first 200 bench tail fights (Act 3 elites and bosses, 28+ cards), h128, 2 attempts on the same seeds. Live width 5x32 with 2-turn play-outs (32 s) vs large 8x64 with 3-turn play-outs (114 s).
- **Result:**
  - win 0.330 for both ("any attempt wins" 0.425 for both);
  - seeds won by large and lost by live: 6.2%; the reverse: 6.2%.
- **What this shows:** on these fights, a 3.5x larger search with the same network is not a stronger player. The "frontier" (large wins where live loses) is symmetric noise, so distilling large-search decisions would add nothing. Improvement has to come from the networks (policy and value), or these fights are mostly unwinnable for any play.
- **What it does not show:** whether a much deeper search (play-outs to the fight's end) or a different search would help; whether the fights are winnable (certification by any play has not been run).

## E9. Expert-iteration networks as players (paired with the labels, 5x32, 8 attempts)

| player | eval win | mix win | tail win | corpus win | end HP eval / tail |
|---|---|---|---|---|---|
| r2 (policy and value from rounds 1-2) | -0.007 +- 0.004 | +0.004 +- 0.004 | -0.017 +- 0.006 | -0.005 +- 0.003 | -1.12 / -0.91 |
| h128 policy, mean of h128 and r2 values | +0.003 +- 0.003 | +0.003 +- 0.003 | -0.001 +- 0.005 | +0.002 +- 0.002 | +0.09 / +0.06 |

- **What this shows:** distilling the live search's decisions (softmax of the options' estimates at tau 0.02) into the policy makes the player worse, most on the tail. Adding the better-calibrated value to h128's is neutral.
- **With E8:** neither a bigger search nor imitation of the live search improves the player.
- **Hypothesis (untested):** the search samples its play-outs from the policy; a sharper policy gives less diverse play-outs and worse estimates.
- **Follow-up (sharpness, 15.8k recorded 5x32 decisions):**

| network | policy entropy | search's best = prior top-1 |
|---|---|---|
| h128 | 0.659 | 0.592 |
| r1 | 1.164 | 0.530 |
| r2 | 1.284 | 0.538 |

  The distilled policies are flatter, not sharper. With estimate gaps of ~0.01 between top options and tau 0.02, the soft target is nearly uniform over the tried options and erases the prior's ranking. Next test: prior-anchored targets (`exit.py --target anchored`) and a value-only arm.

## E10. Prior-anchored policy targets and a value-only arm (from h128, the same ~157k fights as r2, 3 epochs; paired play vs labels at 5x32, 8 attempts, 1024 roots)

| arm | eval win | corpus win | mix win | tail win | end HP eval / mix / tail |
|---|---|---|---|---|---|
| B anchored (c=2) | +0.002 +- 0.003 | -0.001 +- 0.004 | +0.005 +- 0.004 | +0.009 +- 0.005 | +0.05 / +0.38 +- 0.12 / +0.09 |
| A value-only (policy heads frozen) | -0.001 +- 0.003 | -0.004 +- 0.003 | +0.003 +- 0.004 | +0.012 +- 0.006 | -0.29 / -0.08 / +0.12 |
| r2 soft target (E9) | -0.007 | -0.005 | +0.004 | -0.017 | -1.12 / -0.53 / -0.91 |

- **What this shows:** the soft target caused r2's regression. With the prior-anchored target, the first trained player shows no regression on any set: tail win +1.8 se, mix end HP +3 se.
- **What it does not show:**
  - The effects are about one point of win, at the edge of significance.
  - It is one value of c and one round.
  - Adoption needs a stronger gate: more attempts on the tail, and the certified-winnable subset.

## E11. Multiplayer-only cards in training and benchmark data
- 37 cards are multiplayer-only (`CardMultiplayerConstraint.MultiplayerOnly`), never offered in single player. Nothing filtered them before 2026-10-07:
  - 62% of the training mix (92.7k of 150k fights) held at least one;
  - 63% of the round-2 pool (40.7k of 64.5k);
  - 63% of bench eval (376 of 600) and 61% of bench mix (368 of 600).
- Now flagged in data/catalog.json (`scripts/flag_multiplayer_cards.py`) and excluded everywhere. Training skips fights holding one.
- E1-E10 were measured on data that includes them. Paired comparisons within that data remain valid. Absolute levels are not representative of single-player states, so the benchmark is to be regenerated without these cards.

## E12. The predictor's resolution, not its calibration, is the weak point
- **Method:** r2's Brier on the v1 bench sets (E7) against the noise floor a perfect predictor would hit against 8-attempt labels (mean of p(1-p)/(n-1)).

| set | Brier | floor | excess | per-fight error |
|---|---|---|---|---|
| eval | 0.0289 | 0.0059 | 0.0230 | ~0.15 |
| corpus | 0.0168 | 0.0022 | 0.0146 | ~0.12 |
| mix | 0.0277 | 0.0072 | 0.0205 | ~0.14 |
| tail | 0.0341 | 0.0111 | 0.0230 | ~0.15 |

- **What this shows:** calibrated on average (bias within 0.011), but the win estimate of a typical fight is off by 0.12-0.15. The search uses the same network to judge every leaf, which fits E8 (a bigger search with the same judge finds no more wins). Network judgment is the bottleneck to attack: capacity, PPO on hard fights, lower-variance value targets.
- **Next diagnostics:** headroom (a search on the true future, as an upper bound) and whether the search's candidates miss the best action on tail states.

## E13. Headroom on the tail: the gap is information, not search width; a third of tail fights are lost under every line
- **Method:** `tools/headroom.py`, bench v2 tail (400 Act 2-3 elite/boss fights x 2 attempts, h128). Clairvoyant arms see the true future (diagnostic only).

| arm | win | vs live (paired) |
|---|---|---|
| live 5x32 | 0.362 | |
| clairvoyant 5x32 | 0.475 | +0.113 +- 0.014 |
| clairvoyant 8x64 | 0.482 | +0.120 +- 0.014 |
| policy sampling, clairvoyant, 100 tries | 0.381 | +0.019 +- 0.012 |

- **What it shows:**
  - Widening the search adds +0.007 even with perfect information, consistent with E8: width is not the lever.
  - Most of the +0.11 is information a real player never has. It bounds what better judgment could recover; it is not a target.
  - 37.5% of the tail fights are lost by every arm, clairvoyant included. They carry no policy signal for this player.

## E14. Round 3 collected mostly low-signal fights
- **Method:** the r3 pool (70.5k fights, one attempt each, armB 5x32) scored at the fight start by r3's outcome head (`rl/predictor.py`), against the realised results and the near-miss losses (`tools/nearmiss.py`: within one turn of a win or <= 20% enemy HP left; 2823 of 20.4k losses).

| predicted P(win) | share of fights | realised win | share of near-misses | share of searched decisions |
|---|---|---|---|---|
| < 0.03 | 0.084 | 0.012 | 0.097 | 0.072 |
| 0.03-0.10 | 0.058 | 0.078 | 0.101 | 0.065 |
| 0.10-0.30 | 0.083 | 0.224 | 0.221 | 0.105 |
| 0.30-0.70 | 0.135 | 0.544 | 0.351 | 0.171 |
| 0.70-0.90 | 0.109 | 0.816 | 0.159 | 0.128 |
| 0.90-0.97 | 0.094 | 0.944 | 0.049 | 0.102 |
| >= 0.97 | 0.436 | 0.995 | 0.021 | 0.357 |

- **What it shows:**
  - The predictor is calibrated band by band.
  - 44% of the fights, and 36% of the searched decisions, went to fights the player already wins 99.5% of the time. They hold 2% of the near-misses.
  - The 0.1-0.9 band holds a third of the fights and 73% of the near-misses.
- **Paired play-check (bench v2, h128 labels, same seeds):** r3 (trained on this data plus earlier rounds) vs armB_anchored (earlier rounds only): eval +0.007 vs +0.002, corpus +0.005 vs +0.004, mix -0.000 vs +0.001, tail +0.009 vs +0.012. These are within noise of each other, so the round did not measurably improve the player.
- **Next:** a signal-weighted pool (E15, pending) against a uniform pool of the same size.

## E15. The anchored policy target is mostly noise: most decisions are near-ties
- **Method:** `tools/target_noise.py`. 2000 searched decisions from the signal-pool collection (r4s), rebuilt from seed + prefix. Each was searched twice at 5x32 with fresh seeds, and once at 5x256 as a reference (r3 network).

| measure | value |
|---|---|
| best option repeats across seeds | 0.663 |
| best option = 5x256 reference best | 0.68 |
| se of one option's estimate | 0.078 (return units) |
| median gap, best vs second option | 0.005 |
| states with a gap > 2 se | 0.085 |
| regret of the 5x32 pick under the reference | mean 0.0098; > 0.05 in 5.7% |

- **What it shows:**
  - The anchored target normalises each decision's estimates by min-max, which stretches noise-sized gaps to the full scale. At c = 2 it pushes the prior toward a near-random tried option in most states.
  - Training could not fit it. Holdout policy loss did not fall in r3 or r4s, nor with a 10x policy weight (0.5593 -> 0.5597). KL(target || net) rose from 0.0557 to 0.0588.
- **Change:** `train --qnorm abs` shifts the prior by c x (q - mean q) in return units. A near-tie then moves the prior by about 0.02 logits; a 2-se gap at c = 4 moves it by about 0.9. A/B pending (r4s_abs4 vs r4s).

## E16. Gumbel root vs top-M at equal futures (tail states, Monte Carlo referee)
- **Method:** `tools/bench_search.py`, 150 states from 46 tail fights; regret against a referee that plays out every legal action.
- **Result (regret difference, top-M minus Gumbel; > 0 means Gumbel is better):**
  - 5x32 vs Gumbel 16x160: -0.0115 [-0.0235, -0.0007] at sigma scale 0.1, the default;
  - at sigma scale 0.3: 0.0000 [-0.005, +0.007];
  - at sigma scale 1.0: -0.0016 [-0.009, +0.006];
  - 4x the futures: Gumbel no better (-0.0087 [-0.019, +0.000]), and top-M 5x32x4 no better than 5x32.
  - The referee's best action is in the prior's top 5 in 91% of the states.
- **What it shows:** candidate coverage is not the bottleneck for live picks, and Gumbel halving does not beat top-M there. Its use would be the training target, not live play.

## E17. Network capacity: d = 256 learns faster than d = 128
- **Method:** PPO from scratch, same recipe, one 5090 each. Greedy eval on the same 1500-fight eval set, 2048 episodes (se ~0.010). The learning-rate schedules differ: d128 decays over 6400 iterations, d256 over 1600.

| network | win | HP lost (all) |
|---|---|---|
| d128, 1600 iterations | 0.664 (own log: 0.636) | 0.420 |
| d256, 1600 iterations | 0.696 | 0.402 |
| h128 (live, long training) | 0.704 | 0.388 |
| r3 (h128 + ExIt) | 0.711 | 0.384 |

- d256 led at every eval point from iteration 100 (0.602 vs 0.572), when the two learning rates were still close.
- **Next:** a long d256 run (6400 iterations) to see whether it ends above h128.

## E18. Pool and target A/B: no difference; ExIt fine-tunes of this size do not move the player
- **Method:** three arms trained from r3, 3 epochs, lr 1e-4, each on its own 35k fights (5x32, r3 player), then play-checked on bench v2 (paired with h128's labels).

| arm | eval | corpus | mix | tail | holdout policy loss, before -> after |
|---|---|---|---|---|---|
| r4s: signal pool, anchored minmax c 2 | +0.006 | +0.004 | +0.003 | -0.000 | 0.559 -> 0.564 |
| r4u: uniform pool, anchored minmax c 2 | +0.004 | +0.003 | +0.001 | +0.008 | 0.686 -> 0.690 |
| r4s_abs4: signal pool, anchored abs c 4 | +0.005 | +0.004 | +0.001 | +0.008 | 0.556 -> 0.563 |
| (r3 itself) | +0.007 | +0.005 | -0.000 | +0.009 | |

- Paired se is 0.003-0.006 per cell.
- **What it shows:**
  - Every arm is within noise of r3 and of the others.
  - Holdout losses get worse in the first epoch and never beat the init, for either target. With a 10x policy weight, policy loss is still flat.
  - At 35-70k fights per round, fine-tuning a network trained on many millions of PPO steps only perturbs it. Neither the pool nor the target matters until the update itself can improve on the init.
- **Implication:** pause ExIt rounds on h128-sized networks. The levers with measured effect are capacity (E17) and data at PPO scale. ExIt needs one of two things before it can help: far more fights per update (cheaper collection), or a base network that has not saturated what it is fed.

## E19. ExIt on an unsaturated network: the policy does not move, the outcome head learns fast
- **Method:** the d256 PPO checkpoint at 1600 iterations (greedy 0.696 on the PPO eval set; scalar value, no outcome head). A fresh outcome head was attached. ExIt trained it on all clean collected fights (198k: r1, r2, r2b, diag8, r3, r4s, r4u), anchored minmax c 2, 3 epochs. `bench.py screen`, 4 attempts per fight.

| network | greedy win, paired with the d256 init (eval / corpus / mix / tail) | predictor Brier (eval / corpus / mix / tail) |
|---|---|---|
| d256 init | | 0.190 / 0.193 / 0.201 / 0.540 (fresh head) |
| d256 + ExIt | -0.002 / +0.000 / -0.001 / +0.006 | 0.026 / 0.022 / 0.028 / 0.037 |
| h128 | +0.005 / +0.010 / +0.003 / +0.017 | 0.042 / 0.034 / 0.072 / 0.102 |
| r3 | +0.005 / +0.007 / +0.009 / +0.023 | 0.021 / 0.018 / 0.024 / 0.035 |

- Holdout policy loss went from 0.588 to 0.605.
- **What it shows:**
  - The policy side of ExIt teaches nothing even to a network that is not saturated, so the target is the problem (E15), not saturation.
  - The value side works: a fresh outcome head reaches nearly r3's calibration in 3 epochs, and r3 halves h128's Brier with no bias.
  - ExIt's product today is the predictor. Policy improvement has to come from PPO (the A/B arms) or from a policy target that keeps only significant search preferences.

## E20. Observation audit: visible information the network does not get
- **Method:** an audit of `crates/sts2sim/src/observe.rs` against the game's UI code (`decomp/`), measured over all 34,967 r4s fights (1.25M states before an action; 137k selection decisions).
- **Gaps, most frequent first:**
  1. Calculated card numbers are encoded as 0: calculated damage (Body Slam, Perfected Strike, Gold Axe, Unleash, Murder...), calculated block, and calculated hit counts (Finisher, Barrage, ...). 23.8% of states; Necrobinder 35.6%, where Unleash alone appears in 15% of states.
  2. A selection screen does not say what it is for (discard / exhaust / upgrade / copy ...; 26 sources share the pick-1-from-hand screen). 59.4% of decisions.
  3. 13 powers show a second number that is not encoded (Surrounded facing, Withering Presence, Orbit, ...). Up to 16.3% of states.
  4. Selection candidates are capped at 16 while all 64 stay pickable, and pile screens sort by rarity ascending, so Uncommon and Rare cards are the ones hidden. 4.2% of decisions.
  5. Pile cards lose their enchantment, cost changes and counters. 68.7% of states; impact judged medium to low.
  8. Wrong damage previews for status damage (Burn, Wither, Decay, ...: Strength added). 3.4% of states.
  10. The 64-card pile cap truncates before sorting, which leaks a little hidden order. 0.04% of states.
- **Encoded correctly:** powers (the simulator holds at most 16), relics, potions, enemy identity, intents, look-ahead, orbs, stars, Osty, hand order and costs.
- **Implication:** every network so far was trained blind to gaps 1 and 2. Observation v2 (behind a version switch, so v1 networks keep working) goes into the next from-scratch run.

## E21. Near-miss benchmark: a sensitive metric, but the first set was contaminated
- **Method:** `tools/nearmiss_bench.py` replays fights from their start with the original fight seed and varies only the search seed (5x32, 2 attempts), against the player that collected them (r3, the luck baseline). First set: 1500 near-miss losses and 1500 close wins (<= 10% HP left) from r4s.

| network | near-miss losses won | close wins lost |
|---|---|---|
| r3 (baseline) | 0.335 | 0.292 |
| h128 | +0.002 +- 0.009 | +0.006 +- 0.009 |
| r4s* | +0.014 +- 0.009 | -0.019 +- 0.009 |
| r4s_abs4* | +0.002 +- 0.009 | -0.013 +- 0.008 |
| d256_exit* | -0.010 +- 0.011 | +0.038 +- 0.010 |

  \* trained on these exact fights, so contaminated.
- **What it shows:**
  - These fights are close to coin flips: the same player on fresh search seeds wins a third of its own near-miss losses.
  - The paired se (~0.009) is much tighter than the bench-wide play-check, so the metric can resolve small policy changes.
  - The contaminated rows cannot be read as gains.
- **Fix:** `data/bench/nearmiss.json` is rebuilt from the r4u collection (1230 near-miss losses, 1500 close wins), held out from every r4s-trained arm. The r4s set is kept as `nearmiss_r4s.json`.

## E22. No policy target closes the gap between the network and its own search
- **Method:** `tools/policy_agree.py`. 4000 held-out decision states (from the r4u collection), each searched once at 5x256 as the reference (r3's options). 349 states have a significant reference gap (> 2 paired se, 0.078). Agreement = the network's greedy action equals the reference's best. Four arms from r3, each trained on the r4s data (35k fights, 3 epochs, lr 1e-4).

| policy | agreement, significant states | agreement, all states | greedy win (screen) vs r3 |
|---|---|---|---|
| live 5x32 search (the ceiling) | 0.903 | 0.665 | |
| h128 | 0.630 | 0.534 | |
| r3 | 0.650 | 0.561 | |
| A: hard target (the search's move), policy loss only | 0.662 | 0.538 | -0.004 / -0.009 / +0.008 / +0.014 |
| B: hard target, both losses | 0.645 | 0.541 | +0.003 / +0.001 / +0.004 / +0.006 |
| C: anchored minmax c 2, policy loss only | 0.653 | 0.560 | -0.001 / -0.001 / +0.003 / -0.001 |
| D: CMPO (advantage / 0.078, clipped to [-1, 1]), policy loss only | 0.648 | 0.562 | +0.000 / -0.001 / +0.004 / -0.003 |

- The hard target is learnable: training CE fell from 2.41 to 1.16. But agreement on significant states moved only +0.012, and on all states it fell, because the network learned which near-tie the search happened to pick.
- **Not the observation gap (E20):** on significant states with a calculated-number card in hand, r3 agrees 0.674 (n 86), against 0.643 without (n 263).
- **Breakdown by the reference's best action** (significant states):

| best action | n | r3 agrees | live 5x32 agrees |
|---|---|---|---|
| play a card | 244 | 0.680 | 0.893 |
| pick | 60 | 0.633 | 0.933 |
| end turn | 35 | 0.543 | 0.943 |
| use a potion | 9 | 0.333 | 0.778 |

  The disagreements are spread across decision types. The most common are play vs play (67), pick vs pick (22), and playing on instead of ending the turn (12).
- **Policy-only training breaks the outcome head** (arm A: Brier 0.19), because the trunk moves under an untrained head.
- **Data scaling:** the hard target with both losses on 5x the data (163k fights: r1-r4s; r4u held out for the reference) gives agreement 0.650 on significant states (r3 0.650; the 35k-fight arm 0.645). Holdout CE 2.10 -> 1.13.
- **What it shows:** the policy target was not the main obstacle, and neither is data volume at this scale. On clear decisions the d128 network does not learn what its search knows. Open: capacity (a d256 distillation) or knowledge only the look-ahead provides.

## E23. The policy-search gap on clear decisions is real and survives capacity
- **Capacity:** the hard target on all data (163k fights) into the d256 network (from d256_exit) raised its agreement on significant states from 0.593 to 0.662. That is the d128 level (r3 0.650; d128 on the same data 0.650), not beyond it.
- **Reference noise check:** a second 5x256 reference with a fresh seed.
  - On the first reference's 349 significant states, the two references agree on the best action in 92.6%.
  - 276 states are significant in both. There the references agree 98.6% and the live 5x32 search agrees 95.3%.
- **Agreement on the 276 robust states:**

| policy | agreement |
|---|---|
| live 5x32 search | 0.953 |
| h128 | 0.674 |
| r3 | 0.692 |
| d128, hard target on 35k fights, policy loss only | 0.710 |
| d128, hard target on 163k fights | 0.692 |
| d256, hard target on 163k fights | 0.714 |

- **What it shows:**
  - The ~25-point gap between the network's greedy policy and its own search is not reference noise.
  - Target, data volume (5x) and width (2x) move agreement by at most ~2 points.
  - On clear decisions the search's edge comes from what its look-ahead computes, which one forward pass of these networks does not recover. This is the raw-policy vs search gap known from AlphaZero-style systems.
- **Consequence:**
  - The live player is the search. The greedy policy matters as its candidate generator (top-5 coverage 91%) and as its rollout policy. The rollout check is pending.
  - The lever the evidence points to is the search's judge: value resolution (E12), with lower-variance value targets.
  - Further policy-distillation arms are not planned.
- **Rollout check** (`tools/nearmiss_bench.py`, held-out r4u set; r3 as the search's prior and value, only the play-out policy swapped):

| rollout policy | near-miss losses won | close wins lost |
|---|---|---|
| r3 (baseline) | 0.323 | 0.251 |
| d128, hard target, policy loss only | +0.012 +- 0.011 | +0.025 +- 0.009 |
| d256, hard target, all data | -0.002 +- 0.010 | +0.028 +- 0.009 |
| d128, hard target, all data | -0.002 +- 0.010 | +0.016 +- 0.009 |

  Sharper distilled policies make worse play-outs: close wins are lost more often, consistent with E9's hypothesis. Policy distillation does not reach the player this way either.

## E24. Lower-variance value targets: TD(lambda) helps a little; first training change to move the player
- **Method:** two arms from r3, identical except for the outcome target. Data: 163k fights (r1-r4s; r4u held out for the near-miss bench). Anchored policy target (c 2), 3 epochs.
  - realized: the fight's realized ending;
  - TD(0.9): (1 - lambda) x the frozen init network's outcome distribution at the next searched decision, plus lambda x that decision's target, ending in the realized result (`exit.py --value-target td`).
- **Scoring:** holdout loss against the realized ending; predictor scores on bench v2; the held-out near-miss bench with each network as the search's prior, judge and play-out policy (5x32, 2 attempts, paired with r3).

| | r3 | realized | TD(0.9) |
|---|---|---|---|
| holdout outcome loss (realized) | 1.885 | 1.888 | 1.876 |
| Brier eval / corpus / mix / tail | 0.0205 / 0.0175 / 0.0237 / 0.0348 | 0.0214 / 0.0188 / 0.0220 / 0.0343 | 0.0206 / 0.0181 / 0.0225 / 0.0319 |
| near-miss losses won | 0.319 | +0.002 +- 0.009 | +0.007 +- 0.009 |
| close wins lost | 0.256 | -0.002 +- 0.008 | -0.014 +- 0.008 |

- **What it shows:**
  - TD avoids the first-epoch damage the realized target causes. It is the only arm whose holdout loss on true endings beats the init.
  - It is the best calibrated on the tail.
  - With it as the search's judge, close wins are lost less often (about 1.75 se), so the combined near-miss effect is about +0.02 (~2 se).
- **Not yet shown:** a lambda sweep (0.8, 0.95) and more attempts on the near-miss bench.

## E25. Optimality bracket on near-miss fights
- **Method:** `tools/nearmiss_bench.py eval`, held-out r4u set: 1230 near-miss losses and 1500 close wins, from the fight start, original fight seeds, 1 attempt per arm. Each arm is paired with r3 at live width (the luck baseline).

| arm | near-miss losses won | close wins lost |
|---|---|---|
| r3, 5x32 (live; luck baseline) | 0.328 | 0.247 |
| r3, 5x256 (8x the futures, honest) | 0.352 (+0.024 +- 0.014) | 0.209 (-0.039 +- 0.012) |
| r3, clairvoyant 5x32 (sees the true future) | 0.516 (+0.189 +- 0.015) | 0.129 (-0.119 +- 0.012) |

- **What it shows:**
  - About half of the near-miss losses (48%) are lost even when every future draw and roll is known. On that realization they are effectively unwinnable.
  - The avoidable share of near-miss losses lies between ~0.02 and ~0.19:
    - the lower bound: what a much stronger honest search recovers beyond luck;
    - the upper bound: what perfect information recovers, mostly unattainable by any real player.
  - Since 8x search recovers little, the live solver sits close to the honest frontier that its own judge allows.
  - For scale, the TD value target (E24) moved these numbers about a third as much as 8x search.
- **Cost:** the 5x256 arm took 96 min (whole fights at 8x the live cost).
- **Next:** turn restarts (`turns`), to locate where the avoidable errors are.
