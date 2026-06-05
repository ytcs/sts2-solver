using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>Batch-5 relics: every-N-turns relics (stateless, read TurnNumber) and damage/stars/play-count
/// reactors (hidden hashed-state powers).</summary>
public class RelicTests5
{
    private static (CombatState combat, Monster m) Fight(string relic, int playerHp = 80, int maxEnergy = 3)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), playerHp, 80, maxEnergy, new[] { relic });
        var m = Monsters.CalcifiedCultist(hp: 200);
        var combat = Catalog.SetupCombat(player, new[] { m });
        return (combat, m);
    }

    // ---- Every-N-turns ----

    [Theory] [InlineData("HappyFlower", 3)] [InlineData("FakeHappyFlower", 5)]
    public void EnergyEveryNTurns_Grants_On_Its_Multiple(string relic, int period)
    {
        var (combat, _) = Fight(relic);
        for (int t = 1; t < period; t++) { CombatManager.BeginPlayerTurn(combat); Assert.Equal(3, combat.Player.Energy); }
        CombatManager.BeginPlayerTurn(combat);                 // turn == period
        Assert.Equal(4, combat.Player.Energy);
    }

    [Fact]
    public void Pendulum_Draws_Extra_Every_3rd_Turn()
    {
        var (combat, _) = Fight("Pendulum");
        combat.TurnNumber = 2; Assert.Equal(5, CombatManager.TurnStartDrawCount(combat));
        combat.TurnNumber = 3; Assert.Equal(6, CombatManager.TurnStartDrawCount(combat));
    }

    [Fact]
    public void PollinousCore_Draws_2_Extra_Every_4th_Turn()
    {
        var (combat, _) = Fight("PollinousCore");
        combat.TurnNumber = 3; Assert.Equal(5, CombatManager.TurnStartDrawCount(combat));
        combat.TurnNumber = 4; Assert.Equal(7, CombatManager.TurnStartDrawCount(combat));
    }

    // ---- Damage reactors ----

    [Fact]
    public void CentennialPuzzle_Draws_3_On_First_Unblocked_Damage()
    {
        var player = Catalog.BuildPlayer(Enumerable.Range(0, 12).Select(_ => (CardModel)new StrikeIronclad()).ToList(),
                                         80, 80, 3, new[] { "CentennialPuzzle" });
        var m = Monsters.CalcifiedCultist(hp: 200);
        var combat = Catalog.SetupCombat(player, new[] { m });
        combat.Rng = new Rng(1);
        CombatManager.BeginPlayerTurn(combat);
        int draw = combat.Player.DrawPile.Count;
        Cmd.Attack(combat, m, combat.Player, 6, ValueProp.Move, null);   // first unblocked → draw 3
        Assert.Equal(draw - 3, combat.Player.DrawPile.Count);
        Cmd.Attack(combat, m, combat.Player, 6, ValueProp.Move, null);   // second hit → no more draw
        Assert.Equal(draw - 3, combat.Player.DrawPile.Count);
    }

    [Fact]
    public void DemonTongue_Heals_First_Unblocked_Damage_Each_Turn()
    {
        var (combat, m) = Fight("DemonTongue", playerHp: 50);
        CombatManager.BeginPlayerTurn(combat);
        Cmd.Attack(combat, m, combat.Player, 10, ValueProp.Move, null); // lose 10, heal 10 → net 50
        Assert.Equal(50, combat.Player.CurrentHp);
        Cmd.Attack(combat, m, combat.Player, 10, ValueProp.Move, null); // second this turn → no heal
        Assert.Equal(40, combat.Player.CurrentHp);
        CombatManager.EndPlayerTurn(combat); CombatManager.BeginPlayerTurn(combat);
        Cmd.Attack(combat, m, combat.Player, 10, ValueProp.Move, null); // new turn → heals again
        Assert.Equal(40, combat.Player.CurrentHp);
    }

    [Fact]
    public void BeatingRemnant_Caps_HP_Loss_At_20_Per_Turn()
    {
        var (combat, m) = Fight("BeatingRemnant");
        CombatManager.BeginPlayerTurn(combat);
        Cmd.Attack(combat, m, combat.Player, 30, ValueProp.Move, null); // capped to 20 → 60
        Assert.Equal(60, combat.Player.CurrentHp);
        Cmd.Attack(combat, m, combat.Player, 15, ValueProp.Move, null); // already 20 lost → 0 more
        Assert.Equal(60, combat.Player.CurrentHp);
        CombatManager.EndPlayerTurn(combat); CombatManager.BeginPlayerTurn(combat);
        Cmd.Attack(combat, m, combat.Player, 30, ValueProp.Move, null); // cap resets → -20 → 40
        Assert.Equal(40, combat.Player.CurrentHp);
    }

    // ---- Block / play-count reactors ----

    [Fact]
    public void Vambrace_Doubles_First_Block_Card()
    {
        var (combat, m) = Fight("Vambrace", maxEnergy: 99);
        CombatManager.BeginPlayerTurn(combat);
        combat.Player.Hand.Add(new DefendIronclad());
        CombatManager.PlayCard(combat, combat.Player.Hand[^1], null);   // Defend 5 → doubled to 10
        Assert.Equal(10, combat.Player.Block);
        combat.Player.Hand.Add(new DefendIronclad());
        CombatManager.PlayCard(combat, combat.Player.Hand[^1], null);   // second → normal +5
        Assert.Equal(15, combat.Player.Block);
    }

    [Fact]
    public void ThrowingAxe_Plays_First_Card_Twice()
    {
        var (combat, m) = Fight("ThrowingAxe", maxEnergy: 99);
        CombatManager.BeginPlayerTurn(combat);
        combat.Player.Hand.Add(new StrikeIronclad());
        CombatManager.PlayCard(combat, combat.Player.Hand[^1], m);      // Strike 6 ×2 plays = 12
        Assert.Equal(200 - 12, m.CurrentHp);
        combat.Player.Hand.Add(new StrikeIronclad());
        CombatManager.PlayCard(combat, combat.Player.Hand[^1], m);      // second card → 6
        Assert.Equal(200 - 12 - 6, m.CurrentHp);
    }

    [Fact]
    public void GalacticDust_Power_Blocks_10_Per_10_Stars()
    {
        var (combat, _) = Fight("GalacticDust");
        var p = new GalacticDustPower { Owner = combat.Player };
        combat.Player.Powers.Add(p);
        p.AfterStarsSpent(combat, 7);
        Assert.Equal(0, combat.Player.Block);                  // <10 → no block yet
        p.AfterStarsSpent(combat, 5);                          // total 12 → one 10-block grant
        Assert.Equal(10, combat.Player.Block);
    }

    [Fact]
    public void MiniRegent_Power_Grants_Strength_Once_Per_Turn()
    {
        var (combat, _) = Fight("MiniRegent");
        var p = new MiniRegentPower { Owner = combat.Player };
        combat.Player.Powers.Add(p);
        p.AfterStarsSpent(combat, 2);
        Assert.Equal(1, combat.Player.GetPowerAmount("Strength"));
        p.AfterStarsSpent(combat, 2);                          // same turn → no more
        Assert.Equal(1, combat.Player.GetPowerAmount("Strength"));
        p.AfterSideTurnStart(combat, CombatSide.Player);       // new turn
        p.AfterStarsSpent(combat, 2);
        Assert.Equal(2, combat.Player.GetPowerAmount("Strength"));
    }

    // ---- Registration ----

    [Theory]
    [InlineData("HappyFlower")] [InlineData("FakeHappyFlower")] [InlineData("Pendulum")] [InlineData("PollinousCore")]
    [InlineData("CentennialPuzzle")] [InlineData("DemonTongue")] [InlineData("GalacticDust")] [InlineData("MiniRegent")]
    [InlineData("BeatingRemnant")] [InlineData("Vambrace")] [InlineData("ThrowingAxe")]
    public void Relic_Is_Registered_And_Buildable(string relic)
    {
        Assert.True(Catalog.IsModelledRelic(relic));
        Assert.Equal(relic, Catalog.BuildRelic(relic).Id);
    }
}
