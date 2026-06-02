using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>Literal-number checks of ported Ironclad cards against the game's card data + decompiled
/// OnPlay logic (cost/type/target plus the exact damage/block/power they produce through the pipeline).</summary>
public class CardTests
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
    public void Bludgeon_Deals_32()
    {
        var (c, _, m) = Fight();
        Play(c, new Bludgeon(), m);
        Assert.Equal(60 - 32, m.CurrentHp);
    }

    [Fact]
    public void Inflame_Gains_2_Strength()
    {
        var (c, p, _) = Fight();
        Play(c, new Inflame(), null);
        Assert.Equal(2, p.GetPowerAmount("Strength"));
    }

    [Fact]
    public void IronWave_Blocks_5_And_Deals_5()
    {
        var (c, p, m) = Fight();
        Play(c, new IronWave(), m);
        Assert.Equal(5, p.Block);
        Assert.Equal(60 - 5, m.CurrentHp);
    }

    [Fact]
    public void TwinStrike_Deals_5_Twice()
    {
        var (c, _, m) = Fight();
        Play(c, new TwinStrike(), m);
        Assert.Equal(60 - 10, m.CurrentHp);
    }

    [Fact]
    public void Uppercut_Deals_13_And_Applies_Weak_And_Vulnerable()
    {
        var (c, _, m) = Fight();
        Play(c, new Uppercut(), m);
        Assert.Equal(60 - 13, m.CurrentHp);
        Assert.Equal(1, m.GetPowerAmount("Weak"));
        Assert.Equal(1, m.GetPowerAmount("Vulnerable"));
    }

    [Fact]
    public void Hemokinesis_Loses_2_Unblockable_Then_Deals_15()
    {
        var (c, p, m) = Fight();
        p.GainBlockDirect(10);                       // block does NOT absorb the self-loss
        Play(c, new Hemokinesis(), m);
        Assert.Equal(80 - 2, p.CurrentHp);
        Assert.Equal(60 - 15, m.CurrentHp);
    }

    [Fact]
    public void Thunderclap_Hits_All_Enemies_With_Vulnerable()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 40);
        var m2 = Monsters.CalcifiedCultist(hp: 40);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        Play(combat, new Thunderclap(), null);
        Assert.Equal(40 - 4, m1.CurrentHp);
        Assert.Equal(40 - 4, m2.CurrentHp);
        Assert.Equal(1, m1.GetPowerAmount("Vulnerable"));
        Assert.Equal(1, m2.GetPowerAmount("Vulnerable"));
    }

    [Fact]
    public void MoltenFist_Deals_10_Doubles_Vulnerable_And_Exhausts()
    {
        var (c, p, m) = Fight();
        m.AddPower(new VulnerablePower(), 2);
        var fist = new MoltenFist();
        Play(c, fist, m);
        // 10 base × 1.5 (the 2 Vulnerable already on the target) = 15 lost.
        Assert.Equal(60 - 15, m.CurrentHp);
        Assert.Equal(4, m.GetPowerAmount("Vulnerable"));   // doubled 2 -> 4
        Assert.Contains(fist, p.ExhaustPile);              // self-exhausts, not discarded
        Assert.DoesNotContain(fist, p.DiscardPile);
    }

    [Fact]
    public void HowlFromBeyond_Hits_All_For_16()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 40);
        var m2 = Monsters.CalcifiedCultist(hp: 40);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        Play(combat, new HowlFromBeyond(), null);
        Assert.Equal(40 - 16, m1.CurrentHp);
        Assert.Equal(40 - 16, m2.CurrentHp);
    }

    [Fact]
    public void Stomp_Hits_All_Enemies_For_12()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 40);
        var m2 = Monsters.CalcifiedCultist(hp: 40);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        Play(combat, new Stomp(), null);
        Assert.Equal(40 - 12, m1.CurrentHp);
        Assert.Equal(40 - 12, m2.CurrentHp);
    }

    [Fact]
    public void Break_Deals_20_And_Applies_5_Vulnerable()
    {
        var (c, _, m) = Fight();
        Play(c, new Break(), m);
        Assert.Equal(60 - 20, m.CurrentHp);
        Assert.Equal(5, m.GetPowerAmount("Vulnerable"));
    }

    [Fact]
    public void Breakthrough_Loses_1_Unblockable_Then_Hits_All_For_9()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 40);
        var m2 = Monsters.CalcifiedCultist(hp: 40);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        player.GainBlockDirect(10);                  // the self-loss ignores block
        Play(combat, new Breakthrough(), null);
        Assert.Equal(80 - 1, player.CurrentHp);
        Assert.Equal(40 - 9, m1.CurrentHp);
        Assert.Equal(40 - 9, m2.CurrentHp);
    }

    [Fact]
    public void Taunt_Gains_7_Block_And_Applies_1_Vulnerable()
    {
        var (c, p, m) = Fight();
        Play(c, new Taunt(), m);
        Assert.Equal(7, p.Block);
        Assert.Equal(1, m.GetPowerAmount("Vulnerable"));
    }

    [Fact]
    public void Tremble_Applies_3_Vulnerable_And_Exhausts()
    {
        var (c, p, m) = Fight();
        var t = new Tremble();
        Play(c, t, m);
        Assert.Equal(3, m.GetPowerAmount("Vulnerable"));
        Assert.Contains(t, p.ExhaustPile);
        Assert.DoesNotContain(t, p.DiscardPile);
    }

    [Fact]
    public void DemonForm_Grants_2_Strength_At_Each_Turn_Start_Stacking()
    {
        var (c, p, _) = Fight();
        Play(c, new DemonForm(), null);
        Assert.Equal(0, p.GetPowerAmount("Strength"));   // not applied the turn it is played
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(2, p.GetPowerAmount("Strength"));   // +2 at next turn start
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(4, p.GetPowerAmount("Strength"));   // stacks: +2 again
    }

    [Fact]
    public void Rage_Gains_3_Block_Per_Attack_Then_Expires_At_Turn_End()
    {
        var (c, p, m) = Fight();
        Play(c, new Rage(), null);                       // a Skill — does not trigger itself
        Assert.Equal(0, p.Block);
        Play(c, new StrikeIronclad(), m);                // attack → +3 Block
        Assert.Equal(3, p.Block);
        Play(c, new StrikeIronclad(), m);                // attack → +3 Block
        Assert.Equal(6, p.Block);
        CombatManager.EndPlayerTurn(c);
        Assert.False(p.HasPower("Rage"));                // lasts a single turn
    }

    [Fact]
    public void BodySlam_Deals_Damage_Equal_To_Current_Block()
    {
        var (c, p, m) = Fight();
        p.GainBlockDirect(13);
        Play(c, new BodySlam(), m);
        Assert.Equal(60 - 13, m.CurrentHp);              // damage == block
    }

    [Fact]
    public void Bully_Deals_4_Plus_2_Per_Vulnerable()
    {
        var (c, _, m) = Fight();
        m.AddPower(new VulnerablePower(), 3);
        // base 4 + 2*3 = 10, then Vulnerable ×1.5 in the pipeline = 15.
        Play(c, new Bully(), m);
        Assert.Equal(60 - 15, m.CurrentHp);
    }

    [Fact]
    public void AshenStrike_Deals_6_Plus_3_Per_Exhausted_Card()
    {
        var (c, p, m) = Fight();
        p.ExhaustPile.Add(new StrikeIronclad());
        p.ExhaustPile.Add(new StrikeIronclad());         // 2 exhausted → 6 + 3*2 = 12
        Play(c, new AshenStrike(), m);
        Assert.Equal(60 - 12, m.CurrentHp);
    }

    [Fact]
    public void FlameBarrier_Gains_12_Block_And_Retaliates_4()
    {
        var (c, p, m) = Fight();
        Play(c, new FlameBarrier(), null);
        Assert.Equal(12, p.Block);
        Assert.Equal(4, p.GetPowerAmount("FlameBarrier"));
        // a powered attack from the monster takes 4 thorns damage back
        int before = m.CurrentHp;
        Cmd.Attack(c, m, p, 6, ValueProp.Move, null);
        Assert.Equal(before - 4, m.CurrentHp);
    }

    [Fact]
    public void SetupStrike_Boosts_Later_Attacks_Then_Decays_At_Turn_End()
    {
        var (c, p, m) = Fight();
        Play(c, new SetupStrike(), m);
        Assert.Equal(60 - 7, m.CurrentHp);              // own hit not boosted (Strength applied after)
        Assert.Equal(2, p.GetPowerAmount("Strength"));  // +2 temporary Strength
        Assert.Equal(2, p.GetPowerAmount("SetupStrike"));
        Play(c, new StrikeIronclad(), m);               // 6 + 2 Strength = 8
        Assert.Equal(60 - 7 - 8, m.CurrentHp);
        CombatManager.EndPlayerTurn(c);                 // undone at the player's turn end
        Assert.Equal(0, p.GetPowerAmount("Strength"));
        Assert.False(p.HasPower("SetupStrike"));
    }

    [Fact]
    public void Mangle_Reduces_Enemy_Strength_Until_Its_Turn_Ends()
    {
        var (c, p, m) = Fight();
        m.AddPower(new StrengthPower(), 4);             // enemy baseline Strength 4
        Play(c, new Mangle(), m);
        Assert.Equal(60 - 15, m.CurrentHp);            // 15 damage
        Assert.Equal(4 - 10, m.GetPowerAmount("Strength"));  // -10 temporary => -6
        Assert.Equal(10, m.GetPowerAmount("Mangle"));  // marker present during the enemy turn
        // Drive the enemy turn: Incantation applies Ritual 2 (skips its first tick); Mangle restores +10.
        CombatManager.RollInitialMoves(c, new Rng(0));
        c.CurrentSide = CombatSide.Enemy;
        CombatManager.RunEnemyTurn(c);
        Assert.Equal(4, m.GetPowerAmount("Strength")); // -6 + 10 restore = 4 (Ritual skipped this turn)
        Assert.False(m.HasPower("Mangle"));            // marker removed at the enemy's turn end
    }

    [Fact]
    public void Impervious_Gains_30_Block_And_Exhausts()
    {
        var (c, p, _) = Fight();
        var imp = new Impervious();
        Play(c, imp, null);
        Assert.Equal(30, p.Block);
        Assert.Contains(imp, p.ExhaustPile);
        Assert.DoesNotContain(imp, p.DiscardPile);
    }

    [Fact]
    public void BloodWall_Loses_2_Unblockable_Then_Gains_16_Block()
    {
        var (c, p, _) = Fight();
        p.GainBlockDirect(10);                       // the self-loss ignores block
        Play(c, new BloodWall(), null);
        Assert.Equal(80 - 2, p.CurrentHp);
        Assert.Equal(10 + 16, p.Block);
    }

    [Fact]
    public void Colossus_Gains_5_Block_And_Halves_Vulnerable_Enemy_Attacks()
    {
        var (c, p, m) = Fight();
        Play(c, new Colossus(), null);
        Assert.Equal(5, p.Block);
        Assert.Equal(1, p.GetPowerAmount("Colossus"));
        m.AddPower(new VulnerablePower(), 1);        // the attacker is itself Vulnerable
        // 20 base × 0.5 (Colossus) = 10, minus the 5 block = 5 HP lost.
        Cmd.Attack(c, m, p, 20, ValueProp.Move, null);
        Assert.Equal(80 - 5, p.CurrentHp);
    }

    [Fact]
    public void StoneArmor_Gains_Decaying_Block_At_Each_Turn_End()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);                // turn 1 (block not cleared on turn 1)
        Play(c, new StoneArmor(), null);
        Assert.Equal(4, p.GetPowerAmount("Plating"));
        CombatManager.EndPlayerTurn(c);                  // +4 block at turn end
        Assert.Equal(4, p.Block);
        CombatManager.BeginPlayerTurn(c);                // turn 2: clears block, Plating 4 -> 3
        Assert.Equal(0, p.Block);
        Assert.Equal(3, p.GetPowerAmount("Plating"));
        CombatManager.EndPlayerTurn(c);                  // +3 block at turn end
        Assert.Equal(3, p.Block);
    }

    [Fact]
    public void Dominate_Applies_Vulnerable_And_Gains_Strength_Equal_To_Total()
    {
        var (c, p, m) = Fight();
        m.AddPower(new VulnerablePower(), 2);            // pre-existing Vulnerable
        Play(c, new Dominate(), m);
        Assert.Equal(3, m.GetPowerAmount("Vulnerable")); // 2 + 1
        Assert.Equal(3, p.GetPowerAmount("Strength"));   // = the target's total Vulnerable
    }

    [Fact]
    public void NotYet_Heals_10_Capped_At_Max()
    {
        var (c, p, _) = Fight();
        p.CurrentHp = 50;
        var ny = new NotYet();
        Play(c, ny, null);
        Assert.Equal(60, p.CurrentHp);
        Assert.Contains(ny, p.ExhaustPile);
    }

    [Fact]
    public void Conflagration_Hits_All_Enemies_4_Times_For_2()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 40);
        var m2 = Monsters.CalcifiedCultist(hp: 40);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        Play(combat, new Conflagration(), null);
        Assert.Equal(40 - 8, m1.CurrentHp);              // 4 hits × 2
        Assert.Equal(40 - 8, m2.CurrentHp);
    }

    [Fact]
    public void Rupture_Gains_Strength_From_Card_HpLoss_After_The_Card_Resolves()
    {
        var (c, p, m) = Fight();
        Play(c, new Rupture(), null);
        Play(c, new Hemokinesis(), m);                   // lose 2 HP, then deal 15
        Assert.Equal(80 - 2, p.CurrentHp);
        Assert.Equal(60 - 15, m.CurrentHp);              // 15 NOT boosted — Strength applied after the card
        Assert.Equal(1, p.GetPowerAmount("Strength"));   // +1 from the HP loss
        Play(c, new StrikeIronclad(), m);                // now 6 + 1 = 7
        Assert.Equal(60 - 15 - 7, m.CurrentHp);
    }

    [Fact]
    public void Juggernaut_Deals_5_To_Enemy_On_Each_Block_Gain()
    {
        var (c, p, m) = Fight();
        Play(c, new Juggernaut(), null);
        Play(c, new DefendIronclad(), null);             // gain 5 block -> Juggernaut hits for 5
        Assert.Equal(5, p.Block);
        Assert.Equal(60 - 5, m.CurrentHp);
    }

    [Fact]
    public void Barricade_Keeps_Block_Across_Turns()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);                // turn 1
        Play(c, new Barricade(), null);
        p.GainBlockDirect(10);
        CombatManager.EndPlayerTurn(c);
        CombatManager.BeginPlayerTurn(c);                // turn 2 — block normally clears, but Barricade keeps it
        Assert.Equal(10, p.Block);
    }

    [Fact]
    public void Feed_Gains_3_Max_Hp_On_Kill()
    {
        var (c, p, m) = Fight(monsterHp: 8);             // 10 damage kills
        p.CurrentHp = 50;
        Play(c, new Feed(), m);
        Assert.False(m.IsAlive);
        Assert.Equal(83, p.MaxHp);                       // +3 max
        Assert.Equal(53, p.CurrentHp);                   // +3 current
    }

    [Fact]
    public void Feed_Grants_No_Max_Hp_When_Enemy_Survives()
    {
        var (c, p, m) = Fight(monsterHp: 40);
        Play(c, new Feed(), m);
        Assert.Equal(40 - 10, m.CurrentHp);
        Assert.Equal(80, p.MaxHp);                       // survived -> no gain
    }

    [Fact]
    public void ShrugItOff_Gains_8_Block_And_Draws_1_When_Rng_Present()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        p.DrawPile.Add(new StrikeIronclad());
        p.DrawPile.Add(new StrikeIronclad());
        Play(c, new ShrugItOff(), null);
        Assert.Equal(8, p.Block);
        Assert.Single(p.Hand);                           // drew exactly 1
    }

    [Fact]
    public void ShrugItOff_Draw_Is_NoOp_In_Replay_Mode()
    {
        var (c, p, _) = Fight();                          // c.Rng == null (validator replay mode)
        p.DrawPile.Add(new StrikeIronclad());
        Play(c, new ShrugItOff(), null);
        Assert.Equal(8, p.Block);
        Assert.Empty(p.Hand);                            // no mid-turn draw without an ambient Rng
    }

    [Fact]
    public void PommelStrike_Deals_9_And_Draws_1()
    {
        var (c, p, m) = Fight();
        c.Rng = new Rng(0);
        p.DrawPile.Add(new StrikeIronclad());
        Play(c, new PommelStrike(), m);
        Assert.Equal(60 - 9, m.CurrentHp);
        Assert.Single(p.Hand);
    }

    [Fact]
    public void BattleTrance_Draws_3_Then_Blocks_Further_Draws()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        for (int i = 0; i < 6; i++) p.DrawPile.Add(new StrikeIronclad());
        Play(c, new BattleTrance(), null);
        Assert.Equal(3, p.Hand.Count);                   // drew 3
        Assert.True(p.HasPower("NoDraw"));
        Cmd.Draw(c, 2);                                  // suppressed by NoDraw
        Assert.Equal(3, p.Hand.Count);
    }

    [Fact]
    public void FeelNoPain_Gains_3_Block_Per_Exhausted_Card()
    {
        var (c, p, m) = Fight();
        Play(c, new FeelNoPain(), null);
        Play(c, new Tremble(), m);                       // self-exhausts -> +3 block
        Assert.Equal(3, p.Block);
        Play(c, new MoltenFist(), m);                    // self-exhausts -> +3 block
        Assert.Equal(6, p.Block);
    }

    [Fact]
    public void FeelNoPain_Triggers_On_Ethereal_Exhaust_At_Turn_End()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);
        Play(c, new FeelNoPain(), null);
        p.Hand.Add(new Dazed());                         // Ethereal status card
        CombatManager.EndPlayerTurn(c);                  // Dazed exhausts -> FeelNoPain +3
        Assert.Equal(3, p.Block);
    }

    [Fact]
    public void DarkEmbrace_Draws_1_When_A_Card_Is_Exhausted()
    {
        var (c, p, m) = Fight();
        c.Rng = new Rng(0);
        p.DrawPile.Add(new StrikeIronclad());
        Play(c, new DarkEmbrace(), null);
        Play(c, new Tremble(), m);                       // exhaust -> DarkEmbrace draws 1
        Assert.Single(p.Hand);                           // drew the Strike
    }

    [Fact]
    public void Bloodletting_Loses_3_Hp_And_Gains_2_Energy()
    {
        var (c, p, _) = Fight();                          // starts at 3 energy
        Play(c, new Bloodletting(), null);
        Assert.Equal(80 - 3, p.CurrentHp);
        Assert.Equal(5, p.Energy);                       // 3 - 0 + 2
    }

    [Fact]
    public void Offering_Loses_6_Hp_Gains_2_Energy_Draws_3_And_Exhausts()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        for (int i = 0; i < 5; i++) p.DrawPile.Add(new StrikeIronclad());
        var off = new Offering();
        Play(c, off, null);
        Assert.Equal(80 - 6, p.CurrentHp);
        Assert.Equal(5, p.Energy);                       // 3 + 2
        Assert.Equal(3, p.Hand.Count);                   // drew 3
        Assert.Contains(off, p.ExhaustPile);
    }

    [Fact]
    public void Whirlwind_Hits_All_Enemies_Once_Per_Energy()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 40);
        var m2 = Monsters.CalcifiedCultist(hp: 40);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        player.ResetEnergy();                            // 3 energy -> X = 3
        Play(combat, new Whirlwind(), null);
        Assert.Equal(0, player.Energy);                  // all energy consumed
        Assert.Equal(40 - 15, m1.CurrentHp);             // 5 damage × 3 hits
        Assert.Equal(40 - 15, m2.CurrentHp);
    }

    [Fact]
    public void Anger_Deals_6_And_Adds_A_Copy_To_Discard()
    {
        var (c, p, m) = Fight();
        Play(c, new Anger(), m);
        Assert.Equal(60 - 6, m.CurrentHp);
        Assert.Equal(2, p.DiscardPile.Count(x => x is Anger));  // the played card + its copy
    }

    [Fact]
    public void Headbutt_Deals_9_And_Moves_A_Discard_Card_To_Draw_Top()
    {
        var (c, p, m) = Fight();
        p.DiscardPile.Add(new StrikeIronclad());
        Play(c, new Headbutt(), m);
        Assert.Equal(60 - 9, m.CurrentHp);
        Assert.IsType<StrikeIronclad>(p.DrawPile[0]);           // the discarded Strike is now on top of draw
    }

    // ----- In-card CHOICE promoted to a search decision (Headbutt topdeck target) -----

    [Fact]
    public void Headbutt_Choice_Enumerates_One_Play_Per_Distinct_Discard_Card()
    {
        var (c, p, _) = Fight();
        p.Hand.Add(new Headbutt());
        p.DiscardPile.Add(new Bash());
        p.DiscardPile.Add(new StrikeIronclad());
        p.DiscardPile.Add(new StrikeIronclad());   // a duplicate Strike collapses by StateKey (action abstraction)
        p.MaxEnergy = 3; p.ResetEnergy();

        Assert.Equal(2, new Headbutt().Choices(c).Count());      // {Bash, StrikeIronclad}, not 3

        var plays = new Solver().LegalPlays(c).Where(a => a.CardKey == "Headbutt").ToList();
        Assert.Equal(2, plays.Count);                            // one decision per distinct topdeck choice
        Assert.Contains(plays, a => a.ChoiceKey == "Bash");
        Assert.Contains(plays, a => a.ChoiceKey == "StrikeIronclad");
    }

    [Fact]
    public void Headbutt_Topdecks_The_Chosen_Discard_Card_Overriding_The_Default()
    {
        foreach (var pick in new[] { "Bash", "StrikeIronclad" })
        {
            var (c, p, m) = Fight();
            p.DiscardPile.Add(new Bash());
            p.DiscardPile.Add(new StrikeIronclad());             // the default (null choice) would topdeck this last card
            p.Hand.Add(new Headbutt());
            p.MaxEnergy = 3; p.ResetEnergy();
            CombatManager.PlayCard(c, p.Hand.First(h => h is Headbutt), m, pick);
            Assert.Equal(pick, p.DrawPile[0].StateKey());        // the CHOSEN card is topdecked
        }
    }

    // ----- In-card CHOICE promoted to a search decision (Armaments upgrade target) -----

    [Fact]
    public void Armaments_Choice_Enumerates_One_Play_Per_Distinct_Unupgraded_Hand_Card()
    {
        var (c, p, _) = Fight();
        p.Hand.Clear();
        p.Hand.Add(new Armaments());
        p.Hand.Add(new Bash());
        p.Hand.Add(new StrikeIronclad());
        p.Hand.Add(new StrikeIronclad());          // duplicate Strike collapses by StateKey (action abstraction)
        p.MaxEnergy = 3; p.ResetEnergy();

        var arm = p.Hand.First(h => h is Armaments);
        Assert.Equal(2, arm.Choices(c).Count());   // {Bash, StrikeIronclad} — Armaments excludes ITSELF by reference

        var plays = new Solver().LegalPlays(c).Where(a => a.CardKey == "Armaments").ToList();
        Assert.Equal(2, plays.Count);              // one decision per distinct upgrade target
        Assert.Contains(plays, a => a.ChoiceKey == "Bash");
        Assert.Contains(plays, a => a.ChoiceKey == "StrikeIronclad");
    }

    [Fact]
    public void Armaments_Upgrades_The_Chosen_Hand_Card_Overriding_The_Default()
    {
        foreach (var pick in new[] { "Bash", "StrikeIronclad" })
        {
            var (c, p, _) = Fight();
            p.Hand.Clear();
            p.Hand.Add(new Armaments());
            p.Hand.Add(new Bash());                // the default (null choice) would upgrade this first card
            p.Hand.Add(new StrikeIronclad());
            p.MaxEnergy = 3; p.ResetEnergy();

            CombatManager.PlayCard(c, p.Hand.First(h => h is Armaments), null, pick);

            Assert.Equal(1, p.Hand.Count(h => h.Upgrades == 1));            // exactly one card upgraded
            Assert.Equal($"{pick}+1", p.Hand.First(h => h.Upgrades == 1).StateKey());   // the CHOSEN one
        }
    }

    [Fact]
    public void SwordBoomerang_Hits_3_Times_For_3_On_A_Single_Enemy()
    {
        var (c, _, m) = Fight();
        Play(c, new SwordBoomerang(), null);                    // RandomEnemy -> all hits land on the lone enemy
        Assert.Equal(60 - 9, m.CurrentHp);                      // 3 × 3
    }

    [Fact]
    public void Pyre_Raises_Max_Energy_From_The_Next_Turn()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);                       // turn 1: energy = 3
        Play(c, new Pyre(), null);                              // +1 max energy
        Assert.Equal(4, p.EffectiveMaxEnergy);
        CombatManager.BeginPlayerTurn(c);                      // turn 2 reset -> 4 energy
        Assert.Equal(4, p.Energy);
    }

    [Fact]
    public void EvilEye_Gains_8_Block_Or_16_If_A_Card_Was_Exhausted()
    {
        var (c, p, m) = Fight();
        Play(c, new EvilEye(), null);
        Assert.Equal(8, p.Block);                              // nothing exhausted yet -> single
        Play(c, new Tremble(), m);                            // self-exhausts -> sets the flag
        Play(c, new EvilEye(), null);
        Assert.Equal(8 + 16, p.Block);                        // now doubled
    }

    [Fact]
    public void PactsEnd_Deals_17_To_All_Only_With_3_Exhausted()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 40);
        var m2 = Monsters.CalcifiedCultist(hp: 40);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        Play(combat, new PactsEnd(), null);                  // 0 exhausted -> no damage
        Assert.Equal(40, m1.CurrentHp);
        for (int i = 0; i < 3; i++) player.ExhaustPile.Add(new StrikeIronclad());
        Play(combat, new PactsEnd(), null);                  // 3 exhausted -> 17 to all
        Assert.Equal(40 - 17, m1.CurrentHp);
        Assert.Equal(40 - 17, m2.CurrentHp);
    }

    [Fact]
    public void FightMe_Deals_10_Gains_3_Strength_And_Buffs_The_Enemy()
    {
        var (c, p, m) = Fight();
        Play(c, new FightMe(), m);
        Assert.Equal(60 - 10, m.CurrentHp);                  // 5 × 2
        Assert.Equal(3, p.GetPowerAmount("Strength"));
        Assert.Equal(1, m.GetPowerAmount("Strength"));       // the enemy is buffed too
    }

    [Fact]
    public void TearAsunder_Hits_Once_Plus_Once_Per_Unblocked_Hit_Taken()
    {
        var (c, p, m) = Fight();
        Play(c, new TearAsunder(), m);                       // 0 hits taken -> 1 hit × 5
        Assert.Equal(60 - 5, m.CurrentHp);
        Cmd.Attack(c, m, p, 10, ValueProp.Move, null);       // player takes an unblocked hit
        Cmd.Attack(c, m, p, 10, ValueProp.Move, null);       // and another
        Play(c, new TearAsunder(), m);                       // now 1 + 2 = 3 hits × 5 = 15
        Assert.Equal(60 - 5 - 15, m.CurrentHp);
    }

    [Fact]
    public void DemonicShield_Loses_1_And_Doubles_Block()
    {
        var (c, p, _) = Fight();
        p.GainBlockDirect(8);
        Play(c, new DemonicShield(), null);
        Assert.Equal(80 - 1, p.CurrentHp);
        Assert.Equal(16, p.Block);                           // 8 + 8 (current block)
    }

    [Fact]
    public void InfernalBlade_Exhausts_Itself()
    {
        var (c, p, _) = Fight();
        var ib = new InfernalBlade();
        Play(c, ib, null);
        Assert.Contains(ib, p.ExhaustPile);
    }

    [Fact]
    public void Stoke_Exhausts_The_Rest_Of_The_Hand()
    {
        var (c, p, _) = Fight();
        p.Hand.Add(new StrikeIronclad());
        p.Hand.Add(new DefendIronclad());
        Play(c, new Stoke(), null);                          // exhausts the 2 other cards
        Assert.Equal(2, p.ExhaustPile.Count);
        Assert.True(c.CardExhaustedThisTurn);
    }

    [Fact]
    public void Armaments_Gains_5_Block_And_Upgrades_A_Hand_Card()
    {
        var (c, p, _) = Fight();
        var strike = new StrikeIronclad();
        p.Hand.Add(strike);
        Play(c, new Armaments(), null);
        Assert.Equal(5, p.Block);
        // The hand card is REPLACED by a freshly-cloned upgraded copy (the original instance is left untouched
        // so sibling search branches that share it aren't corrupted — see the soundness note on Armaments.OnPlay).
        Assert.Equal(0, strike.Upgrades);                    // original instance unchanged
        var upgraded = (StrikeIronclad)p.Hand[0];
        Assert.Equal(1, upgraded.Upgrades);                  // hand now holds the upgraded clone
        Assert.Equal(9, upgraded.Damage);                    // 6 + 3
    }

    [Fact]
    public void Dismantle_Hits_Once_Or_Twice_On_Vulnerable()
    {
        var (c, _, m) = Fight();
        Play(c, new Dismantle(), m);                          // not Vulnerable -> 8
        Assert.Equal(60 - 8, m.CurrentHp);
        m.AddPower(new VulnerablePower(), 3);                 // now Vulnerable
        // 8 × 1.5 (Vuln) = 12 per hit, × 2 hits = 24
        Play(c, new Dismantle(), m);
        Assert.Equal(60 - 8 - 24, m.CurrentHp);
    }

    [Fact]
    public void PerfectedStrike_Deals_6_Plus_2_Per_Strike_Card()
    {
        var (c, p, m) = Fight();
        p.DrawPile.Add(new StrikeIronclad());                // Strike-tagged
        p.DrawPile.Add(new StrikeIronclad());
        p.DiscardPile.Add(new TwinStrike());                 // Strike-tagged
        p.Hand.Add(new DefendIronclad());                    // not a Strike
        // 3 Strikes in piles + this Perfected Strike (1) = 4 Strikes -> 6 + 2×4 = 14
        Play(c, new PerfectedStrike(), m);
        Assert.Equal(60 - 14, m.CurrentHp);
    }

    [Fact]
    public void Rampage_Escalates_Its_Own_Damage_Each_Play()
    {
        var (c, _, m) = Fight(monsterHp: 100);
        var r = new Rampage();
        Play(c, r, m);                                       // 9
        Assert.Equal(100 - 9, m.CurrentHp);
        Play(c, r, m);                                       // 9 + 5 = 14
        Assert.Equal(100 - 9 - 14, m.CurrentHp);
        Play(c, r, m);                                       // 14 + 5 = 19
        Assert.Equal(100 - 9 - 14 - 19, m.CurrentHp);
    }

    [Fact]
    public void Pillage_Deals_6_And_Draws()
    {
        var (c, p, m) = Fight();
        c.Rng = new Rng(0);
        p.DrawPile.Add(new DefendIronclad());
        Play(c, new Pillage(), m);
        Assert.Equal(60 - 6, m.CurrentHp);
        Assert.Single(p.Hand);
    }

    [Fact]
    public void ExpectAFight_Gains_Energy_Per_Attack_In_Hand_And_Blocks_Further_Gain()
    {
        var (c, p, _) = Fight();
        p.ResetEnergy();                                     // 3
        p.Hand.Add(new StrikeIronclad());                    // 2 attacks in hand
        p.Hand.Add(new StrikeIronclad());
        p.Hand.Add(new DefendIronclad());                    // a non-attack (doesn't count)
        var eaf = new ExpectAFight(); p.Hand.Add(eaf);
        CombatManager.PlayCard(c, eaf, null);                // costs 2, gains 2 (two Strikes)
        Assert.Equal(3 - 2 + 2, p.Energy);                   // 3
        Assert.True(p.HasPower("NoEnergyGain"));
        Cmd.GainEnergy(c, 5);                                // suppressed
        Assert.Equal(3, p.Energy);
    }

    [Fact]
    public void Vicious_Draws_When_You_Apply_Vulnerable()
    {
        var (c, p, m) = Fight();
        c.Rng = new Rng(0);
        for (int i = 0; i < 3; i++) p.DrawPile.Add(new StrikeIronclad());
        Play(c, new Vicious(), null);
        Play(c, new Bash(), m);                              // Bash applies Vulnerable -> Vicious draws 1
        Assert.Equal(1, p.Hand.Count(x => x is StrikeIronclad));
    }

    [Fact]
    public void Inferno_Self_Damages_And_Torches_All_Enemies_At_Turn_Start()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var m1 = Monsters.CalcifiedCultist(hp: 40);
        var m2 = Monsters.CalcifiedCultist(hp: 40);
        var combat = Catalog.SetupCombat(player, new[] { m1, m2 });
        CombatManager.BeginPlayerTurn(combat);
        Play(combat, new Inferno(), null);                   // SelfDamage 0 -> 1
        Assert.Equal(80, player.CurrentHp);                  // nothing the turn it's played
        CombatManager.BeginPlayerTurn(combat);               // turn 2: lose 1 HP, deal 6 to both enemies
        Assert.Equal(80 - 1, player.CurrentHp);
        Assert.Equal(40 - 6, m1.CurrentHp);
        Assert.Equal(40 - 6, m2.CurrentHp);
    }

    [Fact]
    public void DrumOfBattle_Draws_2()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        for (int i = 0; i < 4; i++) p.DrawPile.Add(new StrikeIronclad());
        Play(c, new DrumOfBattle(), null);
        Assert.Equal(2, p.Hand.Count);
    }

    [Fact]
    public void BurningPact_Exhausts_A_Card_And_Draws_2()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        p.Hand.Add(new StrikeIronclad());                    // a card to exhaust
        for (int i = 0; i < 3; i++) p.DrawPile.Add(new DefendIronclad());
        Play(c, new BurningPact(), null);
        Assert.Equal(1, p.ExhaustPile.Count(x => x is StrikeIronclad));
        Assert.Equal(2, p.Hand.Count(x => x is DefendIronclad));   // drew 2
    }

    [Fact]
    public void Brand_Loses_1_Hp_Exhausts_A_Card_And_Gains_1_Strength()
    {
        var (c, p, _) = Fight();
        p.Hand.Add(new StrikeIronclad());
        Play(c, new Brand(), null);
        Assert.Equal(80 - 1, p.CurrentHp);
        Assert.Equal(1, p.GetPowerAmount("Strength"));
        Assert.Equal(1, p.ExhaustPile.Count(x => x is StrikeIronclad));
    }

    [Fact]
    public void FiendFire_Exhausts_Hand_And_Deals_7_Per_Card()
    {
        var (c, p, m) = Fight();
        p.Hand.Add(new DefendIronclad());
        p.Hand.Add(new DefendIronclad());                    // 2 other cards in hand
        var ff = new FiendFire();
        Play(c, ff, m);                                      // exhausts the 2 Defends, deals 7 × 2 = 14
        Assert.Equal(60 - 14, m.CurrentHp);
        Assert.Equal(2, p.ExhaustPile.Count(x => x is DefendIronclad));
        Assert.Contains(ff, p.ExhaustPile);                 // Fiend Fire itself exhausts too
    }

    [Fact]
    public void SecondWind_Exhausts_NonAttacks_And_Blocks_Per_Card()
    {
        var (c, p, _) = Fight();
        p.Hand.Add(new DefendIronclad());                    // non-attack
        p.Hand.Add(new Inflame());                           // non-attack (power)
        p.Hand.Add(new StrikeIronclad());                    // attack -> NOT exhausted
        Play(c, new SecondWind(), null);
        Assert.Equal(10, p.Block);                           // 2 non-attacks × 5
        Assert.Contains(p.Hand, x => x is StrikeIronclad);   // the Strike stays
    }

    [Fact]
    public void TrueGrit_Gains_7_Block_And_Exhausts_A_Card()
    {
        var (c, p, _) = Fight();
        p.Hand.Add(new StrikeIronclad());
        Play(c, new TrueGrit(), null);
        Assert.Equal(7, p.Block);
        Assert.Equal(1, p.ExhaustPile.Count(x => x is StrikeIronclad));
    }

    [Fact]
    public void Cinder_Deals_18_And_Exhausts_A_Card()
    {
        var (c, p, m) = Fight();
        p.Hand.Add(new StrikeIronclad());
        Play(c, new Cinder(), m);
        Assert.Equal(60 - 18, m.CurrentHp);
        Assert.Equal(1, p.ExhaustPile.Count(x => x is StrikeIronclad));
    }

    [Fact]
    public void Unmovable_Doubles_The_First_Card_Block_Each_Turn()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);
        Play(c, new Unmovable(), null);
        Play(c, new DefendIronclad(), null);                 // first block gain -> 5 × 2 = 10
        Assert.Equal(10, p.Block);
        Play(c, new DefendIronclad(), null);                 // second -> normal 5
        Assert.Equal(15, p.Block);
    }

    [Fact]
    public void Juggling_Adds_A_Copy_On_Every_Third_Attack()
    {
        var (c, p, m) = Fight();
        CombatManager.BeginPlayerTurn(c);
        Play(c, new Juggling(), null);
        Play(c, new StrikeIronclad(), m);                    // 1st attack
        Play(c, new StrikeIronclad(), m);                    // 2nd
        Assert.Empty(p.Hand);
        Play(c, new StrikeIronclad(), m);                    // 3rd -> a Strike copy joins hand
        Assert.Single(p.Hand);
        Assert.IsType<StrikeIronclad>(p.Hand[0]);
    }

    [Fact]
    public void CrimsonMantle_Loses_Hp_And_Gains_Block_At_Turn_Start_Scaling_With_Copies()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);                    // turn 1
        Play(c, new CrimsonMantle(), null);                  // SelfDamage 0 -> 1, block amount 8
        Play(c, new CrimsonMantle(), null);                  // SelfDamage 1 -> 2, block amount 8+8 = 16
        Assert.Equal(0, p.Block);                            // nothing happens the turn it's played
        CombatManager.BeginPlayerTurn(c);                    // turn 2 start: lose 2 HP, gain 16 block
        Assert.Equal(80 - 2, p.CurrentHp);
        Assert.Equal(16, p.Block);
    }

    [Fact]
    public void Spite_Hits_Twice_Only_After_You_Lost_Hp_This_Turn()
    {
        var (c, p, m) = Fight();
        Play(c, new Spite(), m);                             // no HP lost yet -> single 5
        Assert.Equal(60 - 5, m.CurrentHp);
        Play(c, new Bloodletting(), null);                   // lose 3 HP on your own turn -> sets the flag
        Assert.True(c.PlayerLostHpThisTurn);
        Play(c, new Spite(), m);                             // now 5 × 2 = 10
        Assert.Equal(60 - 5 - 10, m.CurrentHp);
    }

    [Fact]
    public void Cruelty_Raises_The_Vulnerable_Multiplier()
    {
        var (c, p, m) = Fight();
        m.AddPower(new VulnerablePower(), 2);
        Play(c, new Cruelty(), null);                        // +25% -> Vulnerable ×1.75
        Play(c, new StrikeIronclad(), m);                    // 6 × 1.75 = 10.5 -> floor 10
        Assert.Equal(60 - 10, m.CurrentHp);
    }

    [Fact]
    public void Unrelenting_Deals_14_And_Makes_The_Next_Attack_Free()
    {
        var (c, p, m) = Fight();
        p.ResetEnergy();                                     // 3
        var u = new Unrelenting(); p.Hand.Add(u);
        CombatManager.PlayCard(c, u, m);                     // costs 2
        Assert.Equal(60 - 14, m.CurrentHp);
        Assert.Equal(1, p.Energy);
        Assert.True(p.HasPower("FreeAttack"));
        var b = new Bludgeon(); p.Hand.Add(b);               // cost 3 — but Free Attack zeroes it
        CombatManager.PlayCard(c, b, m);                     // would throw (1 < 3) if not free
        Assert.Equal(1, p.Energy);                           // unchanged
        Assert.False(p.HasPower("FreeAttack"));             // consumed
    }

    [Fact]
    public void Corruption_Makes_Skills_Cost_0_And_Exhaust()
    {
        var (c, p, _) = Fight();
        p.ResetEnergy();                                     // 3
        var corr = new Corruption(); p.Hand.Add(corr);
        CombatManager.PlayCard(c, corr, null);               // costs 3 -> 0 energy left
        Assert.Equal(0, p.Energy);
        var d = new DefendIronclad(); p.Hand.Add(d);         // a Skill, normally cost 1
        CombatManager.PlayCard(c, d, null);                  // now free + exhausts
        Assert.Equal(5, p.Block);
        Assert.Contains(d, p.ExhaustPile);
        Assert.DoesNotContain(d, p.DiscardPile);
    }

    [Fact]
    public void OneTwoPunch_Makes_The_Next_Attack_Resolve_Twice_Then_Expires()
    {
        var (c, p, m) = Fight();
        Play(c, new OneTwoPunch(), null);                     // a Skill — doesn't trigger itself
        Assert.Equal(1, p.GetPowerAmount("OneTwoPunch"));
        Play(c, new StrikeIronclad(), m);                    // 6 dmg resolved twice = 12
        Assert.Equal(60 - 12, m.CurrentHp);
        Assert.False(p.HasPower("OneTwoPunch"));             // charge consumed
        Play(c, new StrikeIronclad(), m);                    // back to a single 6
        Assert.Equal(60 - 12 - 6, m.CurrentHp);
    }

    [Fact]
    public void ForgottenRitual_Refunds_3_Energy_Only_After_An_Exhaust()
    {
        // No prior exhaust -> no refund (just pays its 1 cost).
        var (c1, p1, _) = Fight();
        p1.ResetEnergy();                                     // 3
        var fr1 = new ForgottenRitual(); p1.Hand.Add(fr1);
        CombatManager.PlayCard(c1, fr1, null);
        Assert.Equal(2, p1.Energy);                           // 3 - 1
        Assert.Contains(fr1, p1.ExhaustPile);                // it exhausts itself

        // A card exhausted earlier this turn -> +3 energy refund.
        var (c2, p2, _) = Fight();
        c2.CardExhaustedThisTurn = true;
        p2.ResetEnergy();                                     // 3
        var fr2 = new ForgottenRitual(); p2.Hand.Add(fr2);
        CombatManager.PlayCard(c2, fr2, null);
        Assert.Equal(3 - 1 + 3, p2.Energy);                  // 5
    }
}
