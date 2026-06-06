using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>
/// The Queen (Act-3 BOSS) + TorchHeadAmalgam. Asserts HP, move cycles/intents, the crippling YOU_ARE_MINE
/// debuffs, BurnBright, the Amalgam-death branch, and the ChainsOfBinding model (−3 cards drawn/turn), against
/// sts2.dll v0.107.0 (MegaCrit.Sts2.Core.Models.Monsters.Queen / TorchHeadAmalgam / QueenBoss).
/// </summary>
public class QueenTests
{
    private static CombatState Combat(params Monster[] m)
    {
        var player = new Player { Name = "P", CurrentHp = 9999, MaxHp = 9999, MaxEnergy = 3 };
        return Catalog.SetupCombat(player, m);
    }

    private static int RunMove(CombatState c, Monster m, string id)
    {
        m.Ai.CurrentMoveId = id;
        int before = c.Player.CurrentHp;
        m.PerformCurrentMove(c);
        return before - c.Player.CurrentHp;
    }

    private static MoveState Move(Monster m, string id) => (MoveState)m.Ai.States[id];

    // ---- TorchHeadAmalgam ----------------------------------------------------------------------

    [Theory]
    [InlineData(0, 199)]
    [InlineData(8, 211)]
    public void Amalgam_Hp(int asc, int expected) => Assert.Equal(expected, Monsters.TorchHeadAmalgam(ascension: asc).MaxHp);

    [Fact]
    public void Amalgam_Chain_Loops_On_Beam()
    {
        var m = Monsters.TorchHeadAmalgam();
        Assert.Equal("TACKLE_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);
        Assert.Equal("BEAM_MOVE", Move(m, "TACKLE_2_MOVE").FollowUp!.Id);
        Assert.Equal("TACKLE_3_MOVE", Move(m, "BEAM_MOVE").FollowUp!.Id);
        Assert.Equal("BEAM_MOVE", Move(m, "TACKLE_4_MOVE").FollowUp!.Id);   // loop back to BEAM
        var c = Combat(m);
        Assert.Equal(18, RunMove(c, m, "TACKLE_MOVE"));
        Assert.Equal(24, RunMove(c, m, "BEAM_MOVE"));    // 8 × 3
        Assert.Equal(14, RunMove(c, m, "TACKLE_3_MOVE")); // weak tackle
    }

    // ---- Queen ---------------------------------------------------------------------------------

    [Theory]
    [InlineData(0, 400)]
    [InlineData(8, 419)]
    public void Queen_Hp(int asc, int expected) => Assert.Equal(expected, Monsters.Queen(ascension: asc).MaxHp);

    [Fact]
    public void PuppetStrings_Makes_The_Player_Draw_Three_Fewer()
    {
        var m = Monsters.Queen();
        var c = Combat(m);
        Assert.Equal(Player.CardsDrawnPerTurn, CombatManager.TurnStartDrawCount(c));   // baseline
        RunMove(c, m, "PUPPET_STRINGS_MOVE");
        Assert.Equal(3, c.Player.GetPowerAmount("QueenChains"));
        Assert.Equal(Player.CardsDrawnPerTurn - 3, CombatManager.TurnStartDrawCount(c));
    }

    [Fact]
    public void YouAreMine_Cripples_The_Player()
    {
        var m = Monsters.Queen();
        var c = Combat(m);
        RunMove(c, m, "YOU_ARE_MINE_MOVE");
        Assert.Equal(99, c.Player.GetPowerAmount("Frail"));
        Assert.Equal(99, c.Player.GetPowerAmount("Weak"));
        Assert.Equal(99, c.Player.GetPowerAmount("Vulnerable"));
    }

    [Fact]
    public void OffWithYourHead_And_Execution_And_Enrage()
    {
        var m = Monsters.Queen();
        var c = Combat(m);
        Assert.Equal(15, RunMove(c, m, "OFF_WITH_YOUR_HEAD_MOVE"));   // 3 × 5
        Assert.Equal(15, RunMove(c, m, "EXECUTION_MOVE"));
        RunMove(c, m, "ENRAGE_MOVE");
        Assert.Equal(2, m.GetPowerAmount("Strength"));
    }

    [Fact]
    public void BurnBright_Buffs_The_Amalgam_And_Blocks()
    {
        var amalgam = Monsters.TorchHeadAmalgam();
        var queen = Monsters.Queen();
        var c = Combat(amalgam, queen);
        RunMove(c, queen, "BURN_BRIGHT_FOR_ME_MOVE");
        Assert.Equal(1, amalgam.GetPowerAmount("Strength"));
        Assert.Equal(20, queen.Block);
    }

    [Fact]
    public void Branch_Burns_While_Amalgam_Lives_Then_Attacks_When_It_Dies()
    {
        var amalgam = Monsters.TorchHeadAmalgam();
        var queen = Monsters.Queen();
        var c = Combat(amalgam, queen);

        // Amalgam alive → after YOU_ARE_MINE the Queen goes to BURN_BRIGHT (and BURN_BRIGHT loops).
        queen.Ai.CurrentMoveId = "YOU_ARE_MINE_MOVE";
        Assert.Equal("BURN_BRIGHT_FOR_ME_MOVE", queen.Ai.EnumerateNext(queen).Single().moveId);

        // Kill the Amalgam → the watcher flips the branch to OFF_WITH_YOUR_HEAD.
        Cmd.Attack(c, c.Player, amalgam, 9999, ValueProp.Unblockable | ValueProp.Unpowered, null);
        Assert.False(amalgam.IsAlive);
        Assert.Equal("OFF_WITH_YOUR_HEAD_MOVE", queen.Ai.EnumerateNext(queen).Single().moveId);
        queen.Ai.CurrentMoveId = "BURN_BRIGHT_FOR_ME_MOVE";
        Assert.Equal("OFF_WITH_YOUR_HEAD_MOVE", queen.Ai.EnumerateNext(queen).Single().moveId);
    }

    [Fact]
    public void Queen_Encounter_Is_Amalgam_And_Queen()
    {
        var enc = Catalog.BuildEliteEncounter("QueenBoss").ToList();
        Assert.Equal(2, enc.Count);
        Assert.Single(enc, m => m.Name == "Queen");
        Assert.Single(enc, m => m.Name == "TorchHeadAmalgam");
    }
}
