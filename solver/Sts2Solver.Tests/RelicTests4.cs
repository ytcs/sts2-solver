using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>Batch-4 relics: stateful "every-Nth-play" / once-per-combat relics, modelled via hidden hashed-counter
/// relic powers. Plays cards to drive the counters and checks the effect fires on the right play.</summary>
public class RelicTests4
{
    // High max-energy so plays are always affordable — we test the relic, not affordability — and energy stays a
    // clean signal (no mid-play energy hacks) for the energy-granting relics.
    private static (CombatState combat, Monster m) Fight(string relic, int monsterHp = 200)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), 80, 80, maxEnergy: 99, relics: new[] { relic });
        var m = Monsters.CalcifiedCultist(hp: monsterHp);
        var combat = Catalog.SetupCombat(player, new[] { m });
        CombatManager.BeginPlayerTurn(combat);
        return (combat, m);
    }

    /// <summary>Play <paramref name="n"/> copies of a fresh card of type <typeparamref name="T"/> at the monster.</summary>
    private static void Play<T>(CombatState combat, Monster m, int n) where T : CardModel, new()
    {
        for (int i = 0; i < n; i++)
        {
            var c = new T();
            combat.Player.Hand.Add(c);
            CombatManager.PlayCard(combat, c, m);
        }
    }

    [Fact]
    public void Kunai_Grants_Dex_Every_3_Attacks()
    {
        var (combat, m) = Fight("Kunai");
        Play<StrikeIronclad>(combat, m, 2);
        Assert.Equal(0, combat.Player.GetPowerAmount("Dexterity"));
        Play<StrikeIronclad>(combat, m, 1);    // 3rd attack
        Assert.Equal(1, combat.Player.GetPowerAmount("Dexterity"));
        Play<StrikeIronclad>(combat, m, 3);    // 6th attack
        Assert.Equal(2, combat.Player.GetPowerAmount("Dexterity"));
    }

    [Fact]
    public void Shuriken_Grants_Strength_Every_3_Attacks()
    {
        var (combat, m) = Fight("Shuriken");
        Play<StrikeIronclad>(combat, m, 3);
        Assert.Equal(1, combat.Player.GetPowerAmount("Strength"));
    }

    [Fact]
    public void Kunai_Counter_Resets_Each_Turn()
    {
        var (combat, m) = Fight("Kunai");
        Play<StrikeIronclad>(combat, m, 2);    // 2 attacks, no grant
        CombatManager.EndPlayerTurn(combat);
        CombatManager.BeginPlayerTurn(combat); // counter resets
        Play<StrikeIronclad>(combat, m, 2);    // 2 more — would be 4 cumulative, but reset means no grant yet
        Assert.Equal(0, combat.Player.GetPowerAmount("Dexterity"));
        Play<StrikeIronclad>(combat, m, 1);    // 3rd this turn
        Assert.Equal(1, combat.Player.GetPowerAmount("Dexterity"));
    }

    [Fact]
    public void OrnamentalFan_Blocks_4_Every_3_Attacks()
    {
        var (combat, m) = Fight("OrnamentalFan");
        Play<StrikeIronclad>(combat, m, 3);
        Assert.Equal(4, combat.Player.Block);
    }

    [Fact]
    public void LetterOpener_Damages_All_Every_3_Skills()
    {
        var (combat, m) = Fight("LetterOpener");
        Play<DefendIronclad>(combat, m, 2);
        Assert.Equal(200, m.CurrentHp);
        Play<DefendIronclad>(combat, m, 1);    // 3rd skill → 5 dmg
        Assert.Equal(195, m.CurrentHp);
    }

    [Fact]
    public void Nunchaku_Grants_Energy_Every_10_Attacks()
    {
        var (combat, m) = Fight("Nunchaku");
        Play<StrikeIronclad>(combat, m, 9);
        int e9 = combat.Player.Energy;
        Play<StrikeIronclad>(combat, m, 1);    // 10th attack: -1 cost +1 Nunchaku → net unchanged
        Assert.Equal(e9, combat.Player.Energy);
        Play<StrikeIronclad>(combat, m, 1);    // 11th: just the -1 cost
        Assert.Equal(e9 - 1, combat.Player.Energy);
    }

    [Fact]
    public void Nunchaku_Counter_Persists_Across_Turns()
    {
        var (combat, m) = Fight("Nunchaku");
        Play<StrikeIronclad>(combat, m, 7);
        CombatManager.EndPlayerTurn(combat);
        CombatManager.BeginPlayerTurn(combat); // per-combat counter does NOT reset
        Play<StrikeIronclad>(combat, m, 2);    // cumulative 9 — no grant yet
        int e9 = combat.Player.Energy;
        Play<StrikeIronclad>(combat, m, 1);    // cumulative 10th → -1 cost +1 grant → net unchanged
        Assert.Equal(e9, combat.Player.Energy);
    }

    [Fact]
    public void TuningFork_Blocks_On_10th_Skill_Not_On_Attacks()
    {
        var (combat, m) = Fight("TuningFork");
        Play<DefendIronclad>(combat, m, 10);     // 10 Defends: 50 block + 7 TuningFork on the 10th
        Assert.Equal(57, combat.Player.Block);
        var (c2, m2) = Fight("TuningFork");
        Play<StrikeIronclad>(c2, m2, 10);        // attacks don't count → no relic block
        Assert.Equal(0, c2.Player.Block);
    }

    [Fact]
    public void IronClub_Draws_Every_4_Cards()
    {
        // Stack the draw pile so IronClub's 4th-card draw has a card to pull; a concrete Rng resolves it eagerly.
        var player = Catalog.BuildPlayer(Enumerable.Range(0, 12).Select(_ => (CardModel)new StrikeIronclad()).ToList(),
                                         80, 80, maxEnergy: 99, relics: new[] { "IronClub" });
        var m = Monsters.CalcifiedCultist(hp: 200);
        var combat = Catalog.SetupCombat(player, new[] { m });
        combat.Rng = new Rng(1);
        CombatManager.BeginPlayerTurn(combat);
        int drawBefore = combat.Player.DrawPile.Count;
        Play<StrikeIronclad>(combat, m, 3);      // 3 plays — no draw yet
        Assert.Equal(drawBefore, combat.Player.DrawPile.Count);
        Play<StrikeIronclad>(combat, m, 1);      // 4th card → IronClub draws 1 from the draw pile
        Assert.Equal(drawBefore - 1, combat.Player.DrawPile.Count);
    }

    [Fact]
    public void Permafrost_Blocks_7_On_First_Power_Only()
    {
        var (combat, m) = Fight("Permafrost");
        Play<Barricade>(combat, m, 1);           // first Power (Barricade grants no block itself) → +7 block
        Assert.Equal(7, combat.Player.Block);
        Play<Barricade>(combat, m, 1);           // second Power → nothing more
        Assert.Equal(7, combat.Player.Block);
    }

    [Fact]
    public void RainbowRing_Grants_Str_Dex_When_All_Three_Types_Played()
    {
        var (combat, m) = Fight("RainbowRing");
        Play<StrikeIronclad>(combat, m, 1);      // attack
        Play<DefendIronclad>(combat, m, 1);      // skill
        Assert.Equal(0, combat.Player.GetPowerAmount("Strength"));
        Play<Barricade>(combat, m, 1);           // power (Str-neutral) → all three → grant
        Assert.Equal(1, combat.Player.GetPowerAmount("Strength"));
        Assert.Equal(1, combat.Player.GetPowerAmount("Dexterity"));
    }

    [Theory]
    [InlineData("Kunai")] [InlineData("Shuriken")] [InlineData("OrnamentalFan")] [InlineData("LetterOpener")]
    [InlineData("Nunchaku")] [InlineData("TuningFork")] [InlineData("IronClub")] [InlineData("Permafrost")]
    [InlineData("RainbowRing")]
    public void Relic_Is_Registered_And_Buildable(string relic)
    {
        Assert.True(Catalog.IsModelledRelic(relic));
        Assert.Equal(relic, Catalog.BuildRelic(relic).Id);
    }
}
