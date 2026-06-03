using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>The multiplayer-only cards (CardMultiplayerConstraint.MultiplayerOnly), modelled as their
/// single-player projection: effects on other players are dropped, "all allies / any ally" resolves to you,
/// and self/enemy payloads are kept verbatim. These cards never appear in a real single-player run but are
/// ported for catalog completeness; the checks below pin the modelled slice against the decompiled OnPlay.
/// Same conventions as ColorlessCardTests2 (Cmd.Draw needs an ambient Rng to actually draw).</summary>
public class MultiplayerCardTests
{
    private static (CombatState combat, Player p, Monster m) Fight(int monsterHp = 60, int playerHp = 80)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: playerHp, maxHp: playerHp);
        var m = Monsters.CalcifiedCultist(hp: monsterHp);
        var combat = Catalog.SetupCombat(player, new[] { m });
        player.ResetEnergy();
        return (combat, player, m);
    }

    private static void Play(CombatState combat, CardModel card, Creature? target, int energy = 0)
    {
        combat.Player.Hand.Add(card);
        combat.Player.MaxEnergy = Math.Max(combat.Player.MaxEnergy, Math.Max(card.Cost, energy));
        combat.Player.ResetEnergy();
        CombatManager.PlayCard(combat, card, target);
    }

    [Fact]
    public void Catalog_Builds_All_Multiplayer_Cards()
    {
        foreach (var name in new[]
        {
            // Colorless (11)
            "BeaconOfHope","BelieveInYou","Coordinate","GangUp","HuddleUp","Intercept","Knockdown","Lift",
            "Mimic","Rally","TagTeam",
            // Other pools (10)
            "DemonicShield","Tank","EnergySurge","Ignition","Flanking","Sneaky","HammerTime","Largesse",
            "GlimpseBeyond","LegionOfBone",
        })
            Assert.NotNull(Catalog.BuildCard(name));
    }

    // ---------------- Colorless: energy / strength ----------------

    [Fact]
    public void BelieveInYou_Gains_Energy_2_Then_3_Upgraded()
    {
        var (c, p, _) = Fight();
        p.MaxEnergy = 3; p.ResetEnergy();
        int before = p.Energy;
        Play(c, new BelieveInYou(), null);
        Assert.Equal(before + 2, p.Energy);          // cost 0, +2

        var (c2, p2, _) = Fight();
        p2.MaxEnergy = 3; p2.ResetEnergy();
        int before2 = p2.Energy;
        Play(c2, new BelieveInYou { Upgrades = 1 }, null);
        Assert.Equal(before2 + 3, p2.Energy);        // +1 on upgrade
    }

    [Fact]
    public void Coordinate_Grants_Temporary_Strength()
    {
        var (c, p, _) = Fight();
        Play(c, new Coordinate(), null);
        Assert.Equal(5, p.GetPowerAmount("Strength"));   // temp strength pushes a +5 Strength delta

        var (c2, p2, _) = Fight();
        Play(c2, new Coordinate { Upgrades = 1 }, null);
        Assert.Equal(8, p2.GetPowerAmount("Strength"));  // 5 + 3
    }

    // ---------------- Colorless: block ----------------

    [Fact]
    public void Lift_Gains_Block_11_Then_16_Upgraded()
    {
        var (c, p, _) = Fight();
        Play(c, new Lift(), null);
        Assert.Equal(11, p.Block);

        var (c2, p2, _) = Fight();
        Play(c2, new Lift { Upgrades = 1 }, null);
        Assert.Equal(16, p2.Block);
    }

    [Fact]
    public void Rally_Gains_Block_12_Then_17_Upgraded()
    {
        var (c, p, _) = Fight();
        Play(c, new Rally(), null);
        Assert.Equal(12, p.Block);

        var (c2, p2, _) = Fight();
        Play(c2, new Rally { Upgrades = 1 }, null);
        Assert.Equal(17, p2.Block);
    }

    [Fact]
    public void Intercept_Gains_Block_9_Then_13_Upgraded()
    {
        var (c, p, _) = Fight();
        Play(c, new Intercept(), null);
        Assert.Equal(9, p.Block);

        var (c2, p2, _) = Fight();
        Play(c2, new Intercept { Upgrades = 1 }, null);
        Assert.Equal(13, p2.Block);
    }

    [Fact]
    public void Mimic_Doubles_Current_Block()
    {
        var (c, p, _) = Fight();
        p.Block = 20;                                  // pre-existing block
        Play(c, new Mimic(), null);
        Assert.Equal(40, p.Block);                     // gains block == current block
    }

    [Fact]
    public void DemonicShield_Loses_1Hp_And_Doubles_Block()
    {
        var (c, p, _) = Fight();
        p.Block = 15;
        Play(c, new DemonicShield(), null);
        Assert.Equal(79, p.CurrentHp);                 // 80 - 1 (unblockable self-damage)
        Assert.Equal(30, p.Block);                     // +15 (== block at time of play)
    }

    [Fact]
    public void DemonicShield_Drops_Exhaust_On_Upgrade()
    {
        Assert.Equal(CardResultPile.Exhaust, new DemonicShield().ResultPile);
        Assert.Equal(CardResultPile.Discard, new DemonicShield { Upgrades = 1 }.ResultPile);
    }

    // ---------------- Colorless: attacks ----------------

    [Fact]
    public void GangUp_Deals_Flat_5_In_Single_Player()
    {
        var (c, _, m) = Fight();
        Play(c, new GangUp(), m);
        Assert.Equal(55, m.CurrentHp);                 // 60 - 5; no ally bonus in single player
    }

    [Fact]
    public void Knockdown_Deals_10_Then_14_Upgraded()
    {
        var (c, _, m) = Fight();
        Play(c, new Knockdown(), m, energy: 3);
        Assert.Equal(50, m.CurrentHp);                 // 60 - 10

        var (c2, _, m2) = Fight();
        Play(c2, new Knockdown { Upgrades = 1 }, m2, energy: 3);
        Assert.Equal(46, m2.CurrentHp);                // 60 - 14
    }

    [Fact]
    public void TagTeam_Deals_11_Then_15_Upgraded()
    {
        var (c, _, m) = Fight();
        Play(c, new TagTeam(), m, energy: 2);
        Assert.Equal(49, m.CurrentHp);                 // 60 - 11

        var (c2, _, m2) = Fight();
        Play(c2, new TagTeam { Upgrades = 1 }, m2, energy: 2);
        Assert.Equal(45, m2.CurrentHp);                // 60 - 15
    }

    // ---------------- Colorless: draw / power ----------------

    [Fact]
    public void HuddleUp_Draws_2()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        for (int i = 0; i < 5; i++) p.DrawPile.Add(new StrikeIronclad());
        Play(c, new HuddleUp(), null);
        Assert.Equal(2, p.Hand.Count);                 // drew 2
    }

    [Fact]
    public void BeaconOfHope_Is_Inert_But_Gains_Innate_On_Upgrade()
    {
        var (c, p, _) = Fight();
        p.Block = 10;
        Play(c, new BeaconOfHope(), null);
        Assert.Equal(10, p.Block);                     // no other allies → power never fires
        Assert.NotNull(p.GetPower("BeaconOfHope"));    // power is still applied (1:1 structure)
        Assert.False(new BeaconOfHope().Innate);
        Assert.True(new BeaconOfHope { Upgrades = 1 }.Innate);
    }

    // ---------------- Ironclad Tank: the self-downside is modelled ----------------

    [Fact]
    public void Tank_Makes_You_Take_Double_Attack_Damage()
    {
        var (c, p, _) = Fight();
        Play(c, new Tank(), null);
        Assert.NotNull(p.GetPower("Tank"));
        // A 10-damage powered attack against the tank now lands for 20.
        Cmd.Attack(c, c.Monsters[0], p, 10, ValueProp.None, null);
        Assert.Equal(60, p.CurrentHp);                 // 80 - 10*2
    }

    [Fact]
    public void Tank_Does_Not_Double_Unpowered_Damage()
    {
        var (c, p, _) = Fight();
        Play(c, new Tank(), null);
        Cmd.Attack(c, c.Monsters[0], p, 10, ValueProp.Unpowered, null);
        Assert.Equal(70, p.CurrentHp);                 // 80 - 10 (unpowered: no ×2)
    }

    // ---------------- Defect / Necrobinder ----------------

    [Fact]
    public void Ignition_Channels_A_Plasma_Orb()
    {
        var (c, p, _) = Fight();
        p.OrbSlots = 3;                                // Defect would have 3 slots; channel is a no-op at 0
        Play(c, new Ignition(), null);
        Assert.Contains(p.Orbs, o => o.Name == "Plasma");
    }

    [Fact]
    public void GlimpseBeyond_Adds_3_Souls_To_Draw_Pile()
    {
        var (c, p, _) = Fight();
        Play(c, new GlimpseBeyond(), null);
        Assert.Equal(3, p.DrawPile.Count(x => x.Name == "Soul"));

        var (c2, p2, _) = Fight();
        Play(c2, new GlimpseBeyond { Upgrades = 1 }, null);
        Assert.Equal(4, p2.DrawPile.Count(x => x.Name == "Soul"));   // +1
    }

    [Fact]
    public void LegionOfBone_Summons_6_Then_8_Upgraded()
    {
        Assert.Equal(6, new LegionOfBone().Summon);
        Assert.Equal(8, new LegionOfBone { Upgrades = 1 }.Summon);
    }
}
