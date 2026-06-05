using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>
/// Unit tests for the Act-1 BOSS ports. Assert HP scaling (Tough), per-move damage/effects (Deadly), the AI
/// move chain/intents, and the registration plumbing — straight from the decompiled game source. Mirrors the
/// MonsterPortTests structure. The WaterfallGiant's death-phase "Steam Eruption" explosion is MODELLED FAITHFULLY
/// (accumulating SteamEruption counter → survive-at-0 → ABOUT_TO_BLOW telegraph → EXPLODE for the counter → die);
/// the death-phase tests below drive that sequence end-to-end.
/// </summary>
public class BossTests
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
    [InlineData(0, 240)]   // base
    [InlineData(7, 240)]   // below ToughEnemies (>=8)
    [InlineData(8, 250)]   // ToughEnemies
    [InlineData(10, 250)]
    public void WaterfallGiant_Hp_Scales_With_Tough(int asc, int expectedHp)
    {
        var m = Monsters.WaterfallGiant(ascension: asc);
        Assert.Equal(expectedHp, m.MaxHp);
        Assert.Equal(expectedHp, m.CurrentHp);
    }

    // ---- AI chain & intents --------------------------------------------------------------------

    [Fact]
    public void WaterfallGiant_Opens_On_Pressurize_Then_Loops_Stomp_Ram_Siphon_Gun_Up()
    {
        var m = Monsters.WaterfallGiant();
        Assert.Equal("PRESSURIZE_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);

        // Deterministic 5-move loop after the one-time Pressurize opener; Pressure Up loops back to Stomp.
        Assert.Equal("STOMP_MOVE", Move(m, "PRESSURIZE_MOVE").FollowUp!.Id);
        Assert.Equal("RAM_MOVE", Move(m, "STOMP_MOVE").FollowUp!.Id);
        Assert.Equal("SIPHON_MOVE", Move(m, "RAM_MOVE").FollowUp!.Id);
        Assert.Equal("PRESSURE_GUN_MOVE", Move(m, "SIPHON_MOVE").FollowUp!.Id);
        Assert.Equal("PRESSURE_UP_MOVE", Move(m, "PRESSURE_GUN_MOVE").FollowUp!.Id);
        Assert.Equal("STOMP_MOVE", Move(m, "PRESSURE_UP_MOVE").FollowUp!.Id);
    }

    [Fact]
    public void WaterfallGiant_Intents_Match_Spec()
    {
        var m = Monsters.WaterfallGiant();
        Assert.Null(Move(m, "PRESSURIZE_MOVE").IntentDamage);   // pure buff
        Assert.Null(Move(m, "SIPHON_MOVE").IntentDamage);       // heal, no attack
        Assert.Equal(15, Move(m, "STOMP_MOVE").IntentDamage);
        Assert.Equal(10, Move(m, "RAM_MOVE").IntentDamage);
        Assert.Equal(20, Move(m, "PRESSURE_GUN_MOVE").IntentDamage);
        Assert.Equal(13, Move(m, "PRESSURE_UP_MOVE").IntentDamage);
    }

    // ---- Move effects (base ascension) ---------------------------------------------------------

    [Fact]
    public void WaterfallGiant_Stomp_Deals_15_And_Applies_1_Weak()
    {
        var m = Monsters.WaterfallGiant();
        var combat = Combat(m);
        Assert.Equal(15, RunMove(combat, m, "STOMP_MOVE"));
        Assert.Equal(1, combat.Player.GetPowerAmount("Weak"));
    }

    [Fact]
    public void WaterfallGiant_Ram_And_PressureUp_Deal_Their_Damage_No_Effects()
    {
        var m = Monsters.WaterfallGiant();
        var combat = Combat(m);
        Assert.Equal(10, RunMove(combat, m, "RAM_MOVE"));
        Assert.Equal(13, RunMove(combat, m, "PRESSURE_UP_MOVE"));
        Assert.Empty(combat.Player.Powers);   // neither applies a debuff
    }

    [Fact]
    public void WaterfallGiant_Pressurize_Is_A_NoOp_On_The_Player_But_Buffs_SteamEruption()
    {
        var m = Monsters.WaterfallGiant();
        var combat = Combat(m);
        Assert.Equal(0, RunMove(combat, m, "PRESSURIZE_MOVE"));   // self-buff: no player-facing effect
        Assert.Equal(15, m.GetPowerAmount("SteamEruption"));      // Pressurize accumulates 15 (base ascension)
        Assert.Empty(combat.Player.Powers);                       // the buff lands on the boss, not the player
    }

    [Fact]
    public void WaterfallGiant_Siphon_Heals_Itself_By_10_Capped_At_MaxHp()
    {
        var m = Monsters.WaterfallGiant();
        var combat = Combat(m);
        m.CurrentHp = 100;                              // hurt the boss so the heal has room
        Assert.Equal(0, RunMove(combat, m, "SIPHON_MOVE"));   // no damage to the player
        Assert.Equal(110, m.CurrentHp);                 // healed 10 (×1 player)

        m.CurrentHp = m.MaxHp - 3;                       // near full: heal clamps to MaxHp
        RunMove(combat, m, "SIPHON_MOVE");
        Assert.Equal(m.MaxHp, m.CurrentHp);
    }

    [Fact]
    public void WaterfallGiant_PressureGun_Ramps_Plus5_Per_Use()
    {
        var m = Monsters.WaterfallGiant();
        var combat = Combat(m);
        Assert.Equal(20, RunMove(combat, m, "PRESSURE_GUN_MOVE"));   // base 20
        Assert.Equal(25, RunMove(combat, m, "PRESSURE_GUN_MOVE"));   // +5
        Assert.Equal(30, RunMove(combat, m, "PRESSURE_GUN_MOVE"));   // +10
        Assert.Equal(3, m.GetPowerAmount("PressureGun"));            // ramp tracked as per-creature state

        // The ramp is per-creature (clones with the monster), not shared across MoveState definitions: a fresh
        // boss still opens at the base 20.
        var fresh = Monsters.WaterfallGiant();
        Assert.Equal(20, RunMove(Combat(fresh), fresh, "PRESSURE_GUN_MOVE"));
    }

    [Fact]
    public void WaterfallGiant_PressureGun_Ramp_Clones_Independently()
    {
        var m = Monsters.WaterfallGiant();
        var combat = Combat(m);
        RunMove(combat, m, "PRESSURE_GUN_MOVE");   // m's ramp -> 1 (gun now at 25)
        var clone = combat.Clone();
        var clonedBoss = (Monster)clone.Monsters[0];

        Assert.Equal(25, RunMove(combat, m, "PRESSURE_GUN_MOVE"));               // original advances: 25
        Assert.Equal(25, RunMove(clone, clonedBoss, "PRESSURE_GUN_MOVE"));        // clone is independent: also 25
        Assert.Equal(2, m.GetPowerAmount("PressureGun"));
        Assert.Equal(2, clonedBoss.GetPowerAmount("PressureGun"));
    }

    // ---- Deadly (>=9) ascension scaling --------------------------------------------------------

    [Fact]
    public void WaterfallGiant_Deadly_Damage_Scales()
    {
        var m = Monsters.WaterfallGiant(ascension: 9);
        var combat = Combat(m);
        Assert.Equal(16, Move(m, "STOMP_MOVE").IntentDamage);
        Assert.Equal(16, RunMove(combat, m, "STOMP_MOVE"));
        Assert.Equal(11, RunMove(combat, m, "RAM_MOVE"));
        Assert.Equal(14, RunMove(combat, m, "PRESSURE_UP_MOVE"));
        Assert.Equal(23, RunMove(combat, m, "PRESSURE_GUN_MOVE"));   // Deadly base 23
        Assert.Equal(28, RunMove(combat, m, "PRESSURE_GUN_MOVE"));   // +5
    }

    [Fact]
    public void WaterfallGiant_Siphon_Heals_15_On_Tough()
    {
        var m = Monsters.WaterfallGiant(ascension: 8);   // ToughEnemies: SiphonHeal 15
        var combat = Combat(m);
        m.CurrentHp = 100;
        RunMove(combat, m, "SIPHON_MOVE");
        Assert.Equal(115, m.CurrentHp);
    }

    // ---- Steam Eruption (death phase) ----------------------------------------------------------

    [Fact]
    public void WaterfallGiant_SteamEruption_Accumulates_Over_Moves()
    {
        var m = Monsters.WaterfallGiant();
        var combat = Combat(m);
        RunMove(combat, m, "PRESSURIZE_MOVE");   // +15
        Assert.Equal(15, m.GetPowerAmount("SteamEruption"));
        RunMove(combat, m, "STOMP_MOVE");        // +3
        RunMove(combat, m, "RAM_MOVE");          // +3
        RunMove(combat, m, "SIPHON_MOVE");       // +3
        RunMove(combat, m, "PRESSURE_GUN_MOVE"); // +3
        RunMove(combat, m, "PRESSURE_UP_MOVE");  // +3
        Assert.Equal(15 + 5 * 3, m.GetPowerAmount("SteamEruption"));   // 30 after the opener + one full loop
    }

    [Fact]
    public void WaterfallGiant_SteamEruption_Deadly_Pressurize_Is_20()
    {
        var m = Monsters.WaterfallGiant(ascension: 9);   // DeadlyEnemies: PressurizeAmount 20
        var combat = Combat(m);
        RunMove(combat, m, "PRESSURIZE_MOVE");
        Assert.Equal(20, m.GetPowerAmount("SteamEruption"));
    }

    /// <summary>The signature mechanic end-to-end: kill → survive (death phase) → ABOUT_TO_BLOW telegraph turn →
    /// EXPLODE for the accumulated counter → die. The explosion is a GUARANTEED hit, so this must never let the
    /// fight end early.</summary>
    [Fact]
    public void WaterfallGiant_At0Hp_Survives_Telegraphs_Then_Explodes_For_Accumulated_Then_Dies()
    {
        var m = Monsters.WaterfallGiant();
        var combat = Combat(m);

        // Accumulate a known eruption counter: Pressurize(15) + Stomp/Ram(3+3) = 21.
        RunMove(combat, m, "PRESSURIZE_MOVE");
        RunMove(combat, m, "STOMP_MOVE");
        RunMove(combat, m, "RAM_MOVE");
        combat.Player.RemovePower("Weak");   // clear Stomp's Weak so it doesn't blunt the explosion
        int expectedBlast = m.GetPowerAmount("SteamEruption");
        Assert.Equal(21, expectedBlast);

        // Bring the boss to 0 HP. It must NOT die — it enters the death phase.
        Cmd.Kill(combat, m);
        Assert.Equal(0, m.CurrentHp);
        Assert.True(m.InDeathPhase);
        Assert.True(m.IsAlive);                  // alive-at-0 so the blow can land
        Assert.False(combat.AllMonstersDead);    // fight is NOT over
        Assert.Equal("ABOUT_TO_BLOW_MOVE", m.Ai.CurrentMoveId);   // AI jumped to the telegraph

        // A further hit on the 0-HP boss is a no-op and does NOT advance/abort the death phase, nor change the blast.
        Cmd.Attack(combat, combat.Player, m, 50, ValueProp.Move, null);
        Assert.True(m.InDeathPhase);
        Assert.Equal(expectedBlast, m.GetPowerAmount("SteamEruption"));

        // Telegraph turn: ABOUT_TO_BLOW performs (a self-stun windup) — no damage to the player, boss still alive.
        int hpBeforeTelegraph = combat.Player.CurrentHp;
        CombatManager.RunEnemyTurn(combat);
        CombatManager.RollNextMoves(combat, new Rng(0));   // advance the telegraph (deterministic: ABOUT_TO_BLOW → EXPLODE)
        Assert.Equal(hpBeforeTelegraph, combat.Player.CurrentHp);   // windup deals nothing
        Assert.True(m.InDeathPhase);
        Assert.True(m.IsAlive);
        Assert.False(combat.AllMonstersDead);
        Assert.Equal("EXPLODE_MOVE", m.Ai.CurrentMoveId);   // ABOUT_TO_BLOW.FollowUp telegraphs EXPLODE

        // Explode turn: deals the accumulated counter to the player, then the boss truly dies.
        int hpBeforeBlast = combat.Player.CurrentHp;
        CombatManager.RunEnemyTurn(combat);
        Assert.Equal(expectedBlast, hpBeforeBlast - combat.Player.CurrentHp);   // guaranteed blast == accumulator
        Assert.False(m.InDeathPhase);
        Assert.False(m.IsAlive);                 // truly dead now
        Assert.True(combat.AllMonstersDead);     // fight is over
    }

    /// <summary>Cloning during the death phase must not leak: the explosion fires independently in each branch and
    /// neither branch's resolution (counter consumption, death) perturbs the other.</summary>
    [Fact]
    public void WaterfallGiant_DeathPhase_Clones_Independently()
    {
        var m = Monsters.WaterfallGiant();
        var combat = Combat(m);
        RunMove(combat, m, "PRESSURIZE_MOVE");   // SteamEruption 15
        Cmd.Kill(combat, m);
        Assert.True(m.InDeathPhase);
        Assert.Equal("ABOUT_TO_BLOW_MOVE", m.Ai.CurrentMoveId);

        var clone = combat.Clone();
        var clonedBoss = (Monster)clone.Monsters[0];
        Assert.True(clonedBoss.InDeathPhase);                       // phase state carried into the clone
        Assert.Equal(15, clonedBoss.GetPowerAmount("SteamEruption"));
        Assert.Equal("ABOUT_TO_BLOW_MOVE", clonedBoss.Ai.CurrentMoveId);

        // Resolve the ORIGINAL all the way to death.
        CombatManager.RunEnemyTurn(combat);   // ABOUT_TO_BLOW
        CombatManager.RollNextMoves(combat, new Rng(0));   // ABOUT_TO_BLOW → EXPLODE
        int origBefore = combat.Player.CurrentHp;
        CombatManager.RunEnemyTurn(combat);   // EXPLODE
        Assert.Equal(15, origBefore - combat.Player.CurrentHp);
        Assert.False(m.IsAlive);
        Assert.True(combat.AllMonstersDead);

        // The clone is untouched: still mid-phase, still alive, still primed to explode for its own 15.
        Assert.True(clonedBoss.InDeathPhase);
        Assert.True(clonedBoss.IsAlive);
        Assert.False(clone.AllMonstersDead);
        CombatManager.RunEnemyTurn(clone);    // ABOUT_TO_BLOW
        CombatManager.RollNextMoves(clone, new Rng(0));   // ABOUT_TO_BLOW → EXPLODE
        int cloneBefore = clone.Player.CurrentHp;
        CombatManager.RunEnemyTurn(clone);    // EXPLODE
        Assert.Equal(15, cloneBefore - clone.Player.CurrentHp);
        Assert.False(clonedBoss.IsAlive);
        Assert.True(clone.AllMonstersDead);
    }

    // ---- Registration plumbing -----------------------------------------------------------------

    [Fact]
    public void WaterfallGiant_Is_Registered_In_Monster_Catalog()
    {
        var m = Catalog.BuildMonster("WaterfallGiant", ascension: 0);
        Assert.Equal("WaterfallGiant", m.Name);
        Assert.Equal(240, m.MaxHp);
    }

    [Fact]
    public void WaterfallGiantBoss_Encounter_Builds_A_Single_Giant()
    {
        Assert.True(Catalog.IsKnownEliteEncounter("WaterfallGiantBoss"));
        var monsters = Catalog.BuildEliteEncounter("WaterfallGiantBoss", ascension: 8);
        Assert.Single(monsters);
        Assert.Equal("WaterfallGiant", monsters[0].Name);
        Assert.Equal(250, monsters[0].MaxHp);   // Tough at ascension 8
    }
}
