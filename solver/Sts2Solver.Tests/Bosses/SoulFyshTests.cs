using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>
/// Unit tests for the Act-1 BOSS SoulFysh. Assert HP scaling (Tough), per-move damage/effects (Deadly), the
/// deterministic AI move chain/intents, the Beckon status-card generation (and its 6 unblockable in-hand bite),
/// the Fade self-Intangible, and the Scream Vulnerable — straight from the decompiled game source. Mirrors the
/// BossTests structure.
/// </summary>
public class SoulFyshTests
{
    private static CombatState Combat(params Monster[] monsters)
    {
        var player = new Player { Name = "P", CurrentHp = 999, MaxHp = 999, MaxEnergy = 3 };
        return Catalog.SetupCombat(player, monsters);
    }

    /// <summary>Telegraph <paramref name="moveId"/> on <paramref name="m"/>, resolve it, return the player HP lost.</summary>
    private static int RunMove(CombatState combat, Monster m, string moveId)
    {
        m.Ai.CurrentMoveId = moveId;
        int before = combat.Player.CurrentHp;
        m.PerformCurrentMove(combat);
        return before - combat.Player.CurrentHp;
    }

    private static MoveState Move(Monster m, string id) => (MoveState)m.Ai.States[id];

    // ---- HP scaling (MinInitialHp == MaxInitialHp: fixed, no roll) ------------------------------

    [Theory]
    [InlineData(0, 211)]   // base
    [InlineData(7, 211)]   // below ToughEnemies (>=8)
    [InlineData(8, 221)]   // ToughEnemies
    [InlineData(10, 221)]
    public void SoulFysh_Hp_Scales_With_Tough(int asc, int expectedHp)
    {
        var m = Monsters.SoulFysh(ascension: asc);
        Assert.Equal(expectedHp, m.MaxHp);
        Assert.Equal(expectedHp, m.CurrentHp);
    }

    [Fact]
    public void SoulFysh_Hp_Override_Respected()
    {
        var m = Monsters.SoulFysh(hp: 123, ascension: 8);
        Assert.Equal(123, m.MaxHp);
        Assert.Equal(123, m.CurrentHp);
    }

    // ---- AI chain & intents --------------------------------------------------------------------

    [Fact]
    public void SoulFysh_Opens_On_Beckon_Then_Loops_DeGas_Gaze_Fade_Scream()
    {
        var m = Monsters.SoulFysh();
        Assert.Equal("BECKON_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);

        Assert.Equal("DE_GAS_MOVE", Move(m, "BECKON_MOVE").FollowUp!.Id);
        Assert.Equal("GAZE_MOVE", Move(m, "DE_GAS_MOVE").FollowUp!.Id);
        Assert.Equal("FADE_MOVE", Move(m, "GAZE_MOVE").FollowUp!.Id);
        Assert.Equal("SCREAM_MOVE", Move(m, "FADE_MOVE").FollowUp!.Id);
        Assert.Equal("BECKON_MOVE", Move(m, "SCREAM_MOVE").FollowUp!.Id);   // loop back
    }

    [Fact]
    public void SoulFysh_Intents_Match_Spec()
    {
        var m = Monsters.SoulFysh();
        Assert.Null(Move(m, "BECKON_MOVE").IntentDamage);   // status generation, no attack
        Assert.Null(Move(m, "FADE_MOVE").IntentDamage);     // pure self-buff
        Assert.Equal(16, Move(m, "DE_GAS_MOVE").IntentDamage);
        Assert.Equal(7, Move(m, "GAZE_MOVE").IntentDamage);
        Assert.Equal(13, Move(m, "SCREAM_MOVE").IntentDamage);
    }

    // ---- Move effects (base ascension) ---------------------------------------------------------

    [Fact]
    public void SoulFysh_DeGas_Deals_16_No_Effects()
    {
        var m = Monsters.SoulFysh();
        var combat = Combat(m);
        Assert.Equal(16, RunMove(combat, m, "DE_GAS_MOVE"));
        Assert.Empty(combat.Player.Powers);
        Assert.Empty(combat.Player.DrawPile);
        Assert.Empty(combat.Player.DiscardPile);
    }

    [Fact]
    public void SoulFysh_Beckon_Adds_One_Card_To_Draw_And_One_To_Discard()
    {
        var m = Monsters.SoulFysh();
        var combat = Combat(m);
        Assert.Equal(0, RunMove(combat, m, "BECKON_MOVE"));   // no direct damage
        Assert.Single(combat.Player.DrawPile);
        Assert.Single(combat.Player.DiscardPile);
        Assert.IsType<Beckon>(combat.Player.DrawPile[0]);
        Assert.IsType<Beckon>(combat.Player.DiscardPile[0]);
        Assert.Empty(combat.Player.Powers);
    }

    [Fact]
    public void SoulFysh_Gaze_Deals_7_And_Adds_One_Beckon_To_Discard()
    {
        var m = Monsters.SoulFysh();
        var combat = Combat(m);
        Assert.Equal(7, RunMove(combat, m, "GAZE_MOVE"));
        Assert.Empty(combat.Player.DrawPile);
        Assert.Single(combat.Player.DiscardPile);
        Assert.IsType<Beckon>(combat.Player.DiscardPile[0]);
    }

    [Fact]
    public void SoulFysh_Scream_Deals_13_And_Applies_3_Vulnerable()
    {
        var m = Monsters.SoulFysh();
        var combat = Combat(m);
        Assert.Equal(13, RunMove(combat, m, "SCREAM_MOVE"));
        Assert.Equal(3, combat.Player.GetPowerAmount("Vulnerable"));
    }

    [Fact]
    public void SoulFysh_Fade_Grants_Itself_2_Intangible_No_Player_Effect()
    {
        var m = Monsters.SoulFysh();
        var combat = Combat(m);
        Assert.Equal(0, RunMove(combat, m, "FADE_MOVE"));     // pure self-buff: no player-facing effect
        Assert.Equal(2, m.GetPowerAmount("Intangible"));
        Assert.Empty(combat.Player.Powers);
    }

    // ---- Fade's Intangible actually caps incoming damage to 1 ----------------------------------

    [Fact]
    public void SoulFysh_While_Intangible_Caps_Each_Incoming_Hit_To_1()
    {
        var m = Monsters.SoulFysh();
        var combat = Combat(m);
        RunMove(combat, m, "FADE_MOVE");
        int before = m.CurrentHp;
        Cmd.Attack(combat, combat.Player, m, 50, ValueProp.Move, null);   // big hit, capped to 1 by Intangible
        Assert.Equal(1, before - m.CurrentHp);
    }

    // ---- Beckon's in-hand bite: 6 unblockable at end of player's turn ---------------------------

    [Fact]
    public void SoulFysh_Beckon_In_Hand_Deals_6_Unblockable_At_Player_Turn_End()
    {
        var m = Monsters.SoulFysh();
        var combat = Combat(m);
        combat.Player.MaxHp = 100;
        combat.Player.CurrentHp = 100;
        combat.Player.Hand.Add(new Beckon());
        combat.Player.Hand.Add(new Beckon());           // two copies => 12
        Cmd.GainBlock(combat, combat.Player, 999, ValueProp.Move, null);   // block does NOT stop it (unblockable)

        int before = combat.Player.CurrentHp;
        CombatManager.EndPlayerTurn(combat);
        Assert.Equal(12, before - combat.Player.CurrentHp);   // 6 per copy, ignoring block
    }

    // ---- Deadly (>=9) ascension scaling --------------------------------------------------------

    [Fact]
    public void SoulFysh_Deadly_Damage_Scales()
    {
        var m = Monsters.SoulFysh(ascension: 9);
        var combat = Combat(m);
        Assert.Equal(17, Move(m, "DE_GAS_MOVE").IntentDamage);
        Assert.Equal(8, Move(m, "GAZE_MOVE").IntentDamage);
        Assert.Equal(15, Move(m, "SCREAM_MOVE").IntentDamage);
        Assert.Equal(17, RunMove(combat, m, "DE_GAS_MOVE"));
        Assert.Equal(8, RunMove(combat, m, "GAZE_MOVE"));
        Assert.Equal(15, RunMove(combat, m, "SCREAM_MOVE"));
        Assert.Equal(3, combat.Player.GetPowerAmount("Vulnerable"));   // Scream's Vulnerable is not Deadly-scaled
    }

    // ---- Full-cycle integration: each move resolves in order, state persists --------------------

    [Fact]
    public void SoulFysh_Full_Cycle_Resolves_In_Order()
    {
        var m = Monsters.SoulFysh();
        var combat = Combat(m);
        // Beckon -> 1 draw + 1 discard
        RunMove(combat, m, "BECKON_MOVE");
        // De Gas -> 16
        Assert.Equal(16, RunMove(combat, m, "DE_GAS_MOVE"));
        // Gaze -> 7 + 1 more discard
        Assert.Equal(7, RunMove(combat, m, "GAZE_MOVE"));
        // Fade -> self Intangible 2
        RunMove(combat, m, "FADE_MOVE");
        // Scream -> 13 + 3 Vulnerable
        Assert.Equal(13, RunMove(combat, m, "SCREAM_MOVE"));

        Assert.Single(combat.Player.DrawPile);          // 1 Beckon from Beckon move
        Assert.Equal(2, combat.Player.DiscardPile.Count); // 1 from Beckon move + 1 from Gaze
        Assert.Equal(2, m.GetPowerAmount("Intangible"));
        Assert.Equal(3, combat.Player.GetPowerAmount("Vulnerable"));
    }
}
