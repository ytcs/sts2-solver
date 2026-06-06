using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>
/// Kaiser Crab (Act-2 two-arm BOSS): Crusher (left) + Rocket (right). Asserts HP, the deterministic cycles +
/// intents, per-move effects, and the two soundness-critical mechanics — the "surrounded" facing back-attack
/// (×1.5 from the arm you're NOT facing) and CrabRage (survivor +6 Str/+99 Block when one arm dies). Against
/// sts2.dll v0.107.0 (MegaCrit.Sts2.Core.Models.Monsters.Crusher / Rocket / KaiserCrabBoss).
/// </summary>
public class KaiserCrabTests
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

    /// <summary>End the player turn so the back-attack powers recompute "am I behind" from the current facing.</summary>
    private static void Recompute(CombatState c) => CombatManager.EndPlayerTurn(c);

    // ---- Crusher -------------------------------------------------------------------------------

    [Theory]
    [InlineData(0, 209)]
    [InlineData(8, 219)]
    public void Crusher_Hp(int asc, int expected) => Assert.Equal(expected, Monsters.Crusher(ascension: asc).MaxHp);

    [Fact]
    public void Crusher_Cycle()
    {
        var m = Monsters.Crusher();
        Assert.Equal("THRASH_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);
        Assert.Equal("ENLARGING_STRIKE_MOVE", Move(m, "THRASH_MOVE").FollowUp!.Id);
        Assert.Equal("BUG_STING_MOVE", Move(m, "ENLARGING_STRIKE_MOVE").FollowUp!.Id);
        Assert.Equal("ADAPT_MOVE", Move(m, "BUG_STING_MOVE").FollowUp!.Id);
        Assert.Equal("GUARDED_STRIKE_MOVE", Move(m, "ADAPT_MOVE").FollowUp!.Id);
        Assert.Equal("THRASH_MOVE", Move(m, "GUARDED_STRIKE_MOVE").FollowUp!.Id);
    }

    [Fact]
    public void Crusher_BugSting_Hits_Twice_And_Debuffs()
    {
        var m = Monsters.Crusher();
        var c = Combat(m);
        c.KaiserFrontId = m.Id;   // face Crusher so its own attack isn't back-attack-boosted
        Recompute(c);
        Assert.Equal(12, RunMove(c, m, "BUG_STING_MOVE"));   // 6 × 2
        Assert.Equal(2, c.Player.GetPowerAmount("Weak"));
        Assert.Equal(2, c.Player.GetPowerAmount("Frail"));
    }

    [Fact]
    public void Crusher_GuardedStrike_Attacks_And_Blocks()
    {
        var m = Monsters.Crusher();
        var c = Combat(m);
        c.KaiserFrontId = m.Id;
        Recompute(c);
        Assert.Equal(12, RunMove(c, m, "GUARDED_STRIKE_MOVE"));
        Assert.Equal(18, m.Block);
    }

    [Fact]
    public void Crusher_Adapt_Buffs_Itself() { var m = Monsters.Crusher(); RunMove(Combat(m), m, "ADAPT_MOVE"); Assert.Equal(2, m.GetPowerAmount("Strength")); }

    // ---- Rocket --------------------------------------------------------------------------------

    [Theory]
    [InlineData(0, 199)]
    [InlineData(8, 209)]
    public void Rocket_Hp(int asc, int expected) => Assert.Equal(expected, Monsters.Rocket(ascension: asc).MaxHp);

    [Fact]
    public void Rocket_Cycle_Ends_On_Recharge()
    {
        var m = Monsters.Rocket();
        Assert.Equal("TARGETING_RETICLE_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);
        Assert.Equal("RECHARGE_MOVE", Move(m, "LASER_MOVE").FollowUp!.Id);
        Assert.Equal("TARGETING_RETICLE_MOVE", Move(m, "RECHARGE_MOVE").FollowUp!.Id);
    }

    [Fact]
    public void Rocket_Recharge_Does_Nothing()
    {
        var m = Monsters.Rocket();
        var c = Combat(m);
        Assert.Equal(0, RunMove(c, m, "RECHARGE_MOVE"));
    }

    // ---- Surrounded: the un-faced arm back-attacks for ×1.5 ------------------------------------

    [Fact]
    public void Facing_An_Arm_Makes_The_Other_Back_Attack()
    {
        var crab = Monsters.KaiserCrab().ToList();
        var crusher = crab[0]; var rocket = crab[1];
        var c = Combat(crusher, rocket);

        // Face the Crusher → it's in front (12), the Rocket is behind (reticle 3 → ×1.5 = 4).
        c.KaiserFrontId = crusher.Id;
        Recompute(c);
        Assert.Equal(12, RunMove(c, crusher, "THRASH_MOVE"));
        Assert.Equal(4, RunMove(c, rocket, "TARGETING_RETICLE_MOVE"));   // floor(3 × 1.5)

        // Face the Rocket → now the Crusher back-attacks (thrash 12 → ×1.5 = 18), Rocket front (3).
        c.KaiserFrontId = rocket.Id;
        Recompute(c);
        Assert.Equal(18, RunMove(c, crusher, "THRASH_MOVE"));            // floor(12 × 1.5)
        Assert.Equal(3, RunMove(c, rocket, "TARGETING_RETICLE_MOVE"));
    }

    [Fact]
    public void Initial_Facing_Leaves_The_Left_Arm_Behind()
    {
        var crab = Monsters.KaiserCrab().ToList();
        var crusher = crab[0]; var rocket = crab[1];
        var c = Combat(crusher, rocket);
        Recompute(c);   // no target yet → default facing: LEFT arm (Crusher) behind
        Assert.Equal(18, RunMove(c, crusher, "THRASH_MOVE"));           // Crusher behind → ×1.5
        Assert.Equal(3, RunMove(c, rocket, "TARGETING_RETICLE_MOVE"));  // Rocket front → normal
    }

    [Fact]
    public void Lone_Surviving_Arm_Does_Not_Back_Attack()
    {
        var crab = Monsters.KaiserCrab().ToList();
        var crusher = crab[0]; var rocket = crab[1];
        var c = Combat(crusher, rocket);
        Cmd.Attack(c, c.Player, rocket, 9999, ValueProp.Unblockable | ValueProp.Unpowered, null);   // kill Rocket
        Assert.False(rocket.IsAlive);
        Recompute(c);
        // Lone arm is faced → no ×1.5. THRASH = 12 base + 6 CrabRage Strength = 18 (would be 27 if back-attacked).
        Assert.Equal(18, RunMove(c, crusher, "THRASH_MOVE"));
    }

    // ---- CrabRage ------------------------------------------------------------------------------

    [Fact]
    public void CrabRage_Enrages_The_Survivor_When_An_Arm_Dies()
    {
        var crab = Monsters.KaiserCrab().ToList();
        var crusher = crab[0]; var rocket = crab[1];
        var c = Combat(crusher, rocket);
        Cmd.Attack(c, c.Player, crusher, 9999, ValueProp.Unblockable | ValueProp.Unpowered, null);   // kill Crusher
        Assert.False(crusher.IsAlive);
        Assert.Equal(6, rocket.GetPowerAmount("Strength"));   // +6 Strength
        Assert.Equal(99, rocket.Block);                       // +99 Block
    }

    [Fact]
    public void KaiserCrab_Encounter_Is_Crusher_And_Rocket()
    {
        var crab = Catalog.BuildEliteEncounter("KaiserCrabBoss").ToList();
        Assert.Equal(2, crab.Count);
        Assert.Single(crab, m => m.Name == "Crusher");
        Assert.Single(crab, m => m.Name == "Rocket");
    }
}
