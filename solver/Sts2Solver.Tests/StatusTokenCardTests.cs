using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>Literal-number checks of the remaining Status pool cards and the Token pool cards against the
/// decompiled OnPlay / in-hand logic. In-hand end-of-turn damage is triggered via EndPlayerTurn (mirroring the
/// Infection/Burn pipeline test).</summary>
public class StatusTokenCardTests
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
    public void Catalog_Builds_All_Status_And_Token_Cards()
    {
        foreach (var name in new[]
        {
            "Beckon","Debris","FranticEscape","Soot","Toxic","Wither","Disintegration","MindRot","Sloth",
            "WasteAway","GiantRock","Luminesce","MinionDiveBomb","MinionSacrifice","MinionStrike",
        })
            Assert.NotNull(Catalog.BuildCard(name));
    }

    // ---------------- Status: in-hand end-of-turn damage ----------------

    [Fact]
    public void Beckon_Deals_6_Unblockable_At_Turn_End()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);
        p.Hand.Add(new Beckon());
        Cmd.GainBlock(c, p, 20, ValueProp.Move, null);   // block does NOT stop it
        CombatManager.EndPlayerTurn(c);
        Assert.Equal(80 - 6, p.CurrentHp);   // 6 dealt through block
        Assert.Equal(20, p.Block);           // block untouched (unblockable bypasses it)
    }

    [Fact]
    public void Beckon_Played_Has_No_Effect()
    {
        var (c, p, _) = Fight();
        var b = new Beckon();
        Play(c, b, null);
        Assert.Equal(80, p.CurrentHp);
        Assert.DoesNotContain(b, p.Hand);
    }

    [Fact]
    public void Toxic_Deals_5_Blockable_At_Turn_End_And_Exhausts_If_Played()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);
        p.Hand.Add(new Toxic());
        Cmd.GainBlock(c, p, 3, ValueProp.Move, null);   // 3 absorbed of 5
        CombatManager.EndPlayerTurn(c);
        Assert.Equal(80 - 2, p.CurrentHp);
        // Playable + Exhaust: playing it removes it for the turn (dodging the hit).
        var (c2, p2, _) = Fight();
        var t = new Toxic();
        Play(c2, t, null);
        Assert.Contains(t, p2.ExhaustPile);
    }

    [Fact]
    public void Wither_Deals_3_Blockable_And_Is_Unplayable()
    {
        var (c, p, _) = Fight();
        CombatManager.BeginPlayerTurn(c);
        p.Hand.Add(new Wither());
        CombatManager.EndPlayerTurn(c);
        Assert.Equal(80 - 3, p.CurrentHp);
        Assert.True(new Wither().Unplayable);
    }

    [Fact]
    public void Inert_Status_Cards_Are_Unplayable_With_No_Effect()
    {
        foreach (var s in new CardModel[] { new Soot(), new Disintegration(), new MindRot(), new Sloth(), new WasteAway() })
            Assert.True(s.Unplayable, $"{s.Name} should be Unplayable");
    }

    [Fact]
    public void Debris_Is_Playable_And_Exhausts()
    {
        var (c, p, _) = Fight();
        var d = new Debris();
        Assert.False(d.Unplayable);
        Play(c, d, null);
        Assert.Contains(d, p.ExhaustPile);
    }

    // ---------------- Token cards ----------------

    [Theory]
    [InlineData(0, 20)]
    [InlineData(1, 24)]
    public void GiantRock_Deals_20_Plus_Upgrade(int upg, int dmg)
    {
        var (c, _, m) = Fight();
        Play(c, (CardModel)new GiantRock().Upgraded(upg), m);
        Assert.Equal(60 - dmg, m.CurrentHp);
    }

    [Fact]
    public void Luminesce_Gains_2_Energy_Exhaust_Retain()
    {
        var (c, p, _) = Fight();
        p.MaxEnergy = 3; p.ResetEnergy();
        int before = p.Energy;
        var l = new Luminesce();
        Assert.True(l.Retain);
        Play(c, l, null);
        Assert.Equal(before + 2, p.Energy);
        Assert.Contains(l, p.ExhaustPile);
    }

    [Fact]
    public void MinionDiveBomb_Deals_13_And_Exhausts()
    {
        var (c, p, m) = Fight();
        var d = new MinionDiveBomb();
        Play(c, d, m);
        Assert.Equal(60 - 13, m.CurrentHp);
        Assert.Contains(d, p.ExhaustPile);
    }

    [Fact]
    public void MinionSacrifice_Gains_8_Block_And_Exhausts()
    {
        var (c, p, _) = Fight();
        var s = new MinionSacrifice();
        Play(c, s, null);
        Assert.Equal(8, p.Block);
        Assert.Contains(s, p.ExhaustPile);
    }

    [Fact]
    public void MinionStrike_Deals_6_And_Is_A_Strike()
    {
        var (c, p, m) = Fight();
        var s = new MinionStrike();
        Assert.True(s.IsStrike);
        Play(c, s, m);
        Assert.Equal(60 - 6, m.CurrentHp);
        Assert.Contains(s, p.ExhaustPile);
    }
}
