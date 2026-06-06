using System.Diagnostics;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Gates the <see cref="EncounterEvaluator"/> tractability gate: a real (large) advice-regime deck must SKIP
/// the exact attempt and go straight to MCTS — without burning the exact budget — while a small,
/// exact-solvable fight still takes the exact path. This is the advisor's speed lever (hundreds of evaluations
/// per removal/pick run), so a regression that re-enabled the doomed exact attempt would silently 3–4× the cost.
/// </summary>
public class EncounterEvaluatorTests
{
    private readonly ITestOutputHelper _out;
    public EncounterEvaluatorTests(ITestOutputHelper o) => _out = o;

    private static CombatState BigDeck(int n)
    {
        var specs = new List<CardModel>();
        for (int i = 0; i < n; i++) specs.Add(Catalog.BuildCard(i % 3 == 2 ? "Bash" : i % 3 == 0 ? "StrikeIronclad" : "DefendIronclad"));
        var player = Catalog.BuildPlayer(specs, 70, 80, 3, new[] { "BurningBlood" });
        return Catalog.SetupCombat(player, new[] { Monsters.Byrdonis(hp: 80) });
    }

    /// <summary>A 24-card deck (&gt; the ExactMaxDrawPile gate) must come back from MCTS quickly even with a huge
    /// exact budget — proving exact was skipped outright, not merely timed out (that would cost the full budget).</summary>
    [Fact]
    public void Gate_Skips_Exact_On_Large_Deck()
    {
        var setup = BigDeck(24);
        var sw = Stopwatch.StartNew();
        var stats = EncounterEvaluator.Evaluate(setup, new EvalOptions
        {
            MaxTurns = 14,
            BudgetSeconds = 30,   // if exact were attempted it would burn all 30s before falling to MCTS
            MctsTrials = 400,
        });
        sw.Stop();
        _out.WriteLine($"large deck: engine={stats.Engine}, {sw.ElapsedMilliseconds} ms (budget 30s)");
        Assert.Equal(EvalEngine.Mcts, stats.Engine);
        Assert.True(sw.ElapsedMilliseconds < 20_000,
            $"took {sw.ElapsedMilliseconds} ms — exact was attempted (not gated out) and burned the budget");
    }

    /// <summary>A small, exact-solvable fight (≤ the gate) still takes the exact path — the gate must not have
    /// thrown away exact accuracy for the cases where it's cheap and exact.</summary>
    [Fact]
    public void Gate_Keeps_Exact_On_Small_Deck()
    {
        var f = CalibrationFixtures.BridgeRung(6);   // 6-card short race — exact solves in a few seconds
        var stats = EncounterEvaluator.Evaluate(f.Setup(), new EvalOptions
        {
            MaxTurns = f.MaxTurns, BudgetSeconds = 60,
        });
        _out.WriteLine($"small deck: engine={stats.Engine}, survival {stats.Survival:P1}");
        Assert.Equal(EvalEngine.Exact, stats.Engine);
    }
}
