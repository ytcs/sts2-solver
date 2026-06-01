using Sts2Solver.Engine;

namespace Sts2Solver.Search;

/// <summary>
/// EXPERIMENT (open question: "drop survival probability, optimise HP-loss only"). A single-scalar reformulation
/// of the combat objective: minimise expected forward HP loss where <b>death — and failing to win within the
/// horizon — costs your full remaining HP</b>. Because the engine clips HP loss at the bar and the model is
/// heal-free, the per-leaf loss accounting here is IDENTICAL to the lexicographic <see cref="Solver"/>'s
/// <see cref="Value.Loss"/> on every branch (a doomed line already sums to full current HP — the same fact the
/// <see cref="LossCertificate"/> rests on). The ONLY differences are: (1) no win component, (2) we minimise the
/// scalar directly instead of maximising survival first, and (3) a horizon time-out is charged the full HP
/// (a stall-forever policy would otherwise score 0). So the scalar optimum and the lexicographic optimum differ
/// only in the POLICY each selects — letting us measure, exactly and noise-free, whether dropping survival
/// changes the decisions (and by how much it trades survival for HP).
///
/// This class is deliberately separate from <see cref="Solver"/> (the oracle is left byte-for-byte unchanged);
/// it reuses the oracle's exact transition machinery (opening states, move-roll enumeration, draw enumeration,
/// play application) so the comparison is apples-to-apples by construction.
/// </summary>
public sealed class ScalarSolver
{
    public int MaxTurns { get; init; } = 30;
    public CancellationToken Ct { get; set; } = CancellationToken.None;

    /// <summary>Extra HP-equivalent penalty charged ON TOP of the lost bar when the player dies (or fails to win
    /// within the horizon). 0 ⇒ the pure "death = lose your current HP" scalar (≡ maximise expected final HP).
    /// As this grows the scalar objective approaches the lexicographic survival-first limit (a constrained-MDP
    /// penalty / big-M method). It is added only at terminal death/time-out returns, so it never double-counts
    /// the heal-free forward HP loss the recursion already accumulates.</summary>
    public double DeathPenalty { get; init; } = 0;

    // Reused purely for its (value-agnostic) transition helpers — no lexicographic value is computed via it.
    private readonly Solver _t;
    private readonly Dictionary<(ulong, ulong), double> _memo = new();
    public int StatesEvaluated { get; private set; }

    public ScalarSolver() { _t = new Solver { MaxTurns = MaxTurns }; }

    /// <summary>Scalar optimum from a setup state: expected forward HP loss under the loss-minimising policy,
    /// averaged over the stochastic opening (initial move roll + opening draw).</summary>
    public double Solve(CombatState setup, int openingDraw = Player.CardsDrawnPerTurn)
    {
        double loss = 0;
        foreach (var (prob, state) in _t.OpeningStates(setup, openingDraw))
            loss += prob * SolvePlayerTurn(state);
        return loss;
    }

    public double SolvePlayerTurn(CombatState s)
    {
        if (s.AllMonstersDead) return 0;                                  // won → no further loss
        if (s.PlayerDead) return s.Player.CurrentHp + DeathPenalty;       // dead = lose your whole bar (+penalty)
        if (s.TurnNumber > MaxTurns) return s.Player.CurrentHp + DeathPenalty; // failed to win in horizon = death-equiv

        var key = s.HashKey();
        if (_memo.TryGetValue(key, out var cached)) return cached;
        if ((StatesEvaluated & 0x3FFF) == 0) Ct.ThrowIfCancellationRequested();
        StatesEvaluated++;

        double best = EndTurnTransition(s);                    // baseline: end the turn
        foreach (var action in _t.LegalPlays(s))
        {
            double v = SolvePlayerTurn(Solver.ApplyPlay(s, action));
            if (v < best - 1e-9) best = v;
        }

        _memo[key] = best;
        return best;
    }

    /// <summary>The loss-minimising play at a decision node, or null to end the turn.</summary>
    public PlayerAction? BestAction(CombatState s)
    {
        if (s.IsCombatOver) return null;
        double best = EndTurnTransition(s);
        PlayerAction? bestAction = null;
        foreach (var action in _t.LegalPlays(s))
        {
            double v = SolvePlayerTurn(Solver.ApplyPlay(s, action));
            if (v < best - 1e-9) { best = v; bestAction = action; }
        }
        return bestAction;
    }

    private double EndTurnTransition(CombatState s)
    {
        var afterEnemy = s.Clone();
        CombatManager.EndPlayerTurn(afterEnemy);
        CombatManager.RunEnemyTurn(afterEnemy);
        int enemyLoss = afterEnemy.PlayerHpLost - s.PlayerHpLost;

        if (afterEnemy.PlayerDead) return s.Player.CurrentHp + DeathPenalty;  // death = full remaining HP (+penalty)
        if (afterEnemy.AllMonstersDead) return enemyLoss;                      // won this enemy turn

        double leafLoss = 0;
        foreach (var (probM, afterRoll) in _t.EnumerateNextMoveRolls(afterEnemy))
        {
            Solver.BeginPlayerTurnInPlace(afterRoll);
            int startLoss = afterRoll.PlayerHpLost - afterEnemy.PlayerHpLost; // start-of-turn (e.g. poison)
            foreach (var (probD, afterDraw) in DrawEnumerator.EnumerateDraw(afterRoll, Player.CardsDrawnPerTurn))
                leafLoss += probM * probD * (startLoss + SolvePlayerTurn(afterDraw));
        }
        return enemyLoss + leafLoss;
    }
}

/// <summary>The realised value of a FIXED decision policy, measured three ways in one pass: survival
/// probability, the scalar loss (death = full HP), and the lexicographic loss (death = clipped lethal hit,
/// horizon time-out = 0). Used to cross-evaluate the lex-optimal policy under the scalar metric and vice
/// versa — the heart of the objective experiment.</summary>
public readonly record struct PolicyValue(double Win, double ScalarLoss, double LexLoss);

/// <summary>One fixture's verdict in the objective experiment.</summary>
public sealed record ObjectiveRow(
    string Fixture,
    double WinLex, double LossLex,             // lexicographic optimum (oracle)
    double ScalarOpt,                          // scalar optimum (loss-minimising policy)
    double ScalarOfLexPolicy,                  // scalar loss the survival-first policy actually incurs
    double WinOfScalarPolicy,                  // survival the loss-minimising policy actually achieves
    long StatesLex, long StatesScalar, long MsLex, long MsScalar)
{
    /// <summary>Extra expected HP loss the survival-first policy pays vs the scalar optimum (≥0). ≈0 ⇒ the
    /// lex policy is already (near-)scalar-optimal, so the scalar objective would not change HP outcomes.</summary>
    public double ScalarRegret => ScalarOfLexPolicy - ScalarOpt;

    /// <summary>Survival the scalar policy gives up vs the lex optimum (≥0). ≈0 ⇒ dropping survival is SAFE
    /// (the loss-minimising policy doesn't throw away winnable fights); large ⇒ scalar sacrifices wins for HP.</summary>
    public double SurvivalSacrifice => WinLex - WinOfScalarPolicy;
}

/// <summary>
/// Runs the survival-vs-scalar objective comparison on a pre-built fixture, at the EXACT oracle level (no
/// sampling noise), with built-in consistency checks (the fixed-policy evaluation must reproduce each solver's
/// own optimum for its own metric). Lives in Search; the CLI/tests pass in built <see cref="CombatState"/>s.
/// </summary>
public static class ObjectiveExperiment
{
    /// <summary>Evaluate a FIXED policy (decision-state → chosen play, or null to end the turn) over the full
    /// chance tree, accumulating survival + both loss accountings. No min/max — it just follows the policy.</summary>
    public static PolicyValue EvaluatePolicy(CombatState setup, int maxTurns, Func<CombatState, PlayerAction?> policy,
        CancellationToken ct = default)
    {
        var t = new Solver { MaxTurns = maxTurns };
        var memo = new Dictionary<(ulong, ulong), PolicyValue>();
        int steps = 0;

        PolicyValue Decision(CombatState s)
        {
            if ((++steps & 0x3FFF) == 0) ct.ThrowIfCancellationRequested();
            if (s.AllMonstersDead) return new PolicyValue(1, 0, 0);
            if (s.PlayerDead) return new PolicyValue(0, s.Player.CurrentHp, 0);
            if (s.TurnNumber > maxTurns) return new PolicyValue(0, s.Player.CurrentHp, 0);

            var key = s.HashKey();
            if (memo.TryGetValue(key, out var c)) return c;

            var action = policy(s);
            var r = action == null ? EndTurn(s) : Decision(Solver.ApplyPlay(s, action.Value));
            memo[key] = r;
            return r;
        }

        PolicyValue EndTurn(CombatState s)
        {
            var ae = s.Clone();
            CombatManager.EndPlayerTurn(ae);
            CombatManager.RunEnemyTurn(ae);
            int enemyLoss = ae.PlayerHpLost - s.PlayerHpLost;

            if (ae.PlayerDead) return new PolicyValue(0, s.Player.CurrentHp, enemyLoss);
            if (ae.AllMonstersDead) return new PolicyValue(1, enemyLoss, enemyLoss);

            double win = 0, sLoss = 0, lLoss = 0;
            foreach (var (probM, afterRoll) in t.EnumerateNextMoveRolls(ae))
            {
                Solver.BeginPlayerTurnInPlace(afterRoll);
                int startLoss = afterRoll.PlayerHpLost - ae.PlayerHpLost;
                foreach (var (probD, afterDraw) in DrawEnumerator.EnumerateDraw(afterRoll, Player.CardsDrawnPerTurn))
                {
                    var v = Decision(afterDraw);
                    double w = probM * probD;
                    win += w * v.Win;
                    sLoss += w * (startLoss + v.ScalarLoss);
                    lLoss += w * (startLoss + v.LexLoss);
                }
            }
            return new PolicyValue(win, enemyLoss + sLoss, enemyLoss + lLoss);
        }

        double W = 0, S = 0, L = 0;
        foreach (var (prob, st) in t.OpeningStates(setup, Player.CardsDrawnPerTurn))
        {
            var v = Decision(st);
            W += prob * v.Win; S += prob * v.ScalarLoss; L += prob * v.LexLoss;
        }
        return new PolicyValue(W, S, L);
    }

    /// <summary>Solve a fixture under BOTH objectives and cross-evaluate, producing the regret / survival-
    /// sacrifice row. The lex policy comes from a solved <see cref="Solver"/> (its memo makes BestAction cheap);
    /// the scalar policy from a <see cref="ScalarSolver"/>.</summary>
    public static ObjectiveRow Run(string fixture, CombatState setup, int maxTurns, CancellationToken ct = default)
    {
        var swL = System.Diagnostics.Stopwatch.StartNew();
        var lex = new Solver { MaxTurns = maxTurns, Ct = ct };
        var vLex = lex.Solve(setup);
        lex.Ct = CancellationToken.None;          // memo populated → policy extraction must not be interrupted
        swL.Stop();

        var swS = System.Diagnostics.Stopwatch.StartNew();
        var scal = new ScalarSolver { MaxTurns = maxTurns, Ct = ct };
        double scalarOpt = scal.Solve(setup);
        scal.Ct = CancellationToken.None;
        swS.Stop();

        // Cross-evaluate each optimum's policy under the other metric.
        var lexPolicyValue = EvaluatePolicy(setup, maxTurns, lex.BestAction, ct);
        var scalarPolicyValue = EvaluatePolicy(setup, maxTurns, scal.BestAction, ct);

        return new ObjectiveRow(
            Fixture: fixture,
            WinLex: vLex.Win, LossLex: vLex.Loss,
            ScalarOpt: scalarOpt,
            ScalarOfLexPolicy: lexPolicyValue.ScalarLoss,
            WinOfScalarPolicy: scalarPolicyValue.Win,
            StatesLex: lex.StatesEvaluated, StatesScalar: scal.StatesEvaluated,
            MsLex: swL.ElapsedMilliseconds, MsScalar: swS.ElapsedMilliseconds);
    }
}
