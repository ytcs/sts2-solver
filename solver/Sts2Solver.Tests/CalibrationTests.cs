using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Regression gate that the *default* sampling engine (heuristic-guided MCTS, faithful λ-rollout leaf)
/// tracks the exact oracle across the DIVERSE fixture suite — starter, Strength/power, debuff, block, aggro
/// and power-engine decks — not just one archetype, so the rollout heuristic can't be tuned to a single
/// style. The fixtures are tuned (compact decks, tight horizon) so exact is solvable in seconds here; the
/// CLI <c>--calibrate</c> mode runs the slower elite sweep for manual tuning.
/// </summary>
public class CalibrationTests
{
    private readonly ITestOutputHelper _out;
    public CalibrationTests(ITestOutputHelper o) => _out = o;

    public static IEnumerable<object[]> Fixtures =>
        CalibrationFixtures.All.Select(f => new object[] { f.Name });

    // Tolerances: calibration measured the default mcts-roll within Δsurv ≤ ~4.5% and Δloss ≤ ~0.2 of exact
    // across the suite (the binding case is the low-survival block fight); these leave headroom for seed
    // variance. Survival is allowed to UNDER-shoot more generously than over-shoot — MCTS is known to
    // underestimate razor-thin survival, which is the documented, acceptable residual (loss stays tight).
    private const double SurvOverTol = 0.06;
    private const double SurvUnderTol = 0.12;
    private const double LossTol = 2.5;

    [Theory]
    [MemberData(nameof(Fixtures))]
    public void MctsRoll_Tracks_Exact(string name)
    {
        var f = CalibrationFixtures.All.First(x => x.Name == name);
        // Budget is generous: these fixtures solve exactly in seconds — the cap only guards against a fixture
        // growing too big. It's a WALL-CLOCK budget, so under heavy parallel-test CPU contention a too-tight
        // cap produces false null timeouts; 240s leaves ample headroom while still catching a genuinely-grown fixture.
        var exact = CalibrationHarness.RunExactBudgeted(f.Setup(), f.MaxTurns, budgetSeconds: 240);
        Assert.NotNull(exact); // fixtures are tuned to be exact-solvable; if this trips, the fixture grew too big
        var roll = CalibrationHarness.RunMcts(f.Setup(), f.MaxTurns, trials: 40_000, heuristicLeaf: false, seed: 1);

        double dSurv = roll.Survival - exact!.Survival;   // signed: negative = underestimate
        _out.WriteLine($"{name}");
        _out.WriteLine($"  exact     : win {exact.Survival:P1}, loss {exact.Loss:F1}  ({exact.Work:N0} st, {exact.Ms} ms)");
        _out.WriteLine($"  mcts-roll : win {roll.Survival:P1}, loss {roll.Loss:F1}  (Δsurv {dSurv:+0.0%;-0.0%}, Δloss {Math.Abs(roll.Loss - exact.Loss):F1})");

        Assert.True(dSurv <= SurvOverTol,
            $"{name}: mcts-roll survival {roll.Survival:P1} over-shoots exact {exact.Survival:P1} (tol +{SurvOverTol:P0})");
        Assert.True(dSurv >= -SurvUnderTol,
            $"{name}: mcts-roll survival {roll.Survival:P1} under-shoots exact {exact.Survival:P1} (tol -{SurvUnderTol:P0})");
        Assert.True(Math.Abs(roll.Loss - exact.Loss) <= LossTol,
            $"{name}: mcts-roll loss {roll.Loss:F1} too far from exact {exact.Loss:F1} (tol {LossTol})");
    }

    /// <summary>The critical play-decision guarantee: MCTS must not report a hard 0% survival on a fight the
    /// search proved winnable — that is the one output that would make a player wrongly skip a beatable elite.
    /// The block fixture (exact ≈ 22%) is a low-but-clearly-winnable fight.</summary>
    [Fact]
    public void Winnable_Fight_Never_Reports_Zero_Survival()
    {
        var f = CalibrationFixtures.All.First(x => x.Archetype == "block");
        var stats = EncounterEvaluator.Evaluate(f.Setup(), new EvalOptions
        {
            MaxTurns = f.MaxTurns,
            BudgetSeconds = 0.0,   // force the MCTS fallback path (skip exact)
            MctsTrials = 40_000,
            Rollouts = 200,
        });
        _out.WriteLine($"{f.Name}: engine={stats.Engine}, survival {stats.Survival:P2}");
        Assert.True(stats.Survival > 0,
            $"winnable fight reported {stats.Survival:P2} survival — would wrongly read as impossible");
    }
}
