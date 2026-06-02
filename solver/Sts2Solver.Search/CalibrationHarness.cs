using System.Diagnostics;
using Sts2Solver.Engine;

namespace Sts2Solver.Search;

/// <summary>One engine's verdict on a fixture: survival, expected HP loss, work done, wall-clock.</summary>
public sealed record EngineResult(string Label, double Survival, double Loss, long Work, long Ms);

/// <summary>
/// Calibration utilities: run a fixture through the exact oracle (ground truth) and through MCTS variants,
/// so we can measure how closely heuristic-guided sampling tracks exact. The caller (CLI / tests) supplies
/// pre-built <see cref="CombatState"/> setups (Search can't see Content's Catalog) and compares the results.
/// </summary>
public static class CalibrationHarness
{
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

    /// <summary>MCTS value under a chosen leaf mode (greedy rollout vs static heuristic leaf).</summary>
    public static EngineResult RunMcts(CombatState setup, int maxTurns, int trials, bool heuristicLeaf, int seed, string? label = null)
    {
        var sw = Stopwatch.StartNew();
        var mcts = new MctsSolver(new MctsOptions
        {
            Trials = trials,
            MaxTurns = maxTurns,
            Seed = seed,
            UseHeuristicLeaf = heuristicLeaf,
        });
        var v = mcts.Solve(setup);
        return new EngineResult(label ?? (heuristicLeaf ? "mcts-heur" : "mcts-roll"), v.Win, v.Loss, mcts.TrialsRun, sw.ElapsedMilliseconds);
    }

    /// <summary>MCTS value using the Phase-C learned value function as the leaf (<see cref="LearnedValue"/>).
    /// Compared against the static-heuristic leaf (<see cref="RunMcts"/> with <c>heuristicLeaf:true</c>) — the
    /// baseline it is meant to beat — and against the exact oracle.</summary>
    public static EngineResult RunMctsLearned(CombatState setup, int maxTurns, int trials, int seed, string? label = null)
    {
        var sw = Stopwatch.StartNew();
        var mcts = new MctsSolver(new MctsOptions
        {
            Trials = trials,
            MaxTurns = maxTurns,
            Seed = seed,
            UseLearnedLeaf = true,
        });
        var v = mcts.Solve(setup);
        return new EngineResult(label ?? "mcts-learn", v.Win, v.Loss, mcts.TrialsRun, sw.ElapsedMilliseconds);
    }
}
