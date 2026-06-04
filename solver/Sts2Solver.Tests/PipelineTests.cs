using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>Literal-number checks of the damage/block pipeline and power timing.</summary>
public class PipelineTests
{
    private static (CombatState combat, Monster m) Fight(int monsterHp = 50)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: 80, maxHp: 80);
        var m = Monsters.CalcifiedCultist(hp: monsterHp);
        var combat = Catalog.SetupCombat(player, new[] { m });
        return (combat, m);
    }

    [Fact]
    public void Strike_Deals_6()
    {
        var (combat, m) = Fight();
        Cmd.Attack(combat, combat.Player, m, 6, ValueProp.Move, null);
        Assert.Equal(44, m.CurrentHp);
    }

    [Fact]
    public void Vulnerable_Multiplies_By_1_5_Floored()
    {
        var (combat, m) = Fight();
        m.AddPower(new VulnerablePower(), 2);
        // 6 * 1.5 = 9
        Cmd.Attack(combat, combat.Player, m, 6, ValueProp.Move, null);
        Assert.Equal(41, m.CurrentHp);
        // 5 * 1.5 = 7.5 -> floor 7
        Cmd.Attack(combat, combat.Player, m, 5, ValueProp.Move, null);
        Assert.Equal(34, m.CurrentHp);
    }

    [Fact]
    public void Bash_Deals_8_And_Applies_2_Vulnerable_Then_Strike_Benefits()
    {
        var (combat, m) = Fight();
        combat.Player.Hand.Add(new Bash());
        combat.Player.ResetEnergy();
        var bash = combat.Player.Hand[0];
        CombatManager.PlayCard(combat, bash, m);
        Assert.Equal(42, m.CurrentHp);                 // 50 - 8
        Assert.Equal(2, m.GetPowerAmount("Vulnerable"));
        // Strike now 6 * 1.5 = 9
        Cmd.Attack(combat, combat.Player, m, 6, ValueProp.Move, null);
        Assert.Equal(33, m.CurrentHp);
    }

    [Fact]
    public void VitalSpark_Taints_Skills_So_Player_Takes_Extra_Damage()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: 80, maxHp: 80);
        var m = Monsters.InfestedPrism(hp: 100);   // VitalSpark 2
        var combat = Catalog.SetupCombat(player, new[] { m });
        m.Ai.CurrentMoveId = "JAB_MOVE";

        // Playing a Skill applies Tainted(2) to the player; an Attack would not.
        player.Hand.Add(new DefendIronclad());
        player.ResetEnergy();
        CombatManager.PlayCard(combat, player.Hand[0], null);
        Assert.Equal(2, player.GetPowerAmount("Tainted"));

        // A powered attack on the player now deals +2 (15 -> 17). (Ignore the Defend block.)
        player.ClearBlock();
        Cmd.Attack(combat, m, player, 15, ValueProp.Move, null);
        Assert.Equal(80 - 17, player.CurrentHp);

        // Tainted is cleared at enemy turn end.
        CombatManager.EndPlayerTurn(combat);
        CombatManager.RunEnemyTurn(combat);
        Assert.Equal(0, player.GetPowerAmount("Tainted"));
    }

    [Fact]
    public void HardenedShell_Caps_Hp_Loss_Per_Turn_And_Resets()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: 80, maxHp: 80);
        var m = Monsters.SkulkingColony(hp: 75);   // HardenedShell 20
        var combat = Catalog.SetupCombat(player, new[] { m });

        // A 30-damage hit is capped to 20 lost.
        Cmd.Attack(combat, combat.Player, m, 30, ValueProp.Move, null);
        Assert.Equal(55, m.CurrentHp);
        // Further damage this turn is fully negated (cap already spent).
        Cmd.Attack(combat, combat.Player, m, 10, ValueProp.Move, null);
        Assert.Equal(55, m.CurrentHp);
        // The recorder dumps the constant cap, not the remaining — Amount stays 20.
        Assert.Equal(20, m.GetPowerAmount("HardenedShell"));

        // New turn resets the cap.
        CombatManager.BeginPlayerTurn(combat);
        Cmd.Attack(combat, combat.Player, m, 15, ValueProp.Move, null);
        Assert.Equal(40, m.CurrentHp);             // 55 - 15 (under the fresh 20 cap)
    }

    [Fact]
    public void Artifact_Negates_Debuffs_Then_Wears_Off()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: 80, maxHp: 80);
        var m = Monsters.MechaKnight(hp: 100);     // Artifact 3
        var combat = Catalog.SetupCombat(player, new[] { m });

        Cmd.ApplyPower(combat, m, new VulnerablePower(), 2, player);   // absorbed (Artifact 3 -> 2)
        Assert.Equal(0, m.GetPowerAmount("Vulnerable"));
        Assert.Equal(2, m.GetPowerAmount("Artifact"));
        Cmd.ApplyPower(combat, m, new WeakPower(), 2, player);         // absorbed (2 -> 1)
        Cmd.ApplyPower(combat, m, new VulnerablePower(), 2, player);   // absorbed (1 -> 0)
        Assert.Equal(0, m.GetPowerAmount("Artifact"));
        Cmd.ApplyPower(combat, m, new VulnerablePower(), 2, player);   // Artifact gone -> lands
        Assert.Equal(2, m.GetPowerAmount("Vulnerable"));
    }

    [Fact]
    public void Slow_Boosts_Incoming_Damage_Per_Card_And_Resets_Each_Turn()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: 80, maxHp: 80);
        var m = Monsters.BygoneEffigy();
        var combat = Catalog.SetupCombat(player, new[] { m });

        Assert.Equal(127, m.CurrentHp);
        Assert.Equal(1, m.GetPowerAmount("Slow"));

        // No cards played yet -> multiplier ×1.0
        Cmd.Attack(combat, player, m, 10, ValueProp.Move, null);
        Assert.Equal(117, m.CurrentHp);                       // 127 - 10

        // Play two non-attack cards -> counter = 2 -> ×1.2 on the next attack
        player.Hand.Add(new DefendIronclad());
        player.Hand.Add(new DefendIronclad());
        player.ResetEnergy();
        CombatManager.PlayCard(combat, player.Hand[0], null);
        CombatManager.PlayCard(combat, player.Hand[0], null);
        Cmd.Attack(combat, player, m, 10, ValueProp.Move, null);
        Assert.Equal(105, m.CurrentHp);                       // 117 - floor(10 * 1.2)

        // Enemy turn start resets the counter (Sleep is a no-op) -> ×1.0 again
        CombatManager.RunEnemyTurn(combat);
        Cmd.Attack(combat, player, m, 10, ValueProp.Move, null);
        Assert.Equal(95, m.CurrentHp);                        // 105 - 10
    }

    [Fact]
    public void Infection_Deals_3_Per_Card_At_Turn_End_And_Is_Blockable()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: 80, maxHp: 80);
        var m = Monsters.CalcifiedCultist();
        var combat = Catalog.SetupCombat(player, new[] { m });
        CombatManager.BeginPlayerTurn(combat);

        player.Hand.Add(new Infection());
        player.Hand.Add(new Infection());
        Cmd.GainBlock(combat, player, 5, ValueProp.Move, null);   // 5 block vs 2×3 = 6 damage

        CombatManager.EndPlayerTurn(combat);
        Assert.Equal(79, player.CurrentHp);   // 5 absorbed, 1 unblocked
        Assert.Equal(0, player.Block);
        // Status cards are discarded with the hand, not played.
        Assert.Equal(2, player.DiscardPile.Count(c => c is Infection));
    }

    [Fact]
    public void PhrogParasite_Death_Spawns_Four_Stunned_Wrigglers()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: 80, maxHp: 80);
        var phrog = Monsters.PhrogParasite(hp: 10);
        var combat = Catalog.SetupCombat(player, new[] { phrog });
        Assert.Equal(4, phrog.GetPowerAmount("Infested"));

        Cmd.Attack(combat, player, phrog, 100, ValueProp.Move, null);   // overkill the phrog

        Assert.False(phrog.IsAlive);
        Assert.False(combat.AllMonstersDead);   // the burst keeps the fight alive (two-phase elite)
        var wrigglers = combat.Monsters.Where(w => w.Name == "Wriggler").ToList();
        Assert.Equal(4, wrigglers.Count);
        Assert.All(wrigglers, w => Assert.True(w.IsAlive));
        Assert.Equal(new[] { 2, 3, 4, 5 }, wrigglers.Select(w => w.Id).OrderBy(x => x).ToArray());
        Assert.All(wrigglers, w => Assert.Equal("SPAWNED_MOVE", w.Ai.CurrentMoveId));  // stunned first turn
        // Slot order seeds the bite/wriggle alternation: 1,3 bite-first; 2,4 wriggle-first.
        Assert.Equal(new[] { "B", "W", "B", "W" }, wrigglers.Select(w => w.Variant).ToArray());
    }

    [Fact]
    public void Wriggle_Grants_2_Strength_And_Adds_An_Infection()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: 80, maxHp: 80);
        var w = Monsters.Wriggler(stunned: false, biteFirst: false);
        var combat = Catalog.SetupCombat(player, new[] { w });

        w.Ai.CurrentMoveId = "WRIGGLE_MOVE";
        w.PerformCurrentMove(combat);

        Assert.Equal(2, w.GetPowerAmount("Strength"));
        Assert.Equal(1, player.DiscardPile.Count(c => c is Infection));
    }

    [Fact]
    public void Block_Absorbs_Then_Breaks()
    {
        var (combat, _) = Fight();
        Cmd.GainBlock(combat, combat.Player, 5, ValueProp.Move, null);
        Assert.Equal(5, combat.Player.Block);
        int lost = Cmd.ApplyDamage(combat, combat.Player, 9, ValueProp.None);
        Assert.Equal(4, lost);                          // 9 - 5 block
        Assert.Equal(76, combat.Player.CurrentHp);      // 80 - 4
        Assert.Equal(0, combat.Player.Block);
    }

    [Fact]
    public void Strength_Adds_Flat_Damage()
    {
        var (combat, m) = Fight();
        m.AddPower(new StrengthPower(), 3);
        Cmd.Attack(combat, m, combat.Player, 9, ValueProp.Move, null);   // 9 + 3
        Assert.Equal(68, combat.Player.CurrentHp);                        // 80 - 12
    }

    [Fact]
    public void Weak_Reduces_Outgoing_Damage()
    {
        var (combat, m) = Fight();
        m.AddPower(new WeakPower(), 1);
        Cmd.Attack(combat, m, combat.Player, 9, ValueProp.Move, null);   // floor(9*0.75)=6
        Assert.Equal(74, combat.Player.CurrentHp);
    }

    [Fact]
    public void Frail_Reduces_Block()
    {
        var (combat, _) = Fight();
        combat.Player.AddPower(new FrailPower(), 1);
        Cmd.GainBlock(combat, combat.Player, 5, ValueProp.Move, null);   // floor(5*0.75)=3
        Assert.Equal(3, combat.Player.Block);
    }

    [Fact]
    public void Vulnerable_Ticks_Down_At_Enemy_Turn_End()
    {
        var (combat, m) = Fight();
        m.AddPower(new VulnerablePower(), 2);
        CombatManager.RollInitialMoves(combat, new Rng(0));
        combat.CurrentSide = CombatSide.Enemy;
        CombatManager.RunEnemyTurn(combat);   // fires AfterSideTurnEnd(Enemy)
        Assert.Equal(1, m.GetPowerAmount("Vulnerable"));
    }

    [Fact]
    public void PlayerOwned_Vulnerable_Survives_Apply_Turn_And_Amplifies_Next_Enemy_Attack()
    {
        // Regression for an OPTIMISTIC gap: a monster-applied Vulnerable on the PLAYER must tick at the ENEMY
        // turn end (not the player's own turn end) and skip the apply-turn tick — exactly the game's
        // VulnerablePower (enemy-turn-end tick) + PowerCmd.SkipNextDurationTick. Previously it ticked at the
        // player's turn end and expired before any enemy attack landed, silently dropping the x1.5.
        var (combat, m) = Fight();
        combat.CurrentSide = CombatSide.Enemy;
        Cmd.ApplyPower(combat, combat.Player, new VulnerablePower(), 1, m);     // monster applies Vuln(1)
        var v = combat.Player.GetPower("Vulnerable")!;
        Assert.True(v.SkipNextTick);                                            // mirrors PowerCmd.SkipNextDurationTick

        v.AfterSideTurnEnd(combat, CombatSide.Enemy);                           // apply (enemy) turn ends: tick skipped
        Assert.Equal(1, combat.Player.GetPowerAmount("Vulnerable"));
        v.AfterSideTurnEnd(combat, CombatSide.Player);                          // player's own turn end: does NOT tick
        Assert.Equal(1, combat.Player.GetPowerAmount("Vulnerable"));

        Cmd.Attack(combat, m, combat.Player, 10, ValueProp.Move, null);         // next enemy attack: 10 x1.5 = 15
        Assert.Equal(80 - 15, combat.Player.CurrentHp);
        v.AfterSideTurnEnd(combat, CombatSide.Enemy);                           // wears off at this enemy turn end
        Assert.Equal(0, combat.Player.GetPowerAmount("Vulnerable"));
    }

    [Fact]
    public void PlayerOwned_Weak_Skips_Apply_Turn_And_Ticks_At_Enemy_Turn_End()
    {
        // Engine default (not a per-card patch): ANY debuff applied to the PLAYER gets SkipNextTick, and
        // Weak/Frail/Vulnerable all tick at the ENEMY turn end. So a player-self-applied Weak survives the turn
        // it lands and weakens the NEXT turn — matching WeakPower + PowerCmd.SkipNextDurationTick. (Previously the
        // engine ticked Weak at the player's own turn end with no central skip, so a self-applied Weak expired
        // immediately — an optimistic over-credit masked only by Doubt/Shame's manual skip.)
        var (combat, m) = Fight();
        Cmd.ApplyPower(combat, combat.Player, new WeakPower(), 1, combat.Player);   // player applies Weak to self
        var w = combat.Player.GetPower("Weak")!;
        Assert.True(w.SkipNextTick);
        w.AfterSideTurnEnd(combat, CombatSide.Player);   // player's own turn end: does NOT tick (ticks at enemy side)
        Assert.Equal(1, combat.Player.GetPowerAmount("Weak"));
        w.AfterSideTurnEnd(combat, CombatSide.Enemy);    // apply-turn enemy end: skip consumed, no tick
        Assert.Equal(1, combat.Player.GetPowerAmount("Weak"));
        w.AfterSideTurnEnd(combat, CombatSide.Enemy);    // next enemy turn end: finally ticks down
        Assert.Equal(0, combat.Player.GetPowerAmount("Weak"));
    }

    [Fact]
    public void CorpseSlug_Moves_Deal_Expected_Damage_And_Frail()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: 80, maxHp: 80);
        var slug = Monsters.CorpseSlug(hp: 27);
        var combat = Catalog.SetupCombat(player, new[] { slug });

        // Whip Slap: 3 x 2 = 6
        Cmd.AttackMulti(combat, slug, combat.Player, 3, 2, ValueProp.Move, null);
        Assert.Equal(74, combat.Player.CurrentHp);
        // Glomp: 8
        Cmd.Attack(combat, slug, combat.Player, 8, ValueProp.Move, null);
        Assert.Equal(66, combat.Player.CurrentHp);
        // Goop: apply 2 Frail to player
        Cmd.ApplyPower(combat, combat.Player, new FrailPower(), 2, slug);
        Assert.Equal(2, combat.Player.GetPowerAmount("Frail"));
    }

    [Fact]
    public void Byrdonis_Territorial_Ramps_Strength_And_Peck_Scales()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: 80, maxHp: 80);
        var byrdonis = Monsters.Byrdonis(hp: 84);
        var combat = Catalog.SetupCombat(player, new[] { byrdonis });
        CombatManager.RollInitialMoves(combat, new Rng(0)); // Swoop is the opener

        // Enemy turn 1: Swoop 17 (Strength 0), then Territorial grants +1 Strength at turn end.
        combat.CurrentSide = CombatSide.Enemy;
        CombatManager.RunEnemyTurn(combat);
        Assert.Equal(63, combat.Player.CurrentHp);          // 80 - 17
        Assert.Equal(1, byrdonis.GetPowerAmount("Strength"));

        // Roll to Peck; enemy turn 2: Peck (3+1 Strength) x3 = 12, then +1 Strength.
        CombatManager.RollNextMoves(combat, new Rng(0));
        CombatManager.RunEnemyTurn(combat);
        Assert.Equal(51, combat.Player.CurrentHp);          // 63 - 12
        Assert.Equal(2, byrdonis.GetPowerAmount("Strength"));
    }

    [Fact]
    public void Ravenous_Devours_Dead_Ally_For_Strength()
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: 80, maxHp: 80);
        var slugA = Monsters.CorpseSlug(hp: 6);
        var slugB = Monsters.CorpseSlug(hp: 27);
        var combat = Catalog.SetupCombat(player, new[] { slugA, slugB });
        Assert.Equal(4, slugB.GetPowerAmount("Ravenous"));

        // Player kills slug A -> slug B devours it for +4 Strength.
        Cmd.Attack(combat, combat.Player, slugA, 6, ValueProp.Move, null);
        Assert.False(slugA.IsAlive);
        Assert.Equal(4, slugB.GetPowerAmount("Strength"));
    }

    [Fact]
    public void Ritual_Grants_Strength_Skipping_Application_Turn()
    {
        var (combat, m) = Fight();
        CombatManager.RollInitialMoves(combat, new Rng(0));
        // Turn 1: Cultist's Incantation applies Ritual 2 to itself, then enemy turn ends.
        combat.CurrentSide = CombatSide.Enemy;
        CombatManager.RunEnemyTurn(combat);   // performs Incantation (Ritual 2), end-of-turn skips
        Assert.Equal(2, m.GetPowerAmount("Ritual"));
        Assert.Equal(0, m.GetPowerAmount("Strength"));   // skipped the application turn
        CombatManager.RollNextMoves(combat, new Rng(0)); // -> Dark Strike

        // Turn 2: Dark Strike (no strength yet), then Ritual grants +2 Strength at end.
        CombatManager.RunEnemyTurn(combat);
        Assert.Equal(2, m.GetPowerAmount("Strength"));
        CombatManager.RollNextMoves(combat, new Rng(0));

        // Turn 3 end -> +2 more.
        CombatManager.RunEnemyTurn(combat);
        Assert.Equal(4, m.GetPowerAmount("Strength"));
    }
}
