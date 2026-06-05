using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>Encounter compositions (which monsters make up a named fight). A partial of
/// <see cref="Catalog"/>. Keyed by the game's encounter class name — the PascalCase of the run save's
/// <c>ENCOUNTER.*</c> id (e.g. <c>ENCOUNTER.SKULKING_COLONY_ELITE → "SkulkingColonyElite"</c>). Content
/// owns monster composition; structured so boss / normal encounters can be added in the same shape.</summary>
public static partial class Catalog
{
    private static readonly Dictionary<string, Func<int, IEnumerable<Monster>>> EliteEncounterFactories =
        new(StringComparer.OrdinalIgnoreCase)
    {
        ["ByrdonisElite"]            = a => new[] { Monsters.Byrdonis(ascension: a) },
        ["BygoneEffigyElite"]        = a => new[] { Monsters.BygoneEffigy(ascension: a) },
        ["PhrogParasiteElite"]       = a => new[] { Monsters.PhrogParasite(ascension: a) },
        ["TerrorEelElite"]           = a => new[] { Monsters.TerrorEel(ascension: a) },
        ["SoulNexusElite"]           = a => new[] { Monsters.SoulNexus(ascension: a) },
        ["MechaKnightElite"]         = a => new[] { Monsters.MechaKnight(ascension: a) },
        ["EntomancerElite"]          = a => new[] { Monsters.Entomancer(ascension: a) },
        ["SkulkingColonyElite"]      = a => new[] { Monsters.SkulkingColony(ascension: a) },
        ["InfestedPrismsElite"]      = a => new[] { Monsters.InfestedPrism(ascension: a) },
        ["PhantasmalGardenersElite"] = a => Monsters.PhantasmalGardeners(ascension: a),
        ["KnightsElite"]             = a => Monsters.Knights(ascension: a),
        ["DecimillipedeElite"]       = a => new[]
        {
            Monsters.DecimillipedeSegment("DecimillipedeSegmentFront", 0, ascension: a),
            Monsters.DecimillipedeSegment("DecimillipedeSegmentMiddle", 1, ascension: a),
            Monsters.DecimillipedeSegment("DecimillipedeSegmentBack", 2, ascension: a),
        },

        // Act-1 (Underdocks) BOSS — single big body. (Steam-Eruption death explosion omitted; see Monsters.WaterfallGiant.)
        ["WaterfallGiantBoss"]       = a => new[] { Monsters.WaterfallGiant(ascension: a) },
    };

    /// <summary>Act-1 (Overgrowth) NORMAL / WEAK encounter compositions. Same shape as the elite table,
    /// keyed by the game's encounter class name. UNIT-TESTED ONLY — these monsters are not yet
    /// trace-validated. Slime-bearing encounters (SlimesNormal/Weak, SlitheringStranglerNormal,
    /// FlyconidNormal) are intentionally absent: they require a Slimed status card that lives outside this
    /// task's editable scope. VineShamblerNormal and RubyRaidersNormal are likewise absent (see Monsters).</summary>
    private static readonly Dictionary<string, Func<int, IEnumerable<Monster>>> NormalEncounterFactories =
        new(StringComparer.OrdinalIgnoreCase)
    {
        ["SnappingJaxfruitNormal"]   = a => new[] { Monsters.SnappingJaxfruit(ascension: a), Monsters.Flyconid(ascension: a) },
        ["CubexConstructNormal"]     = a => new[] { Monsters.CubexConstruct(ascension: a) },
        ["FuzzyWurmCrawlerWeak"]     = a => new[] { Monsters.FuzzyWurmCrawler(ascension: a) },
        ["ShrinkerBeetleWeak"]       = a => new[] { Monsters.ShrinkerBeetle(ascension: a) },
        ["OvergrowthCrawlers"]       = a => new[] { Monsters.ShrinkerBeetle(ascension: a), Monsters.FuzzyWurmCrawler(ascension: a) },
        ["MawlerNormal"]             = a => new[] { Monsters.Mawler(ascension: a) },
        ["NibbitsWeak"]              = a => new[] { Monsters.Nibbit(slot: "alone", ascension: a) },
        ["NibbitsNormal"]            = a => Monsters.Nibbits(ascension: a),
        ["InkletsNormal"]            = a => Monsters.Inklets(ascension: a),
    };

    /// <summary>True if <paramref name="name"/> (an encounter class name) is a known elite composition.</summary>
    public static bool IsKnownEliteEncounter(string name) => EliteEncounterFactories.ContainsKey(name);

    /// <summary>True if <paramref name="name"/> is a known normal/weak encounter composition.</summary>
    public static bool IsKnownNormalEncounter(string name) => NormalEncounterFactories.ContainsKey(name);

    /// <summary>True if <paramref name="name"/> is any known encounter (elite or normal/weak).</summary>
    public static bool IsKnownEncounter(string name) =>
        EliteEncounterFactories.ContainsKey(name) || NormalEncounterFactories.ContainsKey(name);

    /// <summary>The normal/weak encounter class names this catalog can build.</summary>
    public static IReadOnlyCollection<string> KnownNormalEncounters => NormalEncounterFactories.Keys;

    /// <summary>Build the monster list for any known encounter (elite or normal/weak), scaled to
    /// <paramref name="ascension"/>. Throws for an unknown encounter.</summary>
    public static List<Monster> BuildEncounter(string name, int ascension = 10)
    {
        if (EliteEncounterFactories.TryGetValue(name, out var ef)) return ef(ascension).ToList();
        if (NormalEncounterFactories.TryGetValue(name, out var nf)) return nf(ascension).ToList();
        throw new ArgumentException(
            $"Unknown encounter '{name}'. Known elites: {string.Join(", ", EliteEncounterFactories.Keys)}. " +
            $"Known normals: {string.Join(", ", NormalEncounterFactories.Keys)}");
    }

    /// <summary>The elite encounter class names this catalog can build.</summary>
    public static IReadOnlyCollection<string> KnownEliteEncounters => EliteEncounterFactories.Keys;

    /// <summary>Each Act's full elite pool (from the decompiled act definitions). Keyed on the act's THEME so a
    /// run's actual elites identify the act without assuming an index order. Every elite here is ported, and the
    /// pools deliberately span single-target → big multi-enemy (Gardeners ×5, Decimillipede, Knights ×6) so a
    /// deck-strength evaluation over the WHOLE pool values AoE fairly — not just whatever subset a run drew.</summary>
    private static readonly string[][] ActElitePools =
    {
        new[] { "TerrorEelElite", "SkulkingColonyElite", "PhantasmalGardenersElite" },   // Underdocks
        new[] { "ByrdonisElite", "BygoneEffigyElite", "PhrogParasiteElite" },            // Overgrowth
        new[] { "DecimillipedeElite", "EntomancerElite", "InfestedPrismsElite" },        // Hive
        new[] { "KnightsElite", "MechaKnightElite", "SoulNexusElite" },                  // Glory
    };

    /// <summary>A representative, AoE-balanced elite set for deck-strength evaluation: given a run's actual
    /// elites, return its Act's FULL elite pool (so a single-target-heavy run still gets evaluated against the
    /// Act's multi-enemy elites). Falls back to whatever of <paramref name="runElites"/> is buildable when the
    /// Act can't be identified.</summary>
    public static IReadOnlyList<string> RepresentativeElites(IEnumerable<string> runElites)
    {
        var run = runElites.ToList();
        foreach (var pool in ActElitePools)
            if (run.Any(e => pool.Contains(e, StringComparer.Ordinal)))
                return pool;
        return run.Where(IsKnownEliteEncounter).ToList();
    }

    /// <summary>Build the monster list for an elite encounter (by class name), scaled to
    /// <paramref name="ascension"/>. Throws for an unknown encounter (caller may probe with
    /// <see cref="IsKnownEliteEncounter"/> first).</summary>
    public static List<Monster> BuildEliteEncounter(string name, int ascension = 10) =>
        EliteEncounterFactories.TryGetValue(name, out var f)
            ? f(ascension).ToList()
            : throw new ArgumentException(
                $"Unknown elite encounter '{name}'. Known: {string.Join(", ", EliteEncounterFactories.Keys)}");
}
