using System.Collections.Generic;
using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Ranwid;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Oracle-free "diagnostic probes" for the per-character heuristic redesign (see
/// <c>docs/per-character-heuristic-research.md</c>). Each asserts that the deck-strength metric — the unit of
/// truth behind every removal/upgrade/reward suggestion — does not MIS-RANK a build-defining card. They need no
/// exact oracle (they run on the real advice path at the production advice budget), so they gate the large-deck
/// regime where no ground truth exists, and they catch exactly the failure class the redesign targets: a leaf
/// heuristic that can't see a character's resource undervalues the cards that build it.
///
/// Convention: a probe that already holds is a live <c>[Fact]</c>; one that is a KNOWN failure the next phase
/// fixes is <c>[Fact(Skip="Phase N target…")]</c> — un-skipped (turned into a live gate) when that phase lands.
/// </summary>
public class PerCharacterProbes
{
    private readonly ITestOutputHelper _out;
    public PerCharacterProbes(ITestOutputHelper o) => _out = o;

    // The production advice ranking budget (Advisor.AdviceTrials), so probes see exactly what the advice does.
    private static EvalOptions AdviceOpts => new() { BudgetSeconds = 0.0, MctsTrials = Advisor.AdviceTrials, Seed = 1 };

    /// <summary>Deck strength of the Necrobinder starter deck, and of the deck with one copy of
    /// <paramref name="cardToRemove"/> cut, scored against the act-1 elite pool at the advice budget.</summary>
    private (double full, double without) NecrobinderStarterStrength(string cardToRemove)
    {
        var profile = Catalog.FindCharacter("Necrobinder")!;
        var ctx = Companion.BuildCustom(profile, actIndex: 0, asc: 0, hp: profile.StartingHp, deck: profile.StarterDeckSpecs());
        var without = new List<string>(ctx.DeckSpecs);
        Assert.True(without.Remove(cardToRemove), $"{cardToRemove} not in the Necrobinder starter deck");

        double full = Advisor.DeckStrength(ctx.DeckSpecs, ctx.StrengthPool, ctx.Run.MaxEnergy, ctx.RelicNames, AdviceOpts);
        double cut = Advisor.DeckStrength(without, ctx.StrengthPool, ctx.Run.MaxEnergy, ctx.RelicNames, AdviceOpts);
        _out.WriteLine($"Necrobinder starter: full {full:F2}  |  −{cardToRemove} {cut:F2}  (Δ {cut - full:+0.00})");
        return (full, cut);
    }

    /// <summary>LIVE: Unleash (6 + Osty HP, an Osty attack) is worth more than a basic Strike, so cutting it must
    /// make the deck clearly WEAKER. The leaf already credits Unleash's immediate on-play damage (the rollout
    /// applies it), so this holds today — and must keep holding after the Osty redesign.</summary>
    [Fact]
    public void Necrobinder_CuttingUnleash_Weakens_Deck()
    {
        var (full, without) = NecrobinderStarterStrength("Unleash");
        Assert.True(without < full - 1.0,
            $"cutting Unleash should weaken the deck, but strength went {full:F2} → {without:F2}");
    }

    /// <summary>PHASE-1 TARGET (currently RED): Bodyguard (Summon +5 Osty HP) is a premier Osty scaler — growing
    /// Osty raises every future Unleash AND every future DieForYou block — yet the Osty-blind leaf treats it as a
    /// dead play, so cutting it currently RAISES measured strength (the originating bug). After the Phase-1 Osty
    /// enrichment, cutting Bodyguard must no longer look like an improvement. Un-skip when Phase 1 lands.</summary>
    [Fact(Skip = "Phase 1 target: Osty-blind leaf ranks Bodyguard as an improving cut; un-skip after Osty enrichment")]
    public void Necrobinder_CuttingBodyguard_DoesNotImprove_Deck()
    {
        var (full, without) = NecrobinderStarterStrength("Bodyguard");
        Assert.True(without <= full + 0.5,
            $"cutting Bodyguard should not improve the deck, but strength went {full:F2} → {without:F2} (Osty-blind leaf)");
    }
}
