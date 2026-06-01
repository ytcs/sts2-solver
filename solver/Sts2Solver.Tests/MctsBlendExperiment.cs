using Sts2Solver.Content;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Measurement (not a tight gate): how a BLENDED MCTS leaf — convex mix of the faithful rollout and the
/// Phase-C learned value — tracks the exact oracle's SURVIVAL across the calibration suite, swept over the
/// blend weight α. The rollout (α=0) under-estimates razor-thin survival; the learned leaf over-estimates it;
/// the claim is that some intermediate α has lower survival MAE than pure rollout. Deterministic (seeded MCTS).
/// </summary>
public class MctsBlendExperiment
{
    private readonly ITestOutputHelper _out;
    public MctsBlendExperiment(ITestOutputHelper o) => _out = o;

    // Manual measurement tool (slow: full MCTS × several α over the suite), not a CI gate. Run explicitly:
    //   dotnet test --filter "FullyQualifiedName~MctsBlendExperiment"  (remove Skip first, or use a runsettings).
    // Latest (current calibration suite): pure rollout α=0 MAE 0.0075; best blend α=0.25 MAE 0.0027 (block/
    // Byrdonis 18%→24% vs exact 22%); learned 0.0268. Re-tune α after the calibration suite is widened.
    [Fact(Skip = "manual α-sweep measurement tool; not a CI gate (slow)")]
    public void Sweep_Blend_Vs_Exact_Survival()
    {
        double[] blends = { 0.0, 0.25, 0.5, 0.75 };
        var err = new double[blends.Length];
        double learnErr = 0;
        int n = 0;

        foreach (var f in CalibrationFixtures.All)
        {
            var exact = CalibrationHarness.RunExactBudgeted(f.Setup(), f.MaxTurns, budgetSeconds: 120);
            if (exact == null) { _out.WriteLine($"{f.Name}: exact timed out, skipped"); continue; }
            n++;
            var line = $"{f.Name,-34} exact {exact.Survival:P1} | ";
            for (int i = 0; i < blends.Length; i++)
            {
                var r = CalibrationHarness.RunMctsBlend(f.Setup(), f.MaxTurns, trials: 40_000, blend: blends[i], seed: 1);
                err[i] += System.Math.Abs(r.Survival - exact.Survival);
                line += $"b{blends[i]:0.00} {r.Survival:P0}  ";
            }
            var learn = CalibrationHarness.RunMctsLearned(f.Setup(), f.MaxTurns, trials: 40_000, seed: 1);
            learnErr += System.Math.Abs(learn.Survival - exact.Survival);
            line += $"learn {learn.Survival:P0}";
            _out.WriteLine(line);
        }

        Assert.True(n > 0);
        _out.WriteLine("");
        for (int i = 0; i < blends.Length; i++)
            _out.WriteLine($"blend α={blends[i]:0.00}: survival MAE {err[i] / n:F4}");
        _out.WriteLine($"pure learned     : survival MAE {learnErr / n:F4}");

        double rollMae = err[0] / n;
        double bestBlendMae = double.MaxValue; double bestA = 0;
        for (int i = 1; i < blends.Length; i++) if (err[i] / n < bestBlendMae) { bestBlendMae = err[i] / n; bestA = blends[i]; }
        _out.WriteLine($"\npure rollout (α=0) MAE {rollMae:F4}; best blend α={bestA:0.00} MAE {bestBlendMae:F4}");
    }
}
