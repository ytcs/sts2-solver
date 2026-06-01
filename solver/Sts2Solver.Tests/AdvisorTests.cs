using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Ranwid;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Tests for the deck advice engine (<see cref="Advisor"/>): the lexicographic aggregation is correct and
/// deterministic, and the card-removal ranking respects a guaranteed direction — removing a strictly-harmful
/// card (a <c>Burn</c>, which is unplayable and deals 2 when held at turn end) is never worse than keeping it.
/// Fights are kept tiny so <see cref="EncounterEvaluator"/> takes the exact path (deterministic ground truth).
/// </summary>
public class AdvisorTests
{
    private readonly ITestOutputHelper _out;
    public AdvisorTests(ITestOutputHelper o) => _out = o;

    private static readonly EvalOptions Exact = new() { MaxTurns = 10, BudgetSeconds = 60, MctsTrials = 20_000, Seed = 1 };

    private static List<Advisor.Encounter> OneCultist(int hp) => new()
    {
        new Advisor.Encounter("Cultist", () => new List<Monster> { Monsters.CalcifiedCultist(hp) }),
    };

    private static List<string> Specs(params (int n, string spec)[] parts)
    {
        var list = new List<string>();
        foreach (var (n, spec) in parts) for (int i = 0; i < n; i++) list.Add(spec);
        return list;
    }

    [Fact]
    public void ScoreDeck_Is_Valid_And_Deterministic()
    {
        var deck = Specs((4, "StrikeIronclad"), (3, "DefendIronclad"));
        var a = Advisor.ScoreDeck(deck, OneCultist(28), 40, 40, 3, System.Array.Empty<string>(), Exact);
        var b = Advisor.ScoreDeck(deck, OneCultist(28), 40, 40, 3, System.Array.Empty<string>(), Exact);
        Assert.InRange(a.MinSurvival, 0.0, 1.0);
        Assert.True(a.TotalMeanLoss >= 0);
        Assert.Equal(a.MinSurvival, b.MinSurvival, 6);                 // deterministic (exact path)
        Assert.Equal(a.TotalMeanLoss, b.TotalMeanLoss, 6);
    }

    [Fact]
    public void Removing_A_Harmful_Card_Is_Never_Worse()
    {
        var clean = Specs((4, "StrikeIronclad"), (3, "DefendIronclad"));
        var clogged = new List<string>(clean) { "Burn" };            // a Burn can only hurt (clogs + 2 dmg held)

        var sClean = Advisor.ScoreDeck(clean, OneCultist(30), 38, 38, 3, System.Array.Empty<string>(), Exact);
        var sClogged = Advisor.ScoreDeck(clogged, OneCultist(30), 38, 38, 3, System.Array.Empty<string>(), Exact);
        _out.WriteLine($"clean  : surv {sClean.MinSurvival:P1}, loss {sClean.TotalMeanLoss:F2}");
        _out.WriteLine($"clogged: surv {sClogged.MinSurvival:P1}, loss {sClogged.TotalMeanLoss:F2}");

        // Removing the Burn (clogged -> clean) is a weak improvement: clean is better-or-equal, never worse.
        Assert.False(sClogged.BetterThan(sClean), "a deck with a strictly-harmful Burn scored better than without it");
    }

    [Fact]
    public void RemovalAdvice_Ranks_Best_First_And_Finds_The_Harmful_Card()
    {
        var deck = Specs((4, "StrikeIronclad"), (3, "DefendIronclad")).Append("Burn").ToList();
        var (baseline, items) = Advisor.RemovalAdvice(deck, OneCultist(30), 38, 38, 3, System.Array.Empty<string>(), Exact);

        Assert.Equal(deck.Distinct().Count(), items.Count);            // one item per distinct removable card
        // Sorted best-first: no later item is strictly better than an earlier one.
        for (int i = 1; i < items.Count; i++)
            Assert.False(items[i].After.BetterThan(items[i - 1].After), "removal advice is not sorted best-first");
        // The best removal is never worse than the baseline (removing the Burn can't hurt).
        Assert.False(baseline.BetterThan(items[0].After), "the top removal was worse than keeping the whole deck");
        // And that best removal should be the Burn (the only strictly-dead card here).
        _out.WriteLine($"top removal: {items[0].Card} (Δsurv {items[0].SurvivalDelta:+0.0%;-0.0%}, Δloss {items[0].LossDelta:+0.0;-0.0})");
        Assert.Equal("Burn", items[0].Card);
    }

    [Fact]
    public void PickAdvice_Includes_Skip_And_Ranks_Best_First()
    {
        var deck = Specs((4, "StrikeIronclad"), (3, "DefendIronclad"));
        var (_, ranked) = Advisor.PickAdvice(deck, new[] { "StrikeIronclad", "Burn" }, OneCultist(30),
            38, 38, 3, System.Array.Empty<string>(), Exact);

        Assert.Equal(3, ranked.Count);                                  // 2 candidates + skip
        Assert.Contains(ranked, p => p.IsSkip);
        for (int i = 1; i < ranked.Count; i++)
            Assert.False(ranked[i].Score.BetterThan(ranked[i - 1].Score), "pick advice is not sorted best-first");
    }

    [Fact]
    public void PickAdvice_Recommends_Skip_Over_A_Harmful_Card()
    {
        var deck = Specs((4, "StrikeIronclad"), (3, "DefendIronclad"));
        // The only offered card is a Burn (strictly harmful) — taking it can't beat skipping.
        var (skip, ranked) = Advisor.PickAdvice(deck, new[] { "Burn" }, OneCultist(30),
            38, 38, 3, System.Array.Empty<string>(), Exact);
        var burn = ranked.First(p => !p.IsSkip);
        _out.WriteLine($"skip: surv {skip.MinSurvival:P1} loss {skip.TotalMeanLoss:F2}; "
            + $"take Burn: surv {burn.Score.MinSurvival:P1} loss {burn.Score.TotalMeanLoss:F2}");
        Assert.True(ranked[0].IsSkip, "advisor took a strictly-harmful Burn instead of skipping");
        Assert.False(burn.Score.BetterThan(skip), "taking a Burn scored better than skipping it");
    }
}
