using Sts2Solver.Content;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Soundness gates for the survival-vs-scalar objective experiment (<see cref="ScalarSolver"/> /
/// <see cref="ObjectiveExperiment"/>). The experiment's conclusions are only trustworthy if the apparatus is:
///   (1) the fixed-policy evaluator reproduces each solver's OWN optimum for its OWN metric (so cross-metric
///       numbers are apples-to-apples with the oracle), and
///   (2) the death-penalty spectrum actually spans from "maximise expected final HP" (P=0) to the
///       lexicographic survival-first limit (P→∞) — i.e. a huge penalty recovers the oracle's survival exactly.
/// These guard against the experiment silently measuring two subtly different accountings.
/// </summary>
public class ObjectiveExperimentTests
{
    private readonly ITestOutputHelper _out;
    public ObjectiveExperimentTests(ITestOutputHelper o) => _out = o;

    private static CalibrationFixtures.Fixture Fx(string name) =>
        CalibrationFixtures.All.First(f => f.Name == name);

    // Cheap fixtures (small exact trees) for the per-policy consistency checks.
    [Theory]
    [InlineData("power/Inflame-vs-DampCultist")]
    [InlineData("debuff/Uppercut-vs-Byrdonis")]
    [InlineData("aggro/Draw-vs-CorpseSlug")]
    [InlineData("engine/DemonForm-vs-Effigy")]
    public void PolicyEval_Reproduces_Each_Solvers_Own_Optimum(string name)
    {
        var f = Fx(name);

        // Lex oracle, then evaluate ITS policy under our evaluator: Win and LexLoss must match the oracle.
        var lex = new Solver { MaxTurns = f.MaxTurns };
        var vLex = lex.Solve(f.Setup());
        lex.Ct = System.Threading.CancellationToken.None;
        var lexEval = ObjectiveExperiment.EvaluatePolicy(f.Setup(), f.MaxTurns, lex.BestAction);
        Assert.Equal(vLex.Win, lexEval.Win, 6);
        Assert.Equal(vLex.Loss, lexEval.LexLoss, 6);

        // Scalar optimum (P=0), then evaluate ITS policy: the scalar loss must match the scalar optimum.
        var scal = new ScalarSolver { MaxTurns = f.MaxTurns };
        double scalarOpt = scal.Solve(f.Setup());
        var scalEval = ObjectiveExperiment.EvaluatePolicy(f.Setup(), f.MaxTurns, scal.BestAction);
        Assert.Equal(scalarOpt, scalEval.ScalarLoss, 6);

        _out.WriteLine($"{name}: lexWin {vLex.Win:P2} lexLoss {vLex.Loss:F3} | scalarOpt {scalarOpt:F3}");
    }

    // On a fully-winnable fixture (100% survival) there is no survival to trade, so the scalar policy must
    // also survive 100% — the objectives provably coincide.
    [Fact]
    public void Scalar_Matches_Survival_When_Fully_Winnable()
    {
        var f = Fx("aggro/Draw-vs-CorpseSlug");
        var lex = new Solver { MaxTurns = f.MaxTurns };
        var vLex = lex.Solve(f.Setup());
        Assert.True(vLex.Win > 1 - 1e-9, "fixture is meant to be fully winnable");

        var scal = new ScalarSolver { MaxTurns = f.MaxTurns };
        scal.Solve(f.Setup());
        var pv = ObjectiveExperiment.EvaluatePolicy(f.Setup(), f.MaxTurns, scal.BestAction);
        Assert.Equal(vLex.Win, pv.Win, 6);
    }

    // THE KEY CLAIM: the death-penalty spectrum reaches the lexicographic limit. On the one genuine
    // partial-survival calibration fixture (block/Byrdonis, ≈22%), a huge penalty must make the scalar policy
    // survive EXACTLY as well as the survival-first oracle — confirming lexicographic is the P→∞ limit of the
    // single-scalar objective (a constrained-MDP / big-M penalty). At P=0 the scalar policy may survive less
    // (it trades survival for HP); the test asserts the limit closes that gap, and reports the gap as evidence.
    [Fact]
    public void Huge_Death_Penalty_Recovers_Lexicographic_Survival()
    {
        var f = Fx("block/Defends-vs-Byrdonis");
        var lex = new Solver { MaxTurns = f.MaxTurns };
        var vLex = lex.Solve(f.Setup());
        Assert.InRange(vLex.Win, 0.01, 0.99);   // must be a genuine partial-survival fight

        int maxHp = f.Setup().Player.MaxHp;

        var scal0 = new ScalarSolver { MaxTurns = f.MaxTurns, DeathPenalty = 0 };
        scal0.Solve(f.Setup());
        var pv0 = ObjectiveExperiment.EvaluatePolicy(f.Setup(), f.MaxTurns, scal0.BestAction);

        var scalBig = new ScalarSolver { MaxTurns = f.MaxTurns, DeathPenalty = 1e6 * maxHp };
        scalBig.Solve(f.Setup());
        var pvBig = ObjectiveExperiment.EvaluatePolicy(f.Setup(), f.MaxTurns, scalBig.BestAction);

        _out.WriteLine($"lex survive {vLex.Win:P2}; scalar P=0 survive {pv0.Win:P2}; scalar P=∞ survive {pvBig.Win:P2}");
        Assert.Equal(vLex.Win, pvBig.Win, 6);            // P→∞ recovers the survival-first optimum exactly
        Assert.True(pv0.Win <= vLex.Win + 1e-6);          // P=0 never out-survives lexicographic
    }
}
