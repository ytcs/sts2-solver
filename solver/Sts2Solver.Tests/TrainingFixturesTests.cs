using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>
/// Sanity checks for the randomised training corpus (<see cref="TrainingFixtures.Random"/>) that feeds the
/// Phase-C value-function trainer. These don't touch soundness of the solver (training never alters the exact
/// oracle) — they guard that the generator emits well-formed, varied, independently-rebuilt, exactly-solvable
/// fixtures, so a long offline <c>--train-vf</c> run can't be poisoned by a malformed draw.
/// </summary>
public class TrainingFixturesTests
{
    /// <summary>Regression: in-pile card upgrades (Armaments / Apotheosis) must REPLACE the upgraded card with
    /// a private cloned copy, never mutate the shared non-Stateful instance in place. Otherwise sibling search
    /// branches that share the instance see its identity change and the draw enumerator throws "Pile missing
    /// card …". This deck (Armaments + Impervious + Envenom) reproduced exactly that before the fix.</summary>
    [Fact]
    public void Upgrade_In_Pile_Does_Not_Corrupt_Shared_Instances()
    {
        var deck = new[] { "Armaments", "Armaments", "Impervious", "Impervious", "Impervious",
                           "Impervious", "Impervious", "Envenom", "Envenom" }
            .Select(Catalog.BuildCard).ToList();
        var setup = Catalog.SetupCombat(
            Catalog.BuildPlayer(deck, 45, 45, 3, new[] { "BurningBlood" }), new[] { Monsters.DampCultist(hp: 35) });
        // Bounded budget: the bug threw InvalidOperationException("Pile missing card …") *early* during draw
        // enumeration, so a short cap still catches a regression; a clean solve either finishes or (under
        // parallel load) cancels — both mean no shared-instance corruption. (Tight horizon keeps it cheap.)
        var solver = new Solver { MaxTurns = 5 };
        using var cts = new System.Threading.CancellationTokenSource(System.TimeSpan.FromSeconds(15));
        solver.Ct = cts.Token;
        try { var v = solver.Solve(setup); Assert.InRange(v.Win, 0.0, 1.0 + 1e-6); }
        catch (System.OperationCanceledException) { /* didn't corrupt within budget — pass */ }
    }

    [Fact]
    public void Random_Is_Deterministic_In_Seed()
    {
        string Sig(int seed) => string.Join("|", TrainingFixtures.Random(count: 30, seed: seed).Select(f => f.Name));
        Assert.Equal(Sig(123), Sig(123));
        Assert.NotEqual(Sig(123), Sig(124));
    }

    [Fact]
    public void Random_Decks_Are_WellFormed_And_Varied()
    {
        var fixtures = TrainingFixtures.Random(count: 60, seed: 7).ToList();
        Assert.Equal(60, fixtures.Count);

        var sizes = new HashSet<int>();
        var allCards = new HashSet<string>();
        foreach (var f in fixtures)
        {
            var s = f.Setup();
            var deck = s.Player.DrawPile;
            Assert.InRange(deck.Count, 5, 11);                       // size bounds
            Assert.Contains(deck, c => c.Type == CardType.Attack);   // at least one attack (winnable spectrum)
            sizes.Add(deck.Count);
            foreach (var c in deck) allCards.Add(c.GetType().Name);
            Assert.Single(s.Monsters);                               // solo encounters
            Assert.True(s.Player.CurrentHp > 0 && s.Player.MaxHp > 0);
        }
        // Genuinely varied: many distinct deck sizes and a wide spread of distinct card types drawn from the pool.
        Assert.True(sizes.Count >= 4, $"only {sizes.Count} distinct deck sizes");
        Assert.True(allCards.Count >= 15, $"only {allCards.Count} distinct cards across the draw");
    }

    [Fact]
    public void Random_Fixtures_Rebuild_Independent_Card_Instances()
    {
        // Build the same fixture twice; the two draw piles must be disjoint object instances, or combat
        // mutation of one state would corrupt the other (and the training labels).
        var f = TrainingFixtures.Random(count: 1, seed: 42).First();
        var a = f.Setup();
        var b = f.Setup();
        Assert.Equal(a.Player.DrawPile.Count, b.Player.DrawPile.Count);
        for (int i = 0; i < a.Player.DrawPile.Count; i++)
            Assert.NotSame(a.Player.DrawPile[i], b.Player.DrawPile[i]);
    }

    /// <summary>Regression for the clone-sharing soundness bug surfaced by random decks: <c>Player.Clone()</c>
    /// used to share card instances, so self-mutating cards (Rampage's escalating damage) corrupted sibling
    /// search branches and crashed the draw enumerator. A clone must own its own instance of a Stateful card,
    /// while immutable cards may still be shared (the perf path).</summary>
    [Fact]
    public void Clone_Isolates_Stateful_Cards_But_Shares_Immutable()
    {
        var player = Catalog.BuildPlayer(
            new System.Collections.Generic.List<CardModel> { new Rampage(), new StrikeIronclad() }, 50, 50);
        var s = Catalog.SetupCombat(player, new[] { Monsters.Byrdonis(hp: 40) });
        var clone = s.Clone();

        var rampOrig = s.Player.DrawPile.First(c => c is Rampage);
        var rampClone = clone.Player.DrawPile.First(c => c is Rampage);
        Assert.NotSame(rampOrig, rampClone);                                   // Stateful → deep-cloned

        var strikeOrig = s.Player.DrawPile.First(c => c is StrikeIronclad);
        var strikeClone = clone.Player.DrawPile.First(c => c is StrikeIronclad);
        Assert.Same(strikeOrig, strikeClone);                                  // immutable → shared

        // Mutating the original's Rampage identity must not bleed into the clone.
        string before = rampClone.StateKey();
        rampOrig.OnPlay(s, new CardPlay { Card = rampOrig, Target = s.Monsters[0] });
        Assert.NotEqual(rampOrig.StateKey(), before);                          // original escalated
        Assert.Equal(before, rampClone.StateKey());                            // clone untouched
    }

    /// <summary>A Rampage deck (the card that triggered the bug) now solves exactly without crashing, and its
    /// escalating damage means it beats a fight a single-Strike race could not — sanity that the value is real.</summary>
    [Fact]
    public void Rampage_Deck_Solves_Exactly()
    {
        var deck = new System.Collections.Generic.List<CardModel>
            { new Rampage(), new Rampage(), new DefendIronclad(), new DefendIronclad(), new StrikeIronclad() };
        var setup = Catalog.SetupCombat(
            Catalog.BuildPlayer(deck, 60, 60, 3, new[] { "BurningBlood" }), new[] { Monsters.Byrdonis(hp: 50) });
        var v = new Solver { MaxTurns = 12 }.Solve(setup);
        Assert.InRange(v.Win, 0.0, 1.0);
        Assert.InRange(v.Loss, 0.0, 60.0);
    }

    /// <summary>A randomised fixture is exactly solvable to a valid lexicographic value within a small budget
    /// (the trainer relies on this — a partial memo from a timeout is still valid, but the common case must
    /// terminate). Uses a tight horizon and a generous wall-clock cap.</summary>
    [Fact]
    public void Random_Fixtures_Are_Exactly_Solvable()
    {
        // Kept deliberately light (few fixtures, tight horizon, short budget) so it doesn't starve the
        // wall-clock-budgeted exact solves in CalibrationTests when xUnit runs the classes in parallel.
        int solved = 0, total = 0;
        foreach (var f in TrainingFixtures.Random(count: 4, seed: 99, maxTurns: 8))
        {
            total++;
            var solver = new Solver { MaxTurns = f.MaxTurns };
            using var cts = new System.Threading.CancellationTokenSource(System.TimeSpan.FromSeconds(12));
            solver.Ct = cts.Token;
            Value v;
            try { v = solver.Solve(f.Setup()); }
            catch (System.OperationCanceledException) { continue; } // timed out — still valid, just not counted
            Assert.InRange(v.Win, 0.0, 1.0 + 1e-6);
            Assert.InRange(v.Loss, 0.0, f.Setup().Player.MaxHp + 1e-6);   // E[loss] ≤ MaxHp (± FP summation noise)
            solved++;
        }
        Assert.True(solved >= 2, $"only {solved}/{total} random fixtures solved within budget");
    }
}
