using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>
/// The Kin (Act-1 multi-monster BOSS): 2× KinFollower + KinPriest. Asserts HP scaling, the deterministic move
/// cycles + intents, and per-move damage/effects, against sts2.dll v0.107.0
/// (MegaCrit.Sts2.Core.Models.Monsters.KinFollower / KinPriest / TheKinBoss).
/// </summary>
public class TheKinTests
{
    private static CombatState Combat(params Monster[] monsters)
    {
        var player = new Player { Name = "P", CurrentHp = 9999, MaxHp = 9999, MaxEnergy = 3 };
        return Catalog.SetupCombat(player, monsters);
    }

    private static int RunMove(CombatState combat, Monster m, string moveId)
    {
        m.Ai.CurrentMoveId = moveId;
        int before = combat.Player.CurrentHp;
        m.PerformCurrentMove(combat);
        return before - combat.Player.CurrentHp;
    }

    private static MoveState Move(Monster m, string id) => (MoveState)m.Ai.States[id];

    // ---- KinFollower ---------------------------------------------------------------------------

    [Theory]
    [InlineData(0, 59)]   // base = max roll of 58–59
    [InlineData(7, 59)]
    [InlineData(8, 63)]   // Tough = max roll of 62–63
    public void KinFollower_Hp_Scales_With_Tough(int asc, int expected)
        => Assert.Equal(expected, Monsters.KinFollower(ascension: asc).MaxHp);

    [Fact]
    public void KinFollower_Cycles_Slash_Boomerang_Dance()
    {
        var m = Monsters.KinFollower();
        Assert.Equal("QUICK_SLASH_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);
        Assert.Equal("BOOMERANG_MOVE", Move(m, "QUICK_SLASH_MOVE").FollowUp!.Id);
        Assert.Equal("POWER_DANCE_MOVE", Move(m, "BOOMERANG_MOVE").FollowUp!.Id);
        Assert.Equal("QUICK_SLASH_MOVE", Move(m, "POWER_DANCE_MOVE").FollowUp!.Id);
    }

    [Fact]
    public void KinFollower_StartsWithDance_Opens_On_PowerDance()
        => Assert.Equal("POWER_DANCE_MOVE",
            Monsters.KinFollower(startsWithDance: true).Ai.EnumerateInitial(Monsters.KinFollower(startsWithDance: true)).Single().moveId);

    [Fact]
    public void KinFollower_Moves_Deal_Their_Damage()
    {
        var m = Monsters.KinFollower();
        var c = Combat(m);
        Assert.Equal(5, RunMove(c, m, "QUICK_SLASH_MOVE"));
        Assert.Equal(4, RunMove(c, m, "BOOMERANG_MOVE"));   // 2 × 2
    }

    [Fact]
    public void KinFollower_PowerDance_Buffs_Itself()
    {
        var m = Monsters.KinFollower();             // base DanceStrength 2
        var c = Combat(m);
        RunMove(c, m, "POWER_DANCE_MOVE");
        Assert.Equal(2, m.GetPowerAmount("Strength"));
        var md = Monsters.KinFollower(ascension: 9); // Deadly DanceStrength 3
        RunMove(Combat(md), md, "POWER_DANCE_MOVE");
        Assert.Equal(3, md.GetPowerAmount("Strength"));
    }

    // ---- KinPriest ----------------------------------------------------------------------------

    [Theory]
    [InlineData(0, 190)]
    [InlineData(8, 199)]
    public void KinPriest_Hp_Scales_With_Tough(int asc, int expected)
        => Assert.Equal(expected, Monsters.KinPriest(ascension: asc).MaxHp);

    [Fact]
    public void KinPriest_Cycles_Frailty_Weakness_Beam_Ritual()
    {
        var m = Monsters.KinPriest();
        Assert.Equal("ORB_OF_FRAILTY_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);
        Assert.Equal("ORB_OF_WEAKNESS_MOVE", Move(m, "ORB_OF_FRAILTY_MOVE").FollowUp!.Id);
        Assert.Equal("BEAM_MOVE", Move(m, "ORB_OF_WEAKNESS_MOVE").FollowUp!.Id);
        Assert.Equal("RITUAL_MOVE", Move(m, "BEAM_MOVE").FollowUp!.Id);
        Assert.Equal("ORB_OF_FRAILTY_MOVE", Move(m, "RITUAL_MOVE").FollowUp!.Id);
    }

    [Fact]
    public void KinPriest_OrbOfFrailty_Deals_8_And_Applies_Frail()
    {
        var m = Monsters.KinPriest();
        var c = Combat(m);
        Assert.Equal(8, RunMove(c, m, "ORB_OF_FRAILTY_MOVE"));
        Assert.Equal(1, c.Player.GetPowerAmount("Frail"));
    }

    [Fact]
    public void KinPriest_OrbOfWeakness_Deals_8_And_Applies_Weak()
    {
        var m = Monsters.KinPriest();
        var c = Combat(m);
        Assert.Equal(8, RunMove(c, m, "ORB_OF_WEAKNESS_MOVE"));
        Assert.Equal(1, c.Player.GetPowerAmount("Weak"));
    }

    [Fact]
    public void KinPriest_Beam_Hits_3_Times_For_3()
    {
        var m = Monsters.KinPriest();
        var c = Combat(m);
        Assert.Equal(9, RunMove(c, m, "BEAM_MOVE"));   // 3 × 3
        Assert.Equal(3, Move(m, "BEAM_MOVE").IntentHits);
    }

    [Fact]
    public void KinPriest_Ritual_Buffs_Itself()
    {
        var m = Monsters.KinPriest();   // base RitualStrength 2
        var c = Combat(m);
        RunMove(c, m, "RITUAL_MOVE");
        Assert.Equal(2, m.GetPowerAmount("Strength"));
    }

    // ---- The Kin encounter --------------------------------------------------------------------

    [Fact]
    public void TheKin_Encounter_Is_Two_Followers_And_A_Priest()
    {
        var kin = Catalog.BuildEliteEncounter("TheKinBoss").ToList();
        Assert.Equal(3, kin.Count);
        Assert.Equal(2, kin.Count(m => m.Name == "KinFollower"));
        Assert.Single(kin, m => m.Name == "KinPriest");
        // Exactly one follower opens on the dance (slot1's StartsWithDance).
        Assert.Single(kin.Where(m => m.Name == "KinFollower"),
            m => m.Ai.EnumerateInitial(m).Single().moveId == "POWER_DANCE_MOVE");
    }
}
