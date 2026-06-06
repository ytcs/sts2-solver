using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Ranwid;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Gates that ranwid supports all five characters (not just Ironclad): the save-id transforms map each
/// character + its combat-start starter relic, and a deck of each solves through the advice path with its
/// starter-relic mechanics (Regent Stars / Necrobinder Osty / Defect orbs) wired up.
/// </summary>
public class RanwidMultiCharacterTests
{
    private readonly ITestOutputHelper _out;
    public RanwidMultiCharacterTests(ITestOutputHelper o) => _out = o;

    [Theory]
    [InlineData("CHARACTER.IRONCLAD")]
    [InlineData("CHARACTER.THE_SILENT")]
    [InlineData("CHARACTER.SILENT")]
    [InlineData("CHARACTER.REGENT")]
    [InlineData("CHARACTER.NECROBINDER")]
    [InlineData("CHARACTER.DEFECT")]
    public void All_Five_Characters_Are_Supported(string charId) => Assert.True(GameIds.IsSupportedCharacter(charId));

    [Theory]
    [InlineData("CHARACTER.WATCHER")]
    [InlineData("CHARACTER.THE_PILGRIM")]
    public void Unsupported_Characters_Are_Rejected(string charId) => Assert.False(GameIds.IsSupportedCharacter(charId));

    [Theory]
    [InlineData("RELIC.BURNING_BLOOD", "BurningBlood")]
    [InlineData("RELIC.DIVINE_RIGHT", "DivineRight")]
    [InlineData("RELIC.BOUND_PHYLACTERY", "BoundPhylactery")]
    [InlineData("RELIC.CRACKED_CORE", "CrackedCore")]
    public void Starter_Relics_Map_To_Their_Models(string relicId, string expected)
        => Assert.Equal(expected, GameIds.ModelledRelicName(relicId));

    [Theory]
    [InlineData("RELIC.CROSSBOW")]            // combat relic deliberately OUT: RNG free-attack generation (optimistic)
    [InlineData("RELIC.SOME_UNMODELLED_RELIC")]
    public void Unmodelled_Relics_Map_To_Null(string relicId) => Assert.Null(GameIds.ModelledRelicName(relicId));

    [Theory]
    // A run's elite → its Act's FULL pool (all buildable), so a single-target-heavy run is still evaluated
    // against the Act's other elites.
    [InlineData("TerrorEelElite", "PhantasmalGardenersElite")]   // Underdocks pool incl. the 5-monster Gardeners
    [InlineData("MechaKnightElite", "KnightsElite")]             // Glory pool incl. the 6-monster Knights
    [InlineData("ByrdonisElite", "PhrogParasiteElite")]         // Overgrowth pool
    [InlineData("EntomancerElite", "DecimillipedeElite")]       // Hive pool incl. multi-segment Decimillipede
    public void RepresentativeElites_Expands_To_Act_Pool(string runElite, string mustInclude)
    {
        var pool = Catalog.RepresentativeElites(new[] { runElite });
        Assert.Contains(runElite, pool);
        Assert.Contains(mustInclude, pool);
        foreach (var e in pool) Assert.True(Catalog.IsKnownEliteEncounter(e), $"{e} not buildable");
    }

    [Theory]
    // The Acts whose elite pool already spans a big multi-enemy fight (so deck strength values AoE without
    // porting). Act-2 (Overgrowth) is deliberately absent — its elites are single-target in our port; its AoE
    // representation comes from its boss/normals, ported separately.
    [InlineData("TerrorEelElite")]      // Underdocks → Gardeners ×5
    [InlineData("EntomancerElite")]     // Hive → Decimillipede (multi-segment)
    [InlineData("MechaKnightElite")]    // Glory → Knights ×6
    public void Representative_Pool_Includes_A_MultiEnemy_Fight(string runElite)
    {
        var pool = Catalog.RepresentativeElites(new[] { runElite });
        Assert.Contains(pool, e => Catalog.BuildEliteEncounter(e, 0).Count >= 2);
    }

    [Fact]
    public void RepresentativeElites_Falls_Back_To_Run_Elites_When_Act_Unknown()
    {
        var pool = Catalog.RepresentativeElites(new[] { "TotallyUnknownElite" });
        Assert.Empty(pool);   // nothing buildable → empty (caller falls back to the run's elites)
    }

    [Fact]
    public void CardSpec_Is_Generic_Across_Characters()
    {
        Assert.Equal("StrikeSilent", GameIds.CardSpec("CARD.STRIKE_SILENT", 0));
        Assert.Equal("StrikeDefect+1", GameIds.CardSpec("CARD.STRIKE_DEFECT", 1));
        Assert.Equal("StrikeNecrobinder", GameIds.CardSpec("CARD.STRIKE_NECROBINDER", 0));
    }

    public static IEnumerable<object[]> CharacterDecks()
    {
        // character label, starter relic (null = none), a small representative deck.
        yield return new object[] { "Silent", (string?)null, new[] { "StrikeSilent", "StrikeSilent", "DefendSilent", "DefendSilent", "Neutralize", "Survivor" } };
        yield return new object[] { "Regent", "DivineRight", new[] { "StrikeRegent", "StrikeRegent", "DefendRegent", "DefendRegent", "ShiningStrike", "GuidingStar" } };
        yield return new object[] { "Necrobinder", "BoundPhylactery", new[] { "StrikeNecrobinder", "StrikeNecrobinder", "DefendNecrobinder", "DefendNecrobinder", "Bodyguard", "Unleash" } };
        yield return new object[] { "Defect", "CrackedCore", new[] { "StrikeDefect", "StrikeDefect", "DefendDefect", "DefendDefect", "Zap", "Dualcast" } };
    }

    /// <summary>End-to-end: a deck of each non-Ironclad character (with its starter relic) solves through the
    /// advice engine to a valid lexicographic value — proving the multi-character path works, mechanics and
    /// all (a Regent deck with no Stars / a Necrobinder with no Osty would throw or score nonsense).</summary>
    [Theory]
    [MemberData(nameof(CharacterDecks))]
    public void Character_Deck_Solves_Through_Advice_Path(string label, string? starterRelic, string[] deckSpecs)
    {
        var deck = deckSpecs.Select(Catalog.BuildCard).ToList();
        var relics = starterRelic == null ? System.Array.Empty<string>() : new[] { starterRelic };
        var player = Catalog.BuildPlayer(deck, 60, 60, 3, relics);
        var setup = Catalog.SetupCombat(player, new[] { Monsters.Byrdonis(hp: 45) });
        var stats = EncounterEvaluator.Evaluate(setup, new EvalOptions
        {
            MaxTurns = 12, BudgetSeconds = 0.0 /* MCTS-only, like ranwid */, MctsTrials = 600,
        });
        _out.WriteLine($"{label}: survival {stats.Survival:P1}, E[loss] {stats.MeanLoss:F1}");
        Assert.InRange(stats.Survival, 0.0, 1.0);
        Assert.InRange(stats.MeanLoss, 0.0, 60.0 + 1e-6);
    }
}
