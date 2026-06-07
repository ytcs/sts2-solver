using System.Diagnostics;
using Sts2Solver.Engine;

namespace Sts2Solver.Search;

/// <summary>One engine's verdict on a fixture: survival, expected HP loss, work done, wall-clock.</summary>
public sealed record EngineResult(string Label, double Survival, double Loss, long Work, long Ms);

/// <summary>An estimator's spread over repeated runs at a fixed trial budget: the mean and sample stddev of
/// survival and expected HP loss across <see cref="Seeds"/> seeds, plus the survival min/max and the mean
/// per-solve wall-clock. The survival stddev is the *noise floor* of the estimate at this budget — the
/// empirical basis for the advisor's <c>SurvivalBand</c> (two decks closer than the noise floor can't be
/// ranked on survival; HP loss must decide).</summary>
public sealed record SeedStats(
    string Label, int Seeds,
    double SurvMean, double SurvStd, double SurvMin, double SurvMax,
    double LossMean, double LossStd, double MsMean);

/// <summary>The greedy leaf policy's single-turn REGRET vs the exact oracle, averaged (probability-weighted)
/// over the fixture's opening decision states. <see cref="WinRegret"/> = optimal win-prob − greedy win-prob
/// (≥0; the dominant term — a positive value means the heuristic's turn loses winnable lines). <see cref="LossRegret"/>
/// = greedy E[HP loss] − optimal E[HP loss] (≥0; the tie-breaker, meaningful when win-regret is ~0). Both are 0
/// iff the greedy turn plays as well as the oracle. This is the heuristic-quality signal the per-character
/// redesign drives down; it isolates the LEAF policy (the exact side plays the real engine, so any gap is the
/// heuristic's, not the search's).</summary>
public sealed record PolicyRegret(string Label, double WinRegret, double LossRegret, double OptWin, double GreedyWin, double OptLoss, double GreedyLoss);

/// <summary>
/// Calibration utilities: run a fixture through the exact oracle (ground truth) and through MCTS variants,
/// so we can measure how closely heuristic-guided sampling tracks exact. The caller (CLI / tests) supplies
/// pre-built <see cref="CombatState"/> setups (Search can't see Content's Catalog) and compares the results.
/// </summary>
public static class CalibrationHarness
{
    /// <summary>Measure the greedy leaf policy's single-turn regret vs the exact oracle on a fixture (see
    /// <see cref="PolicyRegret"/>). Solves the fixture exactly under a wall-clock budget (returns <c>null</c> on
    /// timeout — the fixture is too big to label), then at each opening decision state compares the exact
    /// optimal value to the value of letting the greedy λ-policy take that whole turn. Draw-free fixtures only
    /// (the greedy turn must be deterministic). <paramref name="aggression"/> is the rollout λ (default 0.5,
    /// matching the production leaf).</summary>
    public static PolicyRegret? MeasurePolicyRegret(
        CombatState setup, int maxTurns, double budgetSeconds, double aggression = 0.5)
    {
        var solver = new Solver { MaxTurns = maxTurns };
        using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(budgetSeconds));
        solver.Ct = cts.Token;
        try { solver.Solve(setup); }                       // populate the memo (ground truth)
        catch (OperationCanceledException) { return null; }
        solver.Ct = CancellationToken.None;                // memo populated → allow on-demand extension w/o cancel

        double wReg = 0, lReg = 0, oWin = 0, gWin = 0, oLoss = 0, gLoss = 0, pSum = 0;
        foreach (var (prob, s) in solver.OpeningStates(setup, Player.CardsDrawnPerTurn))
        {
            var opt = solver.StateValue(s);                // exact optimal from this opening state
            var greedy = solver.GreedyTurnValue(s, aggression);   // greedy takes the turn, exact continues
            wReg += prob * Math.Max(0, opt.Win - greedy.Win);
            lReg += prob * Math.Max(0, greedy.Loss - opt.Loss);
            oWin += prob * opt.Win;  gWin += prob * greedy.Win;
            oLoss += prob * opt.Loss; gLoss += prob * greedy.Loss;
            pSum += prob;
        }
        if (pSum <= 0) return null;
        return new PolicyRegret("regret", wReg / pSum, lReg / pSum, oWin / pSum, gWin / pSum, oLoss / pSum, gLoss / pSum);
    }

    /// <summary>Exact expectimax — the ground-truth value. May be slow; only use on fixtures small enough.</summary>
    public static EngineResult RunExact(CombatState setup, int maxTurns)
    {
        var sw = Stopwatch.StartNew();
        var solver = new Solver { MaxTurns = maxTurns };
        var v = solver.Solve(setup);
        return new EngineResult("exact", v.Win, v.Loss, solver.StatesEvaluated, sw.ElapsedMilliseconds);
    }

    /// <summary>Exact expectimax under a wall-clock budget. Returns <c>null</c> if the fixture is too large
    /// to solve within <paramref name="budgetSeconds"/> (so a calibration run isn't held hostage by one
    /// intractable fight); otherwise the ground-truth value.</summary>
    public static EngineResult? RunExactBudgeted(CombatState setup, int maxTurns, double budgetSeconds)
    {
        var sw = Stopwatch.StartNew();
        var solver = new Solver { MaxTurns = maxTurns };
        using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(budgetSeconds));
        solver.Ct = cts.Token;
        try
        {
            var v = solver.Solve(setup);
            return new EngineResult("exact", v.Win, v.Loss, solver.StatesEvaluated, sw.ElapsedMilliseconds);
        }
        catch (OperationCanceledException)
        {
            return null;
        }
    }

    /// <summary>MCTS value (faithful greedy-rollout leaf — the only leaf).</summary>
    public static EngineResult RunMcts(CombatState setup, int maxTurns, int trials, int seed, string? label = null)
    {
        var sw = Stopwatch.StartNew();
        var mcts = new MctsSolver(new MctsOptions { Trials = trials, MaxTurns = maxTurns, Seed = seed });
        var v = mcts.Solve(setup);
        return new EngineResult(label ?? "mcts", v.Win, v.Loss, mcts.TrialsRun, sw.ElapsedMilliseconds);
    }

    /// <summary>Run MCTS at a fixed trial budget across several seeds and summarise the spread. Each seed gets
    /// a FRESH setup from <paramref name="build"/> (combat states are mutated by search). Exposes the
    /// estimator's noise floor at <paramref name="trials"/> — used by the bridge instrument to size the
    /// advisor's survival band and to check the per-seed estimate is stable enough to rank decks.</summary>
    public static SeedStats RunMctsSeeds(
        Func<CombatState> build, int maxTurns, int trials,
        IReadOnlyList<int> seeds, string? label = null)
    {
        if (seeds.Count == 0) throw new ArgumentException("Need at least one seed.", nameof(seeds));
        var surv = new double[seeds.Count];
        var loss = new double[seeds.Count];
        long msSum = 0;
        for (int i = 0; i < seeds.Count; i++)
        {
            var r = RunMcts(build(), maxTurns, trials, seeds[i]);
            surv[i] = r.Survival;
            loss[i] = r.Loss;
            msSum += r.Ms;
        }
        return new SeedStats(
            Label: label ?? "mcts",
            Seeds: seeds.Count,
            SurvMean: surv.Average(), SurvStd: Std(surv), SurvMin: surv.Min(), SurvMax: surv.Max(),
            LossMean: loss.Average(), LossStd: Std(loss),
            MsMean: (double)msSum / seeds.Count);
    }

    /// <summary>Sample (n−1) stddev; 0 for a single sample.</summary>
    private static double Std(IReadOnlyList<double> xs)
    {
        if (xs.Count < 2) return 0;
        double mean = xs.Average();
        double ss = 0;
        foreach (var x in xs) ss += (x - mean) * (x - mean);
        return Math.Sqrt(ss / (xs.Count - 1));
    }

}
