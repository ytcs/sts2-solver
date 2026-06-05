using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>
/// Unit tests for the Act-2 Overgrowth BOSS Vantom. Assert HP scaling (Tough), per-move damage/effects (Deadly),
/// the deterministic AI move chain/intents, the Dismember 3-Wound generation, the Prepare self-Strength buff (and
/// that it amplifies later attacks), and the soundness-critical Slippery cap (each incoming hit capped to 1, one
/// charge burned per connecting hit) — straight from the decompiled game source. Mirrors SoulFyshTests.
/// </summary>
public class VantomTests
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
    [InlineData(0, 173)]   // base
    [InlineData(7, 173)]   // below ToughEnemies (>=8)
    [InlineData(8, 183)]   // ToughEnemies
    [InlineData(10, 183)]
    public void Vantom_Hp_Scales_With_Tough(int asc, int expectedHp)
    {
        var m = Monsters.Vantom(ascension: asc);
        Assert.Equal(expectedHp, m.MaxHp);
        Assert.Equal(expectedHp, m.CurrentHp);
    }

    [Fact]
    public void Vantom_Hp_Override_Respected()
    {
        var m = Monsters.Vantom(hp: 123, ascension: 8);
        Assert.Equal(123, m.MaxHp);
        Assert.Equal(123, m.CurrentHp);
    }

    // ---- Slippery is applied at construction and Tough-scaled -----------------------------------

    [Theory]
    [InlineData(0, 8)]
    [InlineData(7, 8)]
    [InlineData(8, 9)]
    [InlineData(20, 9)]
    public void Vantom_Opens_With_Slippery(int asc, int expectedAmt)
    {
        var m = Monsters.Vantom(ascension: asc);
        Assert.Equal(expectedAmt, m.GetPowerAmount("Slippery"));
    }

    // ---- AI chain & intents --------------------------------------------------------------------

    [Fact]
    public void Vantom_Opens_On_InkBlot_Then_Loops_InkyLance_Dismember_Prepare()
    {
        var m = Monsters.Vantom();
        Assert.Equal("INK_BLOT_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);

        Assert.Equal("INKY_LANCE_MOVE", Move(m, "INK_BLOT_MOVE").FollowUp!.Id);
        Assert.Equal("DISMEMBER_MOVE", Move(m, "INKY_LANCE_MOVE").FollowUp!.Id);
        Assert.Equal("PREPARE_MOVE", Move(m, "DISMEMBER_MOVE").FollowUp!.Id);
        Assert.Equal("INK_BLOT_MOVE", Move(m, "PREPARE_MOVE").FollowUp!.Id);   // loop back
    }

    [Fact]
    public void Vantom_Intents_Match_Spec()
    {
        var m = Monsters.Vantom();
        Assert.Equal(7, Move(m, "INK_BLOT_MOVE").IntentDamage);
        Assert.Equal(6, Move(m, "INKY_LANCE_MOVE").IntentDamage);
        Assert.Equal(2, Move(m, "INKY_LANCE_MOVE").IntentHits);
        Assert.Equal(26, Move(m, "DISMEMBER_MOVE").IntentDamage);
        Assert.Null(Move(m, "PREPARE_MOVE").IntentDamage);   // pure self-buff
    }

    // ---- Move effects (base ascension) ---------------------------------------------------------

    [Fact]
    public void Vantom_InkBlot_Deals_7_No_Effects()
    {
        var m = Monsters.Vantom();
        var combat = Combat(m);
        Assert.Equal(7, RunMove(combat, m, "INK_BLOT_MOVE"));
        Assert.Empty(combat.Player.Powers);
        Assert.Empty(combat.Player.DiscardPile);
    }

    [Fact]
    public void Vantom_InkyLance_Deals_6_Twice()
    {
        var m = Monsters.Vantom();
        var combat = Combat(m);
        Assert.Equal(12, RunMove(combat, m, "INKY_LANCE_MOVE"));   // 6 x 2
    }

    [Fact]
    public void Vantom_InkyLance_Block_Soaks_Per_Hit()
    {
        var m = Monsters.Vantom();
        var combat = Combat(m);
        Cmd.GainBlock(combat, combat.Player, 6, ValueProp.Unpowered, null);   // soaks exactly one hit
        Assert.Equal(6, RunMove(combat, m, "INKY_LANCE_MOVE"));               // one 6-hit blocked, one lands
    }

    [Fact]
    public void Vantom_Dismember_Deals_26_And_Adds_3_Wounds_To_Discard()
    {
        var m = Monsters.Vantom();
        var combat = Combat(m);
        Assert.Equal(26, RunMove(combat, m, "DISMEMBER_MOVE"));
        Assert.Equal(3, combat.Player.DiscardPile.Count);
        Assert.All(combat.Player.DiscardPile, c => Assert.IsType<Wound>(c));
    }

    [Fact]
    public void Vantom_Prepare_Grants_Itself_2_Strength_No_Player_Effect()
    {
        var m = Monsters.Vantom();
        var combat = Combat(m);
        Assert.Equal(0, RunMove(combat, m, "PREPARE_MOVE"));
        Assert.Equal(2, m.GetPowerAmount("Strength"));
        Assert.Empty(combat.Player.Powers);
    }

    [Fact]
    public void Vantom_Prepare_Strength_Amplifies_Later_Attacks()
    {
        var m = Monsters.Vantom();
        var combat = Combat(m);
        RunMove(combat, m, "PREPARE_MOVE");                  // +2 Strength
        Assert.Equal(9, RunMove(combat, m, "INK_BLOT_MOVE"));    // 7 + 2
        Assert.Equal(16, RunMove(combat, m, "INKY_LANCE_MOVE")); // (6+2) x 2
        Assert.Equal(28, RunMove(combat, m, "DISMEMBER_MOVE"));  // 26 + 2
    }

    // ---- Slippery: caps each incoming hit to 1 and burns a charge per connecting hit -----------

    [Fact]
    public void Vantom_Slippery_Caps_Each_Hit_To_1()
    {
        var m = Monsters.Vantom();   // base: Slippery 8
        var combat = Combat(m);
        int before = m.CurrentHp;
        Cmd.Attack(combat, combat.Player, m, 50, ValueProp.Move, null);   // huge hit -> capped to 1
        Assert.Equal(1, before - m.CurrentHp);
        Assert.Equal(7, m.GetPowerAmount("Slippery"));                    // one charge burned
    }

    [Fact]
    public void Vantom_Slippery_Burns_One_Charge_Per_Connecting_Hit_Then_Takes_Full()
    {
        var m = Monsters.Vantom();   // base: Slippery 8
        var combat = Combat(m);
        int start = m.CurrentHp;
        for (int i = 0; i < 8; i++)
            Cmd.Attack(combat, combat.Player, m, 50, ValueProp.Move, null);   // 8 capped hits -> 8 total damage
        Assert.Equal(0, m.GetPowerAmount("Slippery"));
        Assert.Equal(8, start - m.CurrentHp);

        int afterCharges = m.CurrentHp;
        Cmd.Attack(combat, combat.Player, m, 50, ValueProp.Move, null);       // 9th hit lands FULL
        Assert.Equal(50, afterCharges - m.CurrentHp);
    }

    [Fact]
    public void Vantom_Slippery_Fully_Blocked_Hit_Does_Not_Burn_Charge()
    {
        var m = Monsters.Vantom();
        var combat = Combat(m);
        m.Block = 100;
        Cmd.Attack(combat, combat.Player, m, 5, ValueProp.Move, null);   // fully blocked -> no unblocked damage
        Assert.Equal(8, m.GetPowerAmount("Slippery"));                   // charge preserved
    }

    // ---- Deadly (>=9) ascension scaling ---------------------------------------------------------

    [Fact]
    public void Vantom_Deadly_Damage_Scales()
    {
        var m = Monsters.Vantom(ascension: 9);
        var combat = Combat(m);
        Assert.Equal(8, Move(m, "INK_BLOT_MOVE").IntentDamage);
        Assert.Equal(7, Move(m, "INKY_LANCE_MOVE").IntentDamage);
        Assert.Equal(30, Move(m, "DISMEMBER_MOVE").IntentDamage);
        Assert.Equal(8, RunMove(combat, m, "INK_BLOT_MOVE"));
        Assert.Equal(14, RunMove(combat, m, "INKY_LANCE_MOVE"));  // 7 x 2
        Assert.Equal(30, RunMove(combat, m, "DISMEMBER_MOVE"));
        Assert.Equal(3, combat.Player.DiscardPile.Count);         // Wound count not Deadly-scaled
    }

    // ---- Full-cycle integration: each move resolves in order ------------------------------------

    [Fact]
    public void Vantom_Full_Cycle_Resolves_In_Order()
    {
        var m = Monsters.Vantom();
        var combat = Combat(m);
        Assert.Equal(7, RunMove(combat, m, "INK_BLOT_MOVE"));
        Assert.Equal(12, RunMove(combat, m, "INKY_LANCE_MOVE"));
        Assert.Equal(26, RunMove(combat, m, "DISMEMBER_MOVE"));
        RunMove(combat, m, "PREPARE_MOVE");
        Assert.Equal(3, combat.Player.DiscardPile.Count);   // from Dismember
        Assert.Equal(2, m.GetPowerAmount("Strength"));      // from Prepare
        // After Prepare, the next Ink Blot is amplified.
        Assert.Equal(9, RunMove(combat, m, "INK_BLOT_MOVE"));
    }
}
