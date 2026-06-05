using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>Ascension scaling helpers. STS2 ascension is cumulative; by A10 every level 1–10 is active.
/// Only two levels touch combat stats: <c>ToughEnemies</c> (enum index 8 → active at ascension ≥ 8) raises
/// enemy HP, and <c>DeadlyEnemies</c> (index 9 → ascension ≥ 9) raises enemy damage/amounts. (AscendersBane
/// at 5 adds a starting curse card, modelled separately; the rest are map/economy modifiers.)
/// <c>val</c> is the at-threshold value, <c>below</c> the value beneath it — matching the game's
/// <c>GetValueIfAscension(level, val, below)</c>.</summary>
public static class Asc
{
    public static int Tough(int ascension, int val, int below) => ascension >= 8 ? val : below;
    public static int Deadly(int ascension, int val, int below) => ascension >= 9 ? val : below;
}

/// <summary>Monster factory registration. A partial of <see cref="Catalog"/>; consumed by
/// <c>BuildMonster(name, ascension)</c>. Each factory takes the run's ascension level and scales HP
/// (Tough) and damage/amounts (Deadly) to match the game.</summary>
public static partial class Catalog
{
    private static readonly Dictionary<string, Func<int, Monster>> MonsterFactories = new(StringComparer.OrdinalIgnoreCase)
    {
        ["CalcifiedCultist"] = asc => Monsters.CalcifiedCultist(ascension: asc),
        ["DampCultist"] = asc => Monsters.DampCultist(ascension: asc),
        ["CorpseSlug"] = asc => Monsters.CorpseSlug(ascension: asc),
        ["Byrdonis"] = asc => Monsters.Byrdonis(ascension: asc),
        ["BygoneEffigy"] = asc => Monsters.BygoneEffigy(ascension: asc),
        ["PhrogParasite"] = asc => Monsters.PhrogParasite(ascension: asc),
        ["Wriggler"] = asc => Monsters.Wriggler(ascension: asc),
        ["TerrorEel"] = asc => Monsters.TerrorEel(ascension: asc),
        ["SoulNexus"] = asc => Monsters.SoulNexus(ascension: asc),
        ["MechaKnight"] = asc => Monsters.MechaKnight(ascension: asc),
        ["Entomancer"] = asc => Monsters.Entomancer(ascension: asc),
        ["SkulkingColony"] = asc => Monsters.SkulkingColony(ascension: asc),
        ["InfestedPrism"] = asc => Monsters.InfestedPrism(ascension: asc),
        ["PhantasmalGardener"] = asc => Monsters.PhantasmalGardener(ascension: asc),
        ["FlailKnight"] = asc => Monsters.FlailKnight(ascension: asc),
        ["SpectralKnight"] = asc => Monsters.SpectralKnight(ascension: asc),
        ["MagiKnight"] = asc => Monsters.MagiKnight(ascension: asc),
        ["DecimillipedeSegment"] = asc => Monsters.DecimillipedeSegment(ascension: asc),
        ["Decimillipede"] = asc => Monsters.DecimillipedeSegment("Decimillipede", ascension: asc),
        ["DecimillipedeSegmentFront"] = asc => Monsters.DecimillipedeSegment("DecimillipedeSegmentFront", 0, ascension: asc),
        ["DecimillipedeSegmentMiddle"] = asc => Monsters.DecimillipedeSegment("DecimillipedeSegmentMiddle", 1, ascension: asc),
        ["DecimillipedeSegmentBack"] = asc => Monsters.DecimillipedeSegment("DecimillipedeSegmentBack", 2, ascension: asc),

        // Act-1 (Underdocks) BOSSES.
        ["WaterfallGiant"] = asc => Monsters.WaterfallGiant(ascension: asc),
        ["SoulFysh"] = asc => Monsters.SoulFysh(ascension: asc),
        ["LagavulinMatriarch"] = asc => Monsters.LagavulinMatriarch(ascension: asc),

        // Act-2 (Overgrowth) BOSSES. (CeremonialBeast's Ringing play-restriction is inert pending an engine hook;
        // its damage is exact — must be resolved before bosses enter the deck-strength pool.)
        ["CeremonialBeast"] = asc => Monsters.CeremonialBeast(ascension: asc),
        ["Vantom"] = asc => Monsters.Vantom(ascension: asc),

        // Act-1 (Overgrowth) normal/weak monsters (unit-tested only; not trace-validated).
        ["SnappingJaxfruit"] = asc => Monsters.SnappingJaxfruit(ascension: asc),
        ["Flyconid"] = asc => Monsters.Flyconid(ascension: asc),
        ["CubexConstruct"] = asc => Monsters.CubexConstruct(ascension: asc),
        ["FuzzyWurmCrawler"] = asc => Monsters.FuzzyWurmCrawler(ascension: asc),
        ["ShrinkerBeetle"] = asc => Monsters.ShrinkerBeetle(ascension: asc),
        ["Mawler"] = asc => Monsters.Mawler(ascension: asc),
        ["Nibbit"] = asc => Monsters.Nibbit(ascension: asc),
        ["Inklet"] = asc => Monsters.Inklet(ascension: asc),
    };
}
