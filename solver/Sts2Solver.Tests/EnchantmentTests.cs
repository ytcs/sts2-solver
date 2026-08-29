using System;
using System.Collections.Generic;
using System.Linq;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>
/// The card-enchantment port (sts2.dll v0.107.0 MegaCrit.Sts2.Core.Models.Enchantments). Builds enchanted cards
/// via the "@Name:Amount" build spec and checks each enchant's effect through the real play pipeline.
/// StrikeIronclad = 6 damage, DefendIronclad = 5 block.
/// </summary>
public class EnchantmentTests
{
    private static (CombatState combat, Player p, Monster m) Fight(int monsterHp = 200, int playerHp = 80)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: playerHp, maxHp: playerHp);
        player.MaxEnergy = 3;
        var m = Monsters.CalcifiedCultist(hp: monsterHp);
        var combat = Catalog.SetupCombat(player, new[] { m });
        player.ResetEnergy();
        return (combat, player, m);
    }

    private static void Play(CombatState combat, CardModel card, Creature? target)
    {
        combat.Player.Hand.Add(card);
        combat.Player.MaxEnergy = Math.Max(combat.Player.MaxEnergy, card.EffectiveCost(combat));
        combat.Player.ResetEnergy();
        CombatManager.PlayCard(combat, card, target);
    }

    /// <summary>Replay a just-played card: pull it back out of the discard and play it again (same instance, so a
    /// Stateful enchant's flag/ramp carries over).</summary>
    private static void Replay(CombatState combat, CardModel card, Creature? target)
    {
        combat.Player.DiscardPile.Remove(card);
        Play(combat, card, target);
    }

    [Fact]
    public void Sharp_Adds_Flat_Damage_To_Powered_Attack()
    {
        var (c, _, m) = Fight();
        Play(c, Catalog.BuildCard("StrikeIronclad@Sharp:3"), m);
        Assert.Equal(200 - (6 + 3), m.CurrentHp);
    }

    [Fact]
    public void Inky_Applies_1_Weak_Without_Extra_Damage()
    {
        var (c, _, m) = Fight();
        Play(c, Catalog.BuildCard("StrikeIronclad@Inky"), m);
        Assert.Equal(200 - 6, m.CurrentHp);   // v0.111.0: Inky no longer adds +1 damage
        Assert.Equal(1, m.GetPowerAmount("Weak"));
    }

    [Fact]
    public void Instinct_Doubles_Powered_Damage()
    {
        var (c, _, m) = Fight();
        Play(c, Catalog.BuildCard("StrikeIronclad@Instinct"), m);
        Assert.Equal(200 - 12, m.CurrentHp);
    }

    [Fact]
    public void Corrupted_Multiplies_Damage_By_1_5_And_Costs_2_Hp()
    {
        var (c, p, m) = Fight();
        Play(c, Catalog.BuildCard("StrikeIronclad@Corrupted"), m);
        Assert.Equal(200 - 9, m.CurrentHp);   // floor(6 × 1.5)
        Assert.Equal(80 - 2, p.CurrentHp);    // 2 self-damage on play
    }

    [Fact]
    public void TezcatarasEmber_Zeroes_Cost_And_Adds_3_Damage()
    {
        var (c, _, m) = Fight();
        var card = Catalog.BuildCard("StrikeIronclad@TezcatarasEmber");
        Assert.Equal(0, card.EffectiveCost(c));
        Play(c, card, m);
        Assert.Equal(200 - (6 + 3), m.CurrentHp);
    }

    [Fact]
    public void Nimble_Adds_Flat_Block()
    {
        var (c, p, _) = Fight();
        Play(c, Catalog.BuildCard("DefendIronclad@Nimble:4"), null);
        Assert.Equal(5 + 4, p.Block);
    }

    [Fact]
    public void Adroit_Gains_Block_On_Play()
    {
        var (c, p, m) = Fight();
        Play(c, Catalog.BuildCard("StrikeIronclad@Adroit:5"), m);
        Assert.Equal(5, p.Block);             // Adroit's on-play block
        Assert.Equal(200 - 6, m.CurrentHp);   // the strike still deals its 6
    }

    [Fact]
    public void Steady_Grants_Retain()
    {
        Assert.True(Catalog.BuildCard("StrikeIronclad@Steady").Retain);
        Assert.False(Catalog.BuildCard("StrikeIronclad").Retain);
    }

    [Fact]
    public void RoyallyApproved_Grants_Innate_And_Retain()
    {
        var card = Catalog.BuildCard("StrikeIronclad@RoyallyApproved");
        Assert.True(card.Innate);
        Assert.True(card.Retain);
    }

    [Fact]
    public void SoulsPower_Stops_The_Card_Exhausting()
    {
        var (c, _, _) = Fight();
        var card = Catalog.BuildCard("Impervious@SoulsPower");   // Impervious normally exhausts on play
        Play(c, card, null);
        Assert.Contains(card, c.Player.DiscardPile);
        Assert.DoesNotContain(card, c.Player.ExhaustPile);
    }

    [Fact]
    public void Sown_Grants_Energy_Once()
    {
        var (c, p, _) = Fight();   // MaxEnergy 3
        var card = Catalog.BuildCard("DefendIronclad@Sown:2");   // cost 1
        Play(c, card, null);
        Assert.Equal(3 - 1 + 2, p.Energy);   // reset to 3, −1 cost, +2 Sown
        Replay(c, card, null);
        Assert.Equal(3 - 1, p.Energy);   // second play: reset→3, −1 cost, no Sown (once/combat)
    }

    [Fact]
    public void Vigorous_Boosts_Only_The_First_Play()
    {
        var (c, _, m) = Fight();
        var card = Catalog.BuildCard("StrikeIronclad@Vigorous:4");
        Play(c, card, m);                       // 6 + 4
        Replay(c, card, m);                     // 6 (disabled)
        Assert.Equal(200 - (10 + 6), m.CurrentHp);
    }

    [Fact]
    public void Momentum_Ramps_Each_Subsequent_Play()
    {
        var (c, _, m) = Fight();
        var card = Catalog.BuildCard("StrikeIronclad@Momentum:3");
        Play(c, card, m);                       // 6 (ramp applies after)
        Replay(c, card, m);                     // 6 + 3
        Replay(c, card, m);                     // 6 + 6
        Assert.Equal(200 - (6 + 9 + 12), m.CurrentHp);
    }

    [Fact]
    public void Spiral_Plays_The_Card_Twice()
    {
        var (c, _, m) = Fight();
        Play(c, Catalog.BuildCard("StrikeIronclad@Spiral"), m);
        Assert.Equal(200 - 12, m.CurrentHp);   // 6 × 2
    }

    [Fact]
    public void Glam_Plays_The_Card_Twice_On_The_First_Play_Only()
    {
        var (c, _, m) = Fight();
        var card = Catalog.BuildCard("StrikeIronclad@Glam");
        Play(c, card, m);            // 6 × 2 = 12
        Replay(c, card, m);          // 6 (replay spent)
        Assert.Equal(200 - (12 + 6), m.CurrentHp);
    }

    [Fact]
    public void Enchant_Is_Part_Of_The_Card_Identity()
    {
        Assert.Equal("StrikeIronclad", Catalog.BuildCard("StrikeIronclad").StateKey());
        Assert.Equal("StrikeIronclad@Sharp:3", Catalog.BuildCard("StrikeIronclad@Sharp:3").StateKey());
        Assert.NotEqual(Catalog.BuildCard("StrikeIronclad").StateKey(),
                        Catalog.BuildCard("StrikeIronclad@Sharp:3").StateKey());
    }

    [Fact]
    public void Unmodelled_Enchant_Suffix_Is_Ignored_By_BuildCard()
    {
        // An unmodelled enchant name on the spec builds the plain card (the advisor warns separately).
        var card = Catalog.BuildCard("StrikeIronclad@Clone");
        Assert.Null(card.Enchant);
    }
}
