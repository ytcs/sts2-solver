using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>
/// Unit tests for the Act-2 Overgrowth BOSS CeremonialBeast port (decompile-faithful). Covers HP/Tough scaling,
/// each move's damage/effects/intents through the engine (incl. Deadly), the phase-1 PLOW self-loop with its
/// Strength ramp, the phase-2 SHRILL loop with its Strength ramp, and the SOUNDNESS-critical on-hit PHASE BREAK
/// (PlowPower threshold: sheds Strength, self-stuns into phase 2, fires once, ignores fully-blocked hits).
/// Tests <see cref="Monsters.CeremonialBeast"/> directly (no catalog needed).
/// </summary>
public class CeremonialBeastTests
{
    private static CombatState Combat(params Monster[] monsters)
    {
        var player = new Player { Name = "P", CurrentHp = 999, MaxHp = 999, MaxEnergy = 3 };
        return Catalog.SetupCombat(player, monsters);
    }

    /// <summary>Telegraph <paramref name="moveId"/>, resolve it, and return player HP lost.</summary>
    private static int RunMove(CombatState combat, Monster m, string moveId)
    {
        m.Ai.CurrentMoveId = moveId;
        int before = combat.Player.CurrentHp;
        m.PerformCurrentMove(combat);
        return before - combat.Player.CurrentHp;
    }

    private static MoveState Move(Monster m, string id) => (MoveState)m.Ai.States[id];

    // ---- HP scaling (MinInitialHp == MaxInitialHp; ToughEnemies at asc >= 8) ----
    [Theory]
    [InlineData(0, 252)]
    [InlineData(7, 252)]
    [InlineData(8, 262)]
    [InlineData(9, 262)]
    public void Hp_Scales_With_Tough(int asc, int expectedHp)
        => Assert.Equal(expectedHp, Monsters.CeremonialBeast(ascension: asc).MaxHp);

    [Fact]
    public void Hp_Override_Is_Respected()
        => Assert.Equal(150, Monsters.CeremonialBeast(hp: 150).MaxHp);

    // ---- Opens on STAMP, no powers ----
    [Fact]
    public void Opens_On_Stamp_With_No_Powers()
    {
        var m = Monsters.CeremonialBeast();
        Assert.Empty(m.Powers);
        Assert.Equal("STAMP_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);
    }

    // ---- Phase-1 STAMP: applies the Plow threshold marker to itself, no damage ----
    [Fact]
    public void Stamp_Applies_Plow_Threshold_To_Self_No_Damage()
    {
        var m = Monsters.CeremonialBeast();
        var combat = Combat(m);
        Assert.Equal(0, RunMove(combat, m, "STAMP_MOVE"));
        Assert.Equal(150, m.GetPowerAmount(CeremonialBeastPlowPower.PowerId));
        Assert.Null(Move(m, "STAMP_MOVE").IntentDamage);
    }

    [Fact]
    public void Stamp_Deadly_Threshold_Is_160()
    {
        var m = Monsters.CeremonialBeast(ascension: 9);
        var combat = Combat(m);
        RunMove(combat, m, "STAMP_MOVE");
        Assert.Equal(160, m.GetPowerAmount(CeremonialBeastPlowPower.PowerId));
    }

    // ---- Phase-1 PLOW: damage + self +2 Strength; intent is the base telegraph ----
    [Fact]
    public void Plow_Hits_For_18_And_Gains_2_Strength()
    {
        var m = Monsters.CeremonialBeast();
        var combat = Combat(m);
        Assert.Equal(18, RunMove(combat, m, "PLOW_MOVE"));
        Assert.Equal(2, m.GetPowerAmount("Strength"));
        Assert.Equal(18, Move(m, "PLOW_MOVE").IntentDamage);
        Assert.Equal(1, Move(m, "PLOW_MOVE").IntentHits);
    }

    [Fact]
    public void Plow_Deadly_Hits_For_20()
    {
        var m = Monsters.CeremonialBeast(ascension: 9);
        var combat = Combat(m);
        Assert.Equal(20, RunMove(combat, m, "PLOW_MOVE"));
    }

    [Fact]
    public void Plow_Ramps_With_Accumulated_Strength()
    {
        var m = Monsters.CeremonialBeast();
        var combat = Combat(m);
        Assert.Equal(18, RunMove(combat, m, "PLOW_MOVE"));   // 18 + 0 Str
        Assert.Equal(20, RunMove(combat, m, "PLOW_MOVE"));   // 18 + 2 Str
        Assert.Equal(22, RunMove(combat, m, "PLOW_MOVE"));   // 18 + 4 Str
        Assert.Equal(6, m.GetPowerAmount("Strength"));
    }

    // ---- Phase-1 loop: STAMP -> PLOW -> PLOW (self-loop) ----
    [Fact]
    public void Phase1_Loop_Is_Stamp_Then_Plow_SelfLoop()
    {
        var m = Monsters.CeremonialBeast();
        Assert.Equal("PLOW_MOVE", Move(m, "STAMP_MOVE").FollowUp!.Id);
        Assert.Equal("PLOW_MOVE", Move(m, "PLOW_MOVE").FollowUp!.Id);
    }

    // ---- Phase-2 SHRILL loop wiring: STUN -> BEAST_CRY -> STOMP -> CRUSH -> BEAST_CRY ----
    [Fact]
    public void Phase2_Loop_Is_BeastCry_Stomp_Crush()
    {
        var m = Monsters.CeremonialBeast();
        Assert.Equal("BEAST_CRY_MOVE", Move(m, "STUN_MOVE").FollowUp!.Id);
        Assert.Equal("STOMP_MOVE", Move(m, "BEAST_CRY_MOVE").FollowUp!.Id);
        Assert.Equal("CRUSH_MOVE", Move(m, "STOMP_MOVE").FollowUp!.Id);
        Assert.Equal("BEAST_CRY_MOVE", Move(m, "CRUSH_MOVE").FollowUp!.Id);
    }

    [Fact]
    public void Stun_Move_Deals_No_Damage()
    {
        var m = Monsters.CeremonialBeast();
        var combat = Combat(m);
        Assert.Equal(0, RunMove(combat, m, "STUN_MOVE"));
    }

    // ---- BEAST_CRY: no damage, applies the Ringing marker to the player ----
    [Fact]
    public void BeastCry_Applies_Ringing_To_Player_No_Damage()
    {
        var m = Monsters.CeremonialBeast();
        var combat = Combat(m);
        Assert.Equal(0, RunMove(combat, m, "BEAST_CRY_MOVE"));
        Assert.True(combat.Player.HasPower(CeremonialBeastRingingPower.PowerId));
        Assert.Null(Move(m, "BEAST_CRY_MOVE").IntentDamage);
    }

    [Fact]
    public void Ringing_Clears_At_Player_Turn_End()
    {
        var m = Monsters.CeremonialBeast();
        var combat = Combat(m);
        RunMove(combat, m, "BEAST_CRY_MOVE");
        Assert.True(combat.Player.HasPower(CeremonialBeastRingingPower.PowerId));
        // Game: RingingPower removes itself at the owner's (player's) side-turn end.
        CombatManager.BeginPlayerTurn(combat);
        CombatManager.EndPlayerTurn(combat);
        Assert.False(combat.Player.HasPower(CeremonialBeastRingingPower.PowerId));
    }

    // ---- Phase-2 STOMP / CRUSH damage + CRUSH Strength ramp ----
    [Fact]
    public void Stomp_Hits_For_15()
    {
        var m = Monsters.CeremonialBeast();
        var combat = Combat(m);
        Assert.Equal(15, RunMove(combat, m, "STOMP_MOVE"));
        Assert.Equal(15, Move(m, "STOMP_MOVE").IntentDamage);
    }

    [Fact]
    public void Stomp_Deadly_Hits_For_17()
        => Assert.Equal(17, RunMove(Combat(Monsters.CeremonialBeast(ascension: 9)),
                                    Monsters.CeremonialBeast(ascension: 9), "STOMP_MOVE"));

    [Fact]
    public void Crush_Hits_For_17_And_Gains_3_Strength()
    {
        var m = Monsters.CeremonialBeast();
        var combat = Combat(m);
        Assert.Equal(17, RunMove(combat, m, "CRUSH_MOVE"));
        Assert.Equal(3, m.GetPowerAmount("Strength"));
        Assert.Equal(17, Move(m, "CRUSH_MOVE").IntentDamage);
    }

    [Fact]
    public void Crush_Deadly_Hits_For_19_And_Gains_4_Strength()
    {
        var m = Monsters.CeremonialBeast(ascension: 9);
        var combat = Combat(m);
        Assert.Equal(19, RunMove(combat, m, "CRUSH_MOVE"));
        Assert.Equal(4, m.GetPowerAmount("Strength"));
    }

    [Fact]
    public void Phase2_Attacks_Ramp_With_Crush_Strength()
    {
        var m = Monsters.CeremonialBeast();
        var combat = Combat(m);
        Assert.Equal(17, RunMove(combat, m, "CRUSH_MOVE"));   // 17 + 0 Str, then +3
        Assert.Equal(18, RunMove(combat, m, "STOMP_MOVE"));   // 15 + 3 Str
        Assert.Equal(20, RunMove(combat, m, "CRUSH_MOVE"));   // 17 + 3 Str, then +3
        Assert.Equal(6, m.GetPowerAmount("Strength"));
    }

    // ---- PHASE BREAK (PlowPower): unblocked hit past the threshold -> shed Strength, self-stun, enter phase 2 ----
    [Fact]
    public void Unblocked_Hit_Past_Threshold_Breaks_Phase_And_Sheds_Strength()
    {
        var m = Monsters.CeremonialBeast();   // 252 HP, threshold 150
        var combat = Combat(m);
        RunMove(combat, m, "STAMP_MOVE");     // arm Plow(150)
        m.AddPower(new StrengthPower(), 8);   // simulate the phase-1 charge ramp
        m.Ai.CurrentMoveId = "PLOW_MOVE";     // telegraphing a charge

        // Chunk it from 252 down to 150 (exactly the threshold) with an unblocked player hit.
        Cmd.Attack(combat, combat.Player, m, 102, ValueProp.Move, null);
        Assert.Equal(150, m.CurrentHp);

        Assert.False(m.HasPower(CeremonialBeastPlowPower.PowerId));   // one-shot: marker removed
        Assert.Equal(0, m.GetPowerAmount("Strength"));               // ramp shed
        Assert.Equal("STUN_MOVE", m.Ai.CurrentMoveId);              // telegraph forced to the skip turn
    }

    [Fact]
    public void Phase_Break_Sheds_Temporary_Strength_Too()
    {
        var m = Monsters.CeremonialBeast();
        var combat = Combat(m);
        RunMove(combat, m, "STAMP_MOVE");
        // CoordinatePower is a TemporaryStrengthPower (Sign +1): applying it pushes real Strength onto the owner,
        // exactly like the game's TemporaryStrengthPower the beast's phase break strips before the plain Strength.
        Cmd.ApplyPower(combat, m, new CoordinatePower(), 6, m);
        m.AddPower(new StrengthPower(), 4);
        Assert.Equal(10, m.GetPowerAmount("Strength"));   // 6 (temp-pushed) + 4
        Cmd.Attack(combat, combat.Player, m, 200, ValueProp.Move, null);  // far past threshold
        Assert.False(m.HasPower("Coordinate"));
        Assert.Equal(0, m.GetPowerAmount("Strength"));
    }

    [Fact]
    public void Fully_Blocked_Hit_Does_Not_Break_Phase()
    {
        var m = Monsters.CeremonialBeast();
        var combat = Combat(m);
        RunMove(combat, m, "STAMP_MOVE");
        m.GainBlockDirect(120);
        m.Ai.CurrentMoveId = "PLOW_MOVE";
        // 120 damage fully absorbed by 120 block ⇒ no unblocked damage ⇒ no break, even though HP would be <=150.
        Cmd.Attack(combat, combat.Player, m, 120, ValueProp.Move, null);
        Assert.True(m.HasPower(CeremonialBeastPlowPower.PowerId));
        Assert.Equal("PLOW_MOVE", m.Ai.CurrentMoveId);
    }

    [Fact]
    public void Unblocked_Hit_Above_Threshold_Does_Not_Break_Phase()
    {
        var m = Monsters.CeremonialBeast();   // 252 HP, threshold 150
        var combat = Combat(m);
        RunMove(combat, m, "STAMP_MOVE");
        m.Ai.CurrentMoveId = "PLOW_MOVE";
        Cmd.Attack(combat, combat.Player, m, 50, ValueProp.Move, null);  // 252 -> 202 (> 150)
        Assert.True(m.HasPower(CeremonialBeastPlowPower.PowerId));
        Assert.Equal("PLOW_MOVE", m.Ai.CurrentMoveId);
    }

    [Fact]
    public void Phase_Break_Fires_Only_Once()
    {
        var m = Monsters.CeremonialBeast();
        var combat = Combat(m);
        RunMove(combat, m, "STAMP_MOVE");
        m.Ai.CurrentMoveId = "PLOW_MOVE";
        Cmd.Attack(combat, combat.Player, m, 110, ValueProp.Move, null);   // 252 -> 142, break fires
        Assert.Equal("STUN_MOVE", m.Ai.CurrentMoveId);
        m.Ai.CurrentMoveId = "BEAST_CRY_MOVE";                            // pretend we've advanced into phase 2
        Cmd.Attack(combat, combat.Player, m, 20, ValueProp.Move, null);    // another hit
        Assert.Equal("BEAST_CRY_MOVE", m.Ai.CurrentMoveId);               // not re-routed (marker gone)
    }

    // ---- After the break: STUN is a no-op skip, then the AI flows into BEAST_CRY (phase 2) ----
    [Fact]
    public void After_Break_Stun_Skips_Then_Enters_Phase2()
    {
        var m = Monsters.CeremonialBeast();
        var combat = Combat(m);
        RunMove(combat, m, "STAMP_MOVE");
        m.Ai.CurrentMoveId = "PLOW_MOVE";
        Cmd.Attack(combat, combat.Player, m, 110, ValueProp.Move, null);   // break -> STUN telegraphed
        Assert.Equal("STUN_MOVE", m.Ai.CurrentMoveId);

        int startHp = combat.Player.CurrentHp;
        m.PerformCurrentMove(combat);                                      // STUN: no damage
        Assert.Equal(startHp, combat.Player.CurrentHp);
        // The move AFTER the stun is BEAST_CRY (phase 2 begins).
        Assert.Equal("BEAST_CRY_MOVE", m.Ai.EnumerateNext(m).Single().moveId);
    }
}
