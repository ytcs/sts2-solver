using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// The sampler's analogue of trace-validation: on every fight small enough to solve exactly, the
/// MCTS solver must converge to the exact (P_win, E[HP loss]) oracle within tolerance.
/// </summary>
public class MctsTests
{
    private readonly ITestOutputHelper _out;
    public MctsTests(ITestOutputHelper o) => _out = o;

    private static CombatState IroncladVsCultist(int monsterHp = 41, int playerHp = 80)
    {
        var player = Catalog.BuildPlayer(Catalog.IroncladStarterDeck(), playerHp, 80, relics: new[] { "BurningBlood" });
        return Catalog.SetupCombat(player, new[] { Monsters.CalcifiedCultist(hp: monsterHp) });
    }

    private void AssertConverges(CombatState setup, string name, int trials = 60_000,
        double winTol = 0.02, double lossTol = 1.5, bool actionWidening = false)
    {
        var exact = new Solver().Solve(setup.Clone());
        var mcts = new MctsSolver(new MctsOptions { Trials = trials, Seed = 1, ActionWidening = actionWidening });
        var approx = mcts.Solve(setup.Clone());

        _out.WriteLine($"{name}");
        _out.WriteLine($"  exact : {exact}");
        _out.WriteLine($"  mcts  : {approx}   ({mcts.TrialsRun} trials, {mcts.NodesCreated} nodes)");
        _out.WriteLine($"  delta : win {Math.Abs(approx.Win - exact.Win):F4}, loss {Math.Abs(approx.Loss - exact.Loss):F3}");

        Assert.True(Math.Abs(approx.Win - exact.Win) <= winTol,
            $"{name}: win prob off by {Math.Abs(approx.Win - exact.Win):F4} (exact {exact.Win}, mcts {approx.Win})");
        Assert.True(Math.Abs(approx.Loss - exact.Loss) <= lossTol,
            $"{name}: E[HP loss] off by {Math.Abs(approx.Loss - exact.Loss):F3} (exact {exact.Loss}, mcts {approx.Loss})");
    }

    [Fact]
    public void Converges_On_Cultist() => AssertConverges(IroncladVsCultist(), "Ironclad vs CalcifiedCultist(41)");

    [Fact]
    public void Converges_On_Weak_Cultist() => AssertConverges(IroncladVsCultist(monsterHp: 30), "Ironclad vs CalcifiedCultist(30)");

    [Fact]
    public void Converges_On_Byrdonis()
    {
        var player = Catalog.BuildPlayer(Catalog.IroncladStarterDeck(), 80, 80, relics: new[] { "BurningBlood" });
        var setup = Catalog.SetupCombat(player, new[] { Monsters.Byrdonis() });
        AssertConverges(setup, "Ironclad vs Byrdonis(84) elite", trials: 120_000, winTol: 0.03, lossTol: 2.5);
    }

    [Fact]
    public void Hybrid_Matches_Exact_On_Cultist()
    {
        var setup = IroncladVsCultist();
        var exact = new Solver().Solve(setup.Clone());
        var hybrid = new MctsSolver(new MctsOptions { Trials = 20_000, Seed = 2, HybridExactBelow = 2_000 })
            .Solve(setup.Clone());
        _out.WriteLine($"exact {exact}  |  hybrid {hybrid}");
        Assert.True(Math.Abs(hybrid.Win - exact.Win) <= 0.01);
        Assert.True(Math.Abs(hybrid.Loss - exact.Loss) <= 1.0);
    }

    // ----- Action progressive widening + PUCT: must converge to the SAME oracle as classic UCT* -----
    // (asymptotically consistent — every candidate opens as N→∞ — and empirically MORE sample-efficient on
    // wide-branching fights, since PUCT concentrates the trial budget on the prior-favoured lines).

    [Fact]
    public void Apw_Converges_On_Cultist() =>
        AssertConverges(IroncladVsCultist(), "APW: Ironclad vs CalcifiedCultist(41)", actionWidening: true);

    [Fact]
    public void Apw_Converges_On_Weak_Cultist() =>
        AssertConverges(IroncladVsCultist(monsterHp: 30), "APW: Ironclad vs CalcifiedCultist(30)", actionWidening: true);

    [Fact]
    public void Apw_Converges_On_Byrdonis()
    {
        var player = Catalog.BuildPlayer(Catalog.IroncladStarterDeck(), 80, 80, relics: new[] { "BurningBlood" });
        var setup = Catalog.SetupCombat(player, new[] { Monsters.Byrdonis() });
        AssertConverges(setup, "APW: Ironclad vs Byrdonis(84) elite", trials: 120_000, winTol: 0.03, lossTol: 2.5,
            actionWidening: true);
    }

    // The razor-thin partial-survival fight where classic UCB under-samples the coordinated survival line:
    // APW must recover the exact survival probability (calibration measured 22.1% vs 22.1%).
    [Fact]
    public void Apw_Converges_On_RazorThin_Block_Fixture()
    {
        var f = CalibrationFixtures.All.First(x => x.Name == "block/Defends-vs-Byrdonis");
        var exact = new Solver { MaxTurns = f.MaxTurns }.Solve(f.Setup());
        Assert.InRange(exact.Win, 0.01, 0.99);   // genuinely partial survival
        var mcts = new MctsSolver(new MctsOptions
            { Trials = 40_000, Seed = 1, MaxTurns = f.MaxTurns, ActionWidening = true });
        var approx = mcts.Solve(f.Setup());
        _out.WriteLine($"exact {exact}  |  APW {approx}  ({mcts.NodesCreated} nodes)");
        Assert.True(Math.Abs(approx.Win - exact.Win) <= 0.04,
            $"survival off by {Math.Abs(approx.Win - exact.Win):F4} (exact {exact.Win}, apw {approx.Win})");
        Assert.True(Math.Abs(approx.Loss - exact.Loss) <= 2.0);
    }
}
