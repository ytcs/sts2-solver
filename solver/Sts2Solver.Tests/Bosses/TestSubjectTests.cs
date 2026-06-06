using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>
/// Test Subject (Act-3 3-form revive BOSS). Asserts HP/forms, the Adaptable revive (heal to next form + phase
/// swap + power changes), the Respawns branch, Multi-Claw ramp, Enrage, PainfulStabs Wounds, and Burning Growl,
/// against sts2.dll v0.107.0 (MegaCrit.Sts2.Core.Models.Monsters.TestSubject + its powers).
/// </summary>
public class TestSubjectTests
{
    private static CombatState Combat(Monster m)
    {
        var player = new Player { Name = "P", CurrentHp = 9999, MaxHp = 9999, MaxEnergy = 3 };
        return Catalog.SetupCombat(player, new[] { m });
    }

    private static int RunMove(CombatState c, Monster m, string id)
    {
        m.Ai.CurrentMoveId = id;
        int before = c.Player.CurrentHp;
        m.PerformCurrentMove(c);
        return before - c.Player.CurrentHp;
    }

    private static void Kill(CombatState c, Monster m)
        => Cmd.Attack(c, c.Player, m, 9999, ValueProp.Unblockable | ValueProp.Unpowered, null);

    [Theory]
    [InlineData(0, 100)]
    [InlineData(8, 111)]
    public void First_Form_Hp(int asc, int expected) => Assert.Equal(expected, Monsters.TestSubject(ascension: asc).MaxHp);

    [Fact]
    public void Opens_With_Adaptable_And_Enrage_On_Phase_1()
    {
        var m = Monsters.TestSubject();
        Assert.True(m.HasPower("Adaptable"));
        Assert.True(m.HasPower("Enrage"));
        Assert.Equal("BITE_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);
        var c = Combat(m);
        Assert.Equal(20, RunMove(c, m, "BITE_MOVE"));
        Assert.Equal(14, RunMove(c, m, "SKULL_BASH_MOVE"));
        Assert.Equal(1, c.Player.GetPowerAmount("Vulnerable"));
    }

    [Fact]
    public void Enrage_Gains_Strength_When_Player_Plays_A_Skill()
    {
        var m = Monsters.TestSubject();   // base Enrage 2
        var c = Combat(m);
        var defend = new DefendIronclad();   // a Skill
        c.Player.Hand.Add(defend);
        c.Player.ResetEnergy();
        CombatManager.PlayCard(c, defend, null);
        Assert.Equal(2, m.GetPowerAmount("Strength"));
    }

    [Fact]
    public void First_Death_Revives_To_Second_Form_With_PainfulStabs_And_MultiClaw()
    {
        var m = Monsters.TestSubject();
        var c = Combat(m);
        Kill(c, m);
        Assert.True(m.IsAlive);
        Assert.Equal(1, m.Respawns);
        Assert.Equal(200, m.MaxHp);
        Assert.Equal(200, m.CurrentHp);
        Assert.True(m.HasPower("PainfulStabs"));
        Assert.True(m.HasPower("Adaptable"));   // still revivable once more
        Assert.Equal("RESPAWN_MOVE", m.Ai.CurrentMoveId);
        Assert.Equal("MULTI_CLAW_MOVE", m.Ai.EnumerateNext(m).Single().moveId);   // branch: Respawns < 2
    }

    [Fact]
    public void Second_Death_Revives_To_Third_Form_With_Nemesis_And_Drops_Adaptable()
    {
        var m = Monsters.TestSubject();
        var c = Combat(m);
        Kill(c, m);   // → form 2
        Kill(c, m);   // → form 3
        Assert.True(m.IsAlive);
        Assert.Equal(2, m.Respawns);
        Assert.Equal(300, m.MaxHp);
        Assert.True(m.HasPower("Nemesis"));
        Assert.False(m.HasPower("Adaptable"));      // no more revives
        Assert.False(m.HasPower("PainfulStabs"));
        Assert.Equal("PHASE3_LACERATE_MOVE", m.Ai.EnumerateNext(m).Single().moveId);   // branch: Respawns >= 2
    }

    [Fact]
    public void Third_Death_Is_Fatal()
    {
        var m = Monsters.TestSubject();
        var c = Combat(m);
        Kill(c, m); Kill(c, m);   // forms 2 and 3
        Kill(c, m);               // Adaptable gone → really dies
        Assert.False(m.IsAlive);
    }

    [Fact]
    public void MultiClaw_Ramps_A_Hit_Each_Use()
    {
        var m = Monsters.TestSubject();
        var c = Combat(m);
        // 10 × 3, then 10 × 4, then 10 × 5 = 120.
        int total = RunMove(c, m, "MULTI_CLAW_MOVE") + RunMove(c, m, "MULTI_CLAW_MOVE") + RunMove(c, m, "MULTI_CLAW_MOVE");
        Assert.Equal(30 + 40 + 50, total);
    }

    [Fact]
    public void PainfulStabs_Adds_A_Wound_On_A_Connecting_Hit_Once_Per_Turn()
    {
        var m = Monsters.TestSubject();
        var c = Combat(m);
        Kill(c, m);   // → has PainfulStabs
        RunMove(c, m, "MULTI_CLAW_MOVE");
        Assert.Single(c.Player.DiscardPile.OfType<Wound>());
        RunMove(c, m, "MULTI_CLAW_MOVE");   // same turn → no extra Wound
        Assert.Single(c.Player.DiscardPile.OfType<Wound>());
    }

    [Fact]
    public void BurningGrowl_Adds_Burn_And_Strength()
    {
        var m = Monsters.TestSubject();   // base burn 3, strength 2
        var c = Combat(m);
        RunMove(c, m, "BURNING_GROWL_MOVE");
        Assert.Equal(3, c.Player.DiscardPile.OfType<Burn>().Count());
        Assert.Equal(2, m.GetPowerAmount("Strength"));
    }

    [Fact]
    public void TestSubject_Encounter_Is_A_Single_Monster()
    {
        var enc = Catalog.BuildEliteEncounter("TestSubjectBoss").ToList();
        Assert.Single(enc);
        Assert.Equal("TestSubject", enc[0].Name);
    }
}
