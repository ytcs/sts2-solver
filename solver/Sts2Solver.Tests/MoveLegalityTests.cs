using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>The move-legality + stun subsystem: Normality's per-turn play cap, Enthralled's hand lockout (both
/// HARMS that must be enforced so the search doesn't over-credit), and Whistle's bounded one-turn monster stun.</summary>
public class MoveLegalityTests
{
    private static CombatState Fight(int playerHp = 80)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: playerHp, maxHp: playerHp);
        var combat = Catalog.SetupCombat(player, new[] { Monsters.CalcifiedCultist(hp: 60) });
        player.ResetEnergy();
        return combat;
    }

    // ---------------- Normality: per-turn play cap ----------------

    [Fact]
    public void Normality_Caps_Plays_At_3_While_In_Hand()
    {
        var c = Fight();
        Assert.Equal(CombatState.MaxPlaysPerTurn, c.EffectivePlayCap());   // no cap normally
        c.Player.Hand.Add(new Normality());
        Assert.Equal(3, c.EffectivePlayCap());                            // capped while held
        c.Player.Hand.RemoveAt(c.Player.Hand.Count - 1);
        Assert.Equal(CombatState.MaxPlaysPerTurn, c.EffectivePlayCap());   // cap lifts when it leaves hand
    }

    [Fact]
    public void Normality_Cap_Gates_The_Setup_Play_Bounding()
    {
        // A deck holding Normality must bound + hash PlaysThisTurn so the cap memoises soundly.
        var player = Catalog.BuildPlayer(new List<CardModel> { new Normality(), new StrikeIronclad() }, 80, 80);
        var combat = Catalog.SetupCombat(player, new[] { Monsters.CalcifiedCultist(hp: 60) });
        Assert.True(combat.BoundsPlays);
    }

    // ---------------- Enthralled: hand lockout ----------------

    [Fact]
    public void Enthralled_Locks_All_Other_Cards_While_In_Hand()
    {
        var c = Fight();
        var enthralled = new Enthralled();
        var strike = new StrikeIronclad();
        c.Player.Hand.Add(enthralled);
        c.Player.Hand.Add(strike);
        Assert.True(c.HandPlayLocked);
        Assert.True(c.CardPlayAllowed(enthralled));    // only Enthralled itself may be played
        Assert.False(c.CardPlayAllowed(strike));       // everything else is locked out
        // Playing Enthralled removes it from hand, lifting the lock.
        c.Player.Hand.Remove(enthralled);
        Assert.False(c.HandPlayLocked);
        Assert.True(c.CardPlayAllowed(strike));
    }

    [Fact]
    public void Unplayable_Still_Blocked_By_CardPlayAllowed()
    {
        var c = Fight();
        Assert.False(c.CardPlayAllowed(new Wound()));   // Unplayable
        Assert.True(c.CardPlayAllowed(new StrikeIronclad()));
    }

    // ---------------- Whistle: bounded one-turn stun ----------------

    [Fact]
    public void Whistle_Stuns_Target_Skipping_Its_Next_Move()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var byrdonis = Monsters.Byrdonis(hp: 200);
        var combat = Catalog.SetupCombat(player, new[] { byrdonis });
        CombatManager.RollInitialMoves(combat, new Rng(0));   // Swoop opener (17 dmg)
        player.MaxEnergy = 3; player.ResetEnergy();

        var whistle = new Whistle();
        player.Hand.Add(whistle);
        CombatManager.PlayCard(combat, whistle, byrdonis);
        Assert.Equal(1, byrdonis.StunnedTurns);

        combat.CurrentSide = CombatSide.Enemy;
        CombatManager.RunEnemyTurn(combat);
        Assert.Equal(80, player.CurrentHp);        // Swoop skipped — no damage this turn
        Assert.Equal(0, byrdonis.StunnedTurns);    // stun consumed

        // The delayed move now lands on the following enemy turn (CurrentMoveId was preserved).
        combat.CurrentSide = CombatSide.Enemy;
        CombatManager.RunEnemyTurn(combat);
        Assert.True(player.CurrentHp < 80);        // the delayed attack hits
    }

    [Fact]
    public void Control_Byrdonis_Swoops_For_17_Without_Stun()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80);
        var byrdonis = Monsters.Byrdonis(hp: 200);
        var combat = Catalog.SetupCombat(player, new[] { byrdonis });
        CombatManager.RollInitialMoves(combat, new Rng(0));
        combat.CurrentSide = CombatSide.Enemy;
        CombatManager.RunEnemyTurn(combat);
        Assert.Equal(63, player.CurrentHp);   // 80 - 17, confirming the stun test's 0-damage turn is meaningful
    }

    [Fact]
    public void Cmd_Stun_Is_Bounded_And_Ignores_Dead_Monsters()
    {
        var combat = Fight();
        var m = (Monster)combat.Monsters[0];
        Cmd.Stun(combat, m);
        Assert.Equal(1, m.StunnedTurns);
        Cmd.Stun(combat, m);                  // does not stack beyond the requested count
        Assert.Equal(1, m.StunnedTurns);
        m.CurrentHp = 0;
        Cmd.Stun(combat, m);                  // no-op on a dead monster
        Assert.Equal(1, m.StunnedTurns);
    }
}
