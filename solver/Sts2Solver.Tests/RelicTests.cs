using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>Literal-number checks of the batch-1 combat relics. Each relic reuses an existing power or installs
/// a hidden relic power; these verify the combat-start grants, the turn-numbered effects, and the passive
/// damage/energy modifiers, plus that every entry is registered so ranwid (IsModelledRelic) picks it up.</summary>
public class RelicTests
{
    private static (CombatState combat, Monster m) Fight(string relic, int playerHp = 80, int monsterHp = 80)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: playerHp, maxHp: 80,
                                         maxEnergy: 3, relics: new[] { relic });
        var m = Monsters.CalcifiedCultist(hp: monsterHp);
        var combat = Catalog.SetupCombat(player, new[] { m });   // fires relic OnCombatStart
        return (combat, m);
    }

    // ---- Combat-start grants ----

    [Fact] public void Anchor_Grants_10_Block() => Assert.Equal(10, Fight("Anchor").combat.Player.Block);

    [Fact] public void BronzeScales_Grants_3_Thorns()
        => Assert.Equal(3, Fight("BronzeScales").combat.Player.GetPowerAmount("Thorns"));

    [Fact] public void Vajra_Grants_1_Strength()
        => Assert.Equal(1, Fight("Vajra").combat.Player.GetPowerAmount("Strength"));

    [Fact] public void OddlySmoothStone_Grants_1_Dexterity()
        => Assert.Equal(1, Fight("OddlySmoothStone").combat.Player.GetPowerAmount("Dexterity"));

    [Fact] public void DataDisk_Grants_1_Focus()
        => Assert.Equal(1, Fight("DataDisk").combat.Player.GetPowerAmount("Focus"));

    [Fact] public void Gorget_Grants_4_Plating()
        => Assert.Equal(4, Fight("Gorget").combat.Player.GetPowerAmount("Plating"));

    [Fact]
    public void Akabeko_Adds_8_To_First_Powered_Attack()
    {
        var (combat, m) = Fight("Akabeko");
        Assert.Equal(8, combat.Player.GetPowerAmount("Vigor"));
        Cmd.Attack(combat, combat.Player, m, 6, ValueProp.Move, null);   // 6 + 8 Vigor
        Assert.Equal(80 - 14, m.CurrentHp);
        // Vigor consumed: the next powered attack is unboosted.
        Cmd.Attack(combat, combat.Player, m, 6, ValueProp.Move, null);
        Assert.Equal(80 - 14 - 6, m.CurrentHp);
    }

    [Fact]
    public void BagOfMarbles_Applies_1_Vulnerable_To_All_Enemies()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80, 3, new[] { "BagOfMarbles" });
        var combat = Catalog.SetupCombat(player, new[] { Monsters.CalcifiedCultist(40), Monsters.CalcifiedCultist(40) });
        Assert.All(combat.Monsters, m => Assert.Equal(1, m.GetPowerAmount("Vulnerable")));
    }

    [Fact]
    public void RedMask_Applies_1_Weak_To_All_Enemies()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80, 3, new[] { "RedMask" });
        var combat = Catalog.SetupCombat(player, new[] { Monsters.CalcifiedCultist(40), Monsters.CalcifiedCultist(40) });
        Assert.All(combat.Monsters, m => Assert.Equal(1, m.GetPowerAmount("Weak")));
    }

    [Fact]
    public void BloodVial_Heals_2_At_Combat_Start()
        => Assert.Equal(52, Fight("BloodVial", playerHp: 50).combat.Player.CurrentHp);

    // ---- Recurring / turn-numbered turn-start effects ----

    [Fact]
    public void Sai_Grants_7_Block_Every_Turn()
    {
        var (combat, _) = Fight("Sai");
        CombatManager.BeginPlayerTurn(combat);                 // turn 1
        Assert.Equal(7, combat.Player.Block);
        CombatManager.BeginPlayerTurn(combat);                 // turn 2: block clears, +7 again
        Assert.Equal(7, combat.Player.Block);
    }

    [Fact]
    public void Brimstone_Ramps_Self_And_Enemy_Strength()
    {
        var (combat, m) = Fight("Brimstone");
        CombatManager.BeginPlayerTurn(combat);                 // turn 1: +2 self, +1 enemy
        Assert.Equal(2, combat.Player.GetPowerAmount("Strength"));
        Assert.Equal(1, m.GetPowerAmount("Strength"));
        CombatManager.BeginPlayerTurn(combat);                 // turn 2: ramps
        Assert.Equal(4, combat.Player.GetPowerAmount("Strength"));
        Assert.Equal(2, m.GetPowerAmount("Strength"));
    }

    [Theory]
    [InlineData("Lantern", 1, 4)]      // +1 on turn 1
    [InlineData("VeryHotCocoa", 1, 7)] // +4 on turn 1
    [InlineData("Candelabra", 2, 5)]   // +2 on turn 2
    [InlineData("Chandelier", 3, 6)]   // +3 on turn 3
    public void EnergyRelic_Grants_Bonus_On_Its_Turn(string relic, int turn, int expectedEnergy)
    {
        var (combat, _) = Fight(relic);
        for (int t = 1; t <= turn; t++) CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(expectedEnergy, combat.Player.Energy);
    }

    [Fact]
    public void FestivePopper_Deals_9_To_All_Enemies_On_Turn_1()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80, 3, new[] { "FestivePopper" });
        var combat = Catalog.SetupCombat(player, new[] { Monsters.CalcifiedCultist(40), Monsters.CalcifiedCultist(40) });
        CombatManager.BeginPlayerTurn(combat);
        Assert.All(combat.Monsters, m => Assert.Equal(31, m.CurrentHp));
    }

    // ---- Passive modifiers ----

    [Fact]
    public void Ectoplasm_Grants_Plus_1_Max_Energy()
    {
        var (combat, _) = Fight("Ectoplasm");
        CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(4, combat.Player.Energy);
        Assert.Equal(4, combat.Player.EffectiveMaxEnergy);
    }

    [Fact]
    public void StrikeDummy_Adds_3_To_Strike_Attacks_Only()
    {
        var (combat, m) = Fight("StrikeDummy");
        var strike = new StrikeIronclad();
        Cmd.Attack(combat, combat.Player, m, 6, ValueProp.Move, strike);   // Strike: 6 + 3
        Assert.Equal(80 - 9, m.CurrentHp);
        Cmd.Attack(combat, combat.Player, m, 6, ValueProp.Move, new Bash()); // non-Strike: 6
        Assert.Equal(80 - 9 - 6, m.CurrentHp);
    }

    [Fact]
    public void MiniatureCannon_Adds_3_To_Upgraded_Attacks_Only()
    {
        var (combat, m) = Fight("MiniatureCannon");
        Cmd.Attack(combat, combat.Player, m, 6, ValueProp.Move, new StrikeIronclad().Upgraded(1)); // +3
        Assert.Equal(80 - 9, m.CurrentHp);
        Cmd.Attack(combat, combat.Player, m, 6, ValueProp.Move, new StrikeIronclad());             // unupgraded: 6
        Assert.Equal(80 - 9 - 6, m.CurrentHp);
    }

    // ---- Batch 2: HP-loss reducers + passive modifiers ----

    [Fact]
    public void TungstenRod_Reduces_HP_Loss_By_1()
    {
        var (combat, m) = Fight("TungstenRod");
        Cmd.Attack(combat, m, combat.Player, 6, ValueProp.Move, null);   // 6 - 1
        Assert.Equal(75, combat.Player.CurrentHp);
    }

    [Fact]
    public void TheBoot_Raises_Small_Hits_To_5()
    {
        var (combat, m) = Fight("TheBoot");
        Cmd.Attack(combat, combat.Player, m, 3, ValueProp.Move, null);   // 3 -> 5
        Assert.Equal(75, m.CurrentHp);
        Cmd.Attack(combat, combat.Player, m, 6, ValueProp.Move, null);   // 6 stays 6
        Assert.Equal(69, m.CurrentHp);
    }

    [Fact]
    public void SpikedGauntlets_Adds_Energy_And_Surcharges_Power_Cards()
    {
        var (combat, _) = Fight("SpikedGauntlets");
        CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(4, combat.Player.Energy);                           // +1 max energy
        var baseline = Catalog.SetupCombat(Catalog.BuildPlayer(new List<CardModel>(), 80, 80),
                                           new[] { Monsters.CalcifiedCultist(40) });
        var inflame = new Inflame();                                     // a Power card
        Assert.Equal(CombatManager.ResolveCardCost(baseline, inflame) + 1,
                     CombatManager.ResolveCardCost(combat, inflame));    // Power costs +1
    }

    [Fact]
    public void PaelsBlood_Draws_One_Extra_Each_Turn()
        => Assert.Equal(6, CombatManager.TurnStartDrawCount(Fight("PaelsBlood").combat));

    [Fact]
    public void BlessedAntler_Adds_Energy_And_3_Dazed()
    {
        var (combat, _) = Fight("BlessedAntler");
        Assert.Equal(3, combat.Player.DrawPile.Count(c => c.Name == "Dazed"));
        CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(4, combat.Player.Energy);
    }

    // ---- Registration / advisor pickup ----

    [Theory]
    [InlineData("Anchor")] [InlineData("BronzeScales")] [InlineData("Vajra")] [InlineData("OddlySmoothStone")]
    [InlineData("DataDisk")] [InlineData("Gorget")] [InlineData("Akabeko")] [InlineData("BagOfMarbles")]
    [InlineData("RedMask")] [InlineData("BloodVial")] [InlineData("Sai")] [InlineData("Brimstone")]
    [InlineData("Lantern")] [InlineData("VeryHotCocoa")] [InlineData("Candelabra")] [InlineData("Chandelier")]
    [InlineData("FestivePopper")] [InlineData("Ectoplasm")] [InlineData("StrikeDummy")] [InlineData("FakeStrikeDummy")]
    [InlineData("MiniatureCannon")] [InlineData("TungstenRod")] [InlineData("TheBoot")] [InlineData("SpikedGauntlets")]
    [InlineData("PaelsBlood")] [InlineData("BlessedAntler")]
    public void Relic_Is_Registered_And_Buildable(string relic)
    {
        Assert.True(Catalog.IsModelledRelic(relic));
        Assert.Equal(relic, Catalog.BuildRelic(relic).Id);
    }
}
