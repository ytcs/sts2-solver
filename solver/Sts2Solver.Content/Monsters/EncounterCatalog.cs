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
    };

    /// <summary>True if <paramref name="name"/> (an encounter class name) is a known elite composition.</summary>
    public static bool IsKnownEliteEncounter(string name) => EliteEncounterFactories.ContainsKey(name);

    /// <summary>The elite encounter class names this catalog can build.</summary>
    public static IReadOnlyCollection<string> KnownEliteEncounters => EliteEncounterFactories.Keys;

    /// <summary>Build the monster list for an elite encounter (by class name), scaled to
    /// <paramref name="ascension"/>. Throws for an unknown encounter (caller may probe with
    /// <see cref="IsKnownEliteEncounter"/> first).</summary>
    public static List<Monster> BuildEliteEncounter(string name, int ascension = 10) =>
        EliteEncounterFactories.TryGetValue(name, out var f)
            ? f(ascension).ToList()
            : throw new ArgumentException(
                $"Unknown elite encounter '{name}'. Known: {string.Join(", ", EliteEncounterFactories.Keys)}");
}
