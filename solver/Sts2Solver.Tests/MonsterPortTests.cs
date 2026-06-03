using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Unit tests for the Act-1 (Overgrowth) NORMAL/WEAK monster ports. These assert HP scaling (Tough),
/// per-move damage/effects (Deadly) and intents straight from the decompiled game source. They are
/// UNIT-TESTED ONLY — there are no recorded combat traces for these monsters yet, so they are NOT
/// trace-validated (unlike the elites). Mirrors the structure of SolverTests / CardTests.
/// </summary>
public class MonsterPortTests
{
    private readonly ITestOutputHelper _out;
    public MonsterPortTests(ITestOutputHelper o) => _out = o;

    // ---- helpers -------------------------------------------------------------------------------

    /// <summary>A fresh combat with one big-HP player (so monster hits never kill) and the given monsters.</summary>
    private static CombatState Combat(params Monster[] monsters)
    {
        var player = new Player { Name = "P", CurrentHp = 999, MaxHp = 999, MaxEnergy = 3 };
        return Catalog.SetupCombat(player, monsters);
    }

    /// <summary>Force <paramref name="m"/> to telegraph <paramref name="moveId"/>, resolve it, and return
    /// the player HP lost by that single move.</summary>
    private static int RunMove(CombatState combat, Monster m, string moveId)
    {
        m.Ai.CurrentMoveId = moveId;
        int before = combat.Player.CurrentHp;
        m.PerformCurrentMove(combat);
        return before - combat.Player.CurrentHp;
    }

    private static MoveState Move(Monster m, string id) => (MoveState)m.Ai.States[id];

    // ---- HP scaling ----------------------------------------------------------------------------

    // HP defaults to the MAX roll of the game's range (project convention; trace replay injects the
    // actual rolled HP). The Tough range shifts the whole range up at ascension >= 8.
    [Theory]
    [InlineData(0, 33)]   // base max
    [InlineData(8, 36)]   // ToughEnemies max
    public void SnappingJaxfruit_Hp_Scales(int asc, int expectedHp)
        => Assert.Equal(expectedHp, Monsters.SnappingJaxfruit(ascension: asc).MaxHp);

    [Theory]
    [InlineData(0, 49)]
    [InlineData(8, 53)]
    public void Flyconid_Hp_Scales(int asc, int expectedHp)
        => Assert.Equal(expectedHp, Monsters.Flyconid(ascension: asc).MaxHp);

    [Theory]
    [InlineData(0, 65)]
    [InlineData(8, 70)]
    public void CubexConstruct_Hp_Scales(int asc, int expectedHp)
        => Assert.Equal(expectedHp, Monsters.CubexConstruct(ascension: asc).MaxHp);

    [Theory]
    [InlineData(0, 57)]
    [InlineData(8, 59)]
    public void FuzzyWurmCrawler_Hp_Scales(int asc, int expectedHp)
        => Assert.Equal(expectedHp, Monsters.FuzzyWurmCrawler(ascension: asc).MaxHp);

    [Theory]
    [InlineData(0, 40)]
    [InlineData(8, 42)]
    public void ShrinkerBeetle_Hp_Scales(int asc, int expectedHp)
        => Assert.Equal(expectedHp, Monsters.ShrinkerBeetle(ascension: asc).MaxHp);

    [Theory]
    [InlineData(0, 72)]
    [InlineData(8, 76)]
    public void Mawler_Hp_Scales(int asc, int expectedHp)
        => Assert.Equal(expectedHp, Monsters.Mawler(ascension: asc).MaxHp);

    [Theory]
    [InlineData(0, 46)]
    [InlineData(8, 48)]
    public void Nibbit_Hp_Scales(int asc, int expectedHp)
        => Assert.Equal(expectedHp, Monsters.Nibbit(ascension: asc).MaxHp);

    [Theory]
    [InlineData(0, 17)]
    [InlineData(8, 18)]
    public void Inklet_Hp_Scales(int asc, int expectedHp)
        => Assert.Equal(expectedHp, Monsters.Inklet(ascension: asc).MaxHp);

    // ---- SnappingJaxfruit -----------------------------------------------------------------------

    [Fact]
    public void SnappingJaxfruit_Opens_On_EnergyOrb_And_Ramps_Strength()
    {
        var m = Monsters.SnappingJaxfruit();
        var combat = Combat(m);
        var open = m.Ai.EnumerateInitial(m).Single();
        Assert.Equal("ENERGY_ORB_MOVE", open.moveId);
        Assert.Equal(3, Move(m, "ENERGY_ORB_MOVE").IntentDamage);

        Assert.Equal(3, RunMove(combat, m, "ENERGY_ORB_MOVE"));     // 3 base
        Assert.Equal(2, m.GetPowerAmount("Strength"));
        Assert.Equal(5, RunMove(combat, m, "ENERGY_ORB_MOVE"));     // 3 + 2 Strength
        Assert.Equal(4, m.GetPowerAmount("Strength"));
    }

    [Fact]
    public void SnappingJaxfruit_Deadly_Hits_For_4()
    {
        var m = Monsters.SnappingJaxfruit(ascension: 9);
        Assert.Equal(4, Move(m, "ENERGY_ORB_MOVE").IntentDamage);
        Assert.Equal(4, RunMove(Combat(m), m, "ENERGY_ORB_MOVE"));
    }

    // ---- Flyconid -------------------------------------------------------------------------------

    [Fact]
    public void Flyconid_Opener_Is_FrailSpores_Or_Smash_Weighted_2_1()
    {
        var m = Monsters.Flyconid();
        var open = m.Ai.EnumerateInitial(m).ToDictionary(o => o.moveId, o => o.prob);
        Assert.Equal(2, open.Count);
        Assert.True(open.ContainsKey("FRAIL_SPORES_MOVE") && open.ContainsKey("SMASH_MOVE"));
        Assert.Equal(2.0 / 3.0, open["FRAIL_SPORES_MOVE"], 6);
        Assert.Equal(1.0 / 3.0, open["SMASH_MOVE"], 6);
    }

    [Fact]
    public void Flyconid_Moves_Have_Right_Damage_And_Effects()
    {
        var m = Monsters.Flyconid();
        var combat = Combat(m);
        Assert.Null(Move(m, "VULNERABLE_SPORES_MOVE").IntentDamage);
        Assert.Equal(8, Move(m, "FRAIL_SPORES_MOVE").IntentDamage);
        Assert.Equal(11, Move(m, "SMASH_MOVE").IntentDamage);

        // FrailSpores on a clean player: 8 base damage + apply 2 Frail.
        int dealt = RunMove(combat, m, "FRAIL_SPORES_MOVE");
        Assert.Equal(8, dealt);
        Assert.Equal(2, combat.Player.GetPowerAmount("Frail"));

        // VulnerableSpores is a pure debuff (applies 2 Vulnerable, no damage).
        var fresh = Combat(Monsters.Flyconid());
        var m2 = (Monster)fresh.Monsters[0];
        Assert.Equal(0, RunMove(fresh, m2, "VULNERABLE_SPORES_MOVE"));
        Assert.Equal(2, fresh.Player.GetPowerAmount("Vulnerable"));
    }

    [Fact]
    public void Flyconid_Loop_Is_Vuln3_Frail2_Smash1_NoRepeat()
    {
        var m = Monsters.Flyconid();
        m.Ai.CurrentMoveId = "SMASH_MOVE";          // any non-repeating move
        m.Ai.MoveLog.Add("SMASH_MOVE");
        var next = m.Ai.EnumerateNext(m).ToDictionary(o => o.moveId, o => o.prob);
        // Smash cannot repeat, so only Vulnerable(3) + Frail(2) remain -> 3/5 and 2/5.
        Assert.Equal(3.0 / 5.0, next["VULNERABLE_SPORES_MOVE"], 6);
        Assert.Equal(2.0 / 5.0, next["FRAIL_SPORES_MOVE"], 6);
        Assert.False(next.ContainsKey("SMASH_MOVE"));
    }

    // ---- CubexConstruct -------------------------------------------------------------------------

    [Fact]
    public void CubexConstruct_Starts_With_Block_And_Artifact()
    {
        var m = Monsters.CubexConstruct();
        Assert.Equal(13, m.Block);
        Assert.Equal(1, m.GetPowerAmount("Artifact"));
        Assert.Equal("CHARGE_UP_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);
    }

    [Fact]
    public void CubexConstruct_Chain_And_Ramping_Blast()
    {
        var m = Monsters.CubexConstruct();
        var combat = Combat(m);
        Assert.Equal(0, RunMove(combat, m, "CHARGE_UP_MOVE"));      // pure buff
        Assert.Equal(2, m.GetPowerAmount("Strength"));
        Assert.Equal(9, RunMove(combat, m, "REPEATER_BLAST_MOVE")); // 7 + 2 Str, then +2 Str
        Assert.Equal(4, m.GetPowerAmount("Strength"));
        Assert.Equal(11, RunMove(combat, m, "REPEATER_BLAST_MOVE_2")); // 7 + 4 Str
        Assert.Equal(6, m.GetPowerAmount("Strength"));
        // Expel is 5x2 (+ Strength on each of the 2 hits): (5+6)*2 = 22
        Assert.Equal(22, RunMove(combat, m, "EXPEL_MOVE"));
        Assert.Equal(2, Move(m, "EXPEL_MOVE").IntentHits);
    }

    [Fact]
    public void CubexConstruct_FollowUps_Loop_Through_Blasts()
    {
        var m = Monsters.CubexConstruct();
        Assert.Equal("REPEATER_BLAST_MOVE", Move(m, "CHARGE_UP_MOVE").FollowUp!.Id);
        Assert.Equal("REPEATER_BLAST_MOVE_2", Move(m, "REPEATER_BLAST_MOVE").FollowUp!.Id);
        Assert.Equal("EXPEL_MOVE", Move(m, "REPEATER_BLAST_MOVE_2").FollowUp!.Id);
        Assert.Equal("REPEATER_BLAST_MOVE", Move(m, "EXPEL_MOVE").FollowUp!.Id);
    }

    // ---- FuzzyWurmCrawler -----------------------------------------------------------------------

    [Fact]
    public void FuzzyWurmCrawler_Goop_Inhale_Goop_Cycle_Ramps()
    {
        var m = Monsters.FuzzyWurmCrawler();
        var combat = Combat(m);
        Assert.Equal("FIRST_ACID_GOOP", m.Ai.EnumerateInitial(m).Single().moveId);
        Assert.Equal(4, RunMove(combat, m, "FIRST_ACID_GOOP"));    // 4 base
        Assert.Equal(0, RunMove(combat, m, "INHALE"));             // +7 Strength
        Assert.Equal(7, m.GetPowerAmount("Strength"));
        Assert.Equal(11, RunMove(combat, m, "ACID_GOOP"));         // 4 + 7 Str
        // follow-up chain: FIRST->INHALE->ACID->FIRST
        Assert.Equal("INHALE", Move(m, "FIRST_ACID_GOOP").FollowUp!.Id);
        Assert.Equal("ACID_GOOP", Move(m, "INHALE").FollowUp!.Id);
        Assert.Equal("FIRST_ACID_GOOP", Move(m, "ACID_GOOP").FollowUp!.Id);
    }

    // ---- ShrinkerBeetle + ShrinkPower -----------------------------------------------------------

    [Fact]
    public void ShrinkerBeetle_Opens_Shrink_Then_Chomp_Stomp_Loop()
    {
        var m = Monsters.ShrinkerBeetle();
        Assert.Equal("SHRINKER_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);
        Assert.Equal("CHOMP_MOVE", Move(m, "SHRINKER_MOVE").FollowUp!.Id);
        Assert.Equal("STOMP_MOVE", Move(m, "CHOMP_MOVE").FollowUp!.Id);
        Assert.Equal("CHOMP_MOVE", Move(m, "STOMP_MOVE").FollowUp!.Id);
        Assert.Equal(7, Move(m, "CHOMP_MOVE").IntentDamage);
        Assert.Equal(13, Move(m, "STOMP_MOVE").IntentDamage);
    }

    [Fact]
    public void ShrinkerBeetle_Shrink_Cuts_Player_Powered_Attack_Damage_By_30Pct()
    {
        var m = Monsters.ShrinkerBeetle();
        var combat = Combat(m);
        RunMove(combat, m, "SHRINKER_MOVE");
        Assert.Equal(-1, combat.Player.GetPowerAmount("Shrink"));   // infinite

        // A 10-damage powered player attack should land for 7 (x0.7) on the beetle.
        int hpBefore = m.CurrentHp;
        Cmd.Attack(combat, combat.Player, m, 10, ValueProp.Move, null);
        Assert.Equal(7, hpBefore - m.CurrentHp);
    }

    [Fact]
    public void Shrink_Does_Not_Affect_Unpowered_Damage()
    {
        var m = Monsters.ShrinkerBeetle();
        var combat = Combat(m);
        RunMove(combat, m, "SHRINKER_MOVE");
        int hpBefore = m.CurrentHp;
        Cmd.Attack(combat, combat.Player, m, 10, ValueProp.Unpowered, null);
        Assert.Equal(10, hpBefore - m.CurrentHp);   // unpowered: full damage
    }

    // ---- Mawler ---------------------------------------------------------------------------------

    [Fact]
    public void Mawler_Opens_On_Claw_4x2()
    {
        var m = Monsters.Mawler();
        Assert.Equal("CLAW_MOVE", m.Ai.EnumerateInitial(m).Single().moveId);
        Assert.Equal(4, Move(m, "CLAW_MOVE").IntentDamage);
        Assert.Equal(2, Move(m, "CLAW_MOVE").IntentHits);
        Assert.Equal(8, RunMove(Combat(m), m, "CLAW_MOVE"));   // 4x2
    }

    [Fact]
    public void Mawler_Roar_Applies_3_Vulnerable_Once_Only()
    {
        var m = Monsters.Mawler();
        var combat = Combat(m);
        RunMove(combat, m, "ROAR_MOVE");
        Assert.Equal(3, combat.Player.GetPowerAmount("Vulnerable"));

        // After Roar has been used, the random branch must never offer Roar again (UseOnlyOnce).
        m.Ai.CurrentMoveId = "RIP_AND_TEAR_MOVE";
        m.Ai.MoveLog.Add("ROAR_MOVE");
        m.Ai.MoveLog.Add("RIP_AND_TEAR_MOVE");
        var next = m.Ai.EnumerateNext(m).Select(o => o.moveId).ToHashSet();
        Assert.DoesNotContain("ROAR_MOVE", next);
        Assert.Contains("CLAW_MOVE", next);
    }

    [Fact]
    public void Mawler_RipAndTear_Deadly_Is_16()
    {
        var m = Monsters.Mawler(ascension: 9);
        Assert.Equal(16, Move(m, "RIP_AND_TEAR_MOVE").IntentDamage);
        Assert.Equal(16, RunMove(Combat(m), m, "RIP_AND_TEAR_MOVE"));
    }

    // ---- Nibbit ---------------------------------------------------------------------------------

    [Theory]
    [InlineData("alone", "BUTT_MOVE")]
    [InlineData("front", "SLICE_MOVE")]
    [InlineData("back", "HISS_MOVE")]
    public void Nibbit_Opener_Depends_On_Slot(string slot, string expected)
        => Assert.Equal(expected, Monsters.Nibbit(slot: slot).Ai.EnumerateInitial(Monsters.Nibbit(slot: slot)).Single().moveId);

    [Fact]
    public void Nibbit_Cycle_And_Slice_Gains_Block()
    {
        var m = Monsters.Nibbit(slot: "alone");
        var combat = Combat(m);
        Assert.Equal(12, RunMove(combat, m, "BUTT_MOVE"));
        Assert.Equal(6, RunMove(combat, m, "SLICE_MOVE"));     // 6 damage
        Assert.Equal(5, m.Block);                              // + 5 block
        Assert.Equal(0, RunMove(combat, m, "HISS_MOVE"));      // pure buff
        Assert.Equal(2, m.GetPowerAmount("Strength"));
        Assert.Equal("SLICE_MOVE", Move(m, "BUTT_MOVE").FollowUp!.Id);
        Assert.Equal("HISS_MOVE", Move(m, "SLICE_MOVE").FollowUp!.Id);
        Assert.Equal("BUTT_MOVE", Move(m, "HISS_MOVE").FollowUp!.Id);
    }

    // ---- Inklet + SlipperyPower -----------------------------------------------------------------

    [Fact]
    public void Inklet_Starts_With_Slippery_And_Side_Opener_Is_Jab2_Whirlwind1()
    {
        var m = Monsters.Inklet(middle: false);
        Assert.Equal(1, m.GetPowerAmount("Slippery"));
        var open = m.Ai.EnumerateInitial(m).ToDictionary(o => o.moveId, o => o.prob);
        Assert.Equal(2.0 / 3.0, open["JAB_MOVE"], 6);
        Assert.Equal(1.0 / 3.0, open["WHIRLWIND_MOVE"], 6);
    }

    [Fact]
    public void Inklet_Middle_Opens_On_Whirlwind()
        => Assert.Equal("WHIRLWIND_MOVE", Monsters.Inklet(middle: true).Ai.EnumerateInitial(Monsters.Inklet(middle: true)).Single().moveId);

    [Fact]
    public void Inklet_Slippery_Caps_First_Hit_To_1_Then_Wears_Off()
    {
        var m = Monsters.Inklet();
        var player = new Player { Name = "P", CurrentHp = 999, MaxHp = 999, MaxEnergy = 3 };
        var combat = Catalog.SetupCombat(player, new[] { m });

        int hp0 = m.CurrentHp;
        Cmd.Attack(combat, player, m, 9, ValueProp.Move, null);   // first hit -> capped to 1
        Assert.Equal(1, hp0 - m.CurrentHp);
        Assert.Equal(0, m.GetPowerAmount("Slippery"));            // consumed

        int hp1 = m.CurrentHp;
        Cmd.Attack(combat, player, m, 9, ValueProp.Move, null);   // second hit -> full damage
        Assert.Equal(9, hp1 - m.CurrentHp);
    }

    [Fact]
    public void Inklet_AfterJab_Branches_Gaze_Or_Whirlwind()
    {
        var m = Monsters.Inklet();
        m.Ai.CurrentMoveId = "JAB_MOVE";
        m.Ai.MoveLog.Add("JAB_MOVE");
        var next = m.Ai.EnumerateNext(m).ToDictionary(o => o.moveId, o => o.prob);
        Assert.Equal(0.5, next["PIERCING_GAZE_MOVE"], 6);
        Assert.Equal(0.5, next["WHIRLWIND_MOVE"], 6);
        Assert.Equal(10, Move(m, "PIERCING_GAZE_MOVE").IntentDamage);
        Assert.Equal(2, Move(m, "WHIRLWIND_MOVE").IntentDamage);
        Assert.Equal(3, Move(m, "WHIRLWIND_MOVE").IntentHits);
    }

    // ---- Encounter catalog ----------------------------------------------------------------------

    [Theory]
    [InlineData("SnappingJaxfruitNormal", 2)]
    [InlineData("CubexConstructNormal", 1)]
    [InlineData("FuzzyWurmCrawlerWeak", 1)]
    [InlineData("ShrinkerBeetleWeak", 1)]
    [InlineData("OvergrowthCrawlers", 2)]
    [InlineData("MawlerNormal", 1)]
    [InlineData("NibbitsWeak", 1)]
    [InlineData("NibbitsNormal", 2)]
    [InlineData("InkletsNormal", 3)]
    public void Normal_Encounters_Build_With_Right_Monster_Count(string name, int count)
    {
        Assert.True(Catalog.IsKnownNormalEncounter(name));
        Assert.True(Catalog.IsKnownEncounter(name));
        var monsters = Catalog.BuildEncounter(name);
        Assert.Equal(count, monsters.Count);
        foreach (var mon in monsters)
            Assert.True(mon.MaxHp > 0 && mon.CurrentHp == mon.MaxHp);

        // Encounter-order ids are assigned at combat setup (mirrors the elite path).
        var combat = Catalog.SetupCombat(new Player { Name = "P", CurrentHp = 1, MaxHp = 1 }, monsters);
        for (int i = 0; i < combat.Monsters.Count; i++)
            Assert.Equal(i + 1, combat.Monsters[i].Id);
    }

    [Fact]
    public void NibbitsNormal_Is_Front_Then_Back()
    {
        var monsters = Catalog.BuildEncounter("NibbitsNormal");
        Assert.Equal("SLICE_MOVE", monsters[0].Ai.EnumerateInitial((Monster)monsters[0]).Single().moveId);
        Assert.Equal("HISS_MOVE", monsters[1].Ai.EnumerateInitial((Monster)monsters[1]).Single().moveId);
    }

    [Fact]
    public void Monster_Factories_Are_Registered_In_Catalog()
    {
        foreach (var name in new[] { "SnappingJaxfruit", "Flyconid", "CubexConstruct",
            "FuzzyWurmCrawler", "ShrinkerBeetle", "Mawler", "Nibbit", "Inklet" })
        {
            var m = Catalog.BuildMonster(name);
            Assert.Equal(name, m.Name);
            Assert.True(m.MaxHp > 0);
        }
    }

    // ---- Decimillipede Reattach: a downed segment skips one enemy turn, then reattaches to 25 ----

    /// <summary>A segment downed (0 HP) while another lives is DOWNED, not killed: untargetable, non-Reattach
    /// powers stripped, board NOT cleared. It skips one enemy turn (DEAD_MOVE), then reattaches to 25 on the
    /// next (REATTACH_MOVE) — so a piecemeal kill across turns can't clear the board (closes the optimistic gap).</summary>
    [Fact]
    public void Reattach_Downed_Segment_Skips_A_Turn_Then_Reattaches_To_25()
    {
        var a = Monsters.DecimillipedeSegment("DecimillipedeSegment", starterIdx: 0, hp: 46);
        var b = Monsters.DecimillipedeSegment("DecimillipedeSegment", starterIdx: 1, hp: 46);
        var combat = Combat(a, b);
        CombatManager.BeginPlayerTurn(combat);
        a.AddPower(new StrengthPower(), 2);                // a gained Strength from a Bulk move

        Cmd.Attack(combat, combat.Player, a, 100, ValueProp.Move, null);   // down segment A
        Assert.Equal(0, a.CurrentHp);
        Assert.False(combat.AllMonstersDead);              // B alive ⇒ not a clear
        Assert.Equal(2, a.ReattachIn);                     // scheduled to reattach
        Assert.False(a.HasPower("Strength"));              // non-Reattach powers stripped on downing
        Assert.True(a.HasPower("Reattach"));               // Reattach survives
        Assert.DoesNotContain(a, combat.HittableEnemies);  // untargetable while downed

        CombatManager.EndPlayerTurn(combat);
        CombatManager.RunEnemyTurn(combat);                // enemy turn 1: DEAD_MOVE (skip)
        Assert.Equal(0, a.CurrentHp);
        Assert.Equal(1, a.ReattachIn);

        CombatManager.BeginPlayerTurn(combat);
        CombatManager.EndPlayerTurn(combat);
        CombatManager.RunEnemyTurn(combat);                // enemy turn 2: REATTACH_MOVE (heal to 25)
        Assert.Equal(25, a.CurrentHp);
        Assert.Equal(0, a.ReattachIn);
    }

    /// <summary>Downing EVERY segment within one player turn IS a clear — the blow that downs the last one (no
    /// other segment alive) is a real death, so the board is dead, on-kill fires, and no revival happens.</summary>
    [Fact]
    public void Reattach_All_Segments_Downed_Together_Clears_The_Board()
    {
        var a = Monsters.DecimillipedeSegment("DecimillipedeSegment", starterIdx: 0, hp: 46);
        var b = Monsters.DecimillipedeSegment("DecimillipedeSegment", starterIdx: 1, hp: 46);
        var combat = Combat(a, b);
        CombatManager.BeginPlayerTurn(combat);

        Cmd.Attack(combat, combat.Player, a, 100, ValueProp.Move, null);   // down A (B alive → downed)
        Assert.Equal(2, a.ReattachIn);
        Cmd.Attack(combat, combat.Player, b, 100, ValueProp.Move, null);   // down B (no other alive → real death)
        Assert.Equal(0, b.ReattachIn);                     // not downed — really dead
        Assert.True(combat.AllMonstersDead);               // board cleared — a win

        CombatManager.EndPlayerTurn(combat);
        CombatManager.RunEnemyTurn(combat);
        Assert.Equal(0, a.CurrentHp);                      // fight over — no revival
        Assert.Equal(0, b.CurrentHp);
    }

    // ---- SpectralKnight Hex: while held, all player cards are Ethereal (exhaust at end of turn) ----

    [Fact]
    public void Hex_Exhausts_The_Whole_Hand_At_Turn_End()
    {
        var m = Monsters.SnappingJaxfruit();
        var combat = Combat(m);
        CombatManager.BeginPlayerTurn(combat);
        combat.Player.AddPower(new HexPower(), 1);
        combat.Player.Hand.Add(new StrikeIronclad());
        combat.Player.Hand.Add(new DefendIronclad());      // normally discarded; under Hex it exhausts

        CombatManager.EndPlayerTurn(combat);
        Assert.Empty(combat.Player.Hand);
        Assert.Empty(combat.Player.DiscardPile);           // nothing discarded
        Assert.Equal(2, combat.Player.ExhaustPile.Count);  // both exhausted (made Ethereal by Hex)
    }

    [Fact]
    public void Without_Hex_The_Hand_Is_Discarded_Normally()
    {
        var m = Monsters.SnappingJaxfruit();
        var combat = Combat(m);
        CombatManager.BeginPlayerTurn(combat);
        combat.Player.Hand.Add(new StrikeIronclad());
        combat.Player.Hand.Add(new DefendIronclad());

        CombatManager.EndPlayerTurn(combat);
        Assert.Equal(2, combat.Player.DiscardPile.Count);  // control: discarded, not exhausted
        Assert.Empty(combat.Player.ExhaustPile);
    }

    // ---- MagiKnight Dampen: downgrades the player's upgraded cards (was an optimistic gap for upgraded decks) ----

    [Fact]
    public void Dampen_Downgrades_All_Upgraded_Player_Cards()
    {
        var m = Monsters.MagiKnight();
        var combat = Combat(m);
        combat.Player.Hand.Add(new Bash().Upgraded(1));            // 10 dmg upgraded (8 base)
        combat.Player.DrawPile.Add(new StrikeIronclad().Upgraded(1));
        combat.Player.DiscardPile.Add(new DefendIronclad());      // already base — must be left untouched

        Cmd.ApplyPower(combat, combat.Player, new DampenPower(), 1, m);

        Assert.Equal(0, combat.Player.Hand[0].Upgrades);          // upgraded Bash → base
        Assert.Equal(0, combat.Player.DrawPile[0].Upgrades);      // upgraded Strike → base
        Assert.Equal(0, combat.Player.DiscardPile[0].Upgrades);   // base Defend unchanged
        Assert.IsType<Bash>(combat.Player.Hand[0]);               // identity preserved (still a Bash)
    }

    [Fact]
    public void Dampen_Is_Idempotent_And_HarmlessToUnupgradedDecks()
    {
        var m = Monsters.MagiKnight();
        var combat = Combat(m);
        combat.Player.Hand.Add(new StrikeIronclad());             // unupgraded deck
        Cmd.ApplyPower(combat, combat.Player, new DampenPower(), 1, m);
        Assert.Equal(0, combat.Player.Hand[0].Upgrades);          // no-op, no crash
        Cmd.ApplyPower(combat, combat.Player, new DampenPower(), 1, m);   // re-apply: still fine
        Assert.Equal(0, combat.Player.Hand[0].Upgrades);
    }
}
