# MCTS Skill Benchmark — Roadmap

Status: **roadmap** (new focus: improve MCTS algorithm quality). Companion to `docs/mcts-solver-design.md`.
Two stages: **(A) build a suite that benchmarks the skill of our MCTS + leaf heuristics**, then **(B/C) use it to
drive improvements**, accepting a change only when the suite says it's better.

## 0. What "skill" means here

Skill = **suboptimality of the search's output** vs closed-loop exact expectimax with infinite time. Two outputs,
benchmarked separately because they fail differently:

- **Decision quality** — which card to play / remove (what the advice ships). The headline.
- **Value accuracy** — the HP-loss/win number (feeds deck-strength; a biased value mis-ranks decisions).

The benchmark must be **policy-agnostic**: it scores the search's *output*, never assuming the leaf heuristic is
good. It must also **decompose** the gap into *leaf* (the rollout/Score policy in isolation), *tree* (search at a
budget), and *budget* (sample efficiency), so improvement effort is aimed correctly.

Ground truth comes from a spectrum, not one source:

```
 small fights            mid fights                 large / long fights
 ───────────────────────────────────────────────────────────────────────►
 EXACT closed-loop   determinized-hindsight bound   committed-policy true value
 (gold truth)        + admissible loss bound        + self-ladder (10× budget)
                     = optimality BRACKET            = relative skill
```

## 1. Metrics (the suite)

| # | Metric | Truth source | Measures |
|---|---|---|---|
| **M1** | **Decision regret vs exact** = `exactVal(best action) − exactVal(MCTS action)` in lex (Win, Loss), over sampled reachable decision states; report mean regret + % optimal | exact (small) | end-to-end decision skill |
| **M2** | **Value error/bias** = signed & abs (Δwin, Δloss) of MCTS vs exact | exact (small) | value accuracy (already gated) |
| **M3** | **Sample efficiency** = regret/value vs trial budget curve; "trials to first stable-correct decision"; robustness to seed & exploration constant | exact or self-limit | how well compute → correctness; flags *structural* bias (limit varies with seed/c) |
| **M4** | **Committed-policy true value** = MC replay of the recommended line → real HP loss/win; **self-ladder**: does 10× budget lower loss / flip the decision? | none (oracle-free) | absolute play quality on any fight; relative skill where no oracle |
| **M5** | **Optimality bracket** = `[hindsight-optimal (determinized exact), committed-policy true value]`; width = suboptimality bound; pin exactly where it meets the admissible loss bound | determinization + `LossCertificate` | absolute near-optimality certificate on large fights |
| **M6** | **Leaf-isolation skill** = pure greedy/candidate rollout true value vs exact (small) / bracket (large) | exact / bracket | attributes gap to leaf vs tree |

M1/M2 already largely exist (`Solver.BestAction`/`StateValue`/`GreedyTurnValue`, `MeasurePolicyRegret`,
`CalibrationHarness`). M3 is a thin profiler. M4 reuses `PolicyRollout`. **M5 (determinization-hindsight) is the
main new infra** and the key to the intractable regime. M6 reuses `GreedyHeuristicPolicy`.

### On M1 across the trajectory (not just openings)
Today's regret is opening-state only. Skill must be sampled along **reachable mid-fight states** — generate them by
walking the exact-optimal line *and* the MCTS line a few plies, then measure root decision regret at each. Catches
"plays the first card right, the third wrong."

### On M5 (the heavy artillery)
The exact solver blows up almost entirely on **draw chance nodes**. Fix the RNG to a seed ⇒ a *deterministic*
planning instance, far smaller, often exactly solvable even for big decks. Average the per-seed optimal over seeds:
that hindsight-optimal value **provably bounds the true closed-loop optimum from the optimistic side**
(information-relaxation duality). Bracketed against M4 it yields an *absolute* skill bound where closed-loop exact
is impossible — the closest tractable thing to "exact with infinite time."

## 2. Fixtures

Reuse the existing spectrum, extended to sample decisions (not just whole-fight value):
`CalibrationFixtures.All` (6 archetypes), `EliteSweep`, `RandomDecks` (anti-overfit), `Bridge` ladder (8→30 cards,
the exact→intractable straddle), `PerCharacter`, `PerCharacterLong`. Add a small **decision-fixture** set: specific
(state, candidate-actions) probes where the right call is known or bracket-pinned — including the motivating
removal decisions (does keeping Bodyguard/Unleash lower true loss?). Hold out random decks as the overfit guard.

## 3. Phase A — build the benchmark

- **A1 Decision-regret harness (M1).** Sample reachable states along exact + MCTS lines; compute MCTS root-decision
  regret vs exact. Aggregate mean regret / % optimal per fixture. Gate on the exact-tractable set.
- **A2 Convergence profiler (M3).** Budget ladder (e.g. 2k·4^k) × seeds × exploration constants; emit value &
  top-decision vs budget, trials-to-ε, and a seed/c-invariance check (structural-bias detector).
- **A3 Committed-policy value + self-ladder (M4).** Extract the MCTS recommended line; MC-replay it; compare across
  budgets. Oracle-free; runs on `PerCharacterLong` and the real removal evals.
- **A4 Determinization-hindsight bound (M5).** Per-seed determinized exact solver (no draw chance nodes); average;
  bracket with A3 and the admissible loss bound. The one genuinely new solver path.
- **A5 Leaf-isolation (M6).** Candidate leaf policies' true value vs exact/bracket (reuses `--osty-bench` shape).
- **A6 Report + gates.** A `sts2solve --skill` report over the suite; freeze current numbers as regression gates so
  later changes can't silently lower skill.

## 4. Phase B — diagnose

Run the suite once to produce a **skill profile**: per-fixture decision regret + value bias, convergence rate,
leaf-vs-tree attribution (M6 vs M1), and the list of decisions that **flip with budget** (candidate real errors,
e.g. the Bodyguard/Unleash removal — does its ranking move as budget → limit?). Output: a ranked list of skill
deficits to attack, each with its dominant cause (leaf / tree / budget).

## 5. Phase C — improve (every change A/B'd on the suite)

Candidate levers, chosen per Phase-B attribution:

- **Leaf** — better rollout policy (intent/mechanic-aware where M6 shows the leaf loses; λ-mixing; a cheap 1-ply
  lookahead leaf); a better static leaf-value estimator; or the scaffolded **learned value function**
  (`VfTrainer` / `--train-vf`) as the leaf.
- **Tree** — exploration / PUCT / action-widening schedule tuned against the *whole suite* (not one fixture);
  chance-node handling (DPW params, or determinized / information-set MCTS); transpositions; hybrid exact-at-leaf
  for small subtrees; smarter budget allocation.
- **Priors** — a better action-widening prior (the term that miscalibrated in the Osty experiment): decouple the
  *prior* ranking from the *rollout* seed so a better-playing leaf can't poison move ordering.

Acceptance rule: a change ships only if it **reduces M1 decision regret and/or improves M3 convergence and/or
tightens M5 brackets, with no regression** on the exact-anchored gates (M1/M2) or the seed-stability floor.

## 6. Definition of done (per stage)

- **A:** `sts2solve --skill` produces M1–M6 over the suite; exact-anchored skill numbers frozen as CI gates; the
  large-fight bracket (M5) is computable on `PerCharacterLong`.
- **B:** a written skill profile with ranked, cause-attributed deficits.
- **C:** at least the top deficit closed with a measured, gated improvement — or a documented finding that it's
  budget-bound (needs trials, not a smarter heuristic).

## 7. Reuse map (don't rebuild)

`Solver` (`Solve`/`BestAction`/`StateValue`/`GreedyTurnValue`/`OpeningStates`) · `CalibrationHarness`
(`RunExact*`/`RunMcts`/`RunMctsSeeds`/`MeasurePolicyRegret`) · `MctsSolver` (`GreedyBestPlay`/`GreedyPlayTurn`/
`GreedyTurnPlan`) · `PolicyRollout` + `GreedyHeuristicPolicy` (M4/M6) · `LossCertificate` + `HorizonBound` (M5
lower bound) · `CalibrationFixtures.*` (fixtures) · `VfTrainer` / `TrainingFixtures` (Phase-C leaf option). The new
builds are the **trajectory-state sampler (A1)**, the **convergence profiler (A2)**, and the
**determinization-hindsight solver (A4/M5)**.
