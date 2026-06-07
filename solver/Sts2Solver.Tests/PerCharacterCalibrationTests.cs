using System.Collections.Generic;
using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Regression gate that heuristic-guided MCTS tracks the exact oracle on the PER-CHARACTER fixtures (Osty /
/// poison / orbs / stars mechanics), not just the Ironclad-tuned suite — so the per-character heuristic redesign
/// can't silently break value convergence for a class. Note this gates the SOLVER VALUE (which the tree corrects
/// even from a biased leaf, given enough trials); the LEAF policy's own quality is measured separately by
/// <c>CalibrationHarness.MeasurePolicyRegret</c> (CLI <c>--calibrate --percharacter --regret</c>), and tracked in
/// <c>docs/phase0-baseline.md</c>. See <c>docs/per-character-heuristic-research.md</c>.
/// </summary>
public class PerCharacterCalibrationTests
{
    private readonly ITestOutputHelper _out;
    public PerCharacterCalibrationTests(ITestOutputHelper o) => _out = o;

    // Same asymmetric tolerances as the Ironclad CalibrationTests (MCTS may under-shoot razor-thin survival more
    // than it over-shoots; loss stays tight).
    private const double SurvOverTol = 0.06;
    private const double SurvUnderTol = 0.12;
    private const double LossTol = 2.5;

    public static IEnumerable<object[]> Fixtures =>
        CalibrationFixtures.PerCharacter.Select(f => new object[] { f.Name });

    [Theory]
    [MemberData(nameof(Fixtures))]
    public void MctsValue_Tracks_Exact_PerCharacter(string name)
    {
        var f = CalibrationFixtures.PerCharacter.First(x => x.Name == name);
        var exact = CalibrationHarness.RunExactBudgeted(f.Setup(), f.MaxTurns, budgetSeconds: 240);
        Assert.NotNull(exact);   // fixtures are tuned exact-solvable; if this trips, the fixture grew too big
        var roll = CalibrationHarness.RunMcts(f.Setup(), f.MaxTurns, trials: 40_000, seed: 1);

        double dSurv = roll.Survival - exact!.Survival;
        _out.WriteLine($"{f.Name}");
        _out.WriteLine($"  exact     : win {exact.Survival:P1}, loss {exact.Loss:F1}  ({exact.Work:N0} st, {exact.Ms} ms)");
        _out.WriteLine($"  mcts-roll : win {roll.Survival:P1}, loss {roll.Loss:F1}  (Δsurv {dSurv:+0.0%;-0.0%}, Δloss {Math.Abs(roll.Loss - exact.Loss):F1})");

        Assert.True(dSurv <= SurvOverTol,
            $"{f.Name}: mcts survival {roll.Survival:P1} over-shoots exact {exact.Survival:P1} (tol +{SurvOverTol:P0})");
        Assert.True(dSurv >= -SurvUnderTol,
            $"{f.Name}: mcts survival {roll.Survival:P1} under-shoots exact {exact.Survival:P1} (tol -{SurvUnderTol:P0})");
        Assert.True(Math.Abs(roll.Loss - exact.Loss) <= LossTol,
            $"{f.Name}: mcts loss {roll.Loss:F1} too far from exact {exact.Loss:F1} (tol {LossTol})");
    }
}
