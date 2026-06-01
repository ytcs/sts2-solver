using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>
/// A shared, deliberately *diverse* set of (deck, encounter) fixtures for calibrating the sampling solver
/// against the exact oracle. The point is to span deck archetypes beyond the Ironclad starter — block-heavy,
/// Strength/scaling, debuff-setup, card-draw/aggro, power-engine — so the heuristic policy can't be tuned
/// to one style. Every fixture is kept small enough (compact deck, single enemy, modest HP) that the exact
/// expectimax can still solve it in seconds, so each has a ground-truth label.
///
/// Lives in Content (the only project that can both see <see cref="Catalog"/> and is referenced by the CLI
/// and the test project); Search can't reference Content, so the harness passes the built state in.
/// </summary>
public static class CalibrationFixtures
{
    /// <summary>One calibration scenario. <see cref="Build"/> returns a fresh setup state each call
    /// (combat states are mutated during search, so callers clone per engine).</summary>
    public sealed record Fixture(string Name, string Archetype, Func<CombatState> Build, int MaxTurns)
    {
        public CombatState Setup() => Build();
    }

    // A compact deck spec → card list (e.g. "3xStrikeIronclad" or "Inflame"). Upgrades via the usual "+N".
    private static List<CardModel> Deck(params string[] specs)
    {
        var deck = new List<CardModel>();
        foreach (var spec in specs)
        {
            int x = spec.IndexOf('x');
            if (x > 0 && int.TryParse(spec[..x], out int n))
                for (int i = 0; i < n; i++) deck.Add(Catalog.BuildCard(spec[(x + 1)..]));
            else
                deck.Add(Catalog.BuildCard(spec));
        }
        return deck;
    }

    private static Fixture Make(string name, string archetype, int hp, int energy, int maxTurns,
        Func<Monster> monster, params string[] deck) =>
        new(name, archetype,
            () => Catalog.SetupCombat(
                Catalog.BuildPlayer(Deck(deck), hp, hp, energy, new[] { "BurningBlood" }),
                new[] { monster() }),
            maxTurns);

    /// <summary>The fixture set. Ascension 0 (base stats) keeps fights small; HP/energy/horizon are tuned so
    /// each fight is non-trivial (survival or loss meaningfully &gt; 0) yet exact-solvable in seconds. The
    /// horizon (MaxTurns) is set tight — comfortably above the turns optimal play needs, but well below the
    /// default 30 — because the exact tree's size is dominated by how many stalling turns it must explore;
    /// both engines share the same horizon, so the comparison stays fair.</summary>
    public static IReadOnlyList<Fixture> All { get; } = new[]
    {
        // --- Baseline: the starter deck (the style the heuristic was first tuned on). ---
        Make("starter/Cultist", "starter", hp: 50, energy: 3, maxTurns: 12,
            () => Monsters.CalcifiedCultist(),
            "5xStrikeIronclad", "4xDefendIronclad", "Bash"),

        // --- Strength/power: must PLAY a power (Inflame) that doesn't reduce the position score this turn
        //     but wins the race against a fast-ramping cultist. A greedy-on-current-score policy never
        //     plays it — the canonical overfitting probe. ---
        Make("power/Inflame-vs-DampCultist", "power", hp: 50, energy: 3, maxTurns: 12,
            () => Monsters.DampCultist(),
            "Inflame", "4xStrikeIronclad", "2xDefendIronclad"),

        // --- Debuff setup: Uppercut applies Vulnerable; sequencing the debuff before the Strikes wins the
        //     race vs a ramping elite. Tests whether the policy values setting up Vulnerable. ---
        Make("debuff/Uppercut-vs-Byrdonis", "debuff", hp: 50, energy: 3, maxTurns: 12,
            () => Monsters.Byrdonis(hp: 52),
            "Uppercut", "4xStrikeIronclad", "1xDefendIronclad"),

        // --- Block-defensive: a hard hitter (Byrdonis Swoop 17) where surviving needs real block, not race.
        //     Tests block valuation / over-block discipline under pressure. ---
        Make("block/Defends-vs-Byrdonis", "block", hp: 40, energy: 3, maxTurns: 14,
            () => Monsters.Byrdonis(hp: 58),
            "ShrugItOff", "3xDefendIronclad", "2xStrikeIronclad"),

        // --- Aggro/draw: Pommel Strike + Anger churn cards; CorpseSlug applies Frail (cuts our block) and
        //     opens with +Strength. Tests card-draw value and the Frail/block interaction. ---
        Make("aggro/Draw-vs-CorpseSlug", "aggro", hp: 45, energy: 3, maxTurns: 12,
            () => Monsters.CorpseSlug(),
            "PommelStrike", "Anger", "2xStrikeIronclad", "Bash"),

        // --- Power-engine: Demon Form is pure scaling (+Strength every turn) at a steep up-front tempo cost
        //     vs a tanky fight (BygoneEffigy sleeps then hits hard). The hardest case for a myopic policy:
        //     the right line spends tempo early to compound later. ---
        Make("engine/DemonForm-vs-Effigy", "engine", hp: 70, energy: 3, maxTurns: 15,
            () => Monsters.BygoneEffigy(hp: 70),
            "DemonForm", "4xStrikeIronclad", "1xDefendIronclad"),
    };
}
