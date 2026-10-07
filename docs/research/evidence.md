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
