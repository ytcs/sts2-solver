using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>
/// Unit tests for the Act-3 Hive BOSS KnowledgeDemon port (decompile-faithful). Covers HP/Tough scaling, each
/// move's damage/effects/intents through the engine (Deadly), the deterministic move chain + the curse branch
/// (3 curses then an endless Slap → KnowledgeOverwhelming → Ponder loop), Ponder's self-heal + self-Strength
/// ramp, and the Curse-of-Knowledge special mechanic (worst-case Disintegration ⇒ recurring end-of-player-turn
/// unblockable HP loss, stacking 6/7/8). Tests <see cref="Monsters.KnowledgeDemon"/> directly (no catalog).
/// </summary>
public class KnowledgeDemonTests
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
    [InlineData(0, 379)]
    [InlineData(7, 379)]
    [InlineData(8, 399)]
    [InlineData(9, 399)]
    public void Hp_Scales_With_Tough(int asc, int expectedHp)
        => Assert.Equal(expectedHp, Monsters.KnowledgeDemon(ascension: asc).MaxHp);

    [Fact]
    public void Hp_Override_Is_Respected()
        => Assert.Equal(150, Monsters.KnowledgeDemon(hp: 150).MaxHp);

    // ---- Opens on Curse of Knowledge ----
    [Fact]
    public void Opens_On_Curse_Of_Knowledge()
    {
        var m = Monsters.KnowledgeDemon();
        Assert.Equal("CURSE_OF_KNOWLEDGE_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);
    }

    // ---- Move chain (deterministic FollowUp links) ----
    [Fact]
    public void Move_Chain_Is_Curse_Slap_KnowOver_Ponder()
    {
        var m = Monsters.KnowledgeDemon();
        Assert.Equal("SLAP_MOVE", Move(m, "CURSE_OF_KNOWLEDGE_MOVE").FollowUp!.Id);
        Assert.Equal("KNOWLEDGE_OVERWHELMING_MOVE", Move(m, "SLAP_MOVE").FollowUp!.Id);
        Assert.Equal("PONDER_MOVE", Move(m, "KNOWLEDGE_OVERWHELMING_MOVE").FollowUp!.Id);
        // Ponder's FollowUp is the curse branch (a RandomBranchState, not a MoveState).
        Assert.Equal("CURSE_OF_KNOWLEDGE_BRANCH", Move(m, "PONDER_MOVE").FollowUp!.Id);
    }

    // ---- SLAP ----
    [Fact]
    public void Slap_Hits_For_17_And_Intent()
    {
        var m = Monsters.KnowledgeDemon();
        var combat = Combat(m);
        Assert.Equal(17, RunMove(combat, m, "SLAP_MOVE"));
        Assert.Equal(17, Move(m, "SLAP_MOVE").IntentDamage);
        Assert.Equal(1, Move(m, "SLAP_MOVE").IntentHits);
    }

    [Fact]
    public void Slap_Deadly_Hits_For_18()
    {
        var m = Monsters.KnowledgeDemon(ascension: 9);
        var combat = Combat(m);
        Assert.Equal(18, RunMove(combat, m, "SLAP_MOVE"));
    }

    // ---- KNOWLEDGE_OVERWHELMING (3 hits) ----
    [Fact]
    public void KnowledgeOverwhelming_Hits_Three_Times_For_8_Each()
    {
        var m = Monsters.KnowledgeDemon();
        var combat = Combat(m);
        Assert.Equal(24, RunMove(combat, m, "KNOWLEDGE_OVERWHELMING_MOVE"));   // 8 x 3
        Assert.Equal(8, Move(m, "KNOWLEDGE_OVERWHELMING_MOVE").IntentDamage);
        Assert.Equal(3, Move(m, "KNOWLEDGE_OVERWHELMING_MOVE").IntentHits);
    }

    [Fact]
    public void KnowledgeOverwhelming_Deadly_Is_9x3()
    {
        var m = Monsters.KnowledgeDemon(ascension: 9);
        var combat = Combat(m);
        Assert.Equal(27, RunMove(combat, m, "KNOWLEDGE_OVERWHELMING_MOVE"));   // 9 x 3
    }

    // ---- PONDER (attack + self-heal + self-Strength) ----
    [Fact]
    public void Ponder_Hits_For_11_Heals_30_And_Gains_2_Strength()
    {
        var m = Monsters.KnowledgeDemon(hp: 200);
        m.CurrentHp = 100;
        var combat = Combat(m);
        Assert.Equal(11, RunMove(combat, m, "PONDER_MOVE"));
        Assert.Equal(130, m.CurrentHp);                                       // healed 30
        Assert.Equal(2, m.GetPowerAmount("Strength"));
        Assert.Equal(11, Move(m, "PONDER_MOVE").IntentDamage);
    }

    [Fact]
    public void Ponder_Deadly_Hits_For_13_And_Gains_3_Strength()
    {
        var m = Monsters.KnowledgeDemon(ascension: 9);
        var combat = Combat(m);
        Assert.Equal(13, RunMove(combat, m, "PONDER_MOVE"));
        Assert.Equal(3, m.GetPowerAmount("Strength"));
    }

    [Fact]
    public void Ponder_Heal_Is_Capped_At_Max_Hp()
    {
        var m = Monsters.KnowledgeDemon();
        var combat = Combat(m);
        // Full HP boss: the 30 heal cannot exceed MaxHp.
        int hpBefore = m.CurrentHp;
        RunMove(combat, m, "PONDER_MOVE");
        Assert.Equal(hpBefore, m.CurrentHp);
    }

    [Fact]
    public void Ponder_Strength_Ramps_Subsequent_Slap()
    {
        var m = Monsters.KnowledgeDemon();
        var combat = Combat(m);
        RunMove(combat, m, "PONDER_MOVE");                 // +2 Strength
        Assert.Equal(17 + 2, RunMove(combat, m, "SLAP_MOVE"));   // Slap 17 + 2 Strength
    }

    // ---- CURSE OF KNOWLEDGE: no direct boss damage; applies recurring player HP loss; bumps the counter ----
    [Fact]
    public void Curse_Deals_No_Direct_Damage_But_Applies_Disintegration()
    {
        var m = Monsters.KnowledgeDemon();
        var combat = Combat(m);
        Assert.Equal(0, RunMove(combat, m, "CURSE_OF_KNOWLEDGE_MOVE"));   // no boss attack this turn
        Assert.Equal(6, combat.Player.GetPowerAmount("KdDisintegration")); // worst-case set-0 = 6
        Assert.Equal(1, m.GetPowerAmount("KdCurseCount"));                 // counter incremented
        Assert.Null(Move(m, "CURSE_OF_KNOWLEDGE_MOVE").IntentDamage);
    }

    [Fact]
    public void Three_Curses_Stack_Disintegration_6_7_8()
    {
        var m = Monsters.KnowledgeDemon();
        var combat = Combat(m);
        RunMove(combat, m, "CURSE_OF_KNOWLEDGE_MOVE");   // +6
        RunMove(combat, m, "CURSE_OF_KNOWLEDGE_MOVE");   // +7
        RunMove(combat, m, "CURSE_OF_KNOWLEDGE_MOVE");   // +8
        Assert.Equal(21, combat.Player.GetPowerAmount("KdDisintegration"));   // 6 + 7 + 8
        Assert.Equal(3, m.GetPowerAmount("KdCurseCount"));
    }

    // ---- Disintegration ticks unblockable HP loss at the END of each player turn ----
    [Fact]
    public void Disintegration_Ticks_At_Player_Turn_End_And_Is_Unblockable()
    {
        var m = Monsters.KnowledgeDemon();
        var combat = Combat(m);
        RunMove(combat, m, "CURSE_OF_KNOWLEDGE_MOVE");   // player now has KdDisintegration 6
        combat.Player.GainBlockDirect(50);               // block must NOT absorb the tick
        int hpBefore = combat.Player.CurrentHp;
        CombatManager.BeginPlayerTurn(combat);
        CombatManager.EndPlayerTurn(combat);             // fires the tick
        Assert.Equal(6, hpBefore - combat.Player.CurrentHp);
        Assert.Equal(50, combat.Player.Block);           // block untouched (unblockable)
    }

    [Fact]
    public void Stacked_Disintegration_Ticks_21_Per_Player_Turn()
    {
        var m = Monsters.KnowledgeDemon();
        var combat = Combat(m);
        RunMove(combat, m, "CURSE_OF_KNOWLEDGE_MOVE");
        RunMove(combat, m, "CURSE_OF_KNOWLEDGE_MOVE");
        RunMove(combat, m, "CURSE_OF_KNOWLEDGE_MOVE");
        int hpBefore = combat.Player.CurrentHp;
        CombatManager.BeginPlayerTurn(combat);
        CombatManager.EndPlayerTurn(combat);
        Assert.Equal(21, hpBefore - combat.Player.CurrentHp);   // 6 + 7 + 8 every player turn
    }

    // ---- CURSE BRANCH: counter < 3 ⇒ Curse again; counter >= 3 ⇒ Slap, then endless Slap/KnowOver/Ponder ----
    [Fact]
    public void Branch_Repeats_Curse_While_Counter_Below_Three()
    {
        var m = Monsters.KnowledgeDemon();
        m.Ai.CurrentMoveId = "PONDER_MOVE";
        // counter 0 < 3 ⇒ next after Ponder is Curse again.
        Assert.Equal("CURSE_OF_KNOWLEDGE_MOVE", m.Ai.EnumerateNext(m).Single().moveId);
    }

    [Fact]
    public void Branch_Falls_Through_To_Slap_After_Three_Curses()
    {
        var m = Monsters.KnowledgeDemon();
        m.AddPower(new KdCurseCountPower(), 3);   // counter == 3
        m.Ai.CurrentMoveId = "PONDER_MOVE";
        Assert.Equal("SLAP_MOVE", m.Ai.EnumerateNext(m).Single().moveId);
    }

    [Fact]
    public void Full_Sequence_Is_Three_Curse_Cycles_Then_Slap_Loop()
    {
        var m = Monsters.KnowledgeDemon();
        var combat = Combat(m);
        m.Ai.CurrentMoveId = m.Ai.EnumerateInitial(m).Single().moveId;   // CURSE
        var seq = new System.Collections.Generic.List<string>();
        for (int i = 0; i < 13; i++)
        {
            seq.Add(m.Ai.CurrentMoveId);
            m.PerformCurrentMove(combat);
            m.Ai.CurrentMoveId = m.Ai.EnumerateNext(m).Single().moveId;
        }
        Assert.Equal(new[]
        {
            "CURSE_OF_KNOWLEDGE_MOVE", "SLAP_MOVE", "KNOWLEDGE_OVERWHELMING_MOVE", "PONDER_MOVE",
            "CURSE_OF_KNOWLEDGE_MOVE", "SLAP_MOVE", "KNOWLEDGE_OVERWHELMING_MOVE", "PONDER_MOVE",
            "CURSE_OF_KNOWLEDGE_MOVE", "SLAP_MOVE", "KNOWLEDGE_OVERWHELMING_MOVE", "PONDER_MOVE",
            "SLAP_MOVE",   // counter now 3 ⇒ branch falls through to Slap (no 4th curse)
        }, seq);
        Assert.Equal(3, m.GetPowerAmount("KdCurseCount"));
    }
}
