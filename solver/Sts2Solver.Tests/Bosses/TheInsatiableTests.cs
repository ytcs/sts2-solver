using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>
/// Unit tests for the Act-3 Hive BOSS TheInsatiable. Assert HP scaling (Tough), per-move damage/effects (Deadly),
/// the deterministic AI move chain/intents (opener LIQUIFY then the THRASH → BITE → SALIVATE loop via the second
/// THRASH id), the Salivate self-Strength ramp (and that it amplifies later attacks), and the soundness-critical
/// SANDPIT death-timer: Liquify applies the 4-counter to the player + seeds 6 FranticEscape (3 draw, 3 discard);
/// the counter decrements each enemy turn; at 0 it KILLS the player; and playing a FranticEscape feeds it +1.
/// Straight from the decompiled game source. Mirrors VantomTests.
/// </summary>
public class TheInsatiableTests
{
    private static CombatState Combat(params Monster[] monsters)
    {
        var player = new Player { Name = "P", CurrentHp = 999, MaxHp = 999, MaxEnergy = 3 };
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
    [InlineData(0, 321)]   // base
    [InlineData(7, 321)]   // below ToughEnemies (>=8)
    [InlineData(8, 341)]   // ToughEnemies
    [InlineData(10, 341)]
    public void Insatiable_Hp_Scales_With_Tough(int asc, int expectedHp)
    {
        var m = Monsters.TheInsatiable(ascension: asc);
        Assert.Equal(expectedHp, m.MaxHp);
        Assert.Equal(expectedHp, m.CurrentHp);
    }

    [Fact]
    public void Insatiable_Hp_Override_Respected()
    {
        var m = Monsters.TheInsatiable(hp: 123, ascension: 8);
        Assert.Equal(123, m.MaxHp);
        Assert.Equal(123, m.CurrentHp);
    }

    // ---- AI chain & intents --------------------------------------------------------------------

    [Fact]
    public void Insatiable_Opens_On_Liquify_Then_Loops_Thrash_Bite_Salivate()
    {
        var m = Monsters.TheInsatiable();
        Assert.Equal("LIQUIFY_GROUND_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);

        Assert.Equal("THRASH_MOVE", Move(m, "LIQUIFY_GROUND_MOVE").FollowUp!.Id);
        Assert.Equal("LUNGING_BITE_MOVE", Move(m, "THRASH_MOVE").FollowUp!.Id);
        Assert.Equal("SALIVATE_MOVE", Move(m, "LUNGING_BITE_MOVE").FollowUp!.Id);
        Assert.Equal("THRASH_MOVE_2", Move(m, "SALIVATE_MOVE").FollowUp!.Id);
        Assert.Equal("THRASH_MOVE", Move(m, "THRASH_MOVE_2").FollowUp!.Id);   // loop closes via the 2nd THRASH id
    }

    [Fact]
    public void Insatiable_Intents_Match_Spec()
    {
        var m = Monsters.TheInsatiable();
        Assert.Null(Move(m, "LIQUIFY_GROUND_MOVE").IntentDamage);   // no direct damage (buff + status)
        Assert.Equal(8, Move(m, "THRASH_MOVE").IntentDamage);
        Assert.Equal(2, Move(m, "THRASH_MOVE").IntentHits);
        Assert.Equal(8, Move(m, "THRASH_MOVE_2").IntentDamage);
        Assert.Equal(2, Move(m, "THRASH_MOVE_2").IntentHits);
        Assert.Equal(28, Move(m, "LUNGING_BITE_MOVE").IntentDamage);
        Assert.Null(Move(m, "SALIVATE_MOVE").IntentDamage);         // pure self-buff
    }

    // ---- Move effects (base ascension) ---------------------------------------------------------

    [Fact]
    public void Insatiable_Liquify_No_Damage_Applies_Sandpit_4_And_Seeds_6_FranticEscape()
    {
        var m = Monsters.TheInsatiable();
        var combat = Combat(m);
        Assert.Equal(0, RunMove(combat, m, "LIQUIFY_GROUND_MOVE"));   // no direct damage
        Assert.Equal(4, combat.Player.GetPowerAmount("InsatiableSandpit"));
        Assert.Equal(3, combat.Player.DrawPile.Count);
        Assert.Equal(3, combat.Player.DiscardPile.Count);
        Assert.All(combat.Player.DrawPile, c => Assert.IsType<FranticEscape>(c));
        Assert.All(combat.Player.DiscardPile, c => Assert.IsType<FranticEscape>(c));
    }

    [Fact]
    public void Insatiable_Thrash_Deals_8_Twice()
    {
        var m = Monsters.TheInsatiable();
        var combat = Combat(m);
        Assert.Equal(16, RunMove(combat, m, "THRASH_MOVE"));      // 8 x 2
        Assert.Equal(16, RunMove(combat, m, "THRASH_MOVE_2"));    // identical second id
    }

    [Fact]
    public void Insatiable_Thrash_Block_Soaks_Per_Hit()
    {
        var m = Monsters.TheInsatiable();
        var combat = Combat(m);
        Cmd.GainBlock(combat, combat.Player, 8, ValueProp.Unpowered, null);   // soaks exactly one hit
        Assert.Equal(8, RunMove(combat, m, "THRASH_MOVE"));                   // one 8-hit blocked, one lands
    }

    [Fact]
    public void Insatiable_Bite_Deals_28_No_Effects()
    {
        var m = Monsters.TheInsatiable();
        var combat = Combat(m);
        Assert.Equal(28, RunMove(combat, m, "LUNGING_BITE_MOVE"));
        Assert.Empty(combat.Player.Powers);
    }

    [Fact]
    public void Insatiable_Salivate_Grants_Itself_2_Strength_No_Player_Effect()
    {
        var m = Monsters.TheInsatiable();
        var combat = Combat(m);
        Assert.Equal(0, RunMove(combat, m, "SALIVATE_MOVE"));
        Assert.Equal(2, m.GetPowerAmount("Strength"));
        Assert.Empty(combat.Player.Powers);
    }

    [Fact]
    public void Insatiable_Salivate_Strength_Amplifies_Later_Attacks_And_Compounds()
    {
        var m = Monsters.TheInsatiable();
        var combat = Combat(m);
        RunMove(combat, m, "SALIVATE_MOVE");                       // +2 Strength
        Assert.Equal(20, RunMove(combat, m, "THRASH_MOVE"));       // (8+2) x 2
        Assert.Equal(30, RunMove(combat, m, "LUNGING_BITE_MOVE")); // 28 + 2
        RunMove(combat, m, "SALIVATE_MOVE");                       // +2 more -> 4 total
        Assert.Equal(24, RunMove(combat, m, "THRASH_MOVE"));       // (8+4) x 2
    }

    // ---- SANDPIT death-timer (soundness-critical) ----------------------------------------------

    [Fact]
    public void Insatiable_Sandpit_Decrements_Once_Per_Enemy_Turn_Start()
    {
        var m = Monsters.TheInsatiable();
        var combat = Combat(m);
        // Apply the timer via Liquify, then keep the boss off attacks so only the decrement is observed.
        RunMove(combat, m, "LIQUIFY_GROUND_MOVE");
        Assert.Equal(4, combat.Player.GetPowerAmount("InsatiableSandpit"));

        m.Ai.CurrentMoveId = "SALIVATE_MOVE";   // a no-damage telegraph
        CombatManager.RunEnemyTurn(combat);
        Assert.Equal(3, combat.Player.GetPowerAmount("InsatiableSandpit"));
        m.Ai.CurrentMoveId = "SALIVATE_MOVE";
        CombatManager.RunEnemyTurn(combat);
        Assert.Equal(2, combat.Player.GetPowerAmount("InsatiableSandpit"));
    }

    [Fact]
    public void Insatiable_Sandpit_Kills_Player_When_Counter_Drains()
    {
        var m = Monsters.TheInsatiable();
        var combat = Combat(m);
        // Seed the timer directly at 1 so the next enemy-turn-start decrement drains it to 0.
        Cmd.ApplyPower(combat, combat.Player, new InsatiableSandpitPower(), 1, m);
        Assert.False(combat.PlayerDead);

        m.Ai.CurrentMoveId = "SALIVATE_MOVE";   // no-damage move; the kill must come from the timer itself
        CombatManager.RunEnemyTurn(combat);

        Assert.True(combat.PlayerDead);                                       // game-over countdown fired
        Assert.Equal(0, combat.Player.CurrentHp);
        Assert.False(combat.Player.HasPower("InsatiableSandpit"));           // timer removed on drain
    }

    [Fact]
    public void Insatiable_Playing_FranticEscape_Feeds_The_Timer_Plus_1()
    {
        var m = Monsters.TheInsatiable();
        var combat = Combat(m);
        Cmd.ApplyPower(combat, combat.Player, new InsatiableSandpitPower(), 4, m);

        // Put a FranticEscape in hand and play it — the game's FranticEscape.OnPlay → Sandpit ModifyAmount(+1).
        var escape = new FranticEscape();
        combat.Player.Hand.Add(escape);
        combat.Player.Energy = 3;
        CombatManager.PlayCard(combat, escape, null);

        Assert.Equal(5, combat.Player.GetPowerAmount("InsatiableSandpit"));   // pushed back up
    }

    [Fact]
    public void Insatiable_Escape_Then_Survive_The_Drain()
    {
        var m = Monsters.TheInsatiable();
        var combat = Combat(m);
        Cmd.ApplyPower(combat, combat.Player, new InsatiableSandpitPower(), 1, m);

        // Player plays a FranticEscape (1 -> 2) before the decrement, so it survives the enemy turn.
        var escape = new FranticEscape();
        combat.Player.Hand.Add(escape);
        combat.Player.Energy = 3;
        CombatManager.PlayCard(combat, escape, null);
        Assert.Equal(2, combat.Player.GetPowerAmount("InsatiableSandpit"));

        m.Ai.CurrentMoveId = "SALIVATE_MOVE";
        CombatManager.RunEnemyTurn(combat);
        Assert.False(combat.PlayerDead);                                      // 2 -> 1, still alive
        Assert.Equal(1, combat.Player.GetPowerAmount("InsatiableSandpit"));
    }

    // ---- FranticEscape COST RAMP (closes the previously-flagged optimistic gap) ----
    [Fact]
    public void Insatiable_FranticEscape_Cost_Ramps_Per_Play()
    {
        var combat = Combat(Monsters.TheInsatiable());
        Cmd.ApplyPower(combat, combat.Player, new InsatiableSandpitPower(), 9, combat.Monsters[0]);
        var escape = new FranticEscape();
        Assert.True(escape.Stateful);
        Assert.Equal(1, escape.EffectiveCost(combat));                       // first play costs 1
        combat.Player.Hand.Add(escape);
        combat.Player.Energy = 9;
        CombatManager.PlayCard(combat, escape, null);                        // play 1 → cost ramps to 2
        Assert.Equal(2, escape.EffectiveCost(combat));
        Assert.Equal(2, CombatManager.ResolveCardCost(combat, escape));      // the ramp gates affordability
        combat.Player.Hand.Add(escape);                                      // status card cycles back; replay it
        CombatManager.PlayCard(combat, escape, null);                        // play 2 → cost ramps to 3
        Assert.Equal(3, escape.EffectiveCost(combat));
    }

    [Fact]
    public void FranticEscape_Cost_Ramp_Survives_Clone_And_Hashes()
    {
        var combat = Combat(Monsters.TheInsatiable());
        var escape = new FranticEscape();
        escape.OnPlay(combat, new CardPlay { Card = escape });               // ramp once
        Assert.Equal("FranticEscape/1", escape.StateKey());                  // counter folded into the hash key
        var clone = (CardModel)escape.Clone();
        Assert.Equal("FranticEscape/1", clone.StateKey());                   // deep-cloned (Stateful) → counter survives
    }

    // ---- Deadly (>=9) ascension scaling ---------------------------------------------------------

    [Fact]
    public void Insatiable_Deadly_Damage_Scales()
    {
        var m = Monsters.TheInsatiable(ascension: 9);
        var combat = Combat(m);
        Assert.Equal(9, Move(m, "THRASH_MOVE").IntentDamage);
        Assert.Equal(31, Move(m, "LUNGING_BITE_MOVE").IntentDamage);
        Assert.Equal(18, RunMove(combat, m, "THRASH_MOVE"));       // 9 x 2
        Assert.Equal(31, RunMove(combat, m, "LUNGING_BITE_MOVE"));
        RunMove(combat, m, "SALIVATE_MOVE");                       // +3 Strength on Deadly
        Assert.Equal(3, m.GetPowerAmount("Strength"));
    }

    // ---- Full-cycle integration: opener then the loop resolves in order -------------------------

    [Fact]
    public void Insatiable_Full_Cycle_Resolves_In_Order()
    {
        var m = Monsters.TheInsatiable();
        var combat = Combat(m);
        Assert.Equal(0, RunMove(combat, m, "LIQUIFY_GROUND_MOVE"));   // opener: no damage
        Assert.Equal(4, combat.Player.GetPowerAmount("InsatiableSandpit"));
        Assert.Equal(16, RunMove(combat, m, "THRASH_MOVE"));          // 8 x 2
        Assert.Equal(28, RunMove(combat, m, "LUNGING_BITE_MOVE"));
        RunMove(combat, m, "SALIVATE_MOVE");                          // +2 Strength
        Assert.Equal(2, m.GetPowerAmount("Strength"));
        Assert.Equal(20, RunMove(combat, m, "THRASH_MOVE_2"));        // (8+2) x 2 — Salivate ramp shows
    }
}
