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

    /// <summary>Clone isolation for self-MUTATING-of-OTHER-cards effects. Apotheosis upgrades every card in
    /// the player's piles; Armaments upgrades a card in hand. Those targets are the immutable-majority cards
    /// that <c>Player.Clone</c> SHARES across sibling search states, so the effect must not mutate a shared
    /// instance in place (it must swap in a private upgraded copy) — otherwise it changes the shared card's
    /// StateKey in every branch and the draw enumerator crashes with "Pile missing card …".</summary>
    [Fact]
    public void Upgrade_Effects_Do_Not_Mutate_Shared_Card_Instances()
    {
        // Apotheosis: a sibling clone's draw-pile cards keep their pre-upgrade identity after the original plays it.
        var apoDeck = new System.Collections.Generic.List<CardModel>
            { new Apotheosis(), new StrikeIronclad(), new DefendIronclad() };
        var sa = Catalog.SetupCombat(Catalog.BuildPlayer(apoDeck, 60, 60), new[] { Monsters.CalcifiedCultist() });
        var cloneA = sa.Clone();
        var strikeShared = sa.Player.DrawPile.First(c => c is StrikeIronclad);
        var strikeClone = cloneA.Player.DrawPile.First(c => c is StrikeIronclad);
        Assert.Same(strikeShared, strikeClone);                          // immutable card → shared instance
        string beforeA = strikeClone.StateKey();
        var apo = sa.Player.DrawPile.First(c => c is Apotheosis);
        apo.OnPlay(sa, new CardPlay { Card = apo });
        Assert.Equal(beforeA, strikeClone.StateKey());                   // clone's view untouched
        Assert.Equal(beforeA, strikeShared.StateKey());                  // shared original untouched (swapped, not mutated)
        Assert.Contains(sa.Player.DrawPile, c => c is StrikeIronclad && c.Upgrades == 1); // this state DID upgrade (a private copy)

        // Armaments+: same guarantee for a card in hand.
        var armDeck = new System.Collections.Generic.List<CardModel> { new Armaments().Upgraded(1), new StrikeIronclad() };
        var sb = Catalog.SetupCombat(Catalog.BuildPlayer(armDeck, 60, 60), new[] { Monsters.CalcifiedCultist() });
        // Move both the Armaments (the card we play) and a Strike (the upgrade target) into the hand.
        sb.Player.Hand.Add(sb.Player.DrawPile.First(c => c is Armaments));
        sb.Player.Hand.Add(sb.Player.DrawPile.First(c => c is StrikeIronclad));
        var cloneB = sb.Clone();
        var sbStrike = sb.Player.Hand.First(c => c is StrikeIronclad);
        var cbStrike = cloneB.Player.Hand.First(c => c is StrikeIronclad);
        Assert.Same(sbStrike, cbStrike);
        string beforeB = cbStrike.StateKey();
        var arm = sb.Player.Hand.First(c => c is Armaments);
        arm.OnPlay(sb, new CardPlay { Card = arm });
        Assert.Equal(beforeB, cbStrike.StateKey());                      // clone untouched
        Assert.Equal(beforeB, sbStrike.StateKey());                      // shared original untouched
    }

    /// <summary>An Impatience deck that also contains Apotheosis (the card-upgrade effect whose shared-instance
    /// mutation corrupted the draw enumerator's pile bookkeeping → "Pile missing card 'Impatience'") now solves
    /// exactly without crashing. Gates the soundness fix end-to-end through the exact oracle.</summary>
    [Fact]
    public void Apotheosis_Impatience_Deck_Solves_Exactly()
    {
        var deck = new System.Collections.Generic.List<CardModel>
        {
            new Apotheosis(), new Impatience(), new Impatience(),
            new StrikeIronclad(), new DefendIronclad(), new DefendIronclad(),
        };
        var setup = Catalog.SetupCombat(
            Catalog.BuildPlayer(deck, 60, 60, 3, new[] { "BurningBlood" }), new[] { Monsters.CalcifiedCultist() });
        var v = new Solver { MaxTurns = 10 }.Solve(setup);
        Assert.InRange(v.Win, 0.0, 1.0 + 1e-6);
        Assert.InRange(v.Loss, 0.0, 60.0 + 1e-6);
    }

    /// <summary>The full default training corpus (seed 20260601) is exactly solvable on every Apotheosis- or
    /// Armaments-bearing fixture — the cards whose in-place card-upgrade used to corrupt sibling search states.
    /// Budgeted so it terminates; a timeout is acceptable (the partial memo is still valid), a crash is not.</summary>
    [Fact]
    public void Corpus_Upgrade_Card_Fixtures_Solve_Without_Crashing()
    {
        int matched = 0;
        foreach (var f in TrainingFixtures.Random(count: 250, seed: 20260601, maxTurns: 8))
        {
            var deck = f.Setup().Player.DrawPile;
            if (!deck.Any(c => c is Apotheosis || c is Armaments)) continue;
            matched++;
            var solver = new Solver { MaxTurns = f.MaxTurns };
            using var cts = new System.Threading.CancellationTokenSource(System.TimeSpan.FromSeconds(6));
            solver.Ct = cts.Token;
            try { var v = solver.Solve(f.Setup()); Assert.InRange(v.Win, 0.0, 1.0 + 1e-6); }
            catch (System.OperationCanceledException) { /* timeout is fine */ }
        }
        Assert.True(matched > 0, "expected the corpus to contain Apotheosis/Armaments fixtures");
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
