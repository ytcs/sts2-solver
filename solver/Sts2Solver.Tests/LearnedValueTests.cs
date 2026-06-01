using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Tests for the Phase-C learned value function (<see cref="LearnedValue"/>): feature-extraction sanity
/// (always valid, regardless of the trained weights) and the calibration gate that the learned leaf at least
/// matches — and ideally beats — the static <see cref="CombatHeuristic.Evaluate"/> leaf against the exact
/// oracle, which is the documented goal of the phase.
/// </summary>
public class LearnedValueTests
{
    private readonly ITestOutputHelper _out;
    public LearnedValueTests(ITestOutputHelper o) => _out = o;

    private static CombatState Decision(int playerHp, int monsterHp)
    {
        var player = Catalog.BuildPlayer(Catalog.IroncladStarterDeck(), playerHp, 80, relics: new[] { "BurningBlood" });
        var setup = Catalog.SetupCombat(player, new[] { Monsters.Byrdonis(hp: monsterHp) });
        // Advance to a real decision state (roll opening move + draw 5).
        var rng = new Rng(7);
        CombatManager.RollInitialMoves(setup, rng);
        CombatManager.BeginPlayerTurn(setup);
        CombatManager.DrawCards(setup, Player.CardsDrawnPerTurn, rng);
        return setup;
    }

    [Fact]
    public void Features_Are_Finite_And_Sized()
    {
        foreach (var (php, mhp) in new[] { (80, 48), (12, 90), (60, 30), (5, 200) })
        {
            var f = LearnedValue.Features(Decision(php, mhp), maxTurns: 20);
            Assert.Equal(LearnedValue.FeatureCount, f.Length);
            foreach (var x in f) Assert.True(double.IsFinite(x), $"non-finite feature for hp{php}/m{mhp}");
        }
    }

    [Fact]
    public void Evaluate_Returns_Valid_Lexicographic_Value()
    {
        var s = Decision(60, 48);
        var v = LearnedValue.Evaluate(s, maxTurns: 20);
        Assert.InRange(v.Win, 0.0, 1.0);
        Assert.InRange(v.Loss, 0.0, s.Player.MaxHp);
    }

    [Fact]
    public void Features_Are_Deterministic()
    {
        var a = LearnedValue.Features(Decision(50, 60), 18);
        var b = LearnedValue.Features(Decision(50, 60), 18);
        Assert.Equal(a, b);
    }

    /// <summary>The Phase-C deliverable: the learned value function predicts the exact survival probability
    /// of decision states MORE accurately than the static <see cref="CombatHeuristic.Evaluate"/> baseline
    /// (feature 0). Measured as mean-abs-error over a large sample of exactly-labelled states drawn from
    /// HELD-OUT fixtures (HP/enemy combos outside the training grid), so this is genuine generalisation.</summary>
    [Fact]
    public void LearnedLeaf_Predicts_Survival_Better_Than_Static_Baseline()
    {
        CombatState ByrdonisFight(int php, int mhp) =>
            Catalog.SetupCombat(
                Catalog.BuildPlayer(BlockyDeck(), php, php, 3, new[] { "BurningBlood" }),
                new[] { Monsters.Byrdonis(hp: mhp) });

        // Held out from TrainingFixtures (enemy HP ∈ {40,70}, player HP ∈ {38,60}); TerrorEel is the
        // razor-thin out-of-sample case from the project notes.
        var fixtures = new (CombatState, int)[]
        {
            (ByrdonisFight(45, 64), 16),
            (ByrdonisFight(55, 52), 16),
            (Catalog.SetupCombat(Catalog.BuildPlayer(BlockyDeck(), 50, 50, 3, new[] { "BurningBlood" }),
                                 new[] { Monsters.TerrorEel(hp: 57) }), 16),
        };

        var examples = VfTrainer.Collect(fixtures, budgetSeconds: 25, sampleRate: 0.3, seed: 99);
        Assert.True(examples.Count > 200, $"too few held-out labels ({examples.Count})");

        double learnedErr = 0, baselineErr = 0;
        foreach (var e in examples)
        {
            double learned = LearnedValue.Predict(e.F, LearnedValue.Weights.WinWeights, LearnedValue.Weights.WinBias, logistic: true);
            double baseline = e.F[0];                       // feature 0 = the static heuristic's survival
            learnedErr += Math.Abs(learned - e.Win);
            baselineErr += Math.Abs(baseline - e.Win);
        }
        learnedErr /= examples.Count;
        baselineErr /= examples.Count;
        _out.WriteLine($"held-out states={examples.Count}  win MAE: learned {learnedErr:F4} vs baseline {baselineErr:F4}");

        Assert.True(learnedErr < baselineErr,
            $"learned VF win MAE {learnedErr:F4} did not beat the static baseline {baselineErr:F4}");
    }

    private static List<CardModel> BlockyDeck() => new()
    {
        new ShrugItOff(), new DefendIronclad(), new DefendIronclad(),
        new DefendIronclad(), new StrikeIronclad(), new StrikeIronclad(),
    };
}
