using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>
/// The training corpus for the Phase-C learned value function (<c>VfTrainer</c> / CLI <c>--train-vf</c>):
/// exact-solvable (deck × enemy × HP) fixtures from which the trainer harvests exact-solver labels across
/// the full survival spectrum (trivial win … razor-thin … clear loss). Two sources, unioned by <see cref="All"/>:
///   • a small CURATED grid of named archetypes (starter/aggro/block/debuff/weak/draw) × monsters × HP — the
///     same hand-picked coverage as before; and
///   • a RANDOMISED draw (<see cref="Random"/>) that samples decks of varying size and composition from the
///     whole deck-buildable <see cref="Catalog.CardPool"/>, so the regression sees a far wider variety of deck
///     types and sizes than the curated archetypes alone.
/// Distinct from the hand-curated <see cref="CalibrationFixtures"/>, which stays the held-out measuring stick.
/// Everything is kept compact enough that the exact oracle solves it within the trainer's per-fixture
/// wall-clock budget (a partial memo from a timeout still yields valid labels).
/// </summary>
public static class TrainingFixtures
{
    private static List<CardModel> Deck(params string[] specs)
    {
        var deck = new List<CardModel>();
        foreach (var spec in specs)
        {
            int x = spec.IndexOf('x');
            if (x > 0 && int.TryParse(spec[..x], out int n))
                for (int i = 0; i < n; i++) deck.Add(Catalog.BuildCard(spec[(x + 1)..]));
            else deck.Add(Catalog.BuildCard(spec));
        }
        return deck;
    }

    private static readonly (string name, string[] specs)[] Decks =
    {
        ("starter",  new[] { "5xStrikeIronclad", "4xDefendIronclad", "Bash" }),
        ("aggro",    new[] { "5xStrikeIronclad", "Bash" }),
        ("block",    new[] { "ShrugItOff", "3xDefendIronclad", "2xStrikeIronclad" }),
        ("debuff",   new[] { "Uppercut", "4xStrikeIronclad", "DefendIronclad" }),
        ("weak",     new[] { "Neutralize", "3xDefendIronclad", "2xStrikeIronclad" }),
        ("draw",     new[] { "PommelStrike", "Anger", "2xStrikeIronclad", "Bash" }),
    };

    private static readonly (string name, Func<int, Monster> build)[] Monsters_ =
    {
        ("CalcifiedCultist", hp => Monsters.CalcifiedCultist(hp)),
        ("DampCultist",      hp => Monsters.DampCultist(hp)),
        ("CorpseSlug",       hp => Monsters.CorpseSlug(hp)),
        ("Byrdonis",         hp => Monsters.Byrdonis(hp)),
        ("BygoneEffigy",     hp => Monsters.BygoneEffigy(hp)),
        ("TerrorEel",        hp => Monsters.TerrorEel(hp)),
    };

    // Enemy/player HP sweep difficulty across the spectrum (trivial win … razor-thin … clear loss) while
    // keeping the grid small enough that every exact solve finishes quickly.
    private static readonly int[] EnemyHps = { 40, 70 };
    private static readonly int[] PlayerHps = { 38, 60 };

    /// <summary>The full training corpus: the curated archetype grid followed by <paramref name="randomCount"/>
    /// randomised-deck fixtures. <paramref name="maxTurns"/> is a tight shared horizon for the curated grid
    /// (kept small so the exact solves stay fast; the trainer applies its own per-fixture wall-clock budget on
    /// top). Deterministic across calls — the random source is freshly seeded each enumeration.</summary>
    public static IEnumerable<CalibrationFixtures.Fixture> All(int maxTurns = 16, int randomCount = 80, int seed = 20260601)
    {
        foreach (var (dn, specs) in Decks)
            foreach (var (mn, build) in Monsters_)
                foreach (int ehp in EnemyHps)
                    foreach (int php in PlayerHps)
                    {
                        // capture locals
                        var specsL = specs; int ehpL = ehp, phpL = php; var buildL = build;
                        yield return new CalibrationFixtures.Fixture(
                            $"{dn}/{mn}@e{ehp}/p{php}", dn,
                            () => Catalog.SetupCombat(
                                Catalog.BuildPlayer(Deck(specsL), phpL, phpL, 3, new[] { "BurningBlood" }),
                                new[] { buildL(ehpL) }),
                            maxTurns);
                    }

        foreach (var f in Random(randomCount, seed, Math.Min(maxTurns, 14)))
            yield return f;
    }

    // ---------- randomised decks drawn from the full card pool ----------

    private static readonly int[] RandEnemyHps = { 35, 50, 65, 80 };
    private static readonly int[] RandPlayerHps = { 30, 45, 60 };

    /// <summary>Draw a random deck as a list of card-name specs. To keep the exact solve tractable while still
    /// sampling broadly from the pool, a deck is built from a small number of DISTINCT card types
    /// (2–5) repeated to the target size — fewer distinct cards in hand means far less expectimax branching,
    /// yet composition and size still vary widely across fixtures. At least one Attack is guaranteed so the
    /// fight is sometimes winnable, keeping the full survival spectrum represented (not just losses).</summary>
    private static List<string> RandomDeckSpecs(Random rng)
    {
        var pool = Catalog.CardPool;
        int distinctK = 2 + rng.Next(4);          // 2..5 distinct card types
        int size = 5 + rng.Next(7);               // 5..11 cards

        var chosen = new List<string>();
        var seen = new HashSet<string>();
        while (chosen.Count < distinctK)
        {
            var c = pool[rng.Next(pool.Count)];
            if (seen.Add(c)) chosen.Add(c);
        }

        var specs = new List<string>(size);
        for (int i = 0; i < size; i++) specs.Add(chosen[rng.Next(chosen.Count)]);

        // Guarantee at least one Attack INSTANCE in the deck (so the fight is sometimes winnable, keeping the
        // full survival spectrum represented). The fill above can miss an attack even if one was chosen, so
        // enforce it on the materialised deck: overwrite a slot with a random pool attack if none is present.
        if (!specs.Any(c => Catalog.BuildCard(c).Type == CardType.Attack))
        {
            var attacks = pool.Where(c => Catalog.BuildCard(c).Type == CardType.Attack).ToList();
            specs[rng.Next(specs.Count)] = attacks[rng.Next(attacks.Count)];
        }
        return specs;
    }

    /// <summary>Randomised-deck fixtures: <paramref name="count"/> decks sampled from the full
    /// <see cref="Catalog.CardPool"/> (varying size and composition), each paired with a random monster and a
    /// random enemy/player HP from the spread. Deterministic in <paramref name="seed"/>. Each fixture rebuilds
    /// fresh card instances inside its <c>Build</c> lambda, so repeated <c>Setup()</c> calls never share
    /// mutable card state.</summary>
    public static IEnumerable<CalibrationFixtures.Fixture> Random(int count = 80, int seed = 20260601, int maxTurns = 14)
    {
        var rng = new Random(seed);
        for (int k = 0; k < count; k++)
        {
            var specs = RandomDeckSpecs(rng);
            var (mn, build) = Monsters_[rng.Next(Monsters_.Length)];
            int ehp = RandEnemyHps[rng.Next(RandEnemyHps.Length)];
            int php = RandPlayerHps[rng.Next(RandPlayerHps.Length)];

            // capture locals
            var specsL = specs; var buildL = build; int ehpL = ehp, phpL = php;
            yield return new CalibrationFixtures.Fixture(
                $"rand{k}/{mn}@e{ehp}/p{php}/d{specs.Count}", "random",
                () => Catalog.SetupCombat(
                    Catalog.BuildPlayer(specsL.Select(Catalog.BuildCard).ToList(), phpL, phpL, 3, new[] { "BurningBlood" }),
                    new[] { buildL(ehpL) }),
                maxTurns);
        }
    }
}
