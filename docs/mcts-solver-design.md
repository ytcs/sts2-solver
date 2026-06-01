# Sampling/MCTS Solver Mode — Design (from SOTA literature review)

_Created 2026-05-30. Basis: a deep, adversarially-verified literature review (25 claims, all confirmed
3‑0) plus a scan of Slay the Spire / CCG solver prior art. Citations at the bottom._

## Problem recap

Plan a single Slay the Spire 2 combat optimally. It is a **single-agent MDP with a perfect known
model** (our engine), not a game vs. an adversary and not RL from an unknown environment:

- Player actions are deterministic (play a card at a target; end turn).
- Stochasticity is **factored** into two chance-node kinds:
  - **enemy-move roll** — small, known, cheaply enumerable;
  - **card draw** — multivariate hypergeometric over the deck multiset; exact distribution is
    enumerable but combinatorially large, and worsens as enemies inject status cards mid-combat.
- Objective is **strict lexicographic**: maximize P(win) first, then minimize E[HP loss].
- States form a **DAG** with many transpositions (already memoized on a 128-bit structural hash).
- We already have an **exact lexicographic expectimax** that is optimal for small fights (~500k
  states) but blows up on large ones (e.g. PhrogParasite: two phases, 4 summons, deck grows with
  Infection). That stays as the ground-truth oracle.

## Verdict: the right paradigm

**Closed-loop chance-node MCTS with explicit known-probability backups** — specifically the
**THTS family (Keller & Helmert 2013)**, building **UCT\* = DP-UCT + limited trial length**.

Why not the alternatives:

- **Determinization / PIMC / ISMCTS is the wrong frame here.** Its pathologies (strategy fusion,
  non-locality) come from *hidden-information asymmetry*. We have no hidden information beyond draw
  order, which is just chance with a fully known distribution. So we expand chance explicitly and
  back up by true probabilities rather than fixing the deck and pretending it's deterministic.
- **Naive sparse sampling (Kearns–Mansour–Ng)** gives state-count-independent near-optimal actions
  but costs `O((kC)^H)`, exponential in horizon — provably unavoidable without structure. It is the
  *justification* for sampling, not a usable algorithm for our deep fights. We need the structure
  (known probs, transpositions, abstraction) that THTS/abstraction add.
- **Vanilla UCT** wastes the model: Monte-Carlo visit-count averaging ignores the known transition
  probabilities and degenerates on large-support chance nodes.

Domain prior art agrees by negative example: the only hobby StS solver targeting *our* objective
(orzAtalod's "Loss-Calculator") uses **Monte Carlo + A\*** (sampling + heuristic search — i.e. the
THTS mix); a brute-force expectimax solver (cvennevik) explicitly notes exhaustive search is
infeasible (matching our experience); and the strongest playing bot (bottled_ai) is greedy
one-turn-lookahead heuristics — good play, not optimal planning.

## Recommended architecture

A DP-UCT / UCT\* spine specialized to our factored, known-model, lexicographic problem.

### 1. Backups — Partial Bellman, not Monte-Carlo averaging (THE key model-exploiting move)
At a chance node, weight each **explicated** outcome by its **true probability**, normalized by the
explicated mass `P^k`:

```
Q^k(chance) = R + ( Σ_{explicated d} P(d) · V^k(d) ) / P^k     where P^k = Σ_{explicated d} P(d)
```

As outcomes are explicated, `P^k → 1` and `Q^k → Q*` (the Bellman optimum). Crucially, a Partial
Bellman backup is a **recomputation of the Bellman equation from children**, so it composes
correctly over a DAG/transposition table — no double-counting, unlike visit-count accumulation.
Decision nodes back up by the **lexicographic max** (our existing `Value.BetterThan`).

### 2. Chance nodes — exact where cheap, Double Progressive Widening where expensive
- **Enemy-move nodes:** enumerate **exactly** (small support). `P^k = 1` from creation. This is the
  strongest possible variance reduction and makes explicit Monte-Carlo variance-reduction tricks
  (antithetics, CRN) moot for these nodes.
- **Draw nodes:** the deck multiset means many orderings collapse to the same hand (and then, via the
  structural hash, to the same state). Strategy: **enumerate exactly when the count of *distinct*
  resulting states ≤ K; otherwise Double Progressive Widening** (Couetoux et al. 2011): add a new
  sampled outcome child only when `⌈C·n^β⌉` exceeds the current child count, else reselect an existing
  child. The transposition table makes DPW reselection natural (re-sampled identical hands merge).
  Tune `β` upward the larger the draw's impact (literature: best β rose ~0.2→0.6 as event impact
  grew); start β≈0.3, K≈a few hundred distinct outcomes.

### 3. Lexicographic value & selection — do NOT scalarize
Scalarization provably need not yield the lexicographic optimum, and no fixed weight reliably
dominates the second objective. Keep the value a pair `(w, h)` = `(P(win), E[HP loss])` and:

- **Backup** component-wise (both are expectations ⇒ linear under the probability-weighting above);
  decision nodes take the lexicographic max.
- **Selection — Lexicographic UCB.** Exploit a domain fact the general theory lacks: **HP loss is
  bounded by current HP ≤ maxHP**, so normalize `h̃ = h / maxHP ∈ [0,1]`. Then both components are
  in `[0,1]` and UCB is well-scaled. Select by lexicographic comparison of upper-confidence values:
  primary `w_a + c·√(ln N / n_a)`; when two actions' primary intervals overlap (statistically tied on
  win prob), break by the secondary `(1 − h̃_a) + c·√(ln N / n_a)`. This keeps HP-loss exploration
  from starving win-prob optimization — the lexicographic-bandit / Threshold-UCT principle.
- The known model **neutralizes the Lex-MAB "identifiability problem"** (you can't certify
  lexicographic-optimal arms under ties when learning): we *compute* exact enemy-move probabilities
  and exact ties rather than estimate them.

### 4. DAG-aware (Monte Carlo Graph Search)
Key nodes by the 128-bit structural hash in a transposition table; transposing children share value
estimates. Partial Bellman recomputation (§1) is what makes this sound.

### 5. Trial length & leaf evaluation (UCT\*)
- **Phase 1 (baseline):** DP-UCT with **rollouts** to terminal under a fast greedy policy for leaf
  value — a working, model-faithful estimator of `(win?, hploss)`.
- **Phase 2 (UCT\*):** bound trial length to end at a freshly expanded tip node and evaluate it with a
  **heuristic** instead of a full rollout (focuses search near the root; UCT\* beat plain UCT and the
  IPPC-2011 winner on 7/8 domains in 1/16 the time). Heuristic seed: survive-the-telegraphed-attack +
  (enemy HP remaining vs. our per-turn damage potential). Non-admissible heuristics are allowed.

### 6. Hybrid with the exact solver
Below a subtree-size/turn threshold, call the **existing memoized exact expectimax** (optimal, fast)
and cache it — gives exact endgames and a clean anytime story: exact where feasible, UCT\* above.

## Validation plan (project philosophy: validate, don't trust)
The exact expectimax is the oracle. On every fight small enough to solve exactly (Cultists, Byrdonis,
BygoneEffigy), run the sampler and **assert it converges to the exact `(P_win, E[HP loss])` within
tolerance**. This is the sampler's analogue of trace-validation. Only then trust it on PhrogParasite.

## Build order
1. **✅ DONE.** DP-UCT core: exact enemy-move chance + exact/DPW draw chance, Partial Bellman backups,
   transposition table, lexicographic value + Lexicographic-UCB (HP normalized by maxHP), greedy
   rollouts. Convergence test vs. exact oracle on small fights.
2. **✅ DONE.** UCT\* limited trial length: a trial expands one tip and returns its greedy-rollout leaf
   value (it does not recurse to terminal), focusing search near the root — the UCT\* trial-length
   bound. (Static non-rollout heuristic leaf eval is a drop-in via the same seam if rollouts prove too
   slow; rollouts are kept as the default because they are engine-faithful.)
3. **✅ DONE.** Hybrid exact-below-threshold (`MctsOptions.HybridExactBelow`): a decision node whose
   remaining state space is provably small (enemy HP × turns-left ≤ threshold) is resolved by the
   memoized exact expectimax and cached as a solved leaf.
4. **✅ DONE.** Runs on PhrogParasite; reports (P_win, E[HP loss]) with an anytime curve (`--anytime`).

### Implementation (this session)
- `solver/Sts2Solver.Search/MctsSolver.cs` — the sampler (decision/chance nodes, Partial Bellman
  backups, exact + DPW chance, Lex-UCB, greedy rollouts, hybrid oracle).
- `solver/Sts2Solver.Search/DrawEnumerator.cs` — added `DistinctDrawCount` (cheap exact-vs-DPW test)
  and `SampleDraw` (samples a hand with its exact hypergeometric probability for DPW backups).
- `solver/Sts2Solver.Cli/Program.cs` — `--mcts [--trials N] [--anytime] [--hybrid N]`.
- `solver/Sts2Solver.Tests/MctsTests.cs` — convergence-to-oracle tests (Cultist×2, Byrdonis, hybrid).

### Empirical convergence (sampler vs. exact oracle, the validation gate)
| Fight | exact (win, E[HP loss]) | MCTS (win, E[HP loss]) | trials |
|---|---|---|---|
| Ironclad starter vs CalcifiedCultist(41) | 100%, 0.86 | 100%, 0.94 | 60k |
| …vs CalcifiedCultist(30) | 100%, 0.18 | 100%, 0.18 | 60k |
| …vs Byrdonis(84) elite | 100%, 56.38 | 99.99%, 56.21 | 120k |

PhrogParasite (no exact oracle exists — the point of the sampler): a bare Ironclad starter is a hard
**0% win** (the two-phase Wriggler burst + escalating Infection self-damage overwhelm it); strengthening
the deck/energy/HP produces a sensible monotone gradient (e.g. 200 HP / 3 energy / 10 Strikes → ~32%
win; 200 HP / 6 energy → ~93%), confirming the sampler finds wins where they exist rather than being
stuck at a fixed point.

## Known risks / open questions (from the review's own caveats)
- **No cited end-to-end guarantee** for a *known-model lexicographic DP-UCT* exactly; it's a sound
  engineering composition (Partial Bellman convergence + lexicographic max + bounded-second-objective
  UCB), and the exact-oracle convergence test is our empirical guard.
- **DPW's empirics are on continuous support;** our draws are discrete-but-large, so the
  exact-vs-DPW crossover `K` and `β` need tuning. The structural-hash dedup may let us enumerate
  exactly more often than expected.
- **FSSS and MaxUCT** were not separately evaluated; UCT\* is the best-supported pick, but FSSS is a
  plausible alternative if UCT\* underperforms on deep fights.

## Citations
- Keller & Helmert, *Trial-based Heuristic Tree Search for Finite Horizon MDPs*, ICAPS 2013 (THTS,
  DP-UCT, UCT\*). https://gki.informatik.uni-freiburg.de/papers/keller-helmert-icaps2013.pdf
- Kearns, Mansour & Ng, *A Sparse Sampling Algorithm for Near-Optimal Planning in Large MDPs*, 2002.
  https://www.cis.upenn.edu/~mkearns/papers/sparsesampling-journal.pdf
- Couetoux et al., *Continuous Upper Confidence Trees* (Double Progressive Widening), 2011.
  https://ewrl.wordpress.com/wp-content/uploads/2011/08/ewrl2011_submission_29.pdf
- Hostetler, Fern & Dietterich, *Progressive Abstraction Refinement for Sparse Sampling*, UAI 2015.
- Whitehouse, *Monte Carlo Tree Search for Games with Hidden Information and Uncertainty* (ISMCTS;
  PIMC pathologies), PhD thesis. https://etheses.whiterose.ac.uk/id/eprint/8117/
- Hüyük & Tekin, *Analysis of Thompson Sampling for Combinatorial Multi-armed Bandit / Lexicographic
  bandits*, 2019. https://arxiv.org/pdf/1907.11605
- *Threshold-UCT* (constrained-MDP / Pareto-curve MCTS), AAAI 2025. https://arxiv.org/abs/2412.13962
- Chen & Liu, *Pareto Monte Carlo Tree Search* (Pareto-UCB), RSS 2019.
  https://www.roboticsproceedings.org/rss15/p72.pdf
- Domain prior art: orzAtalod/Slay-the-Spire-Loss-Calculator (Monte Carlo + A\*),
  cvennevik/slay-the-spire-solver (brute-force expectimax, infeasible), xaved88/bottled_ai
  (greedy heuristic).
