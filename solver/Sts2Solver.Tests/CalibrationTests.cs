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

    // The fixed seed for the random-deck convergence Theory. Same seed ⇒ same decks ⇒ reproducible test.
    private const int RandomSeed = 12345;
    private const int RandomCount = 6;

    public static IEnumerable<object[]> Fixtures =>
        CalibrationFixtures.All.Select(f => new object[] { f.Name });
    public static IEnumerable<object[]> EliteFixtures =>
        CalibrationFixtures.EliteSweep.Select(f => new object[] { f.Name });
    public static IEnumerable<object[]> RandomFixtures =>
        CalibrationFixtures.RandomDecks(RandomCount, RandomSeed).Select(f => new object[] { f.Name });

    // Tolerances: calibration measured the default mcts-roll within Δsurv ≤ ~4.5% and Δloss ≤ ~0.2 of exact
    // across the suite (the binding case is the low-survival block fight); these leave headroom for seed
    // variance. Survival is allowed to UNDER-shoot more generously than over-shoot — MCTS is known to
    // underestimate razor-thin survival, which is the documented, acceptable residual (loss stays tight).
    private const double SurvOverTol = 0.06;
    private const double SurvUnderTol = 0.12;
    private const double LossTol = 2.5;

    // The six tuned archetype fixtures (starter / power / debuff / block / aggro / engine).
    [Theory]
    [MemberData(nameof(Fixtures))]
    public void MctsRoll_Tracks_Exact(string name)
        => AssertRollTracksExact(CalibrationFixtures.All.First(x => x.Name == name));

    // The single-monster elite sweep (TerrorEel stun / SoulNexus life-drain / MechaKnight windup-burst) — new
    // monster-AI diversity the archetype suite doesn't reach, each carrying an exact ground-truth label.
    [Theory]
    [MemberData(nameof(EliteFixtures))]
    public void MctsRoll_Tracks_Exact_On_Elite(string name)
        => AssertRollTracksExact(CalibrationFixtures.EliteSweep.First(x => x.Name == name));

    // Deterministic random decks (seeded) drawn from the calibration-safe pool — decks the heuristic was NEVER
    // tuned on, so a match-to-exact here is the strongest anti-overfit evidence available short of a full sweep.
    [Theory]
    [MemberData(nameof(RandomFixtures))]
    public void MctsRoll_Tracks_Exact_On_Random_Decks(string name)
        => AssertRollTracksExact(CalibrationFixtures.RandomDecks(RandomCount, RandomSeed).First(x => x.Name == name));

    /// <summary>Shared gate: the default sampling engine (heuristic-guided MCTS, faithful λ-rollout leaf) must
    /// track the exact oracle's survival (within an asymmetric band) and expected HP loss on a fixture.</summary>
    private void AssertRollTracksExact(CalibrationFixtures.Fixture f)
    {
        // Budget is generous: these fixtures solve exactly in seconds — the cap only guards against a fixture
        // growing too big. It's a WALL-CLOCK budget, so under heavy parallel-test CPU contention a too-tight
        // cap produces false null timeouts; 240s leaves ample headroom while still catching a genuinely-grown fixture.
        var exact = CalibrationHarness.RunExactBudgeted(f.Setup(), f.MaxTurns, budgetSeconds: 240);
        Assert.NotNull(exact); // fixtures are tuned to be exact-solvable; if this trips, the fixture grew too big
        var roll = CalibrationHarness.RunMcts(f.Setup(), f.MaxTurns, trials: 40_000, heuristicLeaf: false, seed: 1);

        double dSurv = roll.Survival - exact!.Survival;   // signed: negative = underestimate
        _out.WriteLine($"{f.Name}");
        _out.WriteLine($"  exact     : win {exact.Survival:P1}, loss {exact.Loss:F1}  ({exact.Work:N0} st, {exact.Ms} ms)");
        _out.WriteLine($"  mcts-roll : win {roll.Survival:P1}, loss {roll.Loss:F1}  (Δsurv {dSurv:+0.0%;-0.0%}, Δloss {Math.Abs(roll.Loss - exact.Loss):F1})");

        Assert.True(dSurv <= SurvOverTol,
            $"{f.Name}: mcts-roll survival {roll.Survival:P1} over-shoots exact {exact.Survival:P1} (tol +{SurvOverTol:P0})");
        Assert.True(dSurv >= -SurvUnderTol,
            $"{f.Name}: mcts-roll survival {roll.Survival:P1} under-shoots exact {exact.Survival:P1} (tol -{SurvUnderTol:P0})");
        Assert.True(Math.Abs(roll.Loss - exact.Loss) <= LossTol,
            $"{f.Name}: mcts-roll loss {roll.Loss:F1} too far from exact {exact.Loss:F1} (tol {LossTol})");
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
