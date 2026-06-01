using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>Literal-number checks of ported Colorless cards against the game's card data + decompiled
/// OnPlay logic. Kept separate from CardTests.cs (Ironclad) / SilentCardTests.cs (Silent) so the efforts
/// don't collide. Note: Cmd.Draw is a no-op without an ambient Rng, so card-draw counts are not asserted
/// here (HP-neutral and replayed from traces); these tests cover the HP/Block/debuff effects.</summary>
public class ColorlessCardTests
{
    private static (CombatState combat, Player p, Monster m) Fight(int monsterHp = 60, int playerHp = 80)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: playerHp, maxHp: playerHp);
        var m = Monsters.CalcifiedCultist(hp: monsterHp);
        var combat = Catalog.SetupCombat(player, new[] { m });
        player.ResetEnergy();
        return (combat, player, m);
    }

    private static (CombatState combat, Player p, Monster m1, Monster m2) Fight2(int hp = 60)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: hp);
        var m2 = Monsters.CalcifiedCultist(hp: hp);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        player.ResetEnergy();
        return (combat, player, m1, m2);
    }

    private static void Play(CombatState combat, CardModel card, Creature? target)
    {
        combat.Player.Hand.Add(card);
        combat.Player.MaxEnergy = Math.Max(combat.Player.MaxEnergy, card.Cost);
        combat.Player.ResetEnergy();
        CombatManager.PlayCard(combat, card, target);
    }

    // ----- Catalog wiring -----

    [Fact]
    public void Catalog_Builds_Colorless_Cards()
    {
        Assert.IsType<FlashOfSteel>(Catalog.BuildCard("FlashOfSteel"));
        Assert.IsType<MindBlast>(Catalog.BuildCard("MindBlast"));
        Assert.IsType<Panache>(Catalog.BuildCard("Panache"));
        Assert.Equal(2, Catalog.BuildCard("HandOfGreed+1").Cost);   // still 2 energy after upgrade
    }

    // ----- Attacks -----

    [Fact]
    public void FlashOfSteel_Deals_5_At_Cost_0()
    {
        var (c, _, m) = Fight();
        var f = new FlashOfSteel();
        Assert.Equal(0, f.Cost);
        Play(c, f, m);
        Assert.Equal(60 - 5, m.CurrentHp);
    }

    [Fact]
    public void FlashOfSteel_Upgraded_Deals_7()
    {
        var (c, _, m) = Fight();
        Play(c, (CardModel)new FlashOfSteel().Upgraded(1), m);
        Assert.Equal(60 - 7, m.CurrentHp);
    }

    [Fact]
    public void DramaticEntrance_Hits_All_For_11_And_Exhausts()
    {
        var (c, p, m1, m2) = Fight2(40);
        var d = new DramaticEntrance();
        Play(c, d, null);
        Assert.Equal(40 - 11, m1.CurrentHp);
        Assert.Equal(40 - 11, m2.CurrentHp);
        Assert.Contains(d, p.ExhaustPile);
    }

    [Fact]
    public void MindBlast_Deals_Damage_Equal_To_Draw_Pile_Size()
    {
        var (c, p, m) = Fight();
        for (int i = 0; i < 7; i++) p.DrawPile.Add(new FlashOfSteel());
        Play(c, new MindBlast(), m);
        Assert.Equal(60 - 7, m.CurrentHp);
    }

    [Fact]
    public void MindBlast_Upgraded_Costs_0()
    {
        Assert.Equal(1, new MindBlast().Cost);
        Assert.Equal(0, new MindBlast().Upgraded(1).Cost);
    }

    [Fact]
    public void HandOfGreed_Deals_20_At_Cost_2()
    {
        var (c, _, m) = Fight(monsterHp: 60);
        var h = new HandOfGreed();
        Assert.Equal(2, h.Cost);
        Play(c, h, m);
        Assert.Equal(60 - 20, m.CurrentHp);
    }

    [Fact]
    public void Clash_Deals_14_At_Cost_0()
    {
        var (c, _, m) = Fight();
        var clash = new Clash();
        Assert.Equal(0, clash.Cost);
        Play(c, clash, m);
        Assert.Equal(60 - 14, m.CurrentHp);
    }

    // ----- Skills -----

    [Fact]
    public void Finesse_Gains_4_Block_At_Cost_0()
    {
        var (c, p, _) = Fight();
        var f = new Finesse();
        Assert.Equal(0, f.Cost);
        Play(c, f, null);
        Assert.Equal(4, p.Block);
    }

    [Fact]
    public void DarkShackles_Reduces_Strength_Until_Turn_Ends()
    {
        var (c, p, m) = Fight();
        m.AddPower(new StrengthPower(), 5);
        var ds = new DarkShackles();
        Play(c, ds, m);
        Assert.Equal(5 - 9, m.GetPowerAmount("Strength"));   // -4 during the enemy turn
        Assert.Equal(9, m.GetPowerAmount("DarkShackles"));
        Assert.Contains(ds, p.ExhaustPile);
        CombatManager.RollInitialMoves(c, new Rng(0));
        c.CurrentSide = CombatSide.Enemy;
        CombatManager.RunEnemyTurn(c);
        Assert.Equal(5, m.GetPowerAmount("Strength"));        // restored at enemy turn end
        Assert.False(m.HasPower("DarkShackles"));
    }

    [Fact]
    public void MasterOfStrategy_Exhausts()
    {
        var (c, p, _) = Fight();
        var mos = new MasterOfStrategy();
        Assert.Equal(0, mos.Cost);
        Play(c, mos, null);
        Assert.Contains(mos, p.ExhaustPile);
    }

    [Fact]
    public void ThinkingAhead_Puts_A_Hand_Card_On_Top_Of_Draw_And_Exhausts()
    {
        var (c, p, _) = Fight();
        var keep = new FlashOfSteel();
        p.Hand.Add(keep);
        var ta = new ThinkingAhead();
        Play(c, ta, null);
        Assert.Equal(keep, p.DrawPile[0]);            // a hand card moved to the draw-pile top
        Assert.DoesNotContain(keep, p.Hand);
        Assert.Contains(ta, p.ExhaustPile);
    }

    [Fact]
    public void PanicButton_Gains_30_Block_And_Blocks_Block_For_2_Turns()
    {
        var (c, p, _) = Fight();
        var pb = new PanicButton();
        Play(c, pb, null);
        Assert.Equal(30, p.Block);
        Assert.Equal(2, p.GetPowerAmount("NoBlock"));
        Assert.Contains(pb, p.ExhaustPile);
        // While NoBlock is active, further block gains are nullified.
        Cmd.GainBlock(c, p, 5, ValueProp.Move, null);
        Assert.Equal(30, p.Block);                     // no new block added
    }

    [Fact]
    public void PanicButton_NoBlock_Expires_After_2_Turns()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);
        Play(c, new PanicButton(), null);
        Assert.Equal(2, p.GetPowerAmount("NoBlock"));
        CombatManager.EndPlayerTurn(c);                // NoBlock 2 -> 1
        CombatManager.RunEnemyTurn(c);
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(1, p.GetPowerAmount("NoBlock"));
        CombatManager.EndPlayerTurn(c);                // NoBlock 1 -> 0, removed
        CombatManager.RunEnemyTurn(c);
        CombatManager.BeginPlayerTurn(c);
        Assert.False(p.HasPower("NoBlock"));
        Cmd.GainBlock(c, p, 7, ValueProp.Move, null);
        Assert.Equal(7, p.Block);                      // block works again
    }

    // ----- Powers -----

    [Fact]
    public void Panache_Deals_10_To_All_Every_5_Cards()
    {
        var (c, p, m1, m2) = Fight2(60);
        Play(c, new Panache(), null);                  // play #1 (the Panache card itself counts)
        for (int i = 0; i < 3; i++) Play(c, new Finesse(), null);   // plays #2,3,4
        Assert.Equal(60, m1.CurrentHp);                // not yet
        Play(c, new Finesse(), null);                  // play #5 → 10 to all
        Assert.Equal(60 - 10, m1.CurrentHp);
        Assert.Equal(60 - 10, m2.CurrentHp);
    }

    [Fact]
    public void TheBomb_Explodes_For_40_After_3_Turns()
    {
        var (c, p, m1, m2) = Fight2(80);
        CombatManager.BeginPlayerTurn(c);
        Play(c, new TheBomb(), null);
        Assert.Equal(40, p.GetPowerAmount("TheBomb"));
        // Turn 1 end: countdown 3 -> 2
        CombatManager.EndPlayerTurn(c); CombatManager.RunEnemyTurn(c); CombatManager.BeginPlayerTurn(c);
        Assert.Equal(80, m1.CurrentHp);
        // Turn 2 end: 2 -> 1
        CombatManager.EndPlayerTurn(c); CombatManager.RunEnemyTurn(c); CombatManager.BeginPlayerTurn(c);
        Assert.Equal(80, m1.CurrentHp);
        // Turn 3 end: 1 -> 0 → explode 40 to all
        CombatManager.EndPlayerTurn(c);
        Assert.Equal(80 - 40, m1.CurrentHp);
        Assert.Equal(80 - 40, m2.CurrentHp);
        Assert.False(p.HasPower("TheBomb"));
    }

    [Fact]
    public void Mayhem_Applies_Its_Power()
    {
        var (c, p, _) = Fight();
        Play(c, new Mayhem(), null);
        Assert.True(p.HasPower("Mayhem"));
    }

    // ----- HP-neutral / RNG subset cards -----

    [Fact]
    public void Apotheosis_Upgrades_Cards_In_Piles_And_Exhausts()
    {
        var (c, p, _) = Fight();
        p.Hand.Add(new FlashOfSteel());
        p.DrawPile.Add(new Clash());
        var apo = new Apotheosis();
        Play(c, apo, null);
        // Apotheosis swaps in private upgraded copies (it must NOT mutate the shared instances in place — see
        // the soundness note on Apotheosis.OnPlay), so inspect the cards now in the piles.
        Assert.Equal(1, p.Hand.First(h => h is FlashOfSteel).Upgrades);
        Assert.Equal(1, p.DrawPile.First(h => h is Clash).Upgrades);
        Assert.Contains(apo, p.ExhaustPile);
    }

    [Fact]
    public void Metamorphosis_And_Enlightenment_Exhaust_Inertly()
    {
        var (c, p, m) = Fight();
        var meta = new Metamorphosis();
        var enl = new Enlightenment();
        Play(c, meta, null);
        Play(c, enl, null);
        Assert.Contains(meta, p.ExhaustPile);
        Assert.Contains(enl, p.ExhaustPile);
        Assert.Equal(60, m.CurrentHp);                 // no HP effect
    }

    [Fact]
    public void Purity_Exhausts_Up_To_3_Hand_Cards()
    {
        var (c, p, _) = Fight();
        for (int i = 0; i < 5; i++) p.Hand.Add(new Finesse());
        var purity = new Purity();
        Play(c, purity, null);
        // 3 of the 5 Finesses exhausted (plus Purity itself).
        Assert.Equal(3, p.ExhaustPile.Count(card => card is Finesse));
        Assert.Contains(purity, p.ExhaustPile);
    }
}
