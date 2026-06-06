using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Ranwid;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Tests for the deck advice engine (<see cref="Advisor"/>), which ranks by the deck-strength index (0–100 at
/// a fixed 100 HP): the metric is valid + deterministic, and the removal/pick rankings respect a guaranteed
/// direction — a strictly-harmful card (a <c>Burn</c>: unplayable, deals 2 when held at turn end) makes the
/// deck no stronger, so removing it is the best cut and taking it loses to skipping. Fights are kept small so
/// <see cref="EncounterEvaluator"/> stays deterministic (exact, or seeded MCTS).
/// </summary>
public class AdvisorTests
{
    private readonly ITestOutputHelper _out;
    public AdvisorTests(ITestOutputHelper o) => _out = o;

    private static readonly EvalOptions Exact = new() { MaxTurns = 12, BudgetSeconds = 60, MctsTrials = 20_000, Seed = 1 };

    // A meaty-enough single elite that a 100-HP deck still takes real damage over several turns, so a clogging
    // Burn measurably lowers the deck's strength (vs a trivial fight where it wouldn't matter).
    private static List<Advisor.Encounter> OneByrdonis(int hp) => new()
    {
        new Advisor.Encounter("Byrdonis", () => new List<Monster> { Monsters.Byrdonis(hp) }),
    };

    private static List<string> Specs(params (int n, string spec)[] parts)
    {
        var list = new List<string>();
        foreach (var (n, spec) in parts) for (int i = 0; i < n; i++) list.Add(spec);
        return list;
    }

    [Fact]
    public void DeckStrength_Is_Valid_And_Deterministic()
    {
        var deck = Specs((5, "StrikeIronclad"), (3, "DefendIronclad"));
        double a = Advisor.DeckStrength(deck, OneByrdonis(55), 3, System.Array.Empty<string>(), Exact);
        double b = Advisor.DeckStrength(deck, OneByrdonis(55), 3, System.Array.Empty<string>(), Exact);
        Assert.InRange(a, 0.0, 100.0);
        Assert.Equal(a, b, 6);   // deterministic (same seed)
    }

    [Fact]
    public void Removing_A_Harmful_Card_Is_Never_Worse()
    {
        var clean = Specs((5, "StrikeIronclad"), (3, "DefendIronclad"));
        var clogged = new List<string>(clean) { "Burn" };            // a Burn can only hurt (clogs + 2 dmg held)

        double sClean = Advisor.DeckStrength(clean, OneByrdonis(55), 3, System.Array.Empty<string>(), Exact);
        double sClogged = Advisor.DeckStrength(clogged, OneByrdonis(55), 3, System.Array.Empty<string>(), Exact);
        _out.WriteLine($"clean {sClean:F1}/100, clogged {sClogged:F1}/100");
        Assert.True(sClean >= sClogged - 1e-6, "a deck with a strictly-harmful Burn was stronger than without it");
    }

    [Fact]
    public void RemovalAdvice_Ranks_Best_First_And_Finds_The_Harmful_Card()
    {
        var deck = Specs((5, "StrikeIronclad"), (3, "DefendIronclad")).Append("Burn").ToList();
        var (baseline, items) = Advisor.RemovalAdvice(deck, OneByrdonis(55), 3, System.Array.Empty<string>(), Exact);

        Assert.Equal(deck.Distinct().Count(), items.Count);            // one item per distinct removable card
        for (int i = 1; i < items.Count; i++)                          // sorted by resulting strength, best-first
            Assert.True(items[i - 1].Strength >= items[i].Strength - 1e-9, "removal advice is not sorted best-first");
        Assert.True(items[0].Strength >= baseline - 1e-6, "the top removal was weaker than keeping the whole deck");
        _out.WriteLine($"baseline {baseline:F1}; top cut {items[0].Card} → {items[0].Strength:F1} (Δ{items[0].Delta:+0.0;-0.0})");
        Assert.Equal("Burn", items[0].Card);                          // the only strictly-dead card is the best cut
    }

    [Fact]
    public void UpgradeAdvice_Ranks_Best_First_And_Skips_Unupgradeable()
    {
        // A Burn (Status) and an already-upgraded Bash+1 are NOT upgrade candidates; the basic cards are.
        var deck = Specs((5, "StrikeIronclad"), (3, "DefendIronclad"), (1, "Bash+1")).Append("Burn").ToList();
        var (baseline, items) = Advisor.UpgradeAdvice(deck, OneByrdonis(55), 3, System.Array.Empty<string>(), Exact);

        Assert.DoesNotContain(items, i => i.Card == "Burn");          // Status — nothing to upgrade
        Assert.DoesNotContain(items, i => i.Card == "Bash+1");        // already upgraded — nothing more to advise
        Assert.Contains(items, i => i.Card == "StrikeIronclad" && i.Upgraded == "StrikeIronclad+1");
        for (int i = 1; i < items.Count; i++)                         // sorted by resulting strength, best-first
            Assert.True(items[i - 1].Strength >= items[i].Strength - 1e-9, "upgrade advice is not sorted best-first");
        Assert.True(items[0].Strength >= baseline - 1e-6, "the top upgrade was weaker than the deck as-is");
        _out.WriteLine($"baseline {baseline:F1}; top upgrade {items[0].Upgraded} → {items[0].Strength:F1} (Δ{items[0].Delta:+0.0;-0.0})");
    }

    [Fact]
    public void PickAdvice_Includes_Skip_And_Ranks_Best_First()
    {
        var deck = Specs((5, "StrikeIronclad"), (3, "DefendIronclad"));
        var (_, ranked) = Advisor.PickAdvice(deck, new[] { "StrikeIronclad", "Burn" }, OneByrdonis(55),
            3, System.Array.Empty<string>(), Exact);

        Assert.Equal(3, ranked.Count);                                  // 2 candidates + skip
        Assert.Contains(ranked, p => p.IsSkip);
        for (int i = 1; i < ranked.Count; i++)
            Assert.True(ranked[i - 1].Strength >= ranked[i].Strength - 1e-9, "pick advice is not sorted best-first");
    }

    [Fact]
    public void PickAdvice_Recommends_Skip_Over_A_Harmful_Card()
    {
        var deck = Specs((5, "StrikeIronclad"), (3, "DefendIronclad"));
        // The only offered card is a Burn (strictly harmful) — taking it can't beat skipping.
        var (skip, ranked) = Advisor.PickAdvice(deck, new[] { "Burn" }, OneByrdonis(55),
            3, System.Array.Empty<string>(), Exact);
        var burn = ranked.First(p => !p.IsSkip);
        _out.WriteLine($"skip {skip:F1}/100; take Burn {burn.Strength:F1}/100");
        Assert.True(ranked[0].IsSkip, "advisor took a strictly-harmful Burn instead of skipping");
        Assert.True(skip >= burn.Strength - 1e-6, "taking a Burn scored better than skipping it");
    }
}
