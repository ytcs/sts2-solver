using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>
/// Unit tests for the Act-1 BOSS LagavulinMatriarch port (decompile-faithful). Covers HP/Tough scaling, each
/// move's damage/effects/intents through the engine (Deadly), the awake 4-move loop, and the sleep/wake special
/// mechanic — both the 3-turn TIMEOUT wake and the on-UNBLOCKED-hit early wake (with the self-stun + Plating
/// strip). Tests <see cref="Monsters.LagavulinMatriarch"/> directly (no catalog needed).
/// </summary>
public class LagavulinMatriarchTests
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
    [InlineData(0, 222)]
    [InlineData(7, 222)]
    [InlineData(8, 233)]
    [InlineData(9, 233)]
    public void Hp_Scales_With_Tough(int asc, int expectedHp)
        => Assert.Equal(expectedHp, Monsters.LagavulinMatriarch(ascension: asc).MaxHp);

    [Fact]
    public void Hp_Override_Is_Respected()
        => Assert.Equal(150, Monsters.LagavulinMatriarch(hp: 150).MaxHp);

    // ---- Opens asleep ----
    [Fact]
    public void Opens_Asleep_With_Plating_And_Asleep()
    {
        var m = Monsters.LagavulinMatriarch();
        Assert.True(m.HasPower("MatriarchAsleep"));
        Assert.Equal(3, m.GetPowerAmount("MatriarchAsleep"));
        Assert.True(m.HasPower("MatriarchPlating"));
        Assert.Equal(12, m.GetPowerAmount("MatriarchPlating"));
        // Initial telegraphed move is the no-op SLEEP_MOVE.
        Assert.Equal("SLEEP_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);
    }

    [Fact]
    public void Sleep_Move_Deals_No_Damage()
    {
        var m = Monsters.LagavulinMatriarch();
        var combat = Combat(m);
        Assert.Equal(0, RunMove(combat, m, "SLEEP_MOVE"));
    }

    // ---- Awake-loop moves (damage / effects / intents) ----
    [Fact]
    public void Slash_Hits_For_19_And_Intent()
    {
        var m = Monsters.LagavulinMatriarch();
        var combat = Combat(m);
        Assert.Equal(19, RunMove(combat, m, "SLASH_MOVE"));
        Assert.Equal(19, Move(m, "SLASH_MOVE").IntentDamage);
        Assert.Equal(1, Move(m, "SLASH_MOVE").IntentHits);
    }

    [Fact]
    public void Slash_Deadly_Hits_For_21()
    {
        var m = Monsters.LagavulinMatriarch(ascension: 9);
        var combat = Combat(m);
        Assert.Equal(21, RunMove(combat, m, "SLASH_MOVE"));
    }

    [Fact]
    public void Disembowel_Hits_Twice_For_9_Each()
    {
        var m = Monsters.LagavulinMatriarch();
        var combat = Combat(m);
        Assert.Equal(18, RunMove(combat, m, "DISEMBOWEL_MOVE"));   // 9 x 2
        Assert.Equal(9, Move(m, "DISEMBOWEL_MOVE").IntentDamage);
        Assert.Equal(2, Move(m, "DISEMBOWEL_MOVE").IntentHits);
    }

    [Fact]
    public void Disembowel_Deadly_Is_10x2()
    {
        var m = Monsters.LagavulinMatriarch(ascension: 9);
        var combat = Combat(m);
        Assert.Equal(20, RunMove(combat, m, "DISEMBOWEL_MOVE"));   // 10 x 2
    }

    [Fact]
    public void Slash2_Hits_For_12_And_Gains_12_Block()
    {
        var m = Monsters.LagavulinMatriarch();
        var combat = Combat(m);
        Assert.Equal(12, RunMove(combat, m, "SLASH2_MOVE"));
        Assert.Equal(12, m.Block);                                 // Slash2Block 12 (base)
        Assert.Equal(12, Move(m, "SLASH2_MOVE").IntentDamage);
    }

    [Fact]
    public void Slash2_Deadly_Damage_14_And_Tough_Block_14()
    {
        var m = Monsters.LagavulinMatriarch(ascension: 9);          // Deadly (>=9) also implies Tough (>=8)
        var combat = Combat(m);
        Assert.Equal(14, RunMove(combat, m, "SLASH2_MOVE"));
        Assert.Equal(14, m.Block);
    }

    [Fact]
    public void SoulSiphon_Debuffs_Player_Str_And_Dex_And_Buffs_Self()
    {
        var m = Monsters.LagavulinMatriarch();
        var combat = Combat(m);
        Assert.Equal(0, RunMove(combat, m, "SOUL_SIPHON_MOVE"));    // no damage
        Assert.Equal(-2, combat.Player.GetPowerAmount("Strength"));
        Assert.Equal(-2, combat.Player.GetPowerAmount("Dexterity"));
        Assert.Equal(2, m.GetPowerAmount("Strength"));
        Assert.Null(Move(m, "SOUL_SIPHON_MOVE").IntentDamage);
    }

    // ---- Awake 4-move deterministic loop ----
    [Fact]
    public void Awake_Loop_Is_Slash_Disembowel_Slash2_SoulSiphon()
    {
        var m = Monsters.LagavulinMatriarch();
        Assert.Equal("DISEMBOWEL_MOVE", Move(m, "SLASH_MOVE").FollowUp!.Id);
        Assert.Equal("SLASH2_MOVE", Move(m, "DISEMBOWEL_MOVE").FollowUp!.Id);
        Assert.Equal("SOUL_SIPHON_MOVE", Move(m, "SLASH2_MOVE").FollowUp!.Id);
        Assert.Equal("SLASH_MOVE", Move(m, "SOUL_SIPHON_MOVE").FollowUp!.Id);
    }

    // ---- Sleep branch: stays asleep while Asleep present, else Slash ----
    [Fact]
    public void Sleep_Branch_Loops_While_Asleep_Then_Goes_To_Slash()
    {
        var m = Monsters.LagavulinMatriarch();
        m.Ai.CurrentMoveId = "SLEEP_MOVE";
        // While Asleep present, the FollowUp branch resolves back to SLEEP_MOVE.
        Assert.Equal("SLEEP_MOVE", m.Ai.EnumerateNext(m).Single().moveId);
        // Once Asleep is gone, the branch resolves to SLASH.
        m.RemovePower("MatriarchAsleep");
        Assert.Equal("SLASH_MOVE", m.Ai.EnumerateNext(m).Single().moveId);
    }

    // ---- TIMEOUT wake: 3 enemy turns of sleep, then attacks on the 4th ----
    [Fact]
    public void Timeout_Wakes_After_Three_Sleep_Turns()
    {
        var m = Monsters.LagavulinMatriarch();
        var combat = Combat(m);
        m.Ai.CurrentMoveId = m.Ai.EnumerateInitial(m).Single().moveId;   // SLEEP_MOVE
        int startHp = combat.Player.CurrentHp;

        for (int turn = 1; turn <= 3; turn++)
        {
            CombatManager.BeginPlayerTurn(combat);
            CombatManager.EndPlayerTurn(combat);
            CombatManager.RunEnemyTurn(combat);                           // sleep no-op
            Assert.Equal(startHp, combat.Player.CurrentHp);              // no damage while asleep
            // Advance the AI deterministically (single outcome).
            m.Ai.CurrentMoveId = m.Ai.EnumerateNext(m).Single().moveId;
        }

        // After 3 enemy-turn-ends Asleep has counted 3->0 and is gone; Plating gone too.
        Assert.False(m.HasPower("MatriarchAsleep"));
        Assert.False(m.HasPower("MatriarchPlating"));
        // The 4th telegraphed move is SLASH.
        Assert.Equal("SLASH_MOVE", m.Ai.CurrentMoveId);

        CombatManager.BeginPlayerTurn(combat);
        CombatManager.EndPlayerTurn(combat);
        CombatManager.RunEnemyTurn(combat);                              // SLASH for 19
        Assert.Equal(19, startHp - combat.Player.CurrentHp);
    }

    // ---- Plating grants the asleep boss block each player turn (so the player must break it to wake it) ----
    [Fact]
    public void Plating_Grants_Block_At_Player_Turn_Start()
    {
        var m = Monsters.LagavulinMatriarch();
        var combat = Combat(m);
        m.Ai.CurrentMoveId = "SLEEP_MOVE";
        Assert.Equal(0, m.Block);
        CombatManager.BeginPlayerTurn(combat);                          // Plating grants 12 block
        Assert.Equal(12, m.Block);
    }

    [Fact]
    public void Fully_Blocked_Hit_Does_Not_Wake_The_Boss()
    {
        var m = Monsters.LagavulinMatriarch();
        var combat = Combat(m);
        m.Ai.CurrentMoveId = "SLEEP_MOVE";
        CombatManager.BeginPlayerTurn(combat);                          // 12 block from Plating
        Cmd.Attack(combat, combat.Player, m, 12, ValueProp.Move, null); // fully blocked (<= block)
        Assert.True(m.HasPower("MatriarchAsleep"));                      // still asleep
        Assert.True(m.HasPower("MatriarchPlating"));
        Assert.Equal(0, m.StunnedTurns);
    }

    // ---- ON-HIT wake: an unblocked hit wakes the boss early (strips Plating, self-stuns, next move SLASH) ----
    [Fact]
    public void Unblocked_Hit_Wakes_Boss_Early_And_Self_Stuns()
    {
        var m = Monsters.LagavulinMatriarch();
        var combat = Combat(m);
        m.Ai.CurrentMoveId = "SLEEP_MOVE";
        CombatManager.BeginPlayerTurn(combat);                          // 12 block from Plating
        int hpBefore = m.CurrentHp;
        Cmd.Attack(combat, combat.Player, m, 30, ValueProp.Move, null); // 12 blocked, 18 unblocked

        Assert.Equal(hpBefore - 18, m.CurrentHp);                       // took the unblocked remainder
        Assert.False(m.HasPower("MatriarchAsleep"));                    // woke
        Assert.False(m.HasPower("MatriarchPlating"));                   // armour stripped
        Assert.Equal(1, m.StunnedTurns);                               // self-stun the upcoming move

        // The skipped enemy turn deals no damage, then the AI advances to SLASH.
        int startHp = combat.Player.CurrentHp;
        CombatManager.EndPlayerTurn(combat);
        CombatManager.RunEnemyTurn(combat);                            // stunned: skipped
        Assert.Equal(startHp, combat.Player.CurrentHp);
        m.Ai.CurrentMoveId = m.Ai.EnumerateNext(m).Single().moveId;    // SLEEP_MOVE.FollowUp -> branch -> SLASH
        Assert.Equal("SLASH_MOVE", m.Ai.CurrentMoveId);

        CombatManager.BeginPlayerTurn(combat);
        CombatManager.EndPlayerTurn(combat);
        CombatManager.RunEnemyTurn(combat);                            // SLASH for 19
        Assert.Equal(19, startHp - combat.Player.CurrentHp);
    }

    [Fact]
    public void Early_Wake_Attacks_Sooner_Than_Timeout()
    {
        // Wake on player turn 1 ⇒ SLASH lands on enemy turn 2 (turn 1 = the self-stun), vs the timeout's enemy
        // turn 4. The on-hit path is the faster-attack path (modelled so the boss is never under-credited).
        var m = Monsters.LagavulinMatriarch();
        var combat = Combat(m);
        m.Ai.CurrentMoveId = "SLEEP_MOVE";
        CombatManager.BeginPlayerTurn(combat);
        Cmd.Attack(combat, combat.Player, m, 99, ValueProp.Move, null); // wake
        CombatManager.EndPlayerTurn(combat);
        CombatManager.RunEnemyTurn(combat);                            // enemy turn 1: stunned (skip)
        m.Ai.CurrentMoveId = m.Ai.EnumerateNext(m).Single().moveId;    // -> SLASH

        int startHp = combat.Player.CurrentHp;
        CombatManager.BeginPlayerTurn(combat);
        CombatManager.EndPlayerTurn(combat);
        CombatManager.RunEnemyTurn(combat);                            // enemy turn 2: SLASH
        Assert.Equal(19, startHp - combat.Player.CurrentHp);
    }
}
