# Rebuild evidence (claims record)

Measured results behind `docs/rebuild.md`. Each E#: what was tested -> finding + numbers. Results are lower bounds under the solver used (network, width, attempts stated); never verdicts on decks or plans. Paired = same fight seeds. Widths: MxK = M candidate options x K futures. E1-E10 data include multiplayer-only cards (E11): paired comparisons valid, absolute levels not.

## E1. Is the h128 outcome head a predictor of the best play we have?
600 eval fights, 8 attempts. Head predicts greedy play (win 0.664, pred 0.663) but underestimates 3x8 search (win 0.728, bias -0.065; -19..-23 points in the 0.2-0.8 band). -> Train the predictor on search-played outcomes.

## E2. Knowledge Demon at 0%: is it the deck or the solver?
Silent Act 2 start, `eval --boss --hp 70 --attempts 128`, h128 5x32: deck as played 0.00; + poison engine 1.00; + power scaling 0.98; + shiv package 0.22. -> Whole-deck plans give signal where single-pick pricing is flat (all 0). Lower bounds only.

## E3. Baseline benchmark (h128, labels from live-width search 5x32, `tools/bench.py`)
Bias (pred - search win): eval -0.090, corpus -0.019, mix -0.126, tail -0.144. Ranking 550 deck-variant pairs at 32 attempts: sign agreement 0.63; reference self-agreement only 0.66 (too noisy). -> Ranking reference needs >= 256 attempts; macro pricing needs a calibrated predictor, not fight sampling.

## E4. Round 1 of expert iteration (`rl/exit.py`)
88.6k fights at 3x8 (2.33M decisions), 3 epochs from h128. Bias h128 -> r1: eval -0.090 -> -0.017, corpus -0.019 -> +0.009, mix -0.126 -> -0.028, tail -0.144 -> -0.014; Brier roughly halved on every set.

## E5. Ranking against a strong reference (300 pairs, 256 paired attempts at 3x8 with h128)
Reference split-half agreement 0.794. Worth sign agreement h128 0.788, r1 0.796 (Spearman 0.53); weakest on removals (0.57-0.63) and adds (0.69). One forward pass ranks deck changes about as well as 128 paired play-outs.

## E6. Long-lived search processes slow down (collection safeguards)
Cause: one job looped forever inside one engine step: real-game soft-lock Pillage + Hellraiser + Velvet Choker (Choker refuses auto-plays at 6, refused Strike goes to discard, Pillage redraws it via reshuffle; game's only cap `_infiniteAutoPlayCap = 9` does not apply). Simulator is faithful; the per-step loop guard (20,000 work units; corpus max 174) ends it. `ov::LOOP` now scores as a LOSS in search, real fights and `BatchEnv`; capacity overflows stay neutral. Collection also runs chunked with resume + watchdog. Game bug: `docs/game-bugs.md` 1.

## E7. Round 2 (r2, from r1)
~157k fights (incl. 64.5k at 5x32), S1 look-ahead observations. Bias r2: eval -0.005, corpus +0.005, mix -0.011, tail +0.004; Brier down on all sets; ranking unchanged. Some deciles still miss by ~0.07 (S3 gate not fully met). Adopted as predictor (`models/current.json`); h128 stays search policy.

## E8. Large-budget search is not a better player (loop 2 pilot)
200 tail fights, h128: live 5x32/2-turn vs 8x64/3-turn (3.5x time): win 0.330 both; seeds won only by one side 6.2% each way. -> Width is not the lever; distilling large-search decisions adds nothing.

## E9. Expert-iteration networks as players (paired with the labels, 5x32, 8 attempts)
r2 as policy+value: tail -0.017 +- 0.006, end HP -1 (worse). h128 policy + mean(h128, r2) value: neutral. Cause: soft target (softmax tau 0.02 over ~0.01 gaps) is near-uniform; distilled policies got flatter (entropy 0.66 -> 1.28).

## E10. Prior-anchored policy targets and a value-only arm (from h128, the same ~157k fights as r2, 3 epochs; paired play vs labels at 5x32, 8 attempts, 1024 roots)
Anchored (c=2): no regression (tail +0.009 +- 0.005, mix end HP +0.38 +- 0.12). Value-only: tail +0.012 +- 0.006. Effects ~1 point, edge of significance.

## E11. Multiplayer-only cards in training and benchmark data
37 multiplayer-only cards were in ~62% of training/bench fights. Now flagged in `data/catalog.json` (`scripts/flag_multiplayer_cards.py`) and excluded; training skips fights holding one.

## E12. The predictor's resolution, not its calibration, is the weak point
r2 Brier vs perfect-predictor floor (8-attempt labels): excess 0.015-0.023 on every set = per-fight win error ~0.12-0.15, while bias <= 0.011. -> Attack the judge: capacity, lower-variance value targets.

## E13. Headroom on the tail: the gap is information, not search width; a third of tail fights are lost under every line
`tools/headroom.py`, 400 tail fights x 2: live 5x32 0.362; clairvoyant 5x32 +0.113 +- 0.014; clairvoyant 8x64 +0.120; clairvoyant policy sampling x100 +0.019. 37.5% lost by every arm incl. clairvoyant.

## E14. Round 3 collected mostly low-signal fights
r3 pool (70.5k) by predicted P(win): calibrated per band; 44% of fights at >= 0.97 (realized 0.995) hold 2% of near-misses; band 0.1-0.9 holds 1/3 of fights, 73% of near-misses. r3 vs earlier rounds: within noise. -> Signal-weighted pool (`tools/signal_pool.py`).

## E15. The anchored policy target is mostly noise: most decisions are near-ties
`tools/target_noise.py`, 2000 decisions, 5x32 twice vs 5x256: best repeats 0.663; matches reference 0.68; option se 0.078; median best-second gap 0.005; gap > 2 se in 8.5%; regret mean 0.0098 (> 0.05 in 5.7%). Min-max anchoring stretches noise to full scale; holdout policy loss never falls.

## E16. Gumbel root vs top-M at equal futures (tail states, Monte Carlo referee)
`tools/bench_search.py`, 150 tail states: Gumbel root (removed) no better than top-M 5x32 at any sigma scale (default: worse, -0.0115 [-0.024, -0.001]); 4x futures helps neither. Referee best in prior top-5: 91%.

## E17. Network capacity: d = 256 learns faster than d = 128
PPO from scratch, 1600 iterations, greedy on 1500-fight eval (se ~0.010): d128 0.664, d256 0.696 (h128 long-trained 0.704, r3 0.711). d256 led from iteration 100.

## E18. Pool and target A/B: no difference; ExIt fine-tunes of this size do not move the player
Three arms from r3 on 35k fights each (signal/minmax, uniform/minmax, signal/abs c4): all within noise of r3 (paired se 0.003-0.006); holdout policy loss never beats init. -> 35-70k-fight fine-tunes only perturb a PPO-saturated network.

## E19. ExIt on an unsaturated network: the policy does not move, the outcome head learns fast
d256 PPO init + fresh outcome head, 198k fights: greedy win unchanged (within +-0.006); Brier 0.19-0.54 -> 0.022-0.037 (r3 0.018-0.035) in 3 epochs. -> ExIt's product is the predictor; target, not saturation, blocks policy learning.

## E20. Observation audit: visible information the network does not get
r4s, 1.25M states: calculated card numbers encoded 0 (23.8% of states; Necrobinder 35.6%); selection screen purpose missing (59.4% of decisions); 13 powers' second number missing (<= 16.3%); candidate cap 16 hides rares (4.2%); pile cards lose enchant/cost/counters (68.7%, low impact); status damage previews wrong (3.4%); 64-card pile cap leaks order (0.04%). Fixed in observation v2 (version switch; v1 bit-identical).

## E21. Near-miss benchmark: a sensitive metric, but the first set was contaminated
`tools/nearmiss_bench.py`: replay from fight start, original fight seed, vary search seed, paired with collecting player. Same player wins 0.335 of its own near-miss losses (coin flips); paired se ~0.009. First set contaminated (arms trained on it); `data/bench/nearmiss.json` rebuilt from r4u (1230 near-miss losses, 1500 close wins), held out from r4s-trained arms.

## E22. No policy target closes the gap between the network and its own search
`tools/policy_agree.py`, 349 significant states (5x256 reference): agreement live 5x32 0.903; r3 0.650; hard / hard+value / anchored / CMPO targets 0.645-0.662; 5x data 0.650. Not explained by E20 gaps. Policy-only training breaks the outcome head (Brier 0.19).

## E23. The policy-search gap on clear decisions is real and survives capacity
276 states significant under two independent references (agree 98.6%): live search 0.953; networks 0.674-0.714 (d128/d256, 35k/163k fights). Rollout swap on near-miss bench: sharper distilled play-out policies lose more close wins (+0.016..+0.028 +- 0.009). -> Live player is the search; stack must not depend on policy quality; effort goes to the judge.

## E24. Lower-variance value targets: TD(lambda) helps a little; first training change to move the player
From r3, 163k fights, `exit.py train --value-target td --lam L` vs realized ending. TD(0.9): only arm whose holdout loss on true endings beats init; best tail Brier. Near-miss bench (4 attempts, se ~0.006):

| arm | near-miss losses won | close wins lost | tail Brier |
|---|---|---|---|
| realized | +0.011 | +0.000 | 0.0343 |
| TD 0.8 | +0.023 | -0.014 | 0.0327 |
| TD 0.9 | +0.015 | -0.012 | 0.0319 |
| TD 0.95 | +0.014 | -0.012 | 0.0319 |

Adopted: `--value-target td --lam 0.8`.

## E25. Optimality bracket on near-miss fights
`tools/nearmiss_bench.py eval`, r4u held-out, paired with r3 5x32 (0.328 losses won / 0.247 wins lost): 5x256 honest +0.024 +- 0.014 / -0.039 +- 0.012; clairvoyant 5x32 +0.189 +- 0.015 / -0.119 +- 0.012. 48% of near-miss losses unwinnable even with perfect information. Avoidable share in [~0.02, ~0.19]; live solver near its judge's honest frontier.

## E26. Where avoidable near-miss errors are: spread over the setup turns, not the last turn
`tools/nearmiss_bench.py turns`, 300 near-miss losses, restart from true state:

| restart | live 5x32 | 5x256 honest | clairvoyant |
|---|---|---|---|
| last turn | 0.100 | +0.002 +- 0.012 | +0.070 +- 0.014 |
| 1 turn earlier | 0.122 | +0.008 +- 0.014 | +0.128 +- 0.020 |
| 2 turns earlier | 0.176 | +0.016 +- 0.019 | +0.176 +- 0.023 |

Last turn is already decided; avoidable errors are small misjudgements spread over setup turns -> sharper value judge, not deeper search.

## E27. Promotion: TD(0.8) + cover is the first player measurably better than h128
- **Near-miss bench (held-out, 4 attempts, paired with r3):**
  - cover: +0.009 L->W, +0.001 W->L;
  - TD(0.8): +0.016 / -0.009;
  - TD(0.8) + cover: +0.031 +- 0.007 / -0.009 +- 0.006.
- **Bench v2 play (paired with h128's labels; h128 replaying itself gives <= +-0.001):**

| player | eval | corpus | mix | tail | end HP |
|---|---|---|---|---|---|
| TD(0.8) | +0.008 | +0.005 | +0.005 | +0.012 | +0.4..+0.85 |
| TD(0.8) + cover | +0.005 | +0.004 | +0.003 | +0.024 +- 0.007 | +0.5..+1.1 |

- **Predictor Brier** (eval / corpus / mix / tail), TD(0.8) vs predictor_r2: 0.0206 / 0.0183 / 0.0228 / 0.0327 against 0.0193 / 0.0199 / 0.0249 / 0.0370. Tail bias -0.013 against -0.043.
- **Adopted:** `models/solver_td08.pt` as policy and predictor; the live `Engine` searches with `cover=True`. The harness tables keep top-5 for their time budgets.

## E28. S3 decile bias for solver_td08: pessimistic by ~0.03 in the contested band
- **Method:** fight-start P(win) (8 shuffles) on the 1770 bench v2 fights against h128's 8-attempt labels, in deciles of prediction.
- **Result:** bias +0.027 / +0.036 / +0.027 (se 0.019-0.025) in the deciles with predictions 0.27 / 0.54 / 0.81; |bias| <= 0.008 elsewhere. Max 0.036 > the 0.02 gate. The contested band together is about +0.03 at ~2.3 se.
- **Caveats:**
  - The labels are h128's play, weaker than the live player (E27). The calibration target should be the live player's own outcomes, so the bench needs relabelling.
  - The fix path: combat-loop rounds on signal pools (data concentrated in that band), or a post-hoc calibration layer fitted on held-out labelled play.

## E29. PPO sampler A/B: the p(1-p) signal sampler wins; d256 + obs v2 from scratch beats every earlier network
- **Method:** d256, obs v2, 3200 iterations from scratch, same generated sets (train seed 1, eval seed 22). Uniform arm on an L40S pod; signal arm locally (`--adaptive 25 --adaptive-mode signal --adaptive-decay 0.8`).
- **Eval curves (every 100 iterations):** signal ahead at 27 of 32 points, mean +0.011 (+0.012 from iteration 1600); final 0.714 vs 0.696.
- **Greedy eval, 12k episodes, same env seeds:** signal 0.7142, uniform 0.7094, solver_td08 0.6996, h128 0.6981. Act 3: 0.524 / 0.522 / 0.503 / 0.504.
- **Adopted:** the signal sampler for PPO. The signal checkpoint (`target/runs/v2_signal/ckpt.pt`) is the gen-2 base.
