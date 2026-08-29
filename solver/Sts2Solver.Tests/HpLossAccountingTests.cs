using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Regression gate for HP-loss accounting: play-time HP (Bloodletting, thorns, …) must appear in the
/// lexicographic value, and a horizon miss must charge remaining HP as a loss — matching death /
/// <see cref="LossCertificate"/>. Exact and MCTS stay aligned.
/// </summary>
public class HpLossAccountingTests
{
    private readonly ITestOutputHelper _out;
    public HpLossAccountingTests(ITestOutputHelper o) => _out = o;

    /// <summary>0-energy deck: Bloodletting is the only way to afford Strike, which kills a 6 HP cultist
    /// before it acts. Combat HP lost is exactly Bloodletting's 3.</summary>
    private static CombatState BloodlettingKill()
    {
        var deck = new List<CardModel> { new Bloodletting(), new StrikeIronclad() };
        var player = Catalog.BuildPlayer(deck, currentHp: 50, maxHp: 50, maxEnergy: 0);
        return Catalog.SetupCombat(player, new[] { Monsters.CalcifiedCultist(hp: 6) });
    }

    /// <summary>Unkillable cultist, first move is Incantation (0 damage). One player turn then the
    /// horizon fires — the player still has full HP, which must be charged as the loss.</summary>
    private static CombatState HorizonTimeout(int hp) =>
        Catalog.SetupCombat(
            Catalog.BuildPlayer(new List<CardModel> { new StrikeIronclad() }, hp, hp, maxEnergy: 3),
            new[] { Monsters.CalcifiedCultist(hp: 9999) });

    [Fact]
    public void Bloodletting_SelfDamage_Is_Counted_In_Expected_Loss()
    {
        var value = new Solver().Solve(BloodlettingKill());
        _out.WriteLine(value.ToString());
        Assert.Equal(1.0, value.Win, 6);
        Assert.Equal(3.0, value.Loss, 6);
    }

    [Fact]
    public void Horizon_Timeout_Charges_Remaining_Hp()
    {
        const int hp = 40;
        var value = new Solver { MaxTurns = 1 }.Solve(HorizonTimeout(hp));
        _out.WriteLine(value.ToString());
        Assert.Equal(0.0, value.Win, 6);
        Assert.Equal(hp, value.Loss, 6);
    }

    [Fact]
    public void Mcts_Matches_Exact_On_Bloodletting()
    {
        var setup = BloodlettingKill();
        var exact = new Solver().Solve(setup.Clone());
        var mcts = new MctsSolver(new MctsOptions { Trials = 8_000, Seed = 1 }).Solve(setup.Clone());
        _out.WriteLine($"exact {exact}  |  mcts {mcts}");
        Assert.Equal(1.0, exact.Win, 6);
        Assert.Equal(3.0, exact.Loss, 6);
        Assert.True(Math.Abs(mcts.Win - exact.Win) <= 0.01, $"win {mcts.Win} vs {exact.Win}");
        Assert.True(Math.Abs(mcts.Loss - exact.Loss) <= 0.05, $"loss {mcts.Loss} vs {exact.Loss}");
    }

    [Fact]
    public void Mcts_Matches_Exact_On_Horizon_Timeout()
    {
        const int hp = 40;
        var setup = HorizonTimeout(hp);
        var exact = new Solver { MaxTurns = 1 }.Solve(setup.Clone());
        var mcts = new MctsSolver(new MctsOptions { Trials = 4_000, Seed = 1, MaxTurns = 1 }).Solve(setup.Clone());
        _out.WriteLine($"exact {exact}  |  mcts {mcts}");
        Assert.Equal(0.0, exact.Win, 6);
        Assert.Equal(hp, exact.Loss, 6);
        Assert.True(Math.Abs(mcts.Win - exact.Win) <= 0.01);
        Assert.True(Math.Abs(mcts.Loss - exact.Loss) <= 0.05);
    }

    [Fact]
    public void Affordable_Strike_Is_Preferred_Over_Bloodletting()
    {
        // 3 energy: Strike alone kills. Bloodletting would win too but costs 3 HP — must not be chosen.
        var deck = new List<CardModel> { new Bloodletting(), new StrikeIronclad() };
        var player = Catalog.BuildPlayer(deck, currentHp: 50, maxHp: 50, maxEnergy: 3);
        var setup = Catalog.SetupCombat(player, new[] { Monsters.CalcifiedCultist(hp: 6) });
        var solver = new Solver();
        var value = solver.Solve(setup);
        _out.WriteLine(value.ToString());
        Assert.Equal(1.0, value.Win, 6);
        Assert.Equal(0.0, value.Loss, 6);
    }
}
