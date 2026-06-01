using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Soundness gate for <see cref="HorizonBound"/>: the derived horizon must never change the exact value —
/// i.e. exact-solving at the auto-bound must equal exact-solving at a much larger horizon. A bound cut too
/// short would silently drop winning lines, so this is the critical test. Also checks the bound actually
/// fires on ramping single-enemy fights and bails (returns the default) on the cases it can't prove.
/// </summary>
public class HorizonBoundTests
{
    private readonly ITestOutputHelper _out;
    public HorizonBoundTests(ITestOutputHelper o) => _out = o;

    private static CombatState Fixture(IEnumerable<CardModel> deck, int hp, Monster monster, int energy = 3) =>
        Catalog.SetupCombat(Catalog.BuildPlayer(deck.ToList(), hp, hp, energy, new[] { "BurningBlood" }), new[] { monster });

    private static List<CardModel> NoBlockDeck() =>
        Enumerable.Range(0, 5).Select(_ => (CardModel)new StrikeIronclad()).ToList();

    private static List<CardModel> BlockDeck() => new()
    {
        new ShrugItOff(), new DefendIronclad(), new DefendIronclad(),
        new DefendIronclad(), new StrikeIronclad(), new StrikeIronclad(),
    };

    // A deck whose only mitigation beyond block is applying Weak (Neutralize). Pre-v2 this BAILED (Weak
    // could push real incoming below the trajectory); v2 models permanent Weak (the max the player could
    // sustain — the safe, longer-horizon direction), so the bound now fires and stays sound.
    private static List<CardModel> WeakDeck() => new()
    {
        new Neutralize(), new DefendIronclad(), new DefendIronclad(),
        new DefendIronclad(), new StrikeIronclad(), new StrikeIronclad(),
    };

    // Uppercut applies BOTH Weak and Vulnerable: Weak is modelled (incoming↓), Vulnerable only speeds the
    // player's kills (the idle trajectory ignores it), so the deck no longer bails.
    private static List<CardModel> WeakVulnDeck() => new()
    {
        new Uppercut(), new StrikeIronclad(), new StrikeIronclad(),
        new StrikeIronclad(), new StrikeIronclad(), new DefendIronclad(),
    };

    /// <summary>The core guarantee, on fights small enough that exact at a large horizon is still fast: the
    /// auto-bounded exact value is identical to the large-horizon exact value.</summary>
    [Theory]
    // Pure-survival race (unkillable ramping Byrdonis): low HP ⇒ short fight, the bound fires early.
    [InlineData("survive/Byrdonis", 22)]
    // Realistic low-survival block fight (this is the calibration block fixture's shape).
    [InlineData("block/Byrdonis", 40)]
    // v2: a Weak-applying deck (Neutralize) — previously bailed, now bounds soundly with permanent-Weak.
    [InlineData("weak/Byrdonis", 40)]
    // v2: a Weak+Vulnerable deck (Uppercut) — Weak modelled, Vulnerable ignored by the idle trajectory.
    [InlineData("weakvuln/Byrdonis", 50)]
    public void AutoHorizon_Preserves_Exact_Value(string name, int hp)
    {
        const int big = 40;
        CombatState Build() => name switch
        {
            "survive/Byrdonis" => Fixture(NoBlockDeck(), hp, Monsters.Byrdonis(hp: 9999)),
            "block/Byrdonis" => Fixture(BlockDeck(), hp, Monsters.Byrdonis(hp: 58)),
            "weak/Byrdonis" => Fixture(WeakDeck(), hp, Monsters.Byrdonis(hp: 58)),
            "weakvuln/Byrdonis" => Fixture(WeakVulnDeck(), hp, Monsters.Byrdonis(hp: 52)),
            _ => throw new ArgumentOutOfRangeException(nameof(name)),
        };

        int bound = HorizonBound.Compute(Build(), big);
        var atBound = new Solver { MaxTurns = bound }.Solve(Build());
        var atBig = new Solver { MaxTurns = big }.Solve(Build());
        _out.WriteLine($"{name}: bound={bound} (default {big})  exact@bound={atBound}  exact@big={atBig}");

        Assert.True(bound < big, $"{name}: expected the bound to fire (got {bound}, default {big})");
        Assert.Equal(atBig.Win, atBound.Win, 6);
        Assert.Equal(atBig.Loss, atBound.Loss, 4);
    }

    /// <summary>v2: a multi-enemy fight the deck can't survive (two ramping cultists vs a block-less deck)
    /// now bounds via kill-order reasoning — and the bound must be sound (exact value unchanged vs a larger
    /// horizon). Pre-v2 this bailed unconditionally.</summary>
    [Fact]
    public void MultiEnemy_Bound_Fires_And_Is_Sound()
    {
        CombatState Build() => Catalog.SetupCombat(
            Catalog.BuildPlayer(NoBlockDeck(), 40, 40, 3, new[] { "BurningBlood" }),
            new[] { Monsters.CalcifiedCultist(), Monsters.CalcifiedCultist() });

        const int big = 40;
        int bound = HorizonBound.Compute(Build(), big);
        _out.WriteLine($"multi-enemy bound={bound} (default {big})");
        Assert.True(bound < big, $"expected the multi-enemy bound to fire (got {bound})");

        var atBound = new Solver { MaxTurns = bound }.Solve(Build());
        var atBig = new Solver { MaxTurns = bound + 8 }.Solve(Build());
        _out.WriteLine($"  exact@{bound}={atBound}   exact@{bound + 8}={atBig}");
        Assert.Equal(atBig.Win, atBound.Win, 6);
        Assert.Equal(atBig.Loss, atBound.Loss, 4);
    }

    /// <summary>The multi-enemy bound still bails (to the default) when the deck disqualifies the trajectory
    /// — here a Power card, which could reduce incoming in ways the idle trajectory doesn't model.</summary>
    [Fact]
    public void Bails_On_Multiple_Enemies_With_Power_Card()
    {
        var deck = new List<CardModel> { new Inflame(), new StrikeIronclad(), new StrikeIronclad() };
        var setup = Catalog.SetupCombat(
            Catalog.BuildPlayer(deck, 40, 40, 3, new[] { "BurningBlood" }),
            new[] { Monsters.CalcifiedCultist(), Monsters.CalcifiedCultist() });
        Assert.Equal(40, HorizonBound.Compute(setup, 40));
    }

    [Fact]
    public void Bails_On_Power_Card_Deck()
    {
        var deck = new List<CardModel> { new Inflame(), new StrikeIronclad(), new StrikeIronclad() };
        var setup = Fixture(deck, 40, Monsters.CalcifiedCultist());
        Assert.Equal(40, HorizonBound.Compute(setup, 40));
    }
}
