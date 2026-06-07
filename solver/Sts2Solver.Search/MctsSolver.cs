using Sts2Solver.Engine;

namespace Sts2Solver.Search;

/// <summary>Tunables for the sampling solver. Defaults target small-fight convergence-to-oracle.</summary>
public sealed class MctsOptions
{
    /// <summary>Number of THTS trials (root-to-tip descents) to run.</summary>
    public int Trials = 200_000;

    /// <summary>Search horizon in player turns (mirrors the exact solver's MaxTurns: beyond it = a loss).</summary>
    public int MaxTurns = 30;

    /// <summary>UCB exploration constant c. Both value components live in [0,1] so a single c is well-scaled.</summary>
    public double Exploration = 1.0;

    /// <summary>A chance node enumerates its outcomes exactly when their (distinct) count ≤ this; else DPW.</summary>
    public long ExactChanceThreshold = 4096;

    /// <summary>Double Progressive Widening: allowed children = ⌈C·N^β⌉.</summary>
    public double DpwC = 4.0;
    public double DpwBeta = 0.3;

    /// <summary>RNG seed for rollouts + DPW sampling (search reproducibility).</summary>
    public int Seed = 1;

    /// <summary>Hybrid: defer a decision node to the exact expectimax once its state space is provably
    /// small (subtree ≈ enemy HP × turns-left below this). 0 disables the hybrid.</summary>
    public int HybridExactBelow = 0;

    /// <summary>Each rollout samples its aggression λ uniformly from [Lo, Hi] (0 = all-block, 1 = all-damage),
    /// so a leaf seed can average over the block↔race spectrum. Lo==Hi gives a deterministic policy at that λ;
    /// the default 0.5/0.5 is a deterministic balanced rollout (the calibrated choice). Knobs for experiments.</summary>
    public double RolloutLambdaLo = EnvD("STS2_LAMBDA_LO", 0.5);
    public double RolloutLambdaHi = EnvD("STS2_LAMBDA_HI", 0.5);

    /// <summary>Number of playouts averaged into each new leaf's seed value (each samples its own λ from
    /// [Lo,Hi]). 1 = a single-rollout seed (the calibrated default).</summary>
    public int RolloutSamples = (int)EnvD("STS2_ROLLOUT_SAMPLES", 1);

    /// <summary>Action progressive widening + PUCT (the production default; set STS2_APW=0 for classic UCT*).
    /// Ranks the legal plays by the heuristic policy prior and opens only ⌈<see cref="ApwC"/>·N^<see cref="ApwBeta"/>⌉
    /// of them, best-first — bounding the per-decision branching that grows with card variety. EndTurn (the
    /// oracle's safe baseline) is always opened, and every candidate eventually opens as N→∞, so it stays
    /// asymptotically consistent. Selection switches from UCB to PUCT (prior·c·√N/(1+visits)); the just-opened
    /// child's rollout seed is its first-play value, so no separate FPU term. Classic UCT* (OFF) instead opens
    /// EVERY distinct legal play and force-visits each.</summary>
    public bool ActionWidening = EnvB("STS2_APW", true);

    /// <summary>Action-widening schedule: opened plays = ⌈ApwC·N^ApwBeta⌉ (clamped to [1, #plays]). β∈(0,1).</summary>
    public double ApwC = EnvD("STS2_APW_C", 2.0);
    public double ApwBeta = EnvD("STS2_APW_BETA", 0.5);

    /// <summary>PUCT exploration constant (used only when <see cref="ActionWidening"/> is on). Larger ⇒ trust
    /// the measured child values less and the policy prior / exploration more before committing.</summary>
    public double PuctC = EnvD("STS2_PUCT_C", 1.5);

    private static double EnvD(string k, double dflt) =>
        double.TryParse(Environment.GetEnvironmentVariable(k), out var v) ? v : dflt;
    private static bool EnvB(string k, bool dflt)
    {
        var s = Environment.GetEnvironmentVariable(k);
        return s == null ? dflt : (s == "1" || s.Equals("true", StringComparison.OrdinalIgnoreCase));
    }
}

/// <summary>
/// Sampling/MCTS solver: closed-loop chance-node MCTS in the THTS family (UCT* = DP-UCT + limited trial
/// length), specialised to our factored, known-model, lexicographic MDP. See docs/mcts-solver-design.md.
///
/// - Backups are <b>Partial Bellman</b>: each explicated chance outcome is weighted by its <i>true</i>
///   probability, normalised by the explicated mass P^k (→1 as outcomes are explicated). This is a
///   recomputation of the Bellman equation from children, so it composes correctly over the
///   transposition DAG (no visit-count double counting).
/// - Enemy-move chance is enumerated exactly (small support, P^k=1); large card-draw chance uses
///   exact enumeration below a threshold, else Double Progressive Widening with exact sampled-outcome
///   probabilities.
/// - Value is the lexicographic pair (P(win), E[HP loss]); selection is Lexicographic-UCB with HP loss
///   normalised by maxHP into [0,1]. No scalarization.
/// The exact <see cref="Solver"/> remains the ground-truth oracle this is validated against.
/// </summary>
public sealed class MctsSolver
{
    private readonly MctsOptions _opt;
    private readonly Rng _rng;
    private readonly Dictionary<(ulong, ulong), DecisionNode> _table = new();
    private readonly Solver _exact;   // for the hybrid + (optionally) external comparison

    /// <summary>Optional admissible early-loss certificate (see <see cref="LossCertificate"/>). When set,
    /// decision nodes it proves lost become terminals with their exact value <c>(0, CurrentHp)</c> instead of
    /// a noisy rollout — faster and sharper, and it never prunes a winnable node (so survival is unaffected).</summary>
    public LossCertificate? LossProof { get; set; }

    public int NodesCreated { get; private set; }
    public int TrialsRun { get; private set; }

    /// <summary>True once the search has seen at least one winning line (a won rollout or a victory terminal).
    /// A win was reachable ⇒ true survival is strictly positive — so a caller can floor a backed-up 0.0%
    /// (which only means "no win accrued enough probability mass to register") to a small nonzero value,
    /// rather than report a misleading hard 0% that would make a player wrongly skip a winnable fight.</summary>
    public bool ObservedWin { get; private set; }
    private void Note(Value v) { if (v.Win > 0) ObservedWin = true; }

    public MctsSolver(MctsOptions? options = null)
    {
        _opt = options ?? new MctsOptions();
        _rng = new Rng(_opt.Seed);
        _exact = new Solver { MaxTurns = _opt.MaxTurns };
    }

    // ---------- Top level ----------

    /// <summary>Run the sampler from a combat setup state and return the estimated value at the root.</summary>
    public Value Solve(CombatState setup)
    {
        var root = BuildOpeningChance(setup);
        for (int i = 0; i < _opt.Trials; i++)
        {
            VisitChance(root);
            TrialsRun++;
        }
        return root.V;
    }

    /// <summary>Run the sampler, reporting the root value every <paramref name="every"/> trials (anytime curve).</summary>
    public IEnumerable<(int trials, Value value)> SolveAnytime(CombatState setup, int every)
    {
        var root = BuildOpeningChance(setup);
        for (int i = 1; i <= _opt.Trials; i++)
        {
            VisitChance(root);
            TrialsRun++;
            if (i % every == 0) yield return (i, root.V);
        }
        if (_opt.Trials % every != 0) yield return (_opt.Trials, root.V);
    }

    // ---------- Decision nodes ----------

    private DecisionNode GetOrCreateDecision(CombatState state)
    {
        var key = state.HashKey();
        if (_table.TryGetValue(key, out var existing)) return existing;

        var d = new DecisionNode { State = state, Key = key };
        NodesCreated++;
        if (state.AllMonstersDead) { d.Terminal = true; d.V = new Value(1, 0); }
        else if (state.PlayerDead) { d.Terminal = true; d.V = new Value(0, 0); }
        else if (state.TurnNumber > _opt.MaxTurns) { d.Terminal = true; d.V = new Value(0, 0); }
        else if (LossProof != null && LossProof.IsProvablyLost(state))
        {
            d.Terminal = true; d.V = new Value(0, state.Player.CurrentHp);   // exact value of a doomed subtree
        }
        else if (_opt.HybridExactBelow > 0 && IsSmall(state))
        {
            d.Terminal = true;                       // treat as a solved leaf (exact oracle)
            d.V = _exact.SolvePlayerTurn(state.Clone());
            d.ExactSolved = true;
        }
        else d.V = SeedValue(state, needAdvance: false, initialRoll: false);   // averaged λ-rollout leaf seed

        Note(d.V);   // record victory terminals / exact-solved wins for the ObservedWin floor
        _table[key] = d;
        return d;
    }

    private bool IsSmall(CombatState s)
    {
        // Cheap upper bound on remaining work: enemy HP to chew through × turns left × hand width.
        int enemyHp = s.Monsters.Where(m => m.IsAlive).Sum(m => m.CurrentHp + m.Block);
        int turnsLeft = Math.Max(1, _opt.MaxTurns - s.TurnNumber + 1);
        return (long)enemyHp * turnsLeft <= _opt.HybridExactBelow;
    }

    private void Expand(DecisionNode d)
    {
        d.Expanded = true;
        d.Edges = new List<Edge>();
        if (d.State.PendingDiscard > 0) { ExpandDiscard(d); return; }   // post-draw discard-of-choice (a MAX)
        if (d.State.PlayerTurnEndForced)   // VoidForm ended the turn on play: the only action is to end it
        {
            d.Edges.Add(new Edge { Action = PlayerAction.EndTurn, IsEndTurn = true, Chance = BuildEndTurnChance(d.State), Prior = 1.0 });
            d.Candidates = new List<PlayerAction>();          // empty (non-null) ⇒ the re-widen guard skips this node
            d.CandidatePriors = System.Array.Empty<double>();
            d.Opened = 0;
            return;
        }
        if (_opt.ActionWidening) { ExpandWidening(d); return; }

        foreach (var action in LegalPlays(d.State))
        {
            var (dec, draw) = PlayTarget(ApplyPlay(d.State, action));
            d.Edges.Add(new Edge { Action = action, PlayChild = dec, DrawChild = draw });
        }
        d.Edges.Add(new Edge { Action = PlayerAction.EndTurn, IsEndTurn = true, Chance = BuildEndTurnChance(d.State) });
    }

    /// <summary>Post-draw discard-of-choice expand: the player MAXes over which distinct hand card to drop (one
    /// per step until <see cref="CombatState.PendingDiscard"/> is exhausted), with no EndTurn / no plays until
    /// the discard is resolved. Few options, so open them all with a uniform prior (so PUCT explores them).</summary>
    private void ExpandDiscard(DecisionNode d)
    {
        var hand = d.State.Player.Hand;
        if (hand.Count == 0)   // nothing to discard: clear the obligation, run any continuation, continue
        {
            var cleared = d.State.Clone();
            cleared.PendingDiscard = 0;
            CombatManager.ApplyPostDiscard(cleared);   // HiddenDaggers adds its Shivs here (no-op for Acrobatics/Prepared)
            d.Edges.Add(new Edge { Action = new PlayerAction("Discard done", null, -1), PlayChild = GetOrCreateDecision(cleared), Prior = 1.0 });
            return;
        }
        var seen = new HashSet<string>();
        foreach (var card in hand)
        {
            var key = card.StateKey();
            if (!seen.Add(key)) continue;   // symmetric discards collapse
            var c = d.State.Clone();
            Cmd.DiscardFromHand(c, c.Player.Hand.First(h => h.StateKey() == key));   // may trigger a Sly auto-play
            c.PendingDiscard--;
            if (c.PendingDiscard == 0) CombatManager.ApplyPostDiscard(c);   // last discard resolved → run the continuation
            var (dec, draw) = PlayTarget(c);   // route through a DrawNode if a Sly Reflex's auto-play deferred a draw
            d.Edges.Add(new Edge { Action = new PlayerAction($"Discard {key}", null, -1), PlayChild = dec, DrawChild = draw });
        }
        double uniform = 1.0 / d.Edges.Count;
        foreach (var e in d.Edges) e.Prior = uniform;
    }

    /// <summary>Action-widening expand: always open EndTurn (the safe baseline), then rank the card plays by a
    /// cheap heuristic policy prior (lower resulting <see cref="CombatHeuristic.Score"/> = better play) and open
    /// only the best-first prefix sized by the widening schedule. Ranking costs one Score per distinct play
    /// (clone-only, no rollout / no node); the expensive per-child rollout seed is paid only for OPENED plays,
    /// which is what bounds the variety blow-up.</summary>
    private void ExpandWidening(DecisionNode d)
    {
        var plays = LegalPlays(d.State).ToList();
        int n = plays.Count;

        // Pseudo-score of ending the turn now = the do-nothing position score (Score already prices in the
        // telegraphed incoming hit), so EndTurn ranks on the same scale as the plays.
        double baseScore = CombatHeuristic.Score(d.State);
        var scores = new double[n];
        for (int i = 0; i < n; i++) scores[i] = CombatHeuristic.Score(ApplyPlay(d.State, plays[i]));

        double mn = baseScore, mx = baseScore;
        for (int i = 0; i < n; i++) { if (scores[i] < mn) mn = scores[i]; if (scores[i] > mx) mx = scores[i]; }
        double tau = mx - mn > 1e-9 ? (mx - mn) / 2.0 : 1.0;          // scale-free softmax temperature
        double W(double sc) => Math.Exp(-(sc - mn) / tau);            // unnormalised prior (lower score ⇒ larger)

        double norm = W(baseScore);
        for (int i = 0; i < n; i++) norm += W(scores[i]);

        d.Edges.Add(new Edge { Action = PlayerAction.EndTurn, IsEndTurn = true,
            Chance = BuildEndTurnChance(d.State), Prior = W(baseScore) / norm });

        var order = Enumerable.Range(0, n).OrderBy(i => scores[i]).ToArray();   // best (lowest score) first
        d.Candidates = order.Select(i => plays[i]).ToList();
        d.CandidatePriors = order.Select(i => W(scores[i]) / norm).ToArray();
        d.Opened = 0;
        if (n > 0) WidenTo(d, TargetOpen(d));   // nothing to widen when EndTurn is the only action
    }

    /// <summary>How many card plays should be open at this node now: ⌈ApwC·N^ApwBeta⌉, clamped to [1, #plays]
    /// (0 when there are no plays — only EndTurn).</summary>
    private int TargetOpen(DecisionNode d)
    {
        int count = d.Candidates!.Count;
        if (count == 0) return 0;
        int target = (int)Math.Ceiling(_opt.ApwC * Math.Pow(Math.Max(1, d.N), _opt.ApwBeta));
        return Math.Clamp(target, 1, count);
    }

    /// <summary>Open card-play edges (best-first) until <paramref name="target"/> are open. Each newly opened
    /// edge materialises its child (and the child's rollout seed) once.</summary>
    private void WidenTo(DecisionNode d, int target)
    {
        while (d.Opened < target && d.Opened < d.Candidates!.Count)
        {
            var action = d.Candidates[d.Opened];
            var (dec, draw) = PlayTarget(ApplyPlay(d.State, action));
            d.Edges.Add(new Edge { Action = action, PlayChild = dec, DrawChild = draw, Prior = d.CandidatePriors![d.Opened] });
            d.Opened++;
        }
    }

    private Value VisitDecision(DecisionNode d)
    {
        d.N++;
        if (d.Terminal) return d.V;
        if (!d.Expanded) { Expand(d); return d.V; }   // tip: expand + return leaf seed (trial ends)

        var e = SelectEdge(d);
        e.Visits++;
        if (e.IsEndTurn) VisitChance(e.Chance!);
        else if (e.DrawChild != null) VisitDraw(e.DrawChild);
        else VisitDecision(e.PlayChild!);

        // Partial Bellman / Bellman recomputation from children: lexicographic max over edges.
        Value best = d.Edges[0].Q;
        for (int i = 1; i < d.Edges.Count; i++)
            if (d.Edges[i].Q.BetterThan(best)) best = d.Edges[i].Q;
        d.V = best;
        return d.V;
    }

    /// <summary>Lexicographic-UCB edge selection (HP loss normalised by maxHP), or lexicographic-PUCT with
    /// action widening when <see cref="MctsOptions.ActionWidening"/> is on.</summary>
    private Edge SelectEdge(DecisionNode d)
    {
        if (_opt.ActionWidening)
        {
            if (d.Candidates is { Count: > 0 }) WidenTo(d, TargetOpen(d));   // open more plays as the node matures
            return SelectPuct(d);                                           // (Candidates is null for discard nodes)
        }

        double maxHp = Math.Max(1, d.State.Player.MaxHp);
        double logN = Math.Log(Math.Max(2, d.N));

        Edge best = null!;
        double bestPrim = double.NegativeInfinity, bestSec = double.NegativeInfinity;
        foreach (var e in d.Edges)
        {
            double prim, sec;
            if (e.Visits == 0) { prim = double.PositiveInfinity; sec = double.PositiveInfinity; }
            else
            {
                double bonus = _opt.Exploration * Math.Sqrt(logN / e.Visits);
                prim = e.Q.Win + bonus;                       // maximise win prob
                sec = (1.0 - e.Q.Loss / maxHp) + bonus;       // then minimise HP loss
            }
            if (best == null || prim > bestPrim + Eps ||
                (Math.Abs(prim - bestPrim) <= Eps && sec > bestSec))
            {
                best = e; bestPrim = prim; bestSec = sec;
            }
        }
        return best;
    }

    /// <summary>Lexicographic PUCT over the OPENED edges: each edge already carries a real value (its child's
    /// rollout seed, or the chance node's seed for EndTurn), so the just-opened child's seed is its first-play
    /// value — no separate FPU term. The exploration term is the standard PUCT prior bonus prior·c·√N/(1+visits),
    /// added to both lexicographic components so the comparison structure matches the UCB path.</summary>
    private Edge SelectPuct(DecisionNode d)
    {
        double maxHp = Math.Max(1, d.State.Player.MaxHp);
        double sqrtN = Math.Sqrt(Math.Max(1, d.N));

        Edge best = null!;
        double bestPrim = double.NegativeInfinity, bestSec = double.NegativeInfinity;
        foreach (var e in d.Edges)
        {
            double u = _opt.PuctC * e.Prior * sqrtN / (1 + e.Visits);
            double prim = e.Q.Win + u;                       // maximise win prob
            double sec = (1.0 - e.Q.Loss / maxHp) + u;       // then minimise HP loss
            if (best == null || prim > bestPrim + Eps ||
                (Math.Abs(prim - bestPrim) <= Eps && sec > bestSec))
            {
                best = e; bestPrim = prim; bestSec = sec;
            }
        }
        return best;
    }

    // ---------- Chance nodes ----------

    private ChanceNode BuildOpeningChance(CombatState setup)
    {
        var ch = new ChanceNode { AfterEnemy = setup.Clone(), EnemyLoss = 0, Initial = true };
        InitChance(ch);
        return ch;
    }

    private ChanceNode BuildEndTurnChance(CombatState decisionState)
    {
        int baseLoss = decisionState.PlayerHpLost;
        var afterEnemy = decisionState.Clone();
        CombatManager.EndPlayerTurn(afterEnemy);
        CombatManager.RunEnemyTurn(afterEnemy);   // deterministic: telegraphed moves
        int enemyLoss = afterEnemy.PlayerHpLost - baseLoss;

        var ch = new ChanceNode { AfterEnemy = afterEnemy, EnemyLoss = enemyLoss, Initial = false };
        if (afterEnemy.PlayerDead) { ch.Terminal = true; ch.V = new Value(0, enemyLoss); return ch; }
        if (afterEnemy.AllMonstersDead) { ch.Terminal = true; ch.V = new Value(1, enemyLoss); ObservedWin = true; return ch; }
        InitChance(ch);
        return ch;
    }

    /// <summary>Decide exact-vs-DPW, precompute the exact outcome queue, and seed the node value.</summary>
    private void InitChance(ChanceNode ch)
    {
        var combos = EnumerateMoveRolls(ch.AfterEnemy, ch.Initial);
        long drawDistinct = DrawEnumerator.DistinctDrawCount(ch.AfterEnemy, Player.CardsDrawnPerTurn);
        ch.Exact = (long)combos.Count * drawDistinct <= _opt.ExactChanceThreshold;

        // In exact mode, generate the outcomes LAZILY (one per chance-node visit, via SelectOutcome's
        // MoveNext) instead of materialising all of them up front. A chance node visited V times then pays for
        // only V outcomes — not all (combos × draws), which for a big deck is thousands of clones the node
        // never needs. The outcome SEQUENCE (order, probabilities, keys) matches eager enumeration, so the
        // exact partial-Bellman backup is unchanged — a pure efficiency win, not an accuracy tradeoff.
        if (ch.Exact)
            ch.PendingEnum = EnumerateExactOutcomes(ch.AfterEnemy, combos).GetEnumerator();

        // Seed the value with averaged λ-rollouts from the post-enemy state (so parents can back up through
        // this edge before it is ever descended).
        var seed = SeedValue(ch.AfterEnemy, needAdvance: true, initialRoll: ch.Initial);
        ch.V = new Value(seed.Win, ch.EnemyLoss + seed.Loss);
    }

    /// <summary>Lazily enumerate the exact joint (move-roll × draw) outcomes of a chance node, in the same
    /// order the old eager fill produced — so explication order, probabilities and keys are identical. Each
    /// move-combo clones the post-enemy state once and advances the player turn; the (already lazy)
    /// <see cref="DrawEnumerator.EnumerateDraw"/> then yields its draws on demand. Suspends between yields, so
    /// the clone for combo K only happens when the node is visited enough to reach it.</summary>
    private static IEnumerable<PendingOutcome> EnumerateExactOutcomes(
        CombatState afterEnemy, List<(double prob, List<(int idx, string moveId)> combo, string key)> combos)
    {
        foreach (var (pM, combo, comboKey) in combos)
        {
            var afterRoll = afterEnemy.Clone();
            foreach (var (idx, moveId) in combo) afterRoll.Monsters[idx].Ai.CurrentMoveId = moveId;
            CombatManager.BeginPlayerTurn(afterRoll);
            afterRoll.PendingDraw = 0;   // discard any turn-start power draw (inert at the boundary, as before)
            int startLoss = afterRoll.PlayerHpLost - afterEnemy.PlayerHpLost;

            int drawIdx = 0;
            int draw = CombatManager.OpeningDrawAfterInnate(afterRoll, CombatManager.TurnStartDrawCount(afterRoll));   // Innate + MachineLearning
            foreach (var (pD, drawn) in DrawEnumerator.EnumerateDraw(afterRoll, draw, fromHandDraw: true))
                yield return new PendingOutcome(pM * pD, startLoss, drawn, $"{comboKey}|{drawIdx++}");
        }
    }

    /// <summary>Average <see cref="MctsOptions.RolloutSamples"/> faithful λ-rollouts into one seed value.
    /// Each playout samples its own aggression λ, so the seed reflects the block↔race spectrum, not a single
    /// line. <paramref name="src"/> is cloned per playout (rollouts mutate state).</summary>
    private Value SeedValue(CombatState src, bool needAdvance, bool initialRoll)
    {
        int k = Math.Max(1, _opt.RolloutSamples);
        if (k == 1) { var v1 = Playout(src.Clone(), needAdvance, initialRoll); Note(v1); return v1; }
        double win = 0, loss = 0;
        for (int i = 0; i < k; i++)
        {
            var v = Playout(src.Clone(), needAdvance, initialRoll);
            Note(v);
            win += v.Win; loss += v.Loss;
        }
        return new Value(win / k, loss / k);
    }

    private void VisitChance(ChanceNode ch)
    {
        ch.N++;
        if (ch.Terminal) return;

        var o = SelectOutcome(ch);
        o.Visits++;
        VisitDecision(o.Child);

        // Partial Bellman backup over explicated outcomes, weighted by true probability.
        double pk = ch.ExplicatedMass;
        double win = 0, loss = 0;
        foreach (var oc in ch.Explicated)
        {
            win += oc.Prob * oc.Child.V.Win;
            loss += oc.Prob * (oc.StartLoss + oc.Child.V.Loss);
        }
        ch.V = new Value(win / pk, ch.EnemyLoss + loss / pk);
    }

    private Outcome SelectOutcome(ChanceNode ch)
    {
        if (ch.Exact)
        {
            // Pull the next exact outcome on demand; when the lazy generator is exhausted, every outcome has
            // been explicated → fall through to reselect among them.
            if (ch.PendingEnum != null && ch.PendingEnum.MoveNext()) return Explicate(ch, ch.PendingEnum.Current);
        }
        else
        {
            int allowed = (int)Math.Ceiling(_opt.DpwC * Math.Pow(ch.N, _opt.DpwBeta));
            if (ch.Explicated.Count < allowed)
            {
                var po = SampleOutcome(ch);
                if (!ch.SeenKeys.Contains(po.Key)) return Explicate(ch, po);
                // duplicate sample: fall through to reselect an existing child.
            }
        }

        // Reselect: focus on probable, least-visited outcomes (DP-UCT style).
        Outcome best = ch.Explicated[0];
        double bestScore = best.Prob / (1 + best.Visits);
        for (int i = 1; i < ch.Explicated.Count; i++)
        {
            double sc = ch.Explicated[i].Prob / (1 + ch.Explicated[i].Visits);
            if (sc > bestScore) { best = ch.Explicated[i]; bestScore = sc; }
        }
        return best;
    }

    private Outcome Explicate(ChanceNode ch, PendingOutcome po)
    {
        var child = GetOrCreateDecision(po.State);
        var o = new Outcome { Prob = po.Prob, StartLoss = po.StartLoss, Child = child, Key = po.Key };
        ch.Explicated.Add(o);
        ch.SeenKeys.Add(po.Key);
        ch.ExplicatedMass += po.Prob;
        return o;
    }

    /// <summary>Sample one (move-combo, draw) outcome from the true joint distribution (DPW path).</summary>
    private PendingOutcome SampleOutcome(ChanceNode ch)
    {
        double pM = 1.0;
        var combo = new List<(int, string)>();
        var keyParts = new List<string>();
        for (int i = 0; i < ch.AfterEnemy.Monsters.Count; i++)
        {
            var m = ch.AfterEnemy.Monsters[i];
            if (!m.IsAlive) continue;
            var dist = ch.Initial ? m.Ai.EnumerateInitial(m) : m.Ai.EnumerateNext(m);
            var picked = _rng.PickWeighted(dist.Select(x => (x.prob, x.moveId)).ToList());
            pM *= dist.First(x => x.moveId == picked).prob;
            combo.Add((i, picked));
            keyParts.Add($"{i}:{picked}");
        }

        var afterRoll = ch.AfterEnemy.Clone();
        foreach (var (idx, moveId) in combo) afterRoll.Monsters[idx].Ai.CurrentMoveId = moveId;
        CombatManager.BeginPlayerTurn(afterRoll);
        afterRoll.PendingDraw = 0;   // discard any turn-start power draw (inert at the boundary, as before)
        int startLoss = afterRoll.PlayerHpLost - ch.AfterEnemy.PlayerHpLost;

        int draw = CombatManager.OpeningDrawAfterInnate(afterRoll, CombatManager.TurnStartDrawCount(afterRoll));   // Innate + MachineLearning
        var (pD, drawn, drawKey) = DrawEnumerator.SampleDraw(afterRoll, draw, _rng, fromHandDraw: true);
        return new PendingOutcome(pM * pD, startLoss, drawn, $"{string.Join(",", keyParts)}|{drawKey}");
    }

    // ---------- Mid-turn draw chance nodes ----------

    /// <summary>Wire a card play's edge target: a normal decision node, or — when the play deferred a mid-turn
    /// draw (<see cref="CombatState.PendingDraw"/> &gt; 0) and combat isn't already decided — a draw chance node
    /// that resolves the drawn hand and continues the same turn.</summary>
    private (DecisionNode? dec, DrawNode? draw) PlayTarget(CombatState postPlay)
    {
        if (postPlay.PendingDraw > 0 && !postPlay.IsCombatOver) return (null, BuildDrawNode(postPlay));
        return (GetOrCreateDecision(postPlay), null);
    }

    private DrawNode BuildDrawNode(CombatState postPlay)
    {
        var baseState = postPlay.Clone();
        int n = baseState.PendingDraw;
        baseState.PendingDraw = 0;   // drained: drawn children are clean decision states (counter not in the key)
        var dn = new DrawNode { Base = baseState, DrawCount = n };
        InitDrawNode(dn);
        return dn;
    }

    /// <summary>Decide exact-vs-DPW over the draw distribution, prime the lazy exact queue, and seed the value
    /// with one faithful rollout that resolves the pending draw concretely.</summary>
    private void InitDrawNode(DrawNode dn)
    {
        dn.Exact = DrawEnumerator.DistinctDrawCount(dn.Base, dn.DrawCount) <= _opt.ExactChanceThreshold;
        if (dn.Exact)
            dn.PendingEnum = EnumerateExactDraws(dn.Base, dn.DrawCount).GetEnumerator();
        dn.V = SeedDrawValue(dn.Base, dn.DrawCount);
    }

    /// <summary>Lazily enumerate the exact draw outcomes (probability, resulting state) of the pending draw.</summary>
    private static IEnumerable<PendingOutcome> EnumerateExactDraws(CombatState baseState, int n)
    {
        int i = 0;
        foreach (var (pD, drawn) in DrawEnumerator.EnumerateDraw(baseState, n))
            yield return new PendingOutcome(pD, 0, drawn, $"d{i++}");
    }

    /// <summary>Seed value: draw the pending cards with the RNG, resolve the post-draw step (EscapePlan block /
    /// a heuristic-default discard), then roll out the rest of the fight. One sampled line (the node refines
    /// toward the true expectation, and the discard toward the player's MAX, as outcomes are explicated).</summary>
    private Value SeedDrawValue(CombatState baseState, int n)
    {
        var s = baseState.Clone();
        s.Rng = _rng;
        CombatManager.DrawCards(s, n, _rng);
        CombatManager.ApplyPostDraw(s);   // EscapePlan block, or set PendingDiscard
        while (s.PendingDiscard > 0 && s.Player.Hand.Count > 0)   // resolve the seed's discard with a default
        {
            Cmd.DiscardFromHand(s, s.Player.Hand[0]);
            s.PendingDiscard--;
        }
        s.PendingDiscard = 0;
        CombatManager.ApplyPostDiscard(s);   // run any discard continuation (HiddenDaggers Shivs)
        var v = Playout(s, needAdvance: false, initialRoll: false);
        Note(v);
        return v;
    }

    private void VisitDraw(DrawNode dn)
    {
        dn.N++;
        var o = SelectDrawOutcome(dn);
        o.Visits++;
        VisitDecision(o.Child);

        // Partial Bellman over explicated draw outcomes, weighted by true probability. No enemy/start loss:
        // drawing is free, so the node's value is exactly the drawn children's lexicographic expectation.
        double pk = dn.ExplicatedMass;
        double win = 0, loss = 0;
        foreach (var oc in dn.Explicated)
        {
            win += oc.Prob * oc.Child.V.Win;
            loss += oc.Prob * oc.Child.V.Loss;
        }
        dn.V = new Value(win / pk, loss / pk);
    }

    private Outcome SelectDrawOutcome(DrawNode dn)
    {
        if (dn.Exact)
        {
            if (dn.PendingEnum != null && dn.PendingEnum.MoveNext()) return ExplicateDraw(dn, dn.PendingEnum.Current);
        }
        else
        {
            int allowed = (int)Math.Ceiling(_opt.DpwC * Math.Pow(dn.N, _opt.DpwBeta));
            if (dn.Explicated.Count < allowed)
            {
                var po = SampleDrawOutcome(dn);
                if (!dn.SeenKeys.Contains(po.Key)) return ExplicateDraw(dn, po);
            }
        }

        Outcome best = dn.Explicated[0];
        double bestScore = best.Prob / (1 + best.Visits);
        for (int i = 1; i < dn.Explicated.Count; i++)
        {
            double sc = dn.Explicated[i].Prob / (1 + dn.Explicated[i].Visits);
            if (sc > bestScore) { best = dn.Explicated[i]; bestScore = sc; }
        }
        return best;
    }

    private Outcome ExplicateDraw(DrawNode dn, PendingOutcome po)
    {
        CombatManager.ApplyPostDraw(po.State);   // EscapePlan block (applied here) / set PendingDiscard (a decision)
        var child = GetOrCreateDecision(po.State);
        var o = new Outcome { Prob = po.Prob, StartLoss = 0, Child = child, Key = po.Key };
        dn.Explicated.Add(o);
        dn.SeenKeys.Add(po.Key);
        dn.ExplicatedMass += po.Prob;
        return o;
    }

    private PendingOutcome SampleDrawOutcome(DrawNode dn)
    {
        var (pD, drawn, key) = DrawEnumerator.SampleDraw(dn.Base, dn.DrawCount, _rng);
        return new PendingOutcome(pD, 0, drawn, key);
    }

    // ---------- Move-roll enumeration (mirrors the exact solver) ----------

    private static List<(double prob, List<(int idx, string moveId)> combo, string key)> EnumerateMoveRolls(
        CombatState s, bool initial)
    {
        var perMonster = new List<(int index, List<(double prob, string moveId)> dist)>();
        for (int i = 0; i < s.Monsters.Count; i++)
        {
            var m = s.Monsters[i];
            if (!m.IsAlive) continue;
            perMonster.Add((i, initial ? m.Ai.EnumerateInitial(m) : m.Ai.EnumerateNext(m)));
        }

        var acc = new List<(double prob, List<(int, string)> combo)> { (1.0, new List<(int, string)>()) };
        foreach (var (index, dist) in perMonster)
        {
            var next = new List<(double, List<(int, string)>)>();
            foreach (var (accProb, accList) in acc)
                foreach (var (prob, moveId) in dist)
                    next.Add((accProb * prob, new List<(int, string)>(accList) { (index, moveId) }));
            acc = next;
        }
        return acc.Select(a => (a.prob, a.combo, string.Join(",", a.combo.Select(c => $"{c.Item1}:{c.Item2}"))))
                  .ToList();
    }

    // ---------- Legal plays (mirrors the exact solver) ----------

    // expandChoices=false collapses every in-card decision to its single DEFAULT action (ChoiceKey null). The
    // SEARCH TREE always expands choices (each is a real decision node); the approximate greedy ROLLOUT does
    // NOT — scoring N choice variants per card per step (each a full ApplyPlay clone) is wasted compute in a
    // noisy leaf estimator, and the rollout already replayed the default anyway. This keeps a choice-heavy deck
    // (e.g. Silent) from multiplying rollout cost by the per-card choice fan-out.
    private static IEnumerable<PlayerAction> LegalPlays(CombatState s, bool expandChoices = true)
    {
        // Same unconditional per-turn play cap as the exact solver — keeps the tree finite and the rollout from
        // spinning on a cost-0 cantrip, and keeps MCTS converging to the (identically capped) oracle.
        if (s.PlaysThisTurn >= s.EffectivePlayCap()) yield break;   // tightened by Normality (≤3) while in hand
        var seen = new HashSet<string>();
        foreach (var card in s.Player.Hand)
        {
            if (!s.CardPlayAllowed(card)) continue;   // Unplayable + Enthralled hand-lockout
            if (!card.IsXCost && CombatManager.ResolveCardCost(s, card) > s.Player.Energy) continue;   // resolved cost = EffectiveCost + power ModifyCardCost (VoidForm/Free Attack/Corruption discounts, Borrowed Time increase) — matches PlayCard's spend
            if (!s.Player.CanAffordStars(card)) continue;   // Regent star cost gates the play
            var ck = card.StateKey();
            var choices = expandChoices ? card.Choices(s).Distinct().ToList() : EmptyChoices;   // empty for the common no-choice card
            if (card.NeedsTarget)
            {
                for (int i = 0; i < s.Monsters.Count; i++)
                {
                    if (!s.Monsters[i].IsAlive) continue;
                    if (choices.Count == 0)
                    {
                        if (seen.Add($"{ck}@{i}")) yield return new PlayerAction($"Play {ck} -> M{i}", ck, i);
                    }
                    else foreach (var choice in choices)
                        if (seen.Add($"{ck}@{i}#{choice}"))
                            yield return new PlayerAction($"Play {ck} -> M{i} [{choice}]", ck, i, choice);
                }
            }
            else if (choices.Count == 0)
            {
                if (seen.Add(ck)) yield return new PlayerAction($"Play {ck}", ck, -1);
            }
            else foreach (var choice in choices)
                if (seen.Add($"{ck}#{choice}"))
                    yield return new PlayerAction($"Play {ck} [{choice}]", ck, -1, choice);
        }
    }

    private static readonly List<string> EmptyChoices = new();

    private static CombatState ApplyPlay(CombatState s, PlayerAction action)
    {
        var c = s.Clone();
        var card = c.Player.Hand.First(h => h.StateKey() == action.CardKey);
        Creature? target = action.TargetMonsterIndex >= 0 ? c.Monsters[action.TargetMonsterIndex] : null;
        CombatManager.PlayCard(c, card, target, action.ChoiceKey);
        return c;
    }

    // ---------- Greedy rollout (faithful leaf evaluator) ----------

    /// <summary>
    /// Play the fight out under a fast greedy policy and return (won?, HP lost from <paramref name="s"/>).
    /// The <i>value</i> is engine-faithful (real damage/block pipeline); only the policy is heuristic.
    /// </summary>
    private Value Playout(CombatState s, bool needAdvance, bool initialRoll)
    {
        int baseline = s.PlayerHpLost;
        s.Rng = _rng;   // a rollout is a concrete driver: mid-turn draws (Shrug It Off, …) resolve for real

        // Resolve any pending post-draw discard-of-choice with a heuristic default before rolling out (the
        // greedy policy doesn't model it; the tree's discard decision node refines toward the player's MAX).
        while (s.PendingDiscard > 0 && s.Player.Hand.Count > 0) { Cmd.DiscardFromHand(s, s.Player.Hand[0]); s.PendingDiscard--; }
        s.PendingDiscard = 0;
        CombatManager.ApplyPostDiscard(s);   // run any discard continuation (HiddenDaggers Shivs) before rolling out

        if (needAdvance)   // s is a post-enemy (or setup) state: advance to the next decision point first
        {
            if (s.PlayerDead) return new Value(0, s.PlayerHpLost - baseline);
            if (s.AllMonstersDead) return new Value(1, s.PlayerHpLost - baseline);
            if (initialRoll) CombatManager.RollInitialMoves(s, _rng);
            else CombatManager.RollNextMoves(s, _rng);
            CombatManager.BeginPlayerTurn(s);
            CombatManager.DrawCards(s, CombatManager.OpeningDrawAfterInnate(s, CombatManager.TurnStartDrawCount(s)), _rng, fromHandDraw: true);
        }

        return RolloutToTerminal(s, baseline);
    }

    /// <summary>Play the fight out to terminal under the greedy λ-policy, returning (won?, HP lost from
    /// <paramref name="baseline"/>). Assumes <paramref name="s"/> is a live decision tip (moves rolled, hand
    /// drawn). Mutates <paramref name="s"/>.</summary>
    private Value RolloutToTerminal(CombatState s, int baseline)
    {
        // Sample an aggression λ for this whole playout: the play order interpolates between all-block and
        // all-damage, so a fixed-λ line is one point on that spectrum. Sampling λ per rollout makes the leaf
        // seed average over the spectrum (turtle…aggro) instead of one biased extreme — the key to seeding
        // do-or-die (low-survival) fights, where the winning line sits at a specific λ a greedy policy misses.
        double aggression = _opt.RolloutLambdaLo +
            (_opt.RolloutLambdaHi - _opt.RolloutLambdaLo) * _rng.NextDouble();

        while (true)
        {
            if (s.AllMonstersDead) return new Value(1, s.PlayerHpLost - baseline);
            if (s.PlayerDead) return new Value(0, s.PlayerHpLost - baseline);
            if (s.TurnNumber > _opt.MaxTurns) return new Value(0, s.PlayerHpLost - baseline);

            PlayTurn(s, aggression);

            CombatManager.EndPlayerTurn(s);
            if (!s.PlayerDead) CombatManager.RunEnemyTurn(s);
            if (s.PlayerDead) return new Value(0, s.PlayerHpLost - baseline);
            if (s.AllMonstersDead) return new Value(1, s.PlayerHpLost - baseline);

            CombatManager.RollNextMoves(s, _rng);
            CombatManager.BeginPlayerTurn(s);
            CombatManager.DrawCards(s, CombatManager.OpeningDrawAfterInnate(s, CombatManager.TurnStartDrawCount(s)), _rng, fromHandDraw: true);   // MachineLearning +draw; Innate no-op past turn 1
        }
    }

    /// <summary>The single play the greedy λ-policy would make at <paramref name="s"/>: the legal play whose
    /// resulting position score is LOWEST and strictly below the current score, or <c>null</c> to end the turn
    /// (no play strictly improves the position). This is the leaf policy's per-decision choice — exposed so the
    /// calibration harness can measure it against the exact oracle (policy-agreement / regret).</summary>
    public static PlayerAction? GreedyBestPlay(CombatState s, double aggression)
    {
        double bestScore = CombatHeuristic.Score(s, aggression);
        PlayerAction? bestAction = null;
        foreach (var action in LegalPlays(s, expandChoices: false))
        {
            double sc = CombatHeuristic.Score(ApplyPlay(s, action), aggression);
            if (sc < bestScore - 1e-9) { bestScore = sc; bestAction = action; }
        }
        return bestAction;
    }

    /// <summary>Greedily play cards that reduce the position score at aggression λ (lower = better), ending
    /// the turn when no play strictly improves it. Public wrapper <see cref="GreedyPlayTurn"/> exposes this
    /// for calibration (the faithful leaf policy, one turn, in place).</summary>
    private static void PlayTurn(CombatState s, double aggression)
    {
        for (int guard = 0; guard < 30; guard++)
        {
            if (GreedyBestPlay(s, aggression) is not { } bestAction) return;   // no play strictly improves it
            var card = s.Player.Hand.First(h => h.StateKey() == bestAction.CardKey);
            Creature? target = bestAction.TargetMonsterIndex >= 0
                ? s.Monsters[bestAction.TargetMonsterIndex] : null;
            CombatManager.PlayCard(s, card, target);
            if (s.PlayerTurnEndForced) return;   // VoidForm ended the turn on play
        }
    }

    /// <summary>Run the greedy λ-rollout policy for ONE player turn in place — the exact leaf policy the
    /// rollout uses. Exposed for the calibration harness's policy-regret measurement; not used in normal solving.</summary>
    public static void GreedyPlayTurn(CombatState s, double aggression) => PlayTurn(s, aggression);

    /// <summary>The card-play labels the greedy λ-policy would make this turn (for calibration/debugging),
    /// mirroring <see cref="Solver.BestTurnPlan"/> so a greedy line can be diffed against the oracle's. Does not
    /// mutate <paramref name="s"/>.</summary>
    public static List<string> GreedyTurnPlan(CombatState s, double aggression)
    {
        var plan = new List<string>();
        var cur = s.Clone();
        for (int guard = 0; guard < 30 && !cur.IsCombatOver; guard++)
        {
            if (GreedyBestPlay(cur, aggression) is not { } a) { plan.Add("End turn"); break; }
            plan.Add(a.Label);
            cur = ApplyPlay(cur, a);
            if (cur.PlayerTurnEndForced) { plan.Add("End turn"); break; }
        }
        return plan;
    }

    private const double Eps = 1e-9;

    // ---------- Node types ----------

    private sealed class DecisionNode
    {
        public CombatState State = null!;
        public (ulong, ulong) Key;
        public bool Terminal;
        public bool ExactSolved;   // resolved by the hybrid exact oracle
        public bool Expanded;
        public List<Edge> Edges = null!;
        public Value V;
        public int N;

        // Action-widening state (null when ActionWidening is off): card plays ranked best-first by the policy
        // prior, the matching normalised priors, and how many of them are currently opened as edges.
        public List<PlayerAction>? Candidates;
        public double[]? CandidatePriors;
        public int Opened;
    }

    private sealed class Edge
    {
        public PlayerAction Action;
        public bool IsEndTurn;
        public DecisionNode? PlayChild;
        public DrawNode? DrawChild;   // set instead of PlayChild when the play deferred a mid-turn draw
        public ChanceNode? Chance;
        public int Visits;
        public double Prior;   // policy prior for this action (action-widening / PUCT only)
        public Value Q => IsEndTurn ? Chance!.V : (DrawChild != null ? DrawChild.V : PlayChild!.V);   // plays cost no HP in scope
    }

    private sealed class ChanceNode
    {
        public CombatState AfterEnemy = null!;   // post enemy turn (or the setup state, when Initial)
        public int EnemyLoss;
        public bool Initial;
        public bool Terminal;
        public bool Exact;
        public IEnumerator<PendingOutcome>? PendingEnum;   // exact-mode outcomes, generated LAZILY on demand
        public readonly List<Outcome> Explicated = new();
        public readonly HashSet<string> SeenKeys = new();
        public double ExplicatedMass;
        public Value V;
        public int N;
    }

    private sealed class Outcome
    {
        public double Prob;
        public int StartLoss;
        public DecisionNode Child = null!;
        public string Key = "";
        public int Visits;
    }

    /// <summary>A MID-TURN draw chance node: a play deferred a draw (<see cref="CombatState.PendingDraw"/>),
    /// so the drawn hand is resolved here as a chance node over the exact draw distribution, continuing the
    /// SAME player turn. Unlike <see cref="ChanceNode"/> there is no enemy turn and no HP-loss term — drawing
    /// is free — so the backup is a plain probability-weighted average of the drawn decision children.</summary>
    private sealed class DrawNode
    {
        public CombatState Base = null!;   // post-play state, PendingDraw drained to DrawCount
        public int DrawCount;
        public bool Exact;
        public IEnumerator<PendingOutcome>? PendingEnum;   // exact-mode draw outcomes, generated lazily
        public readonly List<Outcome> Explicated = new();
        public readonly HashSet<string> SeenKeys = new();
        public double ExplicatedMass;
        public Value V;
        public int N;
    }

    private readonly record struct PendingOutcome(double Prob, int StartLoss, CombatState State, string Key);
}
