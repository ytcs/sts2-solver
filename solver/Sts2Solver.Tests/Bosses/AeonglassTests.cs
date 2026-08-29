using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>
/// Unit tests for the Act-4 Glory BOSS Aeonglass. Assert HP scaling (Tough), the combat-start powers (self
/// Artifact 3, Withering-Presence counter 6), the deterministic AI move chain/intents, per-move damage/effects
/// (Deadly): Ebb's attack + the 33 Block it gains (the 2026-06 patch replaced the old −3 Str/−3 Dex drain with
/// this Block, relocated from Increasing Intensity), Eye Lasers' 2-hit attack, and the soundness-critical
/// Increasing Intensity ramp — the fake-upgrade of every in-play Wither (+3 each), the WitherAmount fresh
/// Withers, and the triangular self-Strength gain (no Block — moved to Ebb). Plus the Withering-Presence "every 6
/// cards played → 1 Wither to hand (at the current level)" mechanic, and the Wither turn-end-in-hand chip damage
/// (base 3 + 3·level).
/// </summary>
public class AeonglassTests
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

    // ---- HP scaling (MinInitialHp == MaxInitialHp: fixed, no roll) ------------------------------

    [Theory]
    [InlineData(0, 512)]    // base
    [InlineData(7, 512)]    // below ToughEnemies (>=8)
    [InlineData(8, 535)]    // ToughEnemies
    [InlineData(20, 535)]
    public void Aeonglass_Hp_Scales_With_Tough(int asc, int expectedHp)
    {
        var m = Monsters.Aeonglass(ascension: asc);
        Assert.Equal(expectedHp, m.MaxHp);
        Assert.Equal(expectedHp, m.CurrentHp);
    }

    [Fact]
    public void Aeonglass_Hp_Override_Respected()
    {
        var m = Monsters.Aeonglass(hp: 200, ascension: 8);
        Assert.Equal(200, m.MaxHp);
        Assert.Equal(200, m.CurrentHp);
    }

    // ---- Combat-start powers --------------------------------------------------------------------

    [Fact]
    public void Aeonglass_Opens_With_Artifact3_And_WitheringPresence6()
    {
        var m = Monsters.Aeonglass();
        Assert.Equal(3, m.GetPowerAmount("Artifact"));
        Assert.Equal(6, m.GetPowerAmount(AeonglassWitheringPresencePower.PowerId));
    }

    // ---- AI chain & intents --------------------------------------------------------------------

    [Fact]
    public void Aeonglass_Opens_On_Ebb_Then_Loops_EyeLasers_IncreasingIntensity()
    {
        var m = Monsters.Aeonglass();
        Assert.Equal("EBB_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);

        Assert.Equal("EYE_LASERS_MOVE", Move(m, "EBB_MOVE").FollowUp!.Id);
        Assert.Equal("INCREASING_INTENSITY_MOVE", Move(m, "EYE_LASERS_MOVE").FollowUp!.Id);
        Assert.Equal("EBB_MOVE", Move(m, "INCREASING_INTENSITY_MOVE").FollowUp!.Id);   // loop back
    }

    [Fact]
    public void Aeonglass_Intents_Match_Spec()
    {
        var m = Monsters.Aeonglass();
        Assert.Equal(22, Move(m, "EBB_MOVE").IntentDamage);
        Assert.Equal(11, Move(m, "EYE_LASERS_MOVE").IntentDamage);
        Assert.Equal(2, Move(m, "EYE_LASERS_MOVE").IntentHits);
        Assert.Null(Move(m, "INCREASING_INTENSITY_MOVE").IntentDamage);   // no attack telegraph
    }

    // ---- EBB: attack + 33 Block (2026-06 patch — drain removed, Block moved here) ----------------

    [Fact]
    public void Aeonglass_Ebb_Deals_22_And_Gains_33_Block()
    {
        var m = Monsters.Aeonglass();
        var combat = Combat(m);
        Assert.Equal(22, RunMove(combat, m, "EBB_MOVE"));
        Assert.Equal(33, m.Block);                                   // EbbBlock (relocated from Increasing Intensity)
    }

    [Fact]
    public void Aeonglass_Ebb_Does_Not_Drain_Player_Str_Or_Dex()
    {
        var m = Monsters.Aeonglass();
        var combat = Combat(m);
        RunMove(combat, m, "EBB_MOVE");
        // The old EbbPower drain is gone post-patch: the player's stats are untouched.
        Assert.Equal(0, combat.Player.GetPowerAmount("Strength"));
        Assert.Equal(0, combat.Player.GetPowerAmount("Dexterity"));
    }

    [Fact]
    public void Aeonglass_Ebb_Attack_Picks_Up_Boss_Strength()
    {
        var m = Monsters.Aeonglass();
        var combat = Combat(m);
        Cmd.ApplyPower(combat, m, new StrengthPower(), 4, m);   // simulate one Increasing-Intensity buff
        Assert.Equal(26, RunMove(combat, m, "EBB_MOVE"));        // 22 + 4
    }

    // ---- EYE_LASERS: 2-hit multi-attack --------------------------------------------------------

    [Fact]
    public void Aeonglass_EyeLasers_Deals_11_Twice()
    {
        var m = Monsters.Aeonglass();
        var combat = Combat(m);
        Assert.Equal(22, RunMove(combat, m, "EYE_LASERS_MOVE"));   // 11 x 2
    }

    [Fact]
    public void Aeonglass_EyeLasers_Block_Soaks_Per_Hit()
    {
        var m = Monsters.Aeonglass();
        var combat = Combat(m);
        Cmd.GainBlock(combat, combat.Player, 11, ValueProp.Unpowered, null);   // soaks exactly one hit
        Assert.Equal(11, RunMove(combat, m, "EYE_LASERS_MOVE"));               // one hit blocked, one lands
    }

    // ---- INCREASING_INTENSITY: the ramp --------------------------------------------------------

    [Fact]
    public void Aeonglass_IncreasingIntensity_First_Use_Spawns_1_Wither_Strength3_NoBlock()
    {
        var m = Monsters.Aeonglass();   // base: WitherAmount 1, base Strength 3
        var combat = Combat(m);
        RunMove(combat, m, "INCREASING_INTENSITY_MOVE");

        // 1 fresh Wither to discard, at level 1 (3 + 3·1 damage).
        Assert.Single(combat.Player.DiscardPile);
        var w = Assert.IsType<AeonglassWither>(combat.Player.DiscardPile[0]);
        Assert.Equal(1, w.Level);
        Assert.Equal(6, w.Damage);

        Assert.Equal(3, m.GetPowerAmount("Strength"));     // base 3 + AdditionalStrength 0
        Assert.Equal(0, m.Block);                           // 2026-06 patch moved the Block to EBB
        Assert.Equal(1, m.GetPowerAmount(AeonglassIntensityPower.PowerId));   // completed-II count
    }

    [Fact]
    public void Aeonglass_IncreasingIntensity_Strength_Ramps_Triangularly()
    {
        var m = Monsters.Aeonglass();   // base Strength 3
        var combat = Combat(m);
        RunMove(combat, m, "INCREASING_INTENSITY_MOVE");   // +3  -> total 3
        Assert.Equal(3, m.GetPowerAmount("Strength"));
        RunMove(combat, m, "INCREASING_INTENSITY_MOVE");   // +4  -> total 7
        Assert.Equal(7, m.GetPowerAmount("Strength"));
        RunMove(combat, m, "INCREASING_INTENSITY_MOVE");   // +5  -> total 12
        Assert.Equal(12, m.GetPowerAmount("Strength"));
        Assert.Equal(3, m.GetPowerAmount(AeonglassIntensityPower.PowerId));
    }

    [Fact]
    public void Aeonglass_IncreasingIntensity_FakeUpgrades_Existing_Withers_In_All_Piles()
    {
        var m = Monsters.Aeonglass();
        var combat = Combat(m);
        // Seed a level-0 Wither in each pile.
        combat.Player.Hand.Add(new AeonglassWither { Level = 0 });
        combat.Player.DrawPile.Add(new AeonglassWither { Level = 0 });
        combat.Player.DiscardPile.Add(new AeonglassWither { Level = 0 });
        combat.Player.ExhaustPile.Add(new AeonglassWither { Level = 0 });

        RunMove(combat, m, "INCREASING_INTENSITY_MOVE");   // -> everything to level 1
        Assert.Equal(1, ((AeonglassWither)combat.Player.Hand[0]).Level);
        Assert.Equal(1, ((AeonglassWither)combat.Player.DrawPile[0]).Level);
        Assert.Equal(1, ((AeonglassWither)combat.Player.ExhaustPile[0]).Level);
        // Discard pile now holds the bumped seed (level 1) plus the fresh one (level 1).
        Assert.All(combat.Player.DiscardPile.Cast<AeonglassWither>(), w => Assert.Equal(1, w.Level));

        RunMove(combat, m, "INCREASING_INTENSITY_MOVE");   // -> everything to level 2
        Assert.Equal(2, ((AeonglassWither)combat.Player.Hand[0]).Level);
        Assert.All(combat.Player.DiscardPile.Cast<AeonglassWither>(), w => Assert.Equal(2, w.Level));
    }

    [Fact]
    public void Aeonglass_IncreasingIntensity_Deadly_Spawns_2_Withers_BaseStrength4()
    {
        var m = Monsters.Aeonglass(ascension: 9);   // WitherAmount 2, base Strength 4
        var combat = Combat(m);
        RunMove(combat, m, "INCREASING_INTENSITY_MOVE");
        Assert.Equal(2, combat.Player.DiscardPile.Count);
        Assert.All(combat.Player.DiscardPile.Cast<AeonglassWither>(), w => Assert.Equal(1, w.Level));
        Assert.Equal(4, m.GetPowerAmount("Strength"));   // base 4 + 0
    }

    // ---- WITHER turn-end-in-hand chip damage (base 3 + 3·level) ---------------------------------

    [Fact]
    public void Aeonglass_Wither_In_Hand_Deals_3_At_Turn_End_Then_Ramps()
    {
        var m = Monsters.Aeonglass();
        var combat = Combat(m);
        combat.Player.Hand.Add(new AeonglassWither { Level = 0 });
        int before = combat.Player.CurrentHp;
        CombatManager.EndPlayerTurn(combat);
        Assert.Equal(3, before - combat.Player.CurrentHp);   // level 0 -> 3

        var w2 = new AeonglassWither { Level = 2 };           // a ramped Wither
        Assert.Equal(9, w2.Damage);                           // 3 + 3·2
    }

    [Fact]
    public void Aeonglass_Wither_In_Hand_Is_Blockable()
    {
        var m = Monsters.Aeonglass();
        var combat = Combat(m);
        combat.Player.Hand.Add(new AeonglassWither { Level = 1 });   // 6 damage
        Cmd.GainBlock(combat, combat.Player, 6, ValueProp.Unpowered, null);
        int before = combat.Player.CurrentHp;
        CombatManager.EndPlayerTurn(combat);
        Assert.Equal(0, before - combat.Player.CurrentHp);   // fully blocked
    }

    // ---- WITHERING PRESENCE: every 6 player cards played -> 1 Wither to hand --------------------

    [Fact]
    public void Aeonglass_WitheringPresence_Spawns_Wither_Every_6_Cards()
    {
        var m = Monsters.Aeonglass();
        var combat = Combat(m);
        var presence = (AeonglassWitheringPresencePower)m.GetPower(AeonglassWitheringPresencePower.PowerId)!;
        var dummy = new DefendIronclad();

        for (int i = 0; i < 5; i++) presence.AfterCardPlayed(combat, dummy);
        Assert.Empty(combat.Player.Hand);              // not yet
        Assert.Equal(1, presence.Amount);              // CardsLeft 1

        presence.AfterCardPlayed(combat, dummy);       // 6th card
        Assert.Single(combat.Player.Hand);             // one Wither added to HAND
        Assert.IsType<AeonglassWither>(combat.Player.Hand[0]);
        Assert.Equal(6, presence.Amount);              // counter reset to 6
    }

    [Fact]
    public void Aeonglass_WitheringPresence_Wither_Opens_At_Current_Level()
    {
        var m = Monsters.Aeonglass();
        var combat = Combat(m);
        RunMove(combat, m, "INCREASING_INTENSITY_MOVE");   // bumps the fake-upgrade level to 1
        var presence = (AeonglassWitheringPresencePower)m.GetPower(AeonglassWitheringPresencePower.PowerId)!;
        var dummy = new DefendIronclad();
        for (int i = 0; i < 6; i++) presence.AfterCardPlayed(combat, dummy);
        var w = Assert.IsType<AeonglassWither>(combat.Player.Hand.Single());
        Assert.Equal(1, w.Level);   // pre-upgraded to the boss's current WitherUpgradeCount
    }

    [Fact]
    public void Aeonglass_WitheringPresence_Counts_Real_Card_Plays()
    {
        var m = Monsters.Aeonglass();
        var player = new Player { Name = "P", CurrentHp = 9999, MaxHp = 9999, MaxEnergy = 99 };
        for (int i = 0; i < 6; i++) player.Hand.Add(new DefendIronclad());
        var combat = Catalog.SetupCombat(player, new[] { m });
        combat.Player.ResetEnergy();

        foreach (var c in combat.Player.Hand.ToList())
            CombatManager.PlayCard(combat, c, combat.Player);

        // After 6 plays, exactly one Wither has been added to the hand.
        Assert.Single(combat.Player.Hand.OfType<AeonglassWither>());
    }

    // ---- Wither card identity (stateful) clones / keys by level ---------------------------------

    [Fact]
    public void Aeonglass_Wither_StateKey_Encodes_Level_And_Clones_Deep()
    {
        var w = new AeonglassWither { Level = 2 };
        Assert.Equal("Wither+2", w.StateKey());
        Assert.Equal("Wither", new AeonglassWither { Level = 0 }.StateKey());
        var c = (AeonglassWither)w.Clone();
        c.Level = 5;
        Assert.Equal(2, w.Level);   // original unaffected (deep clone)
    }
}
