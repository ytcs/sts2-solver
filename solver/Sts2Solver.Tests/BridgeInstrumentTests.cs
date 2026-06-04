using Sts2Solver.Content;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Regression gate for the bridge instrument's two premises — the foundation the advisor's accuracy/speed
/// tuning rests on:
///   1. The 2k-trial ADVICE estimate tracks the exact oracle at the bridge ladder's exact-anchorable bottom
///      rung (so "2k MCTS is truthful" holds at the tractability boundary, not just on the tiny calibration
///      fixtures).
///   2. The survival estimate is SEED-STABLE at the advice budget — its seed-to-seed stddev is far below the
///      advisor's <c>SurvivalBand</c> — across winnable, contested, AND large-contested fights. This is the
///      empirical basis for the band: it must cover the convergence BIAS, not the (negligible) seed noise.
/// Kept deliberately cheap (small/short fixtures, few seeds) so it fits the suite.
/// </summary>
public class BridgeInstrumentTests
{
    private readonly ITestOutputHelper _out;
    public BridgeInstrumentTests(ITestOutputHelper o) => _out = o;

    private static readonly int[] FourSeeds = { 1, 2, 3, 4 };

    /// <summary>Premise 1: at the exact-anchorable bottom rung, the 2k advice estimate matches exact within a
    /// small band (the bridge family's truth-anchor; cf. CalibrationTests on the tiny fixtures).</summary>
    [Fact]
    public void Bridge_BottomRung_Advice_Tracks_Exact()
    {
        var f = CalibrationFixtures.BridgeRung(6);
        var exact = CalibrationHarness.RunExactBudgeted(f.Setup(), f.MaxTurns, budgetSeconds: 120);
        Assert.NotNull(exact);   // S06 is a short race — exact must solve it (the anchor); if not, it grew
        var ss = CalibrationHarness.RunMctsSeeds(() => f.Setup(), f.MaxTurns, trials: 2_000, FourSeeds);

        _out.WriteLine($"{f.Name}: exact {exact!.Survival:P1}/{exact.Loss:F1}  "
            + $"m@2k {ss.SurvMean:P1}±{ss.SurvStd:P2}/{ss.LossMean:F1}");
        Assert.True(Math.Abs(ss.SurvMean - exact.Survival) <= 0.06,
            $"bridge S06 advice survival {ss.SurvMean:P1} too far from exact {exact.Survival:P1}");
        Assert.True(Math.Abs(ss.LossMean - exact.Loss) <= 2.5,
            $"bridge S06 advice loss {ss.LossMean:F1} too far from exact {exact.Loss:F1}");
    }

    public static IEnumerable<object[]> NoiseProbes()
    {
        // name, fixture builder — a winnable elite, the contested ~92% block fight, and a LARGE contested fight.
        yield return new object[] { "block-92%", (Func<CalibrationFixtures.Fixture>)
            (() => CalibrationFixtures.All.First(f => f.Archetype == "block")) };
        yield return new object[] { "MechaKnight", (Func<CalibrationFixtures.Fixture>)
            (() => CalibrationFixtures.EliteSweep.First(f => f.Name.Contains("MechaKnight"))) };
        yield return new object[] { "contested-large", (Func<CalibrationFixtures.Fixture>)
            (() => CalibrationFixtures.BridgeContestedLarge()) };
    }

    /// <summary>Premise 2: the advice-budget survival estimate is seed-stable — its stddev across seeds is well
    /// under the current SurvivalBand (0.05), so the band is dominated by convergence bias, not noise. Holds on
    /// the contested ~92% fight and a large contested fight, the cases where noise would be worst.</summary>
    [Theory]
    [MemberData(nameof(NoiseProbes))]
    public void Bridge_Survival_Is_Seed_Stable(string label, Func<CalibrationFixtures.Fixture> build)
    {
        var f = build();
        var ss = CalibrationHarness.RunMctsSeeds(() => f.Setup(), f.MaxTurns, trials: 2_000, FourSeeds);
        _out.WriteLine($"{label} ({f.Name}): survival {ss.SurvMean:P1} ± {ss.SurvStd:P2} "
            + $"[{ss.SurvMin:P1}…{ss.SurvMax:P1}] over {ss.Seeds} seeds");

        // Observed noise floor is ~0% (DP-UCT backs up true probabilities); 0.03 leaves headroom for the rare
        // sampled-chance jitter while still asserting noise ≪ the 0.05 SurvivalBand it would otherwise justify.
        Assert.True(ss.SurvStd <= 0.03,
            $"{label}: survival seed-stddev {ss.SurvStd:P2} exceeds the noise-floor bound (3%) — "
            + "the advice estimate is noisier than assumed; SurvivalBand reasoning would need revisiting");
    }
}
