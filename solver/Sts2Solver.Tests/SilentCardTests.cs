using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>Literal-number checks of ported Silent cards against the game's card data + decompiled
/// OnPlay logic. Kept separate from CardTests.cs (Ironclad) so the two efforts don't collide.</summary>
public class SilentCardTests
{
    private static (CombatState combat, Player p, Monster m) Fight(int monsterHp = 60, int playerHp = 80)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: playerHp, maxHp: playerHp);
        var m = Monsters.CalcifiedCultist(hp: monsterHp);
        var combat = Catalog.SetupCombat(player, new[] { m });
        player.ResetEnergy();
        return (combat, player, m);
    }

    private static void Play(CombatState combat, CardModel card, Creature? target)
    {
        combat.Player.Hand.Add(card);
        combat.Player.MaxEnergy = Math.Max(combat.Player.MaxEnergy, card.Cost);
        combat.Player.ResetEnergy();
        CombatManager.PlayCard(combat, card, target);
    }

    [Fact]
    public void StrikeSilent_Deals_6()
    {
        var (c, _, m) = Fight();
        Play(c, new StrikeSilent(), m);
        Assert.Equal(60 - 6, m.CurrentHp);
    }

    [Fact]
    public void DefendSilent_Gains_5_Block()
    {
        var (c, p, _) = Fight();
        Play(c, new DefendSilent(), null);
        Assert.Equal(5, p.Block);
    }

    [Fact]
    public void Neutralize_Deals_3_And_Applies_1_Weak_At_Cost_0()
    {
        var (c, _, m) = Fight();
        var n = new Neutralize();
        Assert.Equal(0, n.Cost);
        Play(c, n, m);
        Assert.Equal(60 - 3, m.CurrentHp);
        Assert.Equal(1, m.GetPowerAmount("Weak"));
    }

    [Fact]
    public void Slice_Deals_6_At_Cost_0()
    {
        var (c, _, m) = Fight();
        var s = new Slice();
        Assert.Equal(0, s.Cost);
        Play(c, s, m);
        Assert.Equal(60 - 6, m.CurrentHp);
    }

    [Fact]
    public void Deflect_Gains_4_Block_At_Cost_0()
    {
        var (c, p, _) = Fight();
        var d = new Deflect();
        Assert.Equal(0, d.Cost);
        Play(c, d, null);
        Assert.Equal(4, p.Block);
    }

    [Fact]
    public void Dash_Gains_10_Block_And_Deals_10()
    {
        var (c, p, m) = Fight();
        Play(c, new Dash(), m);
        Assert.Equal(10, p.Block);
        Assert.Equal(60 - 10, m.CurrentHp);
    }

    [Fact]
    public void SuckerPunch_Deals_8_And_Applies_1_Weak()
    {
        var (c, _, m) = Fight();
        Play(c, new SuckerPunch(), m);
        Assert.Equal(60 - 8, m.CurrentHp);
        Assert.Equal(1, m.GetPowerAmount("Weak"));
    }

    [Fact]
    public void LegSweep_Gains_11_Block_And_Applies_2_Weak()
    {
        var (c, p, m) = Fight();
        Play(c, new LegSweep(), m);
        Assert.Equal(11, p.Block);
        Assert.Equal(2, m.GetPowerAmount("Weak"));
    }

    [Fact]
    public void DaggerSpray_Hits_All_Enemies_Twice_For_4()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 40);
        var m2 = Monsters.CalcifiedCultist(hp: 40);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        Play(combat, new DaggerSpray(), null);
        Assert.Equal(40 - 8, m1.CurrentHp);   // 4 × 2 hits
        Assert.Equal(40 - 8, m2.CurrentHp);
    }

    [Fact]
    public void PoisonedStab_Deals_6_And_Applies_3_Poison()
    {
        var (c, _, m) = Fight();
        Play(c, new PoisonedStab(), m);
        Assert.Equal(60 - 6, m.CurrentHp);
        Assert.Equal(3, m.GetPowerAmount("Poison"));
    }

    [Fact]
    public void DeadlyPoison_Applies_5_Poison()
    {
        var (c, _, m) = Fight();
        Play(c, new DeadlyPoison(), m);
        Assert.Equal(5, m.GetPowerAmount("Poison"));
    }

    [Fact]
    public void Footwork_Gains_2_Dexterity()
    {
        var (c, p, _) = Fight();
        Play(c, new Footwork(), null);
        Assert.Equal(2, p.GetPowerAmount("Dexterity"));
    }

    [Fact]
    public void Haze_Applies_4_Poison_To_All_Enemies()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 40);
        var m2 = Monsters.CalcifiedCultist(hp: 40);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        Play(combat, new Haze(), null);
        Assert.Equal(4, m1.GetPowerAmount("Poison"));
        Assert.Equal(4, m2.GetPowerAmount("Poison"));
    }

    [Fact]
    public void Flechettes_Deals_5_Per_Skill_In_Hand()
    {
        var (c, p, m) = Fight();
        // Seed the hand with 3 Skills (+ a non-skill that shouldn't count).
        p.Hand.Add(new DefendSilent());
        p.Hand.Add(new DeadlyPoison());
        p.Hand.Add(new Deflect());
        p.Hand.Add(new StrikeSilent());   // Attack — not counted
        Play(c, new Flechettes(), m);      // Flechettes itself is an Attack and already out of hand
        Assert.Equal(60 - 5 * 3, m.CurrentHp);
    }

    // ----- Poison mechanic -----

    [Fact]
    public void Poison_Ticks_At_Owner_Turn_Start_And_Decrements()
    {
        var (c, _, m) = Fight(monsterHp: 60);
        m.AddPower(new PoisonPower(), 5);
        // Poison ticks at the start of the OWNER's (monster's) turn.
        c.CurrentSide = CombatSide.Enemy;
        CombatManager.RollInitialMoves(c, new Rng(0));
        CombatManager.RunEnemyTurn(c);
        Assert.Equal(60 - 5, m.CurrentHp);            // dealt 5
        Assert.Equal(4, m.GetPowerAmount("Poison"));  // decremented to 4
    }

    [Fact]
    public void Poison_Is_Unblockable()
    {
        var (c, _, m) = Fight(monsterHp: 60);
        m.GainBlockDirect(10);
        m.AddPower(new PoisonPower(), 3);
        c.CurrentSide = CombatSide.Enemy;
        CombatManager.RollInitialMoves(c, new Rng(0));
        CombatManager.RunEnemyTurn(c);
        Assert.Equal(60 - 3, m.CurrentHp);            // block did not absorb poison
    }

    [Fact]
    public void Poison_Expires_When_It_Hits_Zero()
    {
        var (c, _, m) = Fight(monsterHp: 60);
        m.AddPower(new PoisonPower(), 1);
        c.CurrentSide = CombatSide.Enemy;
        CombatManager.RollInitialMoves(c, new Rng(0));
        CombatManager.RunEnemyTurn(c);
        Assert.Equal(60 - 1, m.CurrentHp);
        Assert.False(m.HasPower("Poison"));           // 1 -> 0, removed
    }

    // ===== Batch 2 =====

    [Fact]
    public void Pinpoint_Deals_15()
    {
        var (c, _, m) = Fight();
        Play(c, new Pinpoint(), m);
        Assert.Equal(60 - 15, m.CurrentHp);
    }

    [Fact]
    public void FlickFlack_Hits_All_For_6()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 40);
        var m2 = Monsters.CalcifiedCultist(hp: 40);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        Play(combat, new FlickFlack(), null);
        Assert.Equal(40 - 6, m1.CurrentHp);
        Assert.Equal(40 - 6, m2.CurrentHp);
    }

    [Fact]
    public void Scare_Applies_1_Weak_All_And_Exhausts_Unupgraded()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 40);
        var m2 = Monsters.CalcifiedCultist(hp: 40);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        var s = new Scare();
        Play(combat, s, null);
        Assert.Equal(1, m1.GetPowerAmount("Weak"));
        Assert.Equal(1, m2.GetPowerAmount("Weak"));
        Assert.Contains(s, player.ExhaustPile);
    }

    [Fact]
    public void Snakebite_Applies_7_Poison()
    {
        var (c, _, m) = Fight();
        Play(c, new Snakebite(), m);
        Assert.Equal(7, m.GetPowerAmount("Poison"));
    }

    [Fact]
    public void BubbleBubble_Adds_9_Poison_Only_If_Already_Poisoned()
    {
        var (c, _, m) = Fight();
        Play(c, new BubbleBubble(), m);
        Assert.Equal(0, m.GetPowerAmount("Poison"));   // not poisoned → nothing
        m.AddPower(new PoisonPower(), 3);
        Play(c, new BubbleBubble(), m);
        Assert.Equal(3 + 9, m.GetPowerAmount("Poison"));
    }

    [Fact]
    public void Suppress_Deals_11_And_Applies_3_Weak_At_Cost_0()
    {
        var (c, _, m) = Fight();
        var s = new Suppress();
        Assert.Equal(0, s.Cost);
        Play(c, s, m);
        Assert.Equal(60 - 11, m.CurrentHp);
        Assert.Equal(3, m.GetPowerAmount("Weak"));
    }

    [Fact]
    public void Shiv_Deals_4_And_Exhausts()
    {
        var (c, p, m) = Fight();
        var s = new Shiv();
        Play(c, s, m);
        Assert.Equal(60 - 4, m.CurrentHp);
        Assert.Contains(s, p.ExhaustPile);
        Assert.DoesNotContain(s, p.DiscardPile);
    }

    [Fact]
    public void CloakAndDagger_Gains_6_Block_And_Adds_1_Shiv()
    {
        var (c, p, _) = Fight();
        Play(c, new CloakAndDagger(), null);
        Assert.Equal(6, p.Block);
        Assert.Equal(1, p.Hand.Count(card => card is Shiv));
    }

    [Fact]
    public void BladeDance_Adds_3_Shivs_And_Exhausts()
    {
        var (c, p, _) = Fight();
        var bd = new BladeDance();
        Play(c, bd, null);
        Assert.Equal(3, p.Hand.Count(card => card is Shiv));
        Assert.Contains(bd, p.ExhaustPile);
    }

    [Fact]
    public void DodgeAndRoll_Gains_4_Now_And_4_Next_Turn()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);     // turn 1 (block not cleared)
        Play(c, new DodgeAndRoll(), null);
        Assert.Equal(4, p.Block);
        Assert.Equal(4, p.GetPowerAmount("BlockNextTurn"));
        CombatManager.EndPlayerTurn(c);
        CombatManager.RunEnemyTurn(c);
        CombatManager.BeginPlayerTurn(c);     // turn 2: clears block, then +4 from BlockNextTurn
        Assert.Equal(4, p.Block);
        Assert.False(p.HasPower("BlockNextTurn"));
    }

    [Fact]
    public void Blur_Block_Survives_One_Extra_Turn()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);     // turn 1
        Play(c, new Blur(), null);
        Assert.Equal(5, p.Block);
        Assert.Equal(1, p.GetPowerAmount("Blur"));
        CombatManager.EndPlayerTurn(c);
        CombatManager.RunEnemyTurn(c);
        CombatManager.BeginPlayerTurn(c);     // turn 2: block NOT cleared (Blur), Blur 1 -> 0
        Assert.Equal(5, p.Block);
        Assert.False(p.HasPower("Blur"));
        CombatManager.EndPlayerTurn(c);
        CombatManager.RunEnemyTurn(c);
        CombatManager.BeginPlayerTurn(c);     // turn 3: now cleared
        Assert.Equal(0, p.Block);
    }

    [Fact]
    public void Afterimage_Gains_1_Block_Per_Card_But_Not_Itself()
    {
        var (c, p, m) = Fight();
        Play(c, new Afterimage(), null);      // the granting card does NOT trigger it
        Assert.Equal(0, p.Block);
        Play(c, new StrikeSilent(), m);       // subsequent card → +1 block
        Assert.Equal(1, p.Block);
        Play(c, new Deflect(), null);         // +4 block from Deflect, +1 from Afterimage
        Assert.Equal(1 + 4 + 1, p.Block);
    }

    [Fact]
    public void NoxiousFumes_Applies_Poison_To_All_At_Turn_Start()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 40);
        var m2 = Monsters.CalcifiedCultist(hp: 40);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        CombatManager.BeginPlayerTurn(combat);
        Play(combat, new NoxiousFumes(), null);
        Assert.Equal(0, m1.GetPowerAmount("Poison"));   // not applied the turn it's played
        CombatManager.BeginPlayerTurn(combat);          // next turn start → 2 Poison each
        Assert.Equal(2, m1.GetPowerAmount("Poison"));
        Assert.Equal(2, m2.GetPowerAmount("Poison"));
    }

    [Fact]
    public void Envenom_Applies_1_Poison_On_Unblocked_Attack()
    {
        var (c, _, m) = Fight();
        Play(c, new Envenom(), null);
        Play(c, new StrikeSilent(), m);                 // unblocked attack damage → 1 Poison
        Assert.Equal(6, 60 - m.CurrentHp);
        Assert.Equal(1, m.GetPowerAmount("Poison"));
    }

    [Fact]
    public void Envenom_Does_Not_Trigger_When_Fully_Blocked()
    {
        var (c, _, m) = Fight();
        m.GainBlockDirect(100);
        Play(c, new Envenom(), null);
        Play(c, new StrikeSilent(), m);                 // fully blocked → no unblocked damage → no Poison
        Assert.Equal(0, m.GetPowerAmount("Poison"));
    }

    [Fact]
    public void PiercingWail_Reduces_All_Enemies_Strength_Until_Their_Turn_Ends()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 40);
        var m2 = Monsters.CalcifiedCultist(hp: 40);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        m1.AddPower(new StrengthPower(), 5);
        m2.AddPower(new StrengthPower(), 5);
        Play(combat, new PiercingWail(), null);
        Assert.Equal(5 - 6, m1.GetPowerAmount("Strength"));   // -1
        Assert.Equal(6, m1.GetPowerAmount("PiercingWail"));
        CombatManager.RollInitialMoves(combat, new Rng(0));
        combat.CurrentSide = CombatSide.Enemy;
        CombatManager.RunEnemyTurn(combat);
        Assert.Equal(5, m1.GetPowerAmount("Strength"));       // restored at enemy turn end
        Assert.False(m1.HasPower("PiercingWail"));
    }

    // ===== Batch 3 =====

    [Fact]
    public void Survivor_Gains_8_Block()
    {
        var (c, p, _) = Fight();
        Play(c, new Survivor(), null);
        Assert.Equal(8, p.Block);
    }

    [Fact]
    public void Survivor_Upgrade_Gains_11_Block()
    {
        var (c, p, _) = Fight();
        Play(c, (CardModel)new Survivor().Upgraded(), null);
        Assert.Equal(11, p.Block);
    }

    [Fact]
    public void Backstab_Deals_11_At_Cost_0_And_Exhausts()
    {
        var (c, p, m) = Fight();
        var b = new Backstab();
        Assert.Equal(0, b.Cost);
        Play(c, b, m);
        Assert.Equal(60 - 11, m.CurrentHp);
        Assert.Contains(b, p.ExhaustPile);
        Assert.DoesNotContain(b, p.DiscardPile);
    }

    [Fact]
    public void Backstab_Upgrade_Deals_15()
    {
        var (c, _, m) = Fight();
        Play(c, (CardModel)new Backstab().Upgraded(), m);
        Assert.Equal(60 - 15, m.CurrentHp);
    }

    [Fact]
    public void DaggerThrow_Deals_9()
    {
        var (c, _, m) = Fight();
        Play(c, new DaggerThrow(), m);
        Assert.Equal(60 - 9, m.CurrentHp);
    }

    [Fact]
    public void DaggerThrow_Upgrade_Deals_12()
    {
        var (c, _, m) = Fight();
        Play(c, (CardModel)new DaggerThrow().Upgraded(), m);
        Assert.Equal(60 - 12, m.CurrentHp);
    }

    [Fact]
    public void Predator_Deals_15_And_Queues_Bonus_Draw()
    {
        var (c, _, m) = Fight();
        Play(c, new Predator(), m);
        Assert.Equal(60 - 15, m.CurrentHp);
        Assert.Equal(2, c.Player.GetPowerAmount("DrawNextTurn"));
    }

    [Fact]
    public void Predator_Bonus_Draw_Power_Consumed_At_Next_Turn_Start()
    {
        var (c, p, m) = Fight();
        CombatManager.BeginPlayerTurn(c);
        Play(c, new Predator(), m);
        Assert.Equal(2, p.GetPowerAmount("DrawNextTurn"));
        CombatManager.EndPlayerTurn(c);
        CombatManager.RunEnemyTurn(c);
        CombatManager.BeginPlayerTurn(c);                 // next turn start: power fires + removed
        Assert.False(p.HasPower("DrawNextTurn"));
    }

    [Fact]
    public void BouncingFlask_Applies_3_Poison_Three_Times_To_Single_Enemy()
    {
        var (c, _, m) = Fight();
        Play(c, new BouncingFlask(), m);
        Assert.Equal(9, m.GetPowerAmount("Poison"));      // 3 bounces × 3 poison, single enemy
    }

    [Fact]
    public void BouncingFlask_Upgrade_Applies_4_Per_Bounce()
    {
        var (c, _, m) = Fight();
        Play(c, (CardModel)new BouncingFlask().Upgraded(), m);
        Assert.Equal(12, m.GetPowerAmount("Poison"));     // 3 bounces × 4 poison
    }

    [Fact]
    public void Caltrops_Deals_3_Back_When_Player_Is_Attacked()
    {
        var (c, p, m) = Fight();
        Play(c, new Caltrops(), null);
        Assert.Equal(3, p.GetPowerAmount("Caltrops"));
        int before = m.CurrentHp;
        Cmd.Attack(c, m, p, 5, ValueProp.Move, null);     // enemy hits player
        Assert.Equal(before - 3, m.CurrentHp);            // 3 thorns back
    }

    [Fact]
    public void Caltrops_Upgrade_Deals_5_Back()
    {
        var (c, p, m) = Fight();
        Play(c, (CardModel)new Caltrops().Upgraded(), null);
        Assert.Equal(5, p.GetPowerAmount("Caltrops"));
        int before = m.CurrentHp;
        Cmd.Attack(c, m, p, 5, ValueProp.Move, null);
        Assert.Equal(before - 5, m.CurrentHp);
    }

    [Fact]
    public void GrandFinale_Deals_60_To_All_Only_When_Draw_Pile_Empty()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 80);
        var m2 = Monsters.CalcifiedCultist(hp: 80);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        // Non-empty draw pile → no effect.
        player.DrawPile.Add(new StrikeSilent());
        Play(combat, new GrandFinale(), null);
        Assert.Equal(80, m1.CurrentHp);
        // Empty draw pile → 60 to all.
        player.DrawPile.Clear();
        Play(combat, new GrandFinale(), null);
        Assert.Equal(80 - 60, m1.CurrentHp);
        Assert.Equal(80 - 60, m2.CurrentHp);
    }

    [Fact]
    public void Skewer_Deals_8_Per_Energy()
    {
        var (c, _, m) = Fight(monsterHp: 80);
        var s = new Skewer();
        Assert.True(s.IsXCost);
        Play(c, s, m);                                    // 3 energy → X = 3
        Assert.Equal(0, c.Player.Energy);
        Assert.Equal(80 - 24, m.CurrentHp);               // 8 × 3
    }

    [Fact]
    public void Adrenaline_Gains_Energy_And_Exhausts()
    {
        var (c, p, _) = Fight();
        var a = new Adrenaline();
        Assert.Equal(0, a.Cost);
        Play(c, a, null);
        Assert.Contains(a, p.ExhaustPile);
        // Energy started at MaxEnergy then +1 from Adrenaline.
        Assert.Equal(p.EffectiveMaxEnergy + 1, p.Energy);
    }

    [Fact]
    public void Backflip_Gains_5_Block()
    {
        var (c, p, _) = Fight();
        Play(c, new Backflip(), null);
        Assert.Equal(5, p.Block);
    }

    [Fact]
    public void Backflip_Upgrade_Gains_8_Block()
    {
        var (c, p, _) = Fight();
        Play(c, (CardModel)new Backflip().Upgraded(), null);
        Assert.Equal(8, p.Block);
    }

    [Fact]
    public void Expertise_Draws_Up_To_Six_Cards()
    {
        var (c, p, _) = Fight();
        // With an ambient Rng and a stocked draw pile, Expertise tops the hand up to 6.
        c.Rng = new Rng(0);
        for (int i = 0; i < 10; i++) p.DrawPile.Add(new StrikeSilent());
        p.Hand.Add(new DefendSilent());                   // 1 card before play
        var e = new Expertise();
        Play(c, e, null);                                 // played from hand → still 1 in hand at OnPlay
        // OnPlay sees hand count 1 (Expertise removed before OnPlay), draws 6-1 = 5.
        Assert.Equal(6, p.Hand.Count);
    }
}
