using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

public class SolverTests
{
    private readonly ITestOutputHelper _out;
    public SolverTests(ITestOutputHelper o) => _out = o;

    private static CombatState IroncladVsCultist(int monsterHp = 41, int playerHp = 80)
    {
        var player = Catalog.BuildPlayer(Catalog.IroncladStarterDeck(), playerHp, 80, relics: new[] { "BurningBlood" });
        var monster = Monsters.CalcifiedCultist(hp: monsterHp);
        return Catalog.SetupCombat(player, new[] { monster });
    }

    [Fact]
    public void IroncladStarter_Beats_Cultist_With_Certainty()
    {
        var solver = new Solver();
        var value = solver.Solve(IroncladVsCultist());
        _out.WriteLine($"Ironclad starter vs CalcifiedCultist(41 HP): {value}");
        _out.WriteLine($"States evaluated: {solver.StatesEvaluated}");

        // The cultist's first turn is a no-damage Incantation, so a starter deck should win every time.
        Assert.Equal(1.0, value.Win, 6);
        // And it should lose some HP (the fight isn't free) but well under the player's pool.
        Assert.InRange(value.Loss, 0.0, 40.0);
    }

    [Fact]
    public void LowerMonsterHp_Never_Costs_More_Hp()
    {
        var solver = new Solver();
        var hard = solver.Solve(IroncladVsCultist(monsterHp: 41));
        var easy = solver.Solve(IroncladVsCultist(monsterHp: 30));
        _out.WriteLine($"41 HP cultist: {hard}");
        _out.WriteLine($"30 HP cultist: {easy}");
        Assert.True(easy.Loss <= hard.Loss + 1e-9, "Weaker monster should not cost more expected HP.");
    }

    [Fact]
    public void Reports_An_Opening_Turn_Plan()
    {
        var solver = new Solver();
        var setup = IroncladVsCultist();
        solver.Solve(setup);
        // Take the most probable opening hand and show its optimal line.
        var (_, state) = solver.OpeningStates(setup, Player.CardsDrawnPerTurn)
            .OrderByDescending(o => o.prob).First();
        var plan = solver.BestTurnPlan(state);
        _out.WriteLine("Opening hand: " + string.Join(", ", state.Player.Hand.Select(c => c.StateKey())));
        _out.WriteLine("Optimal line: " + string.Join(" | ", plan));
        Assert.NotEmpty(plan);
    }
}
