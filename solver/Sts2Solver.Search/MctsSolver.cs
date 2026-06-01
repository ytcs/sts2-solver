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

    /// <summary>Use the static <see cref="CombatHeuristic.Evaluate"/> race model at a new tip instead of a
    /// full greedy rollout to terminal (UCT*'s shortened trial, docs/mcts-solver-design.md §5). Cheaper per
    /// trial ⇒ more trials in a fixed budget, at the cost of a less faithful (closed-form) leaf value.</summary>
    public bool UseHeuristicLeaf = false;

    /// <summary>Use the trained <see cref="LearnedValue"/> regression at a new tip instead of a full rollout
    /// or the static race model — the Phase-C learned leaf. Takes precedence over <see cref="UseHeuristicLeaf"/>.
    /// Aimed at the razor-thin survival regime a greedy rollout under-estimates: the model predicts survival
    /// directly from learned geometry rather than needing a lucky coordinated rollout.</summary>
    public bool UseLearnedLeaf = false;

    /// <summary>Blended leaf: at a fresh tip, combine the faithful greedy rollout (which UNDER-estimates
    /// razor-thin survival — the winning line needs coordinated draw+play a greedy policy misses) with the
    /// learned value (which OVER-estimates it) as a convex mix <c>(1−α)·rollout + α·learned</c>. The exact
    /// truth sits between the two endpoints, so a calibrated α lands closer than either alone. 0 = pure
    /// rollout (the trusted default); ignored when <see cref="UseLearnedLeaf"/>/<see cref="UseHeuristicLeaf"/>
    /// is set (those are closed-form, no rollout). Costs one rollout + one cheap learned eval per tip.</summary>
    public double LeafBlend = EnvD("STS2_LEAF_BLEND", 0.0);

    /// <summary>Each rollout samples its aggression λ uniformly from [Lo, Hi] (0 = all-block, 1 = all-damage),
    /// so leaf seeds average over the block↔race spectrum rather than a single biased greedy line. Lo==Hi
    /// gives a deterministic policy at that λ (set both to 0.5 for the old balanced greedy). Defaults span
    /// the full spectrum.</summary>
    // Default to a deterministic *balanced* rollout (λ=0.5): calibration showed it tracks exact far better
    // than the old race-leaning score, while λ-spread + multi-sample averaging added cost without accuracy
    // (the rare winning lines in razor-thin fights need coordinated draw+play a static rollout misses at any
    // λ — that residual is the learned-value-function's job). Knobs kept for experiments / the VF phase.
    public double RolloutLambdaLo = EnvD("STS2_LAMBDA_LO", 0.5);
    public double RolloutLambdaHi = EnvD("STS2_LAMBDA_HI", 0.5);

    /// <summary>Number of playouts averaged into each new leaf's seed value (each samples its own λ from
    /// [Lo,Hi]). 1 = the classic single-rollout seed (the calibrated default — averaging didn't pay off).</summary>
    public int RolloutSamples = (int)EnvD("STS2_ROLLOUT_SAMPLES", 1);

    private static double EnvD(string k, double dflt) =>
        double.TryParse(Environment.GetEnvironmentVariable(k), out var v) ? v : dflt;
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
        foreach (var action in LegalPlays(d.State))
        {
            var child = GetOrCreateDecision(ApplyPlay(d.State, action));
            d.Edges.Add(new Edge { Action = action, PlayChild = child });
        }
        d.Edges.Add(new Edge { Action = PlayerAction.EndTurn, IsEndTurn = true, Chance = BuildEndTurnChance(d.State) });
    }

    private Value VisitDecision(DecisionNode d)
    {
        d.N++;
        if (d.Terminal) return d.V;
        if (!d.Expanded) { Expand(d); return d.V; }   // tip: expand + return leaf seed (trial ends)

        var e = SelectEdge(d);
        e.Visits++;
        if (e.IsEndTurn) VisitChance(e.Chance!);
        else VisitDecision(e.PlayChild!);

        // Partial Bellman / Bellman recomputation from children: lexicographic max over edges.
        Value best = d.Edges[0].Q;
        for (int i = 1; i < d.Edges.Count; i++)
            if (d.Edges[i].Q.BetterThan(best)) best = d.Edges[i].Q;
        d.V = best;
        return d.V;
    }

    /// <summary>Lexicographic-UCB edge selection (HP loss normalised by maxHP).</summary>
    private Edge SelectEdge(DecisionNode d)
    {
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

        if (ch.Exact)
        {
            ch.Pending = new Queue<PendingOutcome>();
            foreach (var (pM, combo, comboKey) in combos)
            {
                var afterRoll = ch.AfterEnemy.Clone();
                foreach (var (idx, moveId) in combo) afterRoll.Monsters[idx].Ai.CurrentMoveId = moveId;
                CombatManager.BeginPlayerTurn(afterRoll);
                int startLoss = afterRoll.PlayerHpLost - ch.AfterEnemy.PlayerHpLost;

                int drawIdx = 0;
                foreach (var (pD, drawn) in DrawEnumerator.EnumerateDraw(afterRoll, Player.CardsDrawnPerTurn))
                    ch.Pending.Enqueue(new PendingOutcome(pM * pD, startLoss, drawn, $"{comboKey}|{drawIdx++}"));
            }
        }
        // Seed the value with averaged λ-rollouts from the post-enemy state (so parents can back up through
        // this edge before it is ever descended).
        var seed = SeedValue(ch.AfterEnemy, needAdvance: true, initialRoll: ch.Initial);
        ch.V = new Value(seed.Win, ch.EnemyLoss + seed.Loss);
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
            if (ch.Pending!.Count > 0) return Explicate(ch, ch.Pending.Dequeue());
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
        int startLoss = afterRoll.PlayerHpLost - ch.AfterEnemy.PlayerHpLost;

        var (pD, drawn, drawKey) = DrawEnumerator.SampleDraw(afterRoll, Player.CardsDrawnPerTurn, _rng);
        return new PendingOutcome(pM * pD, startLoss, drawn, $"{string.Join(",", keyParts)}|{drawKey}");
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

    private static IEnumerable<PlayerAction> LegalPlays(CombatState s)
    {
        var seen = new HashSet<string>();
        foreach (var card in s.Player.Hand)
        {
            if (card.Unplayable) continue;
            if (card.Cost > s.Player.Energy) continue;
            var ck = card.StateKey();
            if (card.NeedsTarget)
            {
                for (int i = 0; i < s.Monsters.Count; i++)
                {
                    if (!s.Monsters[i].IsAlive) continue;
                    var sig = $"{ck}@{i}";
                    if (seen.Add(sig)) yield return new PlayerAction($"Play {ck} -> M{i}", ck, i);
                }
            }
            else if (seen.Add(ck)) yield return new PlayerAction($"Play {ck}", ck, -1);
        }
    }

    private static CombatState ApplyPlay(CombatState s, PlayerAction action)
    {
        var c = s.Clone();
        var card = c.Player.Hand.First(h => h.StateKey() == action.CardKey);
        Creature? target = action.TargetMonsterIndex >= 0 ? c.Monsters[action.TargetMonsterIndex] : null;
        CombatManager.PlayCard(c, card, target);
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

        if (needAdvance)   // s is a post-enemy (or setup) state: advance to the next decision point first
        {
            if (s.PlayerDead) return new Value(0, s.PlayerHpLost - baseline);
            if (s.AllMonstersDead) return new Value(1, s.PlayerHpLost - baseline);
            if (initialRoll) CombatManager.RollInitialMoves(s, _rng);
            else CombatManager.RollNextMoves(s, _rng);
            CombatManager.BeginPlayerTurn(s);
            CombatManager.DrawCards(s, Player.CardsDrawnPerTurn, _rng);
        }

        // UCT*: bootstrap the tip from a closed-form leaf value instead of rolling out to terminal.
        if (_opt.UseLearnedLeaf || _opt.UseHeuristicLeaf)
        {
            if (s.AllMonstersDead) return new Value(1, s.PlayerHpLost - baseline);
            if (s.PlayerDead || s.TurnNumber > _opt.MaxTurns) return new Value(0, s.PlayerHpLost - baseline);
            var lv = _opt.UseLearnedLeaf ? LearnedValue.Evaluate(s, _opt.MaxTurns)
                                         : CombatHeuristic.Evaluate(s, _opt.MaxTurns);
            return new Value(lv.Win, (s.PlayerHpLost - baseline) + lv.Loss);
        }

        // Blended leaf: convex mix of the learned value (over-estimates razor-thin survival) and the faithful
        // rollout (under-estimates it). At a live tip both estimate value-from-s; terminal tips are exact, so
        // skip the blend there. The learned eval is read-only (clones internally), so it's safe before the
        // rollout, which mutates s.
        if (_opt.LeafBlend > 0)
        {
            if (s.AllMonstersDead) return new Value(1, s.PlayerHpLost - baseline);
            if (s.PlayerDead || s.TurnNumber > _opt.MaxTurns) return new Value(0, s.PlayerHpLost - baseline);
            double advanceLoss = s.PlayerHpLost - baseline;
            var learned = LearnedValue.Evaluate(s, _opt.MaxTurns);
            var rollout = RolloutToTerminal(s, baseline);
            double a = _opt.LeafBlend;
            double rolloutFuture = rollout.Loss - advanceLoss;
            return new Value((1 - a) * rollout.Win + a * learned.Win,
                             advanceLoss + (1 - a) * rolloutFuture + a * learned.Loss);
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
            CombatManager.DrawCards(s, Player.CardsDrawnPerTurn, _rng);
        }
    }

    /// <summary>Greedily play cards that reduce the position score at aggression λ (lower = better), ending
    /// the turn when no play strictly improves it.</summary>
    private static void PlayTurn(CombatState s, double aggression)
    {
        for (int guard = 0; guard < 30; guard++)
        {
            double current = CombatHeuristic.Score(s, aggression);
            PlayerAction? bestAction = null;
            double bestScore = current;
            foreach (var action in LegalPlays(s))
            {
                double sc = CombatHeuristic.Score(ApplyPlay(s, action), aggression);
                if (sc < bestScore - 1e-9) { bestScore = sc; bestAction = action; }
            }
            if (bestAction == null) return;   // no play strictly improves the position
            var card = s.Player.Hand.First(h => h.StateKey() == bestAction.Value.CardKey);
            Creature? target = bestAction.Value.TargetMonsterIndex >= 0
                ? s.Monsters[bestAction.Value.TargetMonsterIndex] : null;
            CombatManager.PlayCard(s, card, target);
        }
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
    }

    private sealed class Edge
    {
        public PlayerAction Action;
        public bool IsEndTurn;
        public DecisionNode? PlayChild;
        public ChanceNode? Chance;
        public int Visits;
        public Value Q => IsEndTurn ? Chance!.V : PlayChild!.V;   // plays cost no HP in scope
    }

    private sealed class ChanceNode
    {
        public CombatState AfterEnemy = null!;   // post enemy turn (or the setup state, when Initial)
        public int EnemyLoss;
        public bool Initial;
        public bool Terminal;
        public bool Exact;
        public Queue<PendingOutcome>? Pending;   // exact-mode remaining outcomes
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

    private readonly record struct PendingOutcome(double Prob, int StartLoss, CombatState State, string Key);
}
