using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>Batch-3 relics: stateless event-hook relics (combat-start / turn-numbered / on-play / on-exhaust /
/// end-of-turn / draw / power-amount). Literal-number checks of each effect.</summary>
public class RelicTests3
{
    private static (CombatState combat, Monster m) Fight(string relic, int playerHp = 80, int monsterHp = 80)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), playerHp, 80, 3, new[] { relic });
        var m = Monsters.CalcifiedCultist(hp: monsterHp);
        var combat = Catalog.SetupCombat(player, new[] { m });
        return (combat, m);
    }

    // ---- Combat-start grants ----

    [Fact] public void FakeAnchor_Grants_4_Block() => Assert.Equal(4, Fight("FakeAnchor").combat.Player.Block);

    [Fact] public void TwistedFunnel_Poisons_All_Enemies_4()
        => Assert.Equal(4, Fight("TwistedFunnel").m.GetPowerAmount("Poison"));

    [Theory] [InlineData("BloodSoakedRose")] [InlineData("PrismaticGem")] [InlineData("Sozu")]
    public void EnergyRelic_Grants_Plus_1_Max_Energy(string relic)
    {
        var (combat, _) = Fight(relic);
        CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(4, combat.Player.Energy);
    }

    [Fact] public void Fiddle_Draws_2_Extra()
        => Assert.Equal(7, CombatManager.TurnStartDrawCount(Fight("Fiddle").combat));

    // ---- Turn-numbered effects ----

    [Fact]
    public void FakeBloodVial_Heals_1_On_Turn_1()
    {
        var (combat, _) = Fight("FakeBloodVial", playerHp: 50);
        CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(51, combat.Player.CurrentHp);
    }

    [Fact]
    public void DivineDestiny_Grants_6_Stars_Turn_1()
    {
        var (combat, _) = Fight("DivineDestiny");
        CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(6, combat.Player.Stars);
    }

    [Fact]
    public void Bread_Loses_2_Turn1_Gains_1_After()
    {
        var (combat, _) = Fight("Bread");
        CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(1, combat.Player.Energy);                 // 3 - 2
        CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(4, combat.Player.Energy);                 // 3 + 1
    }

    [Fact]
    public void PaelsFlesh_Grants_Energy_From_Turn_3()
    {
        var (combat, _) = Fight("PaelsFlesh");
        CombatManager.BeginPlayerTurn(combat); CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(3, combat.Player.Energy);                 // turns 1-2 unaffected
        CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(4, combat.Player.Energy);                 // turn 3: +1
    }

    [Theory] [InlineData("CaptainsWheel", 3, 18)] [InlineData("HornCleat", 2, 14)]
    public void BlockRelic_Grants_On_Its_Turn(string relic, int turn, int block)
    {
        var (combat, _) = Fight(relic);
        for (int t = 1; t <= turn; t++) CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(block, combat.Player.Block);
    }

    [Fact]
    public void SparklingRouge_Grants_Str_And_Dex_Turn_3()
    {
        var (combat, _) = Fight("SparklingRouge");
        for (int t = 1; t <= 3; t++) CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(1, combat.Player.GetPowerAmount("Strength"));
        Assert.Equal(1, combat.Player.GetPowerAmount("Dexterity"));
    }

    [Fact]
    public void MercuryHourglass_Deals_3_To_All_Each_Turn()
    {
        var (combat, m) = Fight("MercuryHourglass");
        CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(77, m.CurrentHp);
    }

    [Fact]
    public void MrStruggles_Deals_TurnNumber_Damage()
    {
        var (combat, m) = Fight("MrStruggles");
        CombatManager.BeginPlayerTurn(combat);                 // turn 1: 1 dmg
        CombatManager.EndPlayerTurn(combat); CombatManager.BeginPlayerTurn(combat);   // turn 2: 2 dmg
        Assert.Equal(80 - 1 - 2, m.CurrentHp);
    }

    [Fact]
    public void RoyalPoison_Self_Damages_4_Turn_1()
    {
        var (combat, _) = Fight("RoyalPoison", playerHp: 50);
        CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(46, combat.Player.CurrentHp);
    }

    [Fact]
    public void RunicCapacitor_Adds_3_Orb_Slots_Turn_1()
    {
        var (combat, _) = Fight("RunicCapacitor");
        CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(3, combat.Player.OrbSlots);
    }

    // ---- Draw modifiers ----

    [Theory] [InlineData("BagOfPreparation")] [InlineData("RingOfTheSnake")]
    public void Turn1DrawRelic_Adds_2_On_Turn_1(string relic)
    {
        var (combat, _) = Fight(relic);
        combat.TurnNumber = 1; Assert.Equal(7, CombatManager.TurnStartDrawCount(combat));
        combat.TurnNumber = 2; Assert.Equal(5, CombatManager.TurnStartDrawCount(combat));
    }

    [Fact]
    public void RingOfTheDrake_Adds_2_For_First_3_Turns()
    {
        var (combat, _) = Fight("RingOfTheDrake");
        combat.TurnNumber = 3; Assert.Equal(7, CombatManager.TurnStartDrawCount(combat));
        combat.TurnNumber = 4; Assert.Equal(5, CombatManager.TurnStartDrawCount(combat));
    }

    [Fact]
    public void BigMushroom_Draws_2_Fewer_Turn_1()
    {
        var (combat, _) = Fight("BigMushroom");
        combat.TurnNumber = 1; Assert.Equal(3, CombatManager.TurnStartDrawCount(combat));
    }

    // ---- End-of-turn effects ----

    [Theory] [InlineData("Orichalcum", 6)] [InlineData("FakeOrichalcum", 3)]
    public void Orichalcum_Blocks_When_Zero_At_Turn_End(string relic, int block)
    {
        var (combat, _) = Fight(relic);
        CombatManager.BeginPlayerTurn(combat);
        CombatManager.EndPlayerTurn(combat);
        Assert.Equal(block, combat.Player.Block);
    }

    [Fact]
    public void CloakClasp_Blocks_By_Hand_Size()
    {
        var (combat, _) = Fight("CloakClasp");
        CombatManager.BeginPlayerTurn(combat);
        combat.Player.Hand.Add(new StrikeIronclad());
        combat.Player.Hand.Add(new DefendIronclad());
        CombatManager.EndPlayerTurn(combat);
        Assert.Equal(2, combat.Player.Block);
    }

    [Fact]
    public void RippleBasin_Blocks_4_When_No_Attacks()
    {
        var (combat, _) = Fight("RippleBasin");
        CombatManager.BeginPlayerTurn(combat);
        CombatManager.EndPlayerTurn(combat);
        Assert.Equal(4, combat.Player.Block);
    }

    [Fact]
    public void ScreamingFlagon_Deals_20_When_Hand_Empty()
    {
        var (combat, m) = Fight("ScreamingFlagon");
        CombatManager.BeginPlayerTurn(combat);
        CombatManager.EndPlayerTurn(combat);
        Assert.Equal(60, m.CurrentHp);
    }

    [Fact]
    public void LunarPastry_Grants_Star_At_Turn_End()
    {
        var (combat, _) = Fight("LunarPastry");
        CombatManager.BeginPlayerTurn(combat);
        CombatManager.EndPlayerTurn(combat);
        Assert.Equal(1, combat.Player.Stars);
    }

    // ---- On-play / on-exhaust / power-amount ----

    [Fact]
    public void IntimidatingHelmet_Blocks_4_On_Cost2_Play()
    {
        var (combat, m) = Fight("IntimidatingHelmet");
        CombatManager.BeginPlayerTurn(combat);
        combat.Player.Hand.Add(new Bash());                    // cost 2
        CombatManager.PlayCard(combat, combat.Player.Hand[^1], m);
        Assert.Equal(4, combat.Player.Block);
    }

    [Fact]
    public void IvoryTile_Gains_Energy_On_Cost3_Play()
    {
        var (combat, m) = Fight("IvoryTile");
        CombatManager.BeginPlayerTurn(combat);                 // energy 3
        combat.Player.Hand.Add(new Bludgeon());                // cost 3
        CombatManager.PlayCard(combat, combat.Player.Hand[^1], m);
        Assert.Equal(1, combat.Player.Energy);                 // 3 - 3 + 1
    }

    [Fact]
    public void DaughterOfTheWind_Blocks_1_Per_Attack()
    {
        var (combat, m) = Fight("DaughterOfTheWind");
        CombatManager.BeginPlayerTurn(combat);
        combat.Player.Hand.Add(new StrikeIronclad());
        CombatManager.PlayCard(combat, combat.Player.Hand[^1], m);
        Assert.Equal(1, combat.Player.Block);
    }

    [Fact]
    public void LostWisp_Deals_8_On_Power_Play()
    {
        var (combat, m) = Fight("LostWisp");
        CombatManager.BeginPlayerTurn(combat);
        combat.Player.Hand.Add(new Inflame());                 // a Power
        CombatManager.PlayCard(combat, combat.Player.Hand[^1], null);
        Assert.Equal(72, m.CurrentHp);
    }

    [Fact]
    public void CharonsAshes_Deals_3_On_Exhaust()
    {
        var (combat, m) = Fight("CharonsAshes");
        combat.Player.Hand.Add(new StrikeIronclad());
        Cmd.ExhaustFromHand(combat, combat.Player.Hand[^1]);
        Assert.Equal(77, m.CurrentHp);
    }

    [Fact]
    public void SneckoSkull_Boosts_Player_Poison_By_1()
    {
        var (combat, m) = Fight("SneckoSkull");
        Cmd.ApplyPower(combat, m, new PoisonPower(), 4, combat.Player);
        Assert.Equal(5, m.GetPowerAmount("Poison"));
    }

    // ---- Registration ----

    [Theory]
    [InlineData("FakeAnchor")] [InlineData("TwistedFunnel")] [InlineData("BloodSoakedRose")] [InlineData("PrismaticGem")]
    [InlineData("Sozu")] [InlineData("Fiddle")] [InlineData("FakeBloodVial")] [InlineData("DivineDestiny")]
    [InlineData("Bread")] [InlineData("PaelsFlesh")] [InlineData("CaptainsWheel")] [InlineData("HornCleat")]
    [InlineData("SparklingRouge")] [InlineData("MercuryHourglass")] [InlineData("MrStruggles")] [InlineData("RoyalPoison")]
    [InlineData("RunicCapacitor")] [InlineData("BagOfPreparation")] [InlineData("RingOfTheSnake")] [InlineData("RingOfTheDrake")]
    [InlineData("BigMushroom")] [InlineData("Orichalcum")] [InlineData("FakeOrichalcum")] [InlineData("CloakClasp")]
    [InlineData("RippleBasin")] [InlineData("ScreamingFlagon")] [InlineData("StoneCalendar")] [InlineData("LunarPastry")]
    [InlineData("IntimidatingHelmet")] [InlineData("IvoryTile")] [InlineData("DaughterOfTheWind")] [InlineData("LostWisp")]
    [InlineData("GamePiece")] [InlineData("CharonsAshes")] [InlineData("SneckoSkull")]
    public void Relic_Is_Registered_And_Buildable(string relic)
    {
        Assert.True(Catalog.IsModelledRelic(relic));
        Assert.Equal(relic, Catalog.BuildRelic(relic).Id);
    }
}
