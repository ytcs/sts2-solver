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
    public void Survivor_Discards_A_Card_With_An_Ambient_Rng()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        p.Hand.Add(new StrikeSilent());                    // a card available to discard
        Play(c, new Survivor(), null);                     // gain 8 Block, then discard 1
        Assert.Equal(8, p.Block);
        Assert.Empty(p.Hand);                              // the Strike was discarded (Survivor left hand on play)
        Assert.Contains(p.DiscardPile, x => x is StrikeSilent);
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
    public void DaggerThrow_Draws_Then_Discards_With_An_Ambient_Rng()
    {
        var (c, p, m) = Fight();
        c.Rng = new Rng(0);
        p.DrawPile.Add(new DefendSilent());                // the card to be drawn
        Play(c, new DaggerThrow(), m);                     // 9 dmg, draw 1, discard 1
        Assert.Equal(60 - 9, m.CurrentHp);                 // damage still lands
        Assert.Empty(p.Hand);                              // drew 1 then discarded 1 ⇒ net hand 0
        Assert.Contains(p.DiscardPile, x => x is DefendSilent);   // the drawn card cycled to discard
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

    // ===== Batch 4 =====

    [Fact]
    public void Abrasive_Gains_1_Dex_And_4_Thorns_And_Reflects()
    {
        var (c, p, m) = Fight();
        Play(c, new Abrasive(), null);
        Assert.Equal(1, p.GetPowerAmount("Dexterity"));
        Assert.Equal(4, p.GetPowerAmount("Thorns"));
        int before = m.CurrentHp;
        Cmd.Attack(c, m, p, 5, ValueProp.Move, null);     // enemy hits player
        Assert.Equal(before - 4, m.CurrentHp);            // 4 thorns back
    }

    [Fact]
    public void Abrasive_Upgrade_Gains_6_Thorns()
    {
        var (c, p, _) = Fight();
        Play(c, (CardModel)new Abrasive().Upgraded(), null);
        Assert.Equal(6, p.GetPowerAmount("Thorns"));
    }

    [Fact]
    public void Assassinate_Deals_10_Applies_1_Vulnerable_At_Cost_0_And_Exhausts()
    {
        var (c, p, m) = Fight();
        var a = new Assassinate();
        Assert.Equal(0, a.Cost);
        Play(c, a, m);
        Assert.Equal(60 - 10, m.CurrentHp);
        Assert.Equal(1, m.GetPowerAmount("Vulnerable"));
        Assert.Contains(a, p.ExhaustPile);
    }

    [Fact]
    public void Assassinate_Upgrade_Deals_13_And_2_Vulnerable()
    {
        var (c, _, m) = Fight();
        Play(c, (CardModel)new Assassinate().Upgraded(), m);
        Assert.Equal(60 - 13, m.CurrentHp);
        Assert.Equal(2, m.GetPowerAmount("Vulnerable"));
    }

    [Fact]
    public void Expose_Removes_Block_And_Artifact_Then_Applies_2_Vulnerable()
    {
        var (c, _, m) = Fight();
        m.GainBlockDirect(10);
        m.AddPower(new ArtifactPower(), 1);
        Play(c, new Expose(), m);
        Assert.Equal(0, m.Block);                          // all block removed
        Assert.False(m.HasPower("Artifact"));              // artifact removed outright (not consumed by vuln)
        Assert.Equal(2, m.GetPowerAmount("Vulnerable"));
    }

    [Fact]
    public void LeadingStrike_Deals_3_And_Adds_2_Shivs()
    {
        var (c, p, m) = Fight();
        Play(c, new LeadingStrike(), m);
        Assert.Equal(60 - 3, m.CurrentHp);
        Assert.Equal(2, p.Hand.Count(card => card is Shiv));
    }

    [Fact]
    public void Malaise_Reduces_Strength_By_X_And_Applies_X_Weak()
    {
        var (c, _, m) = Fight();
        m.AddPower(new StrengthPower(), 5);
        var mal = new Malaise();
        Assert.True(mal.IsXCost);
        Play(c, mal, m);                                   // 3 energy → X = 3
        Assert.Equal(0, c.Player.Energy);
        Assert.Equal(5 - 3, m.GetPowerAmount("Strength"));
        Assert.Equal(3, m.GetPowerAmount("Weak"));
    }

    [Fact]
    public void Malaise_Upgrade_Adds_One_To_X()
    {
        var (c, _, m) = Fight();
        Play(c, (CardModel)new Malaise().Upgraded(), m);   // X=3, +1 → 4
        Assert.Equal(-4, m.GetPowerAmount("Strength"));
        Assert.Equal(4, m.GetPowerAmount("Weak"));
    }

    [Fact]
    public void Pounce_Deals_14_And_Next_Skill_Costs_Zero()
    {
        var (c, p, m) = Fight();
        Play(c, new Pounce(), m);
        Assert.Equal(60 - 14, m.CurrentHp);
        Assert.Equal(1, p.GetPowerAmount("FreeSkill"));
        // Next Skill is discounted to 0; a second Skill pays full price again.
        p.ResetEnergy();
        int before = p.Energy;
        Play(c, new Survivor(), null);                     // cost 1 → 0 via FreeSkill
        Assert.Equal(before, p.Energy);
        Assert.False(p.HasPower("FreeSkill"));
    }

    [Fact]
    public void Ricochet_Hits_Single_Enemy_4_Times_For_3()
    {
        var (c, _, m) = Fight(monsterHp: 80);
        Play(c, new Ricochet(), m);
        Assert.Equal(80 - 12, m.CurrentHp);                // 3 × 4
    }

    [Fact]
    public void Ricochet_Upgrade_Hits_5_Times()
    {
        var (c, _, m) = Fight(monsterHp: 80);
        Play(c, (CardModel)new Ricochet().Upgraded(), m);
        Assert.Equal(80 - 15, m.CurrentHp);                // 3 × 5
    }

    [Fact]
    public void Tactician_Gains_1_Energy()
    {
        var (c, p, _) = Fight();
        Play(c, new Tactician(), null);                    // 3 energy − cost 3 + gain 1 = 1
        Assert.Equal(1, p.Energy);
    }

    [Fact]
    public void Untouchable_Gains_6_Block()
    {
        var (c, p, _) = Fight();
        Play(c, new Untouchable(), null);
        Assert.Equal(6, p.Block);
    }

    [Fact]
    public void StormOfSteel_Converts_Hand_Into_Shivs()
    {
        var (c, p, _) = Fight();
        p.Hand.Add(new StrikeSilent());
        p.Hand.Add(new DefendSilent());
        p.Hand.Add(new Deflect());                         // 3 cards in hand besides StormOfSteel
        Play(c, new StormOfSteel(), null);                 // discards 3, makes 3 Shivs
        Assert.Equal(3, p.Hand.Count(card => card is Shiv));
        // The 3 discarded hand cards + StormOfSteel itself (a Skill → discard) = 4 non-Shiv in discard.
        Assert.Equal(4, p.DiscardPile.Count(card => card is not Shiv));
    }

    [Fact]
    public void StormOfSteel_Upgrade_Makes_Upgraded_Shivs()
    {
        var (c, p, _) = Fight();
        p.Hand.Add(new StrikeSilent());
        Play(c, (CardModel)new StormOfSteel().Upgraded(), null);
        var shiv = (Shiv)p.Hand.First(card => card is Shiv);
        Assert.Equal(1, shiv.Upgrades);
    }

    [Fact]
    public void CalculatedGamble_With_Rng_Discards_Hand_And_Redraws()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        for (int i = 0; i < 10; i++) p.DrawPile.Add(new StrikeSilent());
        p.Hand.Add(new DefendSilent());
        p.Hand.Add(new Deflect());                         // 2 cards in hand besides the gamble
        var g = new CalculatedGamble();
        Play(c, g, null);                                  // discard 2, draw 2
        Assert.Equal(2, p.Hand.Count);
        Assert.Contains(g, p.ExhaustPile);
    }

    [Fact]
    public void CalculatedGamble_Without_Rng_Is_NoOp_Keeping_Hand()
    {
        var (c, p, _) = Fight();                           // Rng null
        p.Hand.Add(new DefendSilent());
        p.Hand.Add(new Deflect());
        Play(c, new CalculatedGamble(), null);
        Assert.Equal(2, p.Hand.Count);                     // hand intact (no destructive discard in search)
    }

    // ===== Batch 5 =====

    [Fact]
    public void Accelerant_Makes_Enemy_Poison_Tick_Twice()
    {
        var (c, p, m) = Fight(monsterHp: 60);
        Play(c, new Accelerant(), null);                   // player Accelerant 1
        m.AddPower(new PoisonPower(), 5);
        c.CurrentSide = CombatSide.Enemy;
        CombatManager.RollInitialMoves(c, new Rng(0));
        CombatManager.RunEnemyTurn(c);                     // poison ticks min(5, 1+1)=2 times: 5 then 4
        Assert.Equal(60 - 9, m.CurrentHp);                 // 5 + 4
        Assert.Equal(3, m.GetPowerAmount("Poison"));       // 5 → 4 → 3
    }

    [Fact]
    public void Accuracy_Adds_Damage_To_Shivs_Only()
    {
        var (c, _, m) = Fight();
        Play(c, new Accuracy(), null);                     // Accuracy 4
        Play(c, new Shiv(), m);
        Assert.Equal(60 - (4 + 4), m.CurrentHp);           // Shiv 4 + Accuracy 4
        int after = m.CurrentHp;
        Play(c, new StrikeSilent(), m);                    // non-Shiv unaffected
        Assert.Equal(after - 6, m.CurrentHp);
    }

    [Fact]
    public void Anticipate_Grants_Temp_Dexterity_Removed_At_Turn_End()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);
        Play(c, new Anticipate(), null);
        Assert.Equal(2, p.GetPowerAmount("Dexterity"));    // +2 dex now
        Play(c, new DefendSilent(), null);                 // 5 + 2 dex = 7 block
        Assert.Equal(7, p.Block);
        CombatManager.EndPlayerTurn(c);
        Assert.Equal(0, p.GetPowerAmount("Dexterity"));    // temp dex undone (Dexterity allows negative → lingers at 0, HP-neutral)
        Assert.False(p.HasPower("Anticipate"));
    }

    [Fact]
    public void Strangle_Deals_Damage_Per_Card_Played()
    {
        var (c, _, m) = Fight(monsterHp: 60);
        Play(c, new Strangle(), m);                        // 8 damage + Strangle 2 (self does not trigger)
        Assert.Equal(60 - 8, m.CurrentHp);
        Assert.Equal(2, m.GetPowerAmount("Strangle"));
        Play(c, new DefendSilent(), null);                 // a card → 2 unblockable to enemy
        Assert.Equal(60 - 8 - 2, m.CurrentHp);
    }

    [Fact]
    public void Strangle_Removed_At_Enemy_Turn_End()
    {
        var (c, _, m) = Fight();
        Play(c, new Strangle(), m);
        CombatManager.RollInitialMoves(c, new Rng(0));
        c.CurrentSide = CombatSide.Enemy;
        CombatManager.RunEnemyTurn(c);
        Assert.False(m.HasPower("Strangle"));
    }

    [Fact]
    public void InfiniteBlades_Adds_A_Shiv_Each_Turn_Start()
    {
        var (c, p, _) = Fight();
        Play(c, new InfiniteBlades(), null);
        Assert.Equal(0, p.Hand.Count(card => card is Shiv));   // not the turn it's played
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(1, p.Hand.Count(card => card is Shiv));
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(2, p.Hand.Count(card => card is Shiv));
    }

    [Fact]
    public void PhantomBlades_Boosts_First_Shiv_Each_Turn()
    {
        var (c, _, m) = Fight(monsterHp: 80);
        CombatManager.BeginPlayerTurn(c);
        Play(c, new PhantomBlades(), null);                // +9 to the first Shiv each turn
        Play(c, new Shiv(), m);                            // 4 + 9 = 13
        Assert.Equal(80 - 13, m.CurrentHp);
        int after = m.CurrentHp;
        Play(c, new Shiv(), m);                            // second Shiv: 4 only
        Assert.Equal(after - 4, m.CurrentHp);
    }

    [Fact]
    public void Outbreak_Hits_All_Enemies_Every_Third_Poison()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 80);
        var m2 = Monsters.CalcifiedCultist(hp: 80);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        combat.Player.MaxEnergy = 10; combat.Player.ResetEnergy();
        Play(combat, new Outbreak(), null);                // Outbreak 11
        Play(combat, new DeadlyPoison(), m1);              // poison #1
        Play(combat, new DeadlyPoison(), m1);              // poison #2
        Assert.Equal(80, m2.CurrentHp);                    // not yet
        Play(combat, new DeadlyPoison(), m1);              // poison #3 → 11 to all
        Assert.Equal(80 - 11, m2.CurrentHp);
        Assert.Equal(80 - 11, m1.CurrentHp);               // m1 also takes the AoE (poison itself is not HP yet)
    }

    [Fact]
    public void SerpentForm_Deals_Damage_On_Each_Card_Played()
    {
        var (c, _, m) = Fight(monsterHp: 60);
        Play(c, new SerpentForm(), null);                  // self does not trigger
        Assert.Equal(60, m.CurrentHp);
        Play(c, new DefendSilent(), null);                 // a card → 4 to the enemy
        Assert.Equal(60 - 4, m.CurrentHp);
    }

    [Fact]
    public void Tracking_Doubles_Damage_To_Weak_Enemies()
    {
        var (c, _, m) = Fight(monsterHp: 80);
        Play(c, new Tracking(), null);
        Play(c, new StrikeSilent(), m);                    // not weak → 6
        Assert.Equal(80 - 6, m.CurrentHp);
        int after = m.CurrentHp;
        m.AddPower(new WeakPower(), 2);
        Play(c, new StrikeSilent(), m);                    // weak → 6 × 2 = 12
        Assert.Equal(after - 12, m.CurrentHp);
    }

    [Fact]
    public void Shadowmeld_Doubles_Block_This_Turn()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);
        Play(c, new Shadowmeld(), null);
        Play(c, new DefendSilent(), null);                 // 5 × 2 = 10
        Assert.Equal(10, p.Block);
        CombatManager.EndPlayerTurn(c);
        Assert.False(p.HasPower("Shadowmeld"));
    }

    [Fact]
    public void Burst_Doubles_The_Next_Skill()
    {
        var (c, p, _) = Fight();
        Play(c, new Burst(), null);
        Assert.Equal(1, p.GetPowerAmount("Burst"));
        Play(c, new DefendSilent(), null);                 // played twice → 5 + 5 = 10 block
        Assert.Equal(10, p.Block);
        Assert.False(p.HasPower("Burst"));                 // consumed
        Play(c, new DefendSilent(), null);                 // back to single → +5
        Assert.Equal(15, p.Block);
    }

    [Fact]
    public void FanOfKnives_Adds_4_Shivs()
    {
        var (c, p, _) = Fight();
        c.Player.MaxEnergy = 5; c.Player.ResetEnergy();
        Play(c, new FanOfKnives(), null);
        Assert.Equal(4, p.Hand.Count(card => card is Shiv));
        Assert.True(p.HasPower("FanOfKnives"));
    }

    [Fact]
    public void ShadowStep_Discards_Hand_And_Doubles_Damage_Next_Turn()
    {
        var (c, p, m) = Fight(monsterHp: 80);
        p.Hand.Add(new DefendSilent());
        p.Hand.Add(new Deflect());
        Play(c, new ShadowStep(), null);                   // discards hand, queues DoubleDamage
        Assert.Empty(p.Hand);
        CombatManager.BeginPlayerTurn(c);                  // DoubleDamage applied
        Assert.Equal(1, p.GetPowerAmount("DoubleDamage"));
        Play(c, new StrikeSilent(), m);                    // 6 × 2 = 12
        Assert.Equal(80 - 12, m.CurrentHp);
    }

    [Fact]
    public void BladeOfInk_Adds_Inky_Shivs_That_Deal_5_And_Apply_Weak()
    {
        var (c, p, m) = Fight(monsterHp: 60);
        Play(c, new BladeOfInk(), null);
        var shivs = p.Hand.OfType<Shiv>().ToList();
        Assert.Equal(2, shivs.Count);
        Assert.All(shivs, s => Assert.True(s.Inky));
        Play(c, shivs[0], m);                              // 4 + 1 (Inky) = 5, + 1 Weak
        Assert.Equal(60 - 5, m.CurrentHp);
        Assert.Equal(1, m.GetPowerAmount("Weak"));
    }

    [Fact]
    public void Flanking_And_Sneaky_Are_Inert_In_Single_Player()
    {
        var (c, p, m) = Fight(monsterHp: 60);
        Play(c, new Flanking(), m);                        // applies the (MP-only) debuff
        Assert.True(m.HasPower("Flanking"));
        Play(c, new StrikeSilent(), m);                    // your own attack unaffected → 6
        Assert.Equal(60 - 6, m.CurrentHp);
        c.Player.MaxEnergy = 5; c.Player.ResetEnergy();
        Play(c, new Sneaky(), null);                       // applies, but never grants block in SP
        Assert.True(p.HasPower("Sneaky"));
        int blk = p.Block;
        Play(c, new StrikeSilent(), m);                    // your attack doesn't trigger Sneaky
        Assert.Equal(blk, p.Block);
    }

    // ===== Batch 6 =====

    [Fact]
    public void Finisher_Deals_6_Per_Attack_Played_This_Turn()
    {
        var (c, _, m) = Fight(monsterHp: 80);
        CombatManager.BeginPlayerTurn(c);
        c.Player.MaxEnergy = 10; c.Player.ResetEnergy();
        Play(c, new StrikeSilent(), m);                    // attack #1
        Play(c, new StrikeSilent(), m);                    // attack #2
        Assert.Equal(2, c.AttacksPlayedThisTurn);
        int before = m.CurrentHp;
        Play(c, new Finisher(), m);                        // 6 × 2 = 12 (does not count itself)
        Assert.Equal(before - 12, m.CurrentHp);
    }

    [Fact]
    public void Finisher_Does_Nothing_As_First_Attack()
    {
        var (c, _, m) = Fight(monsterHp: 80);
        CombatManager.BeginPlayerTurn(c);
        Play(c, new Finisher(), m);                        // 0 attacks before → no damage
        Assert.Equal(80, m.CurrentHp);
    }

    [Fact]
    public void MementoMori_Scales_With_Discards_This_Turn()
    {
        var (c, _, m) = Fight(monsterHp: 80);
        CombatManager.BeginPlayerTurn(c);
        Play(c, new MementoMori(), m);                     // 0 discards → 9
        Assert.Equal(80 - 9, m.CurrentHp);
        // Discard 2 cards via Storm of Steel, then Memento Mori scales.
        c.Player.Hand.Add(new StrikeSilent());
        c.Player.Hand.Add(new DefendSilent());
        Play(c, new StormOfSteel(), null);                 // discards 2 (CardsDiscardedThisTurn = 2)
        int before = m.CurrentHp;
        Play(c, new MementoMori(), m);                     // 9 + 4×2 = 17
        Assert.Equal(before - 17, m.CurrentHp);
    }

    [Fact]
    public void PreciseCut_Loses_2_Damage_Per_Card_In_Hand()
    {
        var (c, _, m) = Fight(monsterHp: 80);
        c.Player.Hand.Add(new StrikeSilent());
        c.Player.Hand.Add(new DefendSilent());             // 2 cards besides Precise Cut
        Play(c, new PreciseCut(), m);                      // 13 − 2×2 = 9
        Assert.Equal(80 - 9, m.CurrentHp);
    }

    [Fact]
    public void Mirage_Gains_Block_Equal_To_Enemy_Poison()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 60);
        var m2 = Monsters.CalcifiedCultist(hp: 60);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        m1.AddPower(new PoisonPower(), 5);
        m2.AddPower(new PoisonPower(), 3);
        Play(combat, new Mirage(), null);
        Assert.Equal(8, player.Block);                     // 5 + 3
    }

    [Fact]
    public void EchoingSlash_Repeats_On_Kill()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 10);        // dies to first 10
        var m2 = Monsters.CalcifiedCultist(hp: 25);        // 10 (sweep1) + 10 (sweep2 after m1 dies) = 20
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        Play(combat, new EchoingSlash(), null);
        Assert.False(m1.IsAlive);
        Assert.Equal(25 - 20, m2.CurrentHp);               // two sweeps (one triggered by m1's death)
    }

    [Fact]
    public void WraithForm_Grants_Intangible_Capping_Damage_To_1()
    {
        var (c, p, m) = Fight(monsterHp: 60);
        Play(c, new WraithForm(), null);
        Assert.Equal(2, p.GetPowerAmount("Intangible"));
        Cmd.Attack(c, m, p, 30, ValueProp.Move, null);     // huge hit reduced to 1
        Assert.Equal(80 - 1, p.CurrentHp);
    }

    [Fact]
    public void WraithForm_Intangible_Decrements_At_Enemy_Turn_End()
    {
        var (c, p, _) = Fight();
        Play(c, new WraithForm(), null);
        Assert.Equal(2, p.GetPowerAmount("Intangible"));
        CombatManager.RollInitialMoves(c, new Rng(0));
        c.CurrentSide = CombatSide.Enemy;
        CombatManager.RunEnemyTurn(c);
        Assert.Equal(1, p.GetPowerAmount("Intangible"));   // 2 → 1 at enemy turn end
    }

    [Fact]
    public void WraithForm_Loses_Dexterity_Each_Turn()
    {
        var (c, p, _) = Fight();
        p.AddPower(new DexterityPower(), 3);
        Play(c, new WraithForm(), null);
        CombatManager.BeginPlayerTurn(c);                  // lose 1 Dexterity
        Assert.Equal(2, p.GetPowerAmount("Dexterity"));
    }

    // ===== Batch 7 =====

    [Fact]
    public void TheHunt_Deals_10_And_Exhausts()
    {
        var (c, p, m) = Fight();
        var t = new TheHunt();
        Play(c, t, m);
        Assert.Equal(60 - 10, m.CurrentHp);
        Assert.Contains(t, p.ExhaustPile);
    }

    [Fact]
    public void HandTrick_Gains_7_Block()
    {
        var (c, p, _) = Fight();
        Play(c, new HandTrick(), null);
        Assert.Equal(7, p.Block);
    }

    [Fact]
    public void BulletTime_Makes_Cards_Free_And_Stops_Draw()
    {
        var (c, p, m) = Fight();
        CombatManager.BeginPlayerTurn(c);
        Play(c, new BulletTime(), null);
        Assert.True(p.HasPower("BulletTime"));
        Assert.True(p.HasPower("NoDraw"));
        p.Energy = 0;
        var pin = new Pinpoint();                          // normally cost 3
        p.Hand.Add(pin);
        CombatManager.PlayCard(c, pin, m);                 // plays for free at 0 energy
        Assert.Equal(0, p.Energy);
        Assert.Equal(60 - 15, m.CurrentHp);
        // After the turn ends, Bullet Time is gone.
        CombatManager.EndPlayerTurn(c);
        Assert.False(p.HasPower("BulletTime"));
    }

    [Fact]
    public void UpMySleeve_Adds_3_Shivs_And_Reduces_Its_Own_Cost()
    {
        var (c, p, _) = Fight();
        var u = new UpMySleeve();
        Assert.Equal(2, u.Cost);
        Play(c, u, null);
        Assert.Equal(3, p.Hand.Count(card => card is Shiv));
        Assert.Equal(1, u.Cost);                           // cost reduced by 1
        p.DiscardPile.Clear();
        Play(c, u, null);
        Assert.Equal(6, p.Hand.Count(card => card is Shiv));
        Assert.Equal(0, u.Cost);                           // reduced again, floored at 0
        p.DiscardPile.Clear();
        Play(c, u, null);
        Assert.Equal(0, u.Cost);                           // never goes negative
    }

    [Fact]
    public void UpMySleeve_Cost_Survives_Clone()
    {
        var (c, p, _) = Fight();
        var u = new UpMySleeve();
        Play(c, u, null);                                  // cost → 1
        var clone = c.Clone();
        var clonedU = clone.Player.DiscardPile.OfType<UpMySleeve>().First();
        Assert.Equal(1, clonedU.Cost);                     // Stateful deep-clone preserved the reduction
        Assert.NotSame(u, clonedU);                        // and it is a distinct instance
    }

    // ===== Batch 8 (formerly deferred) =====

    [Fact]
    public void KnifeTrap_Plays_Every_Shiv_From_Exhaust()
    {
        var (c, p, m) = Fight(monsterHp: 80);
        p.ExhaustPile.Add(new Shiv());
        p.ExhaustPile.Add(new Shiv());
        p.ExhaustPile.Add(new DefendSilent());             // non-Shiv ignored
        Play(c, new KnifeTrap(), m);
        Assert.Equal(80 - 8, m.CurrentHp);                 // 4 + 4
        Assert.Equal(2, p.ExhaustPile.Count(card => card is Shiv));   // shivs stay in exhaust
    }

    [Fact]
    public void KnifeTrap_Upgrade_Upgrades_The_Exhaust_Shivs()
    {
        var (c, p, m) = Fight(monsterHp: 80);
        p.ExhaustPile.Add(new Shiv());
        Play(c, (CardModel)new KnifeTrap().Upgraded(), m); // shiv upgraded → 6
        Assert.Equal(80 - 6, m.CurrentHp);
        Assert.Equal(1, p.ExhaustPile.OfType<Shiv>().First().Upgrades);
    }

    [Fact]
    public void Nightmare_Adds_3_Copies_Of_The_Chosen_Card_Next_Turn()
    {
        var (c, p, _) = Fight();
        p.Hand.Add(new StrikeSilent());                    // default selection = first in hand
        var n = new Nightmare();
        Play(c, n, null);
        Assert.Contains(n, p.ExhaustPile);
        Assert.True(p.HasPower("Nightmare"));
        p.Hand.Clear();
        CombatManager.BeginPlayerTurn(c);                  // next turn: 3 copies added
        Assert.Equal(3, p.Hand.Count(card => card is StrikeSilent));
        Assert.False(p.HasPower("Nightmare"));
    }

    [Fact]
    public void Acrobatics_Draws_3_And_Discards_1()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        for (int i = 0; i < 10; i++) p.DrawPile.Add(new StrikeSilent());
        p.Hand.Add(new DefendSilent());
        Play(c, new Acrobatics(), null);                   // hand 1 → draw 3 = 4 → discard 1 = 3
        Assert.Equal(3, p.Hand.Count);
    }

    [Fact]
    public void Acrobatics_Without_Rng_Is_NoOp()
    {
        var (c, p, _) = Fight();                           // Rng null
        p.Hand.Add(new DefendSilent());
        Play(c, new Acrobatics(), null);
        Assert.Single(p.Hand);                             // no draw → no discard
    }

    [Fact]
    public void HiddenDaggers_Discards_2_And_Adds_2_Shivs()
    {
        var (c, p, _) = Fight();
        p.Hand.Add(new StrikeSilent());
        p.Hand.Add(new DefendSilent());
        Play(c, new HiddenDaggers(), null);
        Assert.Equal(2, p.Hand.Count(card => card is Shiv));
        Assert.Equal(0, p.Hand.Count(card => card is not Shiv));   // both originals discarded
    }

    [Fact]
    public void ToolsOfTheTrade_Draws_And_Discards_Each_Turn_Start()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        for (int i = 0; i < 10; i++) p.DrawPile.Add(new StrikeSilent());
        Play(c, new ToolsOfTheTrade(), null);
        Assert.True(p.HasPower("ToolsOfTheTrade"));
        CombatManager.BeginPlayerTurn(c);                  // draw 1 + discard 1 (net draw pile −1)
        Assert.Equal(9, p.DrawPile.Count);
    }

    [Fact]
    public void EscapePlan_Gains_Block_Only_If_Drawn_Card_Is_A_Skill()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        p.DrawPile.Add(new DefendSilent());                // top is a Skill
        Play(c, new EscapePlan(), null);
        Assert.Equal(3, p.Block);
        // Drawing an Attack gives no block.
        var (c2, p2, _) = Fight();
        c2.Rng = new Rng(0);
        p2.DrawPile.Add(new StrikeSilent());               // top is an Attack
        Play(c2, new EscapePlan(), null);
        Assert.Equal(0, p2.Block);
    }

    [Fact]
    public void CorrosiveWave_Poisons_All_Enemies_On_Each_Draw()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 40);
        var m2 = Monsters.CalcifiedCultist(hp: 40);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        combat.Rng = new Rng(0);
        for (int i = 0; i < 5; i++) player.DrawPile.Add(new StrikeSilent());
        Play(combat, new CorrosiveWave(), null);
        Cmd.Draw(combat, 2);                               // 2 draws → 2×2 poison each
        Assert.Equal(4, m1.GetPowerAmount("Poison"));
        Assert.Equal(4, m2.GetPowerAmount("Poison"));
    }

    [Fact]
    public void Speedster_Damages_All_Enemies_On_Mid_Turn_Draw()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 40);
        var m2 = Monsters.CalcifiedCultist(hp: 40);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        combat.Rng = new Rng(0);
        for (int i = 0; i < 5; i++) player.DrawPile.Add(new StrikeSilent());
        combat.Player.MaxEnergy = 5; combat.Player.ResetEnergy();
        Play(combat, new Speedster(), null);
        Cmd.Draw(combat, 1);                               // 1 mid-turn draw → 2 to each enemy
        Assert.Equal(40 - 2, m1.CurrentHp);
        Assert.Equal(40 - 2, m2.CurrentHp);
    }

    [Fact]
    public void Murder_Deals_1_Plus_Cards_Drawn_This_Combat()
    {
        var (c, _, m) = Fight(monsterHp: 60);
        c.TracksCardsDrawn = true;
        c.CardsDrawnThisCombat = 7;
        Play(c, new Murder(), m);                          // 1 + 7 = 8
        Assert.Equal(60 - 8, m.CurrentHp);
    }

    [Fact]
    public void Murder_Deck_Tracks_Cards_Drawn()
    {
        var player = Catalog.BuildPlayer(new List<CardModel> { new Murder() }, 80, 80);
        var combat = Catalog.SetupCombat(player, new[] { Monsters.CalcifiedCultist(hp: 60) });
        Assert.True(combat.TracksCardsDrawn);              // setup detected Murder in the deck
        combat.Rng = new Rng(0);
        for (int i = 0; i < 5; i++) player.DrawPile.Add(new DefendSilent());
        Cmd.Draw(combat, 3);
        Assert.Equal(3, combat.CardsDrawnThisCombat);      // the counter accumulates draws
    }
}
