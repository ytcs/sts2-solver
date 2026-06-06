using Sts2Solver.Ranwid;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>Tests for the advisor's ergonomic card-name resolution (auto-correct + auto-complete). Pure logic,
/// no console — the interactive line editor that calls this is exercised manually.</summary>
public class CardNameMatcherTests
{
    [Fact]
    public void Exact_Match_Is_Not_A_Correction()
    {
        var m = CardNameMatcher.Resolve("Bludgeon");
        Assert.Equal("Bludgeon", m.Canonical);
        Assert.False(m.Corrected);
    }

    [Fact]
    public void Case_And_Separators_Are_Ignored()
    {
        // "iron wave" normalises to the same key as IronWave — an exact match, no correction needed.
        var m = CardNameMatcher.Resolve("iron wave");
        Assert.Equal("IronWave", m.Canonical);
        Assert.False(m.Corrected);
    }

    [Fact]
    public void Unique_Prefix_Auto_Completes()
    {
        var m = CardNameMatcher.Resolve("demonfo");        // only DemonForm starts with this
        Assert.Equal("DemonForm", m.Canonical);
        Assert.True(m.Corrected);
    }

    [Fact]
    public void Typo_Is_Auto_Corrected()
    {
        var m = CardNameMatcher.Resolve("bludgon");        // edit distance 1 from Bludgeon
        Assert.Equal("Bludgeon", m.Canonical);
        Assert.True(m.Corrected);
    }

    [Fact]
    public void Upgrade_Suffix_Is_Preserved()
    {
        var m = CardNameMatcher.Resolve("bludgon+1");
        Assert.Equal("Bludgeon+1", m.Canonical);
        Assert.True(m.Corrected);
    }

    [Fact]
    public void Bare_Plus_Marks_A_Single_Upgrade()
    {
        var m = CardNameMatcher.Resolve("Anger+");           // quick "this reward is upgraded" shorthand
        Assert.Equal("Anger+1", m.Canonical);
        Assert.Equal("Anger+2", CardNameMatcher.Resolve("Anger++").Canonical);
        // and it still resolves through auto-correct on the base name:
        Assert.Equal("Bludgeon+1", CardNameMatcher.Resolve("bludgon+").Canonical);
    }

    [Fact]
    public void Ambiguous_Prefix_Returns_Suggestions_Not_A_Guess()
    {
        var m = CardNameMatcher.Resolve("Strike");         // StrikeIronclad, StrikeSilent, …
        Assert.Null(m.Canonical);
        Assert.Contains("StrikeIronclad", m.Suggestions);
        Assert.Contains("StrikeSilent", m.Suggestions);
    }

    [Fact]
    public void Garbage_Returns_No_Match_But_Offers_Suggestions()
    {
        var m = CardNameMatcher.Resolve("zzzzzzzzzz");
        Assert.Null(m.Canonical);
        Assert.NotEmpty(m.Suggestions);
    }

    [Fact]
    public void Complete_Lists_Prefix_Matches()
    {
        var hits = CardNameMatcher.Complete("def");
        Assert.Contains("DefendIronclad", hits);
        Assert.Contains("DefendSilent", hits);
    }

    [Fact]
    public void Colorless_Cards_Are_In_The_Vocabulary()
    {
        // Colorless cards were added to Catalog.CardPool; the matcher must see them.
        var m = CardNameMatcher.Resolve("apotheosis");
        Assert.Equal("Apotheosis", m.Canonical);
    }
}
