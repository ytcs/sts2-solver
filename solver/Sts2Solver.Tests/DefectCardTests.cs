using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>Literal-number checks of the Defect orb subsystem and its first card batch against the decompiled
/// game: the five orb types (Lightning damage / Frost block / Dark accumulate+evoke / Plasma energy / Glass
/// decay), Focus modification, channel overflow, evoke order (Dualcast), slot management, and the turn-boundary
/// passive triggers (turn-end for most, turn-start for Plasma).</summary>
public class DefectCardTests
{
    private static (CombatState combat, Player p, Monster m) Fight(int monsterHp = 60, int playerHp = 80, int slots = 3)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: playerHp, maxHp: playerHp);
        player.OrbSlots = slots;
        var m = Monsters.CalcifiedCultist(hp: monsterHp);
        var combat = Catalog.SetupCombat(player, new[] { m });
        player.ResetEnergy();
        return (combat, player, m);
    }

    private static (CombatState combat, Player p, Monster a, Monster b) Fight2(int hp = 60, int playerHp = 80, int slots = 3)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: playerHp, maxHp: playerHp);
        player.OrbSlots = slots;
        var a = Monsters.CalcifiedCultist(hp: hp);
        var b = Monsters.CalcifiedCultist(hp: hp);
        var combat = Catalog.SetupCombat(player, new[] { a, b });
        player.ResetEnergy();
        return (combat, player, a, b);
    }

    private static void Play(CombatState combat, CardModel card, Creature? target = null)
    {
        combat.Player.Hand.Add(card);
        combat.Player.MaxEnergy = Math.Max(combat.Player.MaxEnergy, card.Cost);
        combat.Player.Energy = Math.Max(combat.Player.Energy, card.Cost);
        CombatManager.PlayCard(combat, card, target);
    }

    // ---- Channel / evoke / slots ----

    [Fact]
    public void Zap_Channels_A_Lightning_Orb()
    {
        var (c, p, _) = Fight();
        Play(c, new Zap());
        Assert.Single(p.Orbs);
        Assert.IsType<LightningOrb>(p.Orbs[0]);
    }

    [Fact]
    public void Channel_Into_Full_Queue_Evokes_The_Oldest_First()
    {
        var (c, p, m) = Fight(slots: 1);
        OrbOps.Channel(c, new LightningOrb());     // queue [A]
        OrbOps.Channel(c, new LightningOrb());     // full → evoke A (8 dmg), then enqueue B
        Assert.Equal(60 - 8, m.CurrentHp);
        Assert.Single(p.Orbs);
    }

    [Fact]
    public void Dualcast_Evokes_The_Front_Orb_Twice_Then_Removes_It()
    {
        var (c, p, m) = Fight();
        OrbOps.Channel(c, new LightningOrb());
        Play(c, new Dualcast());                   // evoke 8 twice = 16, orb leaves
        Assert.Equal(60 - 16, m.CurrentHp);
        Assert.Empty(p.Orbs);
    }

    [Fact]
    public void Capacitor_Adds_Orb_Slots()
    {
        var (c, p, _) = Fight(slots: 3);
        Play(c, new Capacitor());
        Assert.Equal(5, p.OrbSlots);
        Play(c, (Capacitor)new Capacitor().Upgraded(1));   // +3
        Assert.Equal(8, p.OrbSlots);
    }

    // ---- Lightning ----

    [Fact]
    public void Lightning_Passive_Deals_3_At_Turn_End()
    {
        var (c, _, m) = Fight();
        OrbOps.Channel(c, new LightningOrb());
        CombatManager.EndPlayerTurn(c);
        Assert.Equal(60 - 3, m.CurrentHp);
    }

    [Fact]
    public void BallLightning_Deals_7_And_Channels_Lightning()
    {
        var (c, p, m) = Fight();
        Play(c, new BallLightning(), m);
        Assert.Equal(60 - 7, m.CurrentHp);
        Assert.Single(p.Orbs.OfType<LightningOrb>());
    }

    // ---- Frost ----

    [Fact]
    public void Frost_Passive_Gains_2_Block_At_Turn_End()
    {
        var (c, p, _) = Fight();
        OrbOps.Channel(c, new FrostOrb());
        CombatManager.EndPlayerTurn(c);
        Assert.Equal(2, p.Block);
    }

    [Fact]
    public void Glacier_Gains_Block_And_Channels_Two_Frost()
    {
        var (c, p, _) = Fight();
        Play(c, new Glacier());
        Assert.Equal(6, p.Block);
        Assert.Equal(2, p.Orbs.OfType<FrostOrb>().Count());
    }

    // ---- Dark ----

    [Fact]
    public void Dark_Passive_Accumulates_Then_Evoke_Hits_Weakest()
    {
        var (c, p, a, b) = Fight2(hp: 60);
        b.CurrentHp = 40;                          // b is the weakest
        OrbOps.Channel(c, new DarkOrb());
        CombatManager.EndPlayerTurn(c);            // passive: evokeVal 6 → 12
        OrbOps.EvokeFront(c);                      // deal 12 to weakest (b)
        Assert.Equal(60, a.CurrentHp);
        Assert.Equal(40 - 12, b.CurrentHp);
    }

    [Fact]
    public void Darkness_Channels_Dark_And_Triggers_Its_Passive()
    {
        var (c, p, m) = Fight();
        Play(c, new Darkness());                   // channel Dark, passive 1× → evokeVal 6 → 12
        var dark = Assert.IsType<DarkOrb>(p.Orbs.Single());
        Assert.Equal(12, dark.EvokeVal(c));
    }

    // ---- Plasma (turn START) ----

    [Fact]
    public void Plasma_Passive_Gains_Energy_At_Turn_Start()
    {
        var (c, p, _) = Fight();
        OrbOps.Channel(c, new PlasmaOrb());
        int before = p.Energy;
        CombatManager.BeginPlayerTurn(c);          // turn-start passive: +1 energy (on top of reset)
        Assert.Equal(p.EffectiveMaxEnergy + 1, p.Energy);
    }

    [Fact]
    public void Plasma_Does_Not_Trigger_At_Turn_End()
    {
        var (c, p, m) = Fight();
        OrbOps.Channel(c, new PlasmaOrb());
        int hp = m.CurrentHp;
        CombatManager.EndPlayerTurn(c);            // Plasma is turn-start only → nothing happens here
        Assert.Equal(hp, m.CurrentHp);
        Assert.Single(p.Orbs);
    }

    // ---- Glass ----

    [Fact]
    public void Glass_Passive_Hits_All_Enemies_Then_Decays()
    {
        var (c, p, a, b) = Fight2(hp: 60);
        OrbOps.Channel(c, new GlassOrb());
        CombatManager.EndPlayerTurn(c);            // deal 4 to all, decay 4 → 3
        Assert.Equal(60 - 4, a.CurrentHp);
        Assert.Equal(60 - 4, b.CurrentHp);
        CombatManager.BeginPlayerTurn(c);
        CombatManager.EndPlayerTurn(c);            // deal 3 to all, decay 3 → 2
        Assert.Equal(60 - 4 - 3, a.CurrentHp);
    }

    // ---- Focus ----

    [Fact]
    public void Focus_Raises_Lightning_Passive_And_Evoke()
    {
        var (c, _, m) = Fight();
        Play(c, new Defragment());                 // +1 Focus
        var orb = new LightningOrb();
        Assert.Equal(4, orb.PassiveVal(c));        // 3 + 1
        Assert.Equal(9, orb.EvokeVal(c));          // 8 + 1
    }

    [Fact]
    public void Focus_Does_Not_Affect_Plasma()
    {
        var (c, p, _) = Fight();
        Play(c, new Defragment());                 // +1 Focus
        var plasma = new PlasmaOrb();
        Assert.Equal(1, plasma.PassiveVal(c));     // unchanged
        Assert.Equal(2, plasma.EvokeVal(c));
    }

    // ---- Misc cards ----

    [Fact]
    public void Barrage_Hits_Once_Per_Orb()
    {
        var (c, p, m) = Fight();
        OrbOps.Channel(c, new FrostOrb());
        OrbOps.Channel(c, new FrostOrb());
        OrbOps.Channel(c, new LightningOrb());     // 3 orbs (no evoke — within slots)
        Play(c, new Barrage(), m);                 // 5 × 3 = 15
        Assert.Equal(60 - 15, m.CurrentHp);
    }

    [Fact]
    public void Chill_Channels_A_Frost_Per_Enemy()
    {
        var (c, p, a, b) = Fight2();
        Play(c, new Chill());
        Assert.Equal(2, p.Orbs.OfType<FrostOrb>().Count());
    }

    // ---- Starter / relic ----

    [Fact]
    public void CrackedCore_Grants_Three_Slots_And_A_Lightning_Orb()
    {
        var player = Catalog.BuildPlayer(Catalog.DefectStarterDeck(), 75, 75, relics: new[] { "CrackedCore" });
        var combat = Catalog.SetupCombat(player, new[] { Monsters.CalcifiedCultist(hp: 60) });
        Assert.Equal(3, player.OrbSlots);
        Assert.Single(player.Orbs.OfType<LightningOrb>());
    }

    // ---- Batch 2: block / draw / energy / orb-channel / attack cards ----

    [Fact]
    public void BootSequence_Gains_10_Block_And_Is_Innate_Exhaust()
    {
        var (c, p, _) = Fight();
        var card = new BootSequence();
        Assert.True(card.Innate);
        Assert.Equal(CardResultPile.Exhaust, card.ResultPile);
        Play(c, card);
        Assert.Equal(10, p.Block);
    }

    [Fact]
    public void BootSequence_Upgrade_Adds_3_Block()
    {
        var (c, p, _) = Fight();
        Play(c, (BootSequence)new BootSequence().Upgraded(1));
        Assert.Equal(13, p.Block);
    }

    [Fact]
    public void Leap_Gains_9_Block()
    {
        var (c, p, _) = Fight();
        Play(c, new Leap());
        Assert.Equal(9, p.Block);
    }

    [Fact]
    public void Glasswork_Gains_5_Block_And_Channels_Glass()
    {
        var (c, p, _) = Fight();
        Play(c, new Glasswork());
        Assert.Equal(5, p.Block);
        Assert.Single(p.Orbs.OfType<GlassOrb>());
    }

    [Fact]
    public void ShadowShield_Gains_11_Block_And_Channels_Dark()
    {
        var (c, p, _) = Fight();
        Play(c, new ShadowShield());
        Assert.Equal(11, p.Block);
        Assert.Single(p.Orbs.OfType<DarkOrb>());
    }

    [Fact]
    public void ChargeBattery_Gains_Block_And_Energy_Next_Turn()
    {
        var (c, p, _) = Fight();
        Play(c, new ChargeBattery());
        Assert.Equal(7, p.Block);
        int expected = p.EffectiveMaxEnergy + 1;     // EnergyNextTurn fires after the turn-start reset
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(expected, p.Energy);
    }

    [Fact]
    public void Skim_Draws_3()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        for (int i = 0; i < 5; i++) p.DrawPile.Add(new StrikeDefect());
        Play(c, new Skim());
        Assert.Equal(3, p.Hand.Count(x => x is StrikeDefect));
    }

    [Fact]
    public void Supercritical_Gains_4_Energy_And_Exhausts()
    {
        var (c, p, _) = Fight();
        var card = new Supercritical();
        Assert.Equal(CardResultPile.Exhaust, card.ResultPile);
        int before = p.Energy;
        Play(c, card);
        Assert.Equal(before + 4, p.Energy);
        Assert.Contains(card, p.ExhaustPile);
    }

    [Fact]
    public void Fusion_Channels_Plasma_And_Loses_Exhaust_On_Upgrade()
    {
        var (c, p, _) = Fight();
        Assert.Equal(CardResultPile.Exhaust, new Fusion().ResultPile);
        Assert.Equal(CardResultPile.Discard, new Fusion().Upgraded(1).ResultPile);
        Play(c, new Fusion());
        Assert.Single(p.Orbs.OfType<PlasmaOrb>());
    }

    [Fact]
    public void Rainbow_Channels_Lightning_Frost_Dark_In_Order()
    {
        var (c, p, _) = Fight(slots: 3);
        Play(c, new Rainbow());
        Assert.Collection(p.Orbs,
            o => Assert.IsType<LightningOrb>(o),
            o => Assert.IsType<FrostOrb>(o),
            o => Assert.IsType<DarkOrb>(o));
    }

    [Fact]
    public void Refract_Deals_9_Twice_And_Channels_Two_Glass()
    {
        var (c, p, m) = Fight();
        Play(c, new Refract(), m);
        Assert.Equal(60 - 18, m.CurrentHp);
        Assert.Equal(2, p.Orbs.OfType<GlassOrb>().Count());
    }

    [Fact]
    public void IceLance_Deals_19_And_Channels_Three_Frost()
    {
        var (c, p, m) = Fight();
        Play(c, new IceLance(), m);
        Assert.Equal(60 - 19, m.CurrentHp);
        Assert.Equal(3, p.Orbs.OfType<FrostOrb>().Count());
    }

    [Fact]
    public void MeteorStrike_Deals_24_Channels_Three_Plasma_And_Is_A_Strike()
    {
        var (c, p, m) = Fight();
        var card = new MeteorStrike();
        Assert.True(card.IsStrike);
        Play(c, card, m);
        Assert.Equal(60 - 24, m.CurrentHp);
        Assert.Equal(3, p.Orbs.OfType<PlasmaOrb>().Count());
    }

    [Fact]
    public void SweepingBeam_Hits_All_Enemies_And_Draws()
    {
        var (c, p, a, b) = Fight2(hp: 60);
        c.Rng = new Rng(0);
        p.DrawPile.Add(new StrikeDefect());
        Play(c, new SweepingBeam());
        Assert.Equal(60 - 6, a.CurrentHp);
        Assert.Equal(60 - 6, b.CurrentHp);
        Assert.Single(p.Hand.OfType<StrikeDefect>());
    }

    [Fact]
    public void Null_Deals_10_Applies_2_Weak_And_Channels_Dark()
    {
        var (c, p, m) = Fight();
        Play(c, new Null(), m);
        Assert.Equal(60 - 10, m.CurrentHp);
        Assert.Equal(2, m.GetPowerAmount("Weak"));
        Assert.Single(p.Orbs.OfType<DarkOrb>());
    }

    [Fact]
    public void Defect_Starter_Deck_Is_Faithful()
    {
        var deck = Catalog.DefectStarterDeck();
        Assert.Equal(4, deck.Count(c => c is StrikeDefect));
        Assert.Equal(4, deck.Count(c => c is DefendDefect));
        Assert.Equal(1, deck.Count(c => c is Zap));
        Assert.Equal(1, deck.Count(c => c is Dualcast));
        Assert.Equal(10, deck.Count);
    }
}
