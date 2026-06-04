using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>Literal-number checks of the ported Event/Ancient special-pool cards (Special/) and the curses
/// (Core/Curses.cs) against the game's card data + decompiled OnPlay logic. Kept separate from the
/// per-character card-test files. Cmd.Draw is a no-op without an ambient Rng, so card-draw counts are not
/// asserted (HP-neutral, replayed from traces); these tests cover the HP / Block / power / debuff effects.</summary>
public class SpecialAndCurseCardTests
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

    /// <summary>Fire a held curse's end-of-turn-in-hand effect, exactly as the engine would.</summary>
    private static void EndTurnHolding(CombatState combat, params CardModel[] cards)
    {
        foreach (var c in cards) combat.Player.Hand.Add(c);
        CombatManager.EndPlayerTurn(combat);
    }

    // ======================================================================
    // Catalog wiring
    // ======================================================================

    [Fact]
    public void Catalog_Builds_Special_And_Curse_Cards()
    {
        Assert.IsType<NeowsFury>(Catalog.BuildCard("NeowsFury"));
        Assert.IsType<Maul>(Catalog.BuildCard("Maul"));
        Assert.IsType<ByrdSwoop>(Catalog.BuildCard("ByrdSwoop"));
        Assert.IsType<WraithForm>(Catalog.BuildCard("WraithForm"));
        Assert.IsType<BadLuck>(Catalog.BuildCard("BadLuck"));
        Assert.IsType<Normality>(Catalog.BuildCard("Normality"));
        Assert.Equal(CardRarity.Event, Catalog.BuildCard("Squash").Rarity);
        Assert.Equal(CardRarity.Ancient, Catalog.BuildCard("Whistle").Rarity);
    }

    [Fact]
    public void Special_Cards_Are_In_CardPool_But_Curses_Are_Not()
    {
        Assert.Contains("NeowsFury", Catalog.CardPool);
        Assert.Contains("Squash", Catalog.CardPool);
        Assert.DoesNotContain("BadLuck", Catalog.CardPool);
        Assert.DoesNotContain("Clumsy", Catalog.CardPool);
    }

    // ======================================================================
    // EVENT attacks
    // ======================================================================

    [Fact]
    public void ByrdSwoop_Deals_14_At_Cost_0()
    {
        var (c, _, m) = Fight();
        var b = new ByrdSwoop();
        Assert.Equal(0, b.Cost);
        Play(c, b, m);
        Assert.Equal(60 - 14, m.CurrentHp);
    }

    [Fact]
    public void Exterminate_Hits_All_Enemies_3_Times_4()
    {
        var (c, _, m1, m2) = Fight2(40);
        Play(c, new Exterminate(), m1);
        Assert.Equal(40 - 12, m1.CurrentHp);   // 3 dmg x 4 hits
        Assert.Equal(40 - 12, m2.CurrentHp);
    }

    [Fact]
    public void Peck_Deals_2_Three_Times_And_Upgrade_Adds_A_Hit()
    {
        var (c, _, m) = Fight();
        Play(c, new Peck(), m);
        Assert.Equal(60 - 6, m.CurrentHp);

        var (c2, _, m2) = Fight();
        Play(c2, (CardModel)new Peck().Upgraded(1), m2);
        Assert.Equal(60 - 8, m2.CurrentHp);    // 4 hits
    }

    [Fact]
    public void Squash_Deals_10_And_Applies_2_Vulnerable()
    {
        var (c, _, m) = Fight();
        Play(c, new Squash(), m);
        Assert.Equal(60 - 10, m.CurrentHp);
        Assert.Equal(2, m.GetPowerAmount("Vulnerable"));
    }

    [Fact]
    public void RipAndTear_Deals_7_Twice_To_Single_Enemy()
    {
        var (c, _, m) = Fight();
        Play(c, new RipAndTear(), null);       // RandomEnemy: hits the only living enemy
        Assert.Equal(60 - 14, m.CurrentHp);
    }

    [Fact]
    public void Rebound_Deals_9_And_Applies_Rebound_Marker()
    {
        var (c, p, m) = Fight();
        Play(c, new Rebound(), m);
        Assert.Equal(60 - 9, m.CurrentHp);
        Assert.True(p.HasPower("Rebound"));
    }

    // ======================================================================
    // EVENT skills / powers
    // ======================================================================

    [Fact]
    public void Entrench_Doubles_Current_Block()
    {
        var (c, p, _) = Fight();
        p.GainBlockDirect(10);
        Play(c, new Entrench(), null);
        Assert.Equal(20, p.Block);
    }

    [Fact]
    public void Stack_Gains_Block_Equal_To_Discard_Count()
    {
        var (c, p, _) = Fight();
        p.DiscardPile.Add(new ByrdSwoop());
        p.DiscardPile.Add(new ByrdSwoop());
        p.DiscardPile.Add(new ByrdSwoop());
        Play(c, new Stack(), null);
        Assert.Equal(3, p.Block);
    }

    [Fact]
    public void Outmaneuver_Grants_2_Energy_Next_Turn()
    {
        var (c, p, _) = Fight();
        Play(c, new Outmaneuver(), null);
        CombatManager.EndPlayerTurn(c);
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(p.MaxEnergy + 2, p.Energy);
        Assert.False(p.HasPower("EnergyNextTurn"));   // consumed
    }

    [Fact]
    public void FeedingFrenzy_Grants_5_Strength_This_Turn_Then_Undone()
    {
        var (c, p, _) = Fight();
        Play(c, new FeedingFrenzy(), null);
        Assert.Equal(5, p.GetPowerAmount("Strength"));
        CombatManager.EndPlayerTurn(c);
        Assert.Equal(0, p.GetPowerAmount("Strength"));   // temporary -> removed at turn end
    }

    [Fact]
    public void ToricToughness_Gains_5_And_Regains_5_Next_Two_Turns()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);                 // turn 1 (so later turns clear block)
        Play(c, new ToricToughness(), null);
        Assert.Equal(5, p.Block);
        Assert.Equal(2, p.GetPowerAmount("ToricToughness"));

        CombatManager.EndPlayerTurn(c);
        CombatManager.BeginPlayerTurn(c);                 // turn 2: block cleared, then re-granted
        Assert.Equal(5, p.Block);
        Assert.Equal(1, p.GetPowerAmount("ToricToughness"));

        CombatManager.EndPlayerTurn(c);
        CombatManager.BeginPlayerTurn(c);                 // turn 3: re-granted, then expires
        Assert.Equal(5, p.Block);
        Assert.False(p.HasPower("ToricToughness"));       // expired
    }

    [Theory]
    [InlineData("HelloWorld")]
    [InlineData("ForbiddenGrimoire")]
    public void Inert_Power_Cards_Apply_Their_Marker(string name)
    {
        var (c, p, _) = Fight();
        var card = Catalog.BuildCard(name);
        Play(c, card, null);
        Assert.True(p.Powers.Count >= 1);   // the inert marker is attached
    }

    [Fact]
    public void Distraction_And_DualWield_Are_Inert_And_HP_Neutral()
    {
        var (c, p, m) = Fight();
        Play(c, new Distraction(), null);
        Play(c, new DualWield(), null);
        Assert.Equal(60, m.CurrentHp);
        Assert.Equal(80, p.CurrentHp);
        Assert.Contains(p.ExhaustPile, x => x is Distraction);   // Distraction exhausts
    }

    // ======================================================================
    // ANCIENT cards
    // (MeteorShower + TheSealedThrone are Stars-powered Ancient cards owned by the Regent module; their
    //  behaviour is covered in RegentCardTests, which sets up the Stars resource their StarCost requires.)
    // ======================================================================

    [Fact]
    public void Maul_Deals_5_Twice_And_Escalates_By_1_Per_Play()
    {
        var (c, _, m) = Fight(100);
        var maul = new Maul();
        Play(c, maul, m);
        Assert.Equal(100 - 10, m.CurrentHp);    // 5 x 2
        Play(c, maul, m);                        // same instance, now 6 x 2
        Assert.Equal(100 - 10 - 12, m.CurrentHp);
    }

    [Fact]
    public void NeowsFury_Deals_10_And_Exhausts()
    {
        var (c, p, m) = Fight();
        Play(c, new NeowsFury(), m);
        Assert.Equal(60 - 10, m.CurrentHp);
        Assert.Contains(p.ExhaustPile, x => x is NeowsFury);
    }

    [Fact]
    public void Whistle_Deals_33_And_Exhausts()
    {
        var (c, p, m) = Fight(100);
        Play(c, new Whistle(), m);
        Assert.Equal(100 - 33, m.CurrentHp);
        Assert.Contains(p.ExhaustPile, x => x is Whistle);
    }

    [Fact]
    public void BrightestFlame_Loses_1_Max_Hp_And_Gains_Energy()
    {
        var (c, p, _) = Fight();
        int e0 = p.Energy;
        Play(c, new BrightestFlame(), null);
        Assert.Equal(79, p.MaxHp);
        Assert.Equal(79, p.CurrentHp);
        Assert.Equal(e0 + 2, p.Energy);
    }

    [Fact]
    public void Relax_Gains_15_Block_And_2_Energy_Next_Turn()
    {
        var (c, p, _) = Fight();
        Play(c, new Relax(), null);
        Assert.Equal(15, p.Block);
        CombatManager.EndPlayerTurn(c);
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(p.MaxEnergy + 2, p.Energy);
    }

    [Fact]
    public void Apparition_Grants_Intangible_And_Is_Ethereal_Until_Upgraded()
    {
        var (c, p, _) = Fight();
        var a = new Apparition();
        Assert.True(a.Ethereal);
        Play(c, a, null);
        Assert.Equal(1, p.GetPowerAmount("Intangible"));
        Assert.False(((Apparition)Catalog.BuildCard("Apparition+1")).Ethereal);
    }

    [Fact]
    public void Intangible_Caps_Incoming_HP_Loss_To_1()
    {
        var (c, p, m) = Fight();
        Cmd.ApplyPower(c, p, new IntangiblePower(), 1, p);
        Cmd.Attack(c, m, p, 30, ValueProp.Move, null);   // would be 30
        Assert.Equal(80 - 1, p.CurrentHp);
    }

    [Fact]
    public void WraithForm_Grants_2_Intangible_And_Loses_Dexterity_Each_Turn()
    {
        var (c, p, _) = Fight();
        Play(c, new WraithForm(), null);
        Assert.Equal(2, p.GetPowerAmount("Intangible"));
        CombatManager.EndPlayerTurn(c);
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(-1, p.GetPowerAmount("Dexterity"));   // loses 1 Dexterity per turn
    }

    // ======================================================================
    // CURSES — end-of-turn-in-hand effects
    // ======================================================================

    [Fact]
    public void BadLuck_Loses_13_Hp_At_End_Of_Turn_Unblockable()
    {
        var (c, p, _) = Fight();
        p.GainBlockDirect(50);                  // unblockable -> block ignored
        EndTurnHolding(c, new BadLuck());
        Assert.Equal(80 - 13, p.CurrentHp);
    }

    [Fact]
    public void Decay_Deals_2_At_End_Of_Turn_And_Is_Blocked()
    {
        var (c, p, _) = Fight();
        EndTurnHolding(c, new Decay());
        Assert.Equal(80 - 2, p.CurrentHp);

        var (c2, p2, _) = Fight();
        p2.GainBlockDirect(5);                  // blockable
        EndTurnHolding(c2, new Decay());
        Assert.Equal(80, p2.CurrentHp);
        Assert.Equal(3, p2.Block);
    }

    [Fact]
    public void Regret_Loses_Hp_Equal_To_Hand_Size()
    {
        var (c, p, _) = Fight();
        // Hand at end of turn: Regret + 2 other cards = 3 cards.
        EndTurnHolding(c, new Regret(), new Injury(), new Greed());
        Assert.Equal(80 - 3, p.CurrentHp);
    }

    [Fact]
    public void Doubt_Gains_Weak_That_Survives_To_Next_Turn()
    {
        var (c, p, _) = Fight();
        EndTurnHolding(c, new Doubt());
        Assert.Equal(1, p.GetPowerAmount("Weak"));   // SkipNextTick -> NOT consumed by this turn-end tick
    }

    [Fact]
    public void Shame_Gains_Frail_That_Survives_To_Next_Turn()
    {
        var (c, p, _) = Fight();
        EndTurnHolding(c, new Shame());
        Assert.Equal(1, p.GetPowerAmount("Frail"));
    }

    [Fact]
    public void Doubt_Weakens_Next_Turn_Attacks_Then_Wears_Off()
    {
        var (c, p, m) = Fight(100);
        EndTurnHolding(c, new Doubt());          // gain Weak 1 (Cmd.ApplyPower sets SkipNextTick for player debuffs)
        CombatManager.RunEnemyTurn(c);           // enemy turn end: the skip is consumed, Weak persists
        CombatManager.BeginPlayerTurn(c);        // player's next turn: still Weak (weakens these attacks)
        Assert.Equal(1, p.GetPowerAmount("Weak"));
        CombatManager.EndPlayerTurn(c);          // player turn end does NOT tick Weak — it ticks at the ENEMY turn end
        Assert.Equal(1, p.GetPowerAmount("Weak"));
        p.GetPower("Weak")!.AfterSideTurnEnd(c, CombatSide.Enemy);   // the enemy turn end finally ticks it down
        Assert.Equal(0, p.GetPowerAmount("Weak"));
    }

    // ======================================================================
    // CURSES — pure dilution / playable
    // ======================================================================

    [Theory]
    [InlineData("Clumsy")]
    [InlineData("CurseOfTheBell")]
    [InlineData("Folly")]
    [InlineData("Greed")]
    [InlineData("Injury")]
    [InlineData("PoorSleep")]
    [InlineData("Writhe")]
    [InlineData("Guilty")]
    [InlineData("Debt")]
    [InlineData("Normality")]
    public void Dilution_Curses_Are_Unplayable_And_HP_Neutral(string name)
    {
        var (c, p, _) = Fight();
        var curse = Catalog.BuildCard(name);
        Assert.True(curse.Unplayable);
        EndTurnHolding(c, curse);
        Assert.Equal(80, p.CurrentHp);
    }

    [Fact]
    public void Ethereal_Curses_Exhaust_At_End_Of_Turn()
    {
        var (c, p, _) = Fight();
        var clumsy = new Clumsy();
        Assert.True(clumsy.Ethereal);
        EndTurnHolding(c, clumsy);
        Assert.Contains(p.ExhaustPile, x => x is Clumsy);
    }

    [Fact]
    public void SporeMind_Is_Playable_And_Exhausts_With_No_Effect()
    {
        var (c, p, m) = Fight();
        var s = new SporeMind();
        Assert.False(s.Unplayable);
        Assert.Equal(1, s.Cost);
        Play(c, s, null);
        Assert.Equal(60, m.CurrentHp);
        Assert.Equal(80, p.CurrentHp);
        Assert.Contains(p.ExhaustPile, x => x is SporeMind);
    }

    [Fact]
    public void Enthralled_Is_Playable_Cost_2_No_Op()
    {
        var (c, p, m) = Fight();
        var e = new Enthralled();
        Assert.False(e.Unplayable);
        Assert.Equal(2, e.Cost);
        Play(c, e, null);
        Assert.Equal(60, m.CurrentHp);
        Assert.Equal(80, p.CurrentHp);
    }
}
