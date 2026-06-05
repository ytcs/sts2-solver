using System.Collections.Generic;
using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Ranwid;
using Sts2Solver.Search;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>
/// Gates ranwid's custom (save-less) deck mode: every character exposes a buildable starter profile, the
/// shared <see cref="Companion.BuildContext"/> assembles a scorable Context from a hand-built deck against any
/// Act's elite pool, and editing the deck moves the deck-strength index in the expected direction. This is the
/// path a multiplayer GUEST uses (their run isn't saved locally), so it must work end-to-end without a save.
/// </summary>
public class RanwidCustomDeckTests
{
    private static readonly EvalOptions Fast = new()
    {
        BudgetSeconds = 0.0, Rollouts = 2000, MctsTrials = 400, Seed = 1,
    };

    [Fact]
    public void All_Five_Characters_Have_A_Buildable_Starter_Profile()
    {
        Assert.Equal(5, Catalog.CharacterProfiles.Count);
        foreach (var p in Catalog.CharacterProfiles)
        {
            var specs = p.StarterDeckSpecs();
            Assert.True(specs.Count >= 9, $"{p.Key} starter deck too small: {specs.Count}");
            foreach (var s in specs) Catalog.BuildCard(s);                 // throws if not registered
            Assert.True(p.StartingHp > 0);
            Assert.True(p.MaxEnergy > 0);
            if (p.StarterRelic != null) Assert.True(Catalog.IsModelledRelic(p.StarterRelic), $"{p.Key} relic {p.StarterRelic}");
        }
    }

    [Theory]
    [InlineData("ironclad", "Ironclad")]
    [InlineData("SILENT", "Silent")]
    [InlineData("CHARACTER.DEFECT", "Defect")]
    [InlineData("necro", "Necrobinder")]
    public void FindCharacter_Resolves_By_Key_Or_Id(string query, string expectedKey)
        => Assert.Equal(expectedKey, Catalog.FindCharacter(query)!.Key);

    [Fact]
    public void FindCharacter_Returns_Null_For_Unknown() => Assert.Null(Catalog.FindCharacter("watcher"));

    [Theory]
    [InlineData(0, "TerrorEelElite")]
    [InlineData(1, "ByrdonisElite")]
    [InlineData(2, "DecimillipedeElite")]
    [InlineData(3, "KnightsElite")]
    public void ActElitePool_Returns_That_Acts_Pool(int actIndex, string mustInclude)
    {
        var pool = Catalog.ActElitePool(actIndex);
        Assert.Contains(mustInclude, pool);
        foreach (var e in pool) Assert.True(Catalog.IsKnownEliteEncounter(e));
    }

    [Fact]
    public void ActElitePool_Clamps_Out_Of_Range()
    {
        Assert.Equal(Catalog.ActElitePool(0), Catalog.ActElitePool(-5));
        Assert.Equal(Catalog.ActElitePool(Catalog.ActThemes.Count - 1), Catalog.ActElitePool(99));
    }

    [Fact]
    public void SummarizeSpecs_Groups_And_Counts()
    {
        var s = Companion.SummarizeSpecs(new[] { "StrikeIronclad", "StrikeIronclad", "Bash" });
        Assert.Contains("2x StrikeIronclad", s);
        Assert.Contains("1x Bash", s);
        Assert.Equal("(empty)", Companion.SummarizeSpecs(System.Array.Empty<string>()));
    }

    /// <summary>Build a custom Context the same way RunCustom does, for a given character / act / deck.</summary>
    private static Companion.Context Custom(string charKey, int actIndex, List<string> deck)
    {
        var p = Catalog.FindCharacter(charKey)!;
        var relics = p.StarterRelic is { } r && Catalog.IsModelledRelic(r) ? new List<string> { r } : new List<string>();
        var run = new RunState(0, actIndex, $"ACT.{Catalog.ActThemes[actIndex]}", p.CharacterId,
            p.StartingHp, p.StartingHp, p.MaxEnergy,
            deck.Select(s => new CardEntry(s.Split('+')[0], 0, false)).ToList(),
            relics, System.Array.Empty<string>(), null, 1, 0);
        return Companion.BuildContext(run, "(custom)", deck.ToList(), relics,
            Catalog.ActElitePool(actIndex), new List<string>(), Companion.SummarizeSpecs(deck));
    }

    [Fact]
    public void BuildContext_From_Starter_Deck_Is_Scorable()
    {
        var ctx = Custom("Ironclad", 0, Catalog.CharacterProfiles[0].StarterDeckSpecs());
        Assert.NotEmpty(ctx.StrengthPool);
        Assert.NotEmpty(ctx.Encounters);
        Assert.Equal(10, ctx.DeckSpecs.Count);                              // 5 Strike + 4 Defend + Bash
        var strength = Advisor.DeckStrength(ctx.DeckSpecs, ctx.StrengthPool, ctx.Run.MaxEnergy, ctx.RelicNames, Fast);
        Assert.InRange(strength, 0, 100);
    }

    [Fact]
    public void Adding_A_Strong_Card_Raises_Deck_Strength()
    {
        var baseDeck = Catalog.CharacterProfiles[0].StarterDeckSpecs();
        var withPower = new List<string>(baseDeck) { "DemonForm", "Inflame", "Inflame" };

        var b = Custom("Ironclad", 0, baseDeck);
        var w = Custom("Ironclad", 0, withPower);
        double sBase = Advisor.DeckStrength(b.DeckSpecs, b.StrengthPool, b.Run.MaxEnergy, b.RelicNames, Fast);
        double sWith = Advisor.DeckStrength(w.DeckSpecs, w.StrengthPool, w.Run.MaxEnergy, w.RelicNames, Fast);
        Assert.True(sWith > sBase, $"expected stronger deck to score higher: starter={sBase:F1}, +power={sWith:F1}");
    }

    [Fact]
    public void Each_Character_Starter_Solves_Against_Its_First_Act()
    {
        foreach (var p in Catalog.CharacterProfiles)
        {
            var ctx = Custom(p.Key, 0, p.StarterDeckSpecs());
            double s = Advisor.DeckStrength(ctx.DeckSpecs, ctx.StrengthPool, ctx.Run.MaxEnergy, ctx.RelicNames, Fast);
            Assert.InRange(s, 0, 100);
        }
    }
}
