using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;
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

    // ---- Batch 3: orb-reactive / turn-boundary powers ----

    [Fact]
    public void Thunder_Damages_All_Enemies_On_Lightning_Evoke()
    {
        var (c, p, a, b) = Fight2(hp: 60);
        Cmd.ApplyPower(c, p, new ThunderPower(), 6, p);
        OrbOps.Channel(c, new LightningOrb());
        OrbOps.EvokeFront(c);                     // evoke Lightning (8 to first) + Thunder 6 to all
        Assert.Equal(60 - 8 - 6, a.CurrentHp);    // a takes the evoke (first enemy) + Thunder
        Assert.Equal(60 - 6, b.CurrentHp);        // b takes only Thunder
    }

    [Fact]
    public void Thunder_Does_Not_Fire_On_NonLightning_Evoke()
    {
        var (c, p, m) = Fight();
        Cmd.ApplyPower(c, p, new ThunderPower(), 6, p);
        OrbOps.Channel(c, new FrostOrb());
        OrbOps.EvokeFront(c);                     // Frost evoke = block, no Thunder
        Assert.Equal(60, m.CurrentHp);
        Assert.Equal(5, p.Block);
    }

    [Fact]
    public void Hailstorm_Damages_All_At_Turn_End_With_Frost()
    {
        var (c, p, a, b) = Fight2(hp: 60);
        Cmd.ApplyPower(c, p, new HailstormPower(), 6, p);
        OrbOps.Channel(c, new FrostOrb());
        CombatManager.EndPlayerTurn(c);           // Frost passive (block) + Hailstorm 6 to all
        Assert.Equal(60 - 6, a.CurrentHp);
        Assert.Equal(60 - 6, b.CurrentHp);
    }

    [Fact]
    public void Hailstorm_Silent_Without_Frost()
    {
        var (c, p, m) = Fight();
        Cmd.ApplyPower(c, p, new HailstormPower(), 6, p);
        OrbOps.Channel(c, new LightningOrb());    // not Frost
        CombatManager.EndPlayerTurn(c);
        Assert.Equal(60 - 3, m.CurrentHp);        // only the Lightning passive (3), no Hailstorm
    }

    [Fact]
    public void Storm_Channels_Lightning_On_Power_Play_But_Not_Itself()
    {
        var (c, p, _) = Fight();
        Cmd.ApplyPower(c, p, new StormPower(), 1, p);
        Assert.Empty(p.Orbs);                     // applying Storm didn't channel
        Play(c, new Defragment());                // a Power → Storm channels 1 Lightning
        Assert.Single(p.Orbs.OfType<LightningOrb>());
    }

    [Fact]
    public void Subroutine_Gains_Energy_On_Power_Play()
    {
        var (c, p, _) = Fight();
        Cmd.ApplyPower(c, p, new SubroutinePower(), 1, p);
        p.Energy = 3;
        Play(c, new Defragment());                // costs 1, then +1 from Subroutine
        Assert.Equal(3 - 1 + 1, p.Energy);
    }

    [Fact]
    public void Coolant_Gains_Block_Per_Distinct_Orb_At_Turn_Start()
    {
        var (c, p, _) = Fight();
        Cmd.ApplyPower(c, p, new CoolantPower(), 2, p);
        OrbOps.Channel(c, new LightningOrb());
        OrbOps.Channel(c, new FrostOrb());
        OrbOps.Channel(c, new FrostOrb());        // 2 distinct types (Lightning, Frost)
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(2 * 2, p.Block);
    }

    [Fact]
    public void Smokestack_Damages_All_When_Status_Generated()
    {
        var (c, p, a, b) = Fight2(hp: 60);
        Cmd.ApplyPower(c, p, new SmokestackPower(), 5, p);
        Cmd.GenerateStatusCard(c, new Dazed(), p.DiscardPile);
        Assert.Equal(60 - 5, a.CurrentHp);
        Assert.Equal(60 - 5, b.CurrentHp);
    }

    [Fact]
    public void Loop_Triggers_Front_Orb_Passive_Extra()
    {
        var (c, p, m) = Fight();
        Cmd.ApplyPower(c, p, new LoopPower(), 1, p);
        OrbOps.Channel(c, new LightningOrb());
        CombatManager.BeginPlayerTurn(c);         // Loop: front Lightning passive once extra = 3 dmg
        Assert.Equal(60 - 3, m.CurrentHp);
    }

    [Fact]
    public void Spinner_Channels_Glass_At_Turn_Start()
    {
        var (c, p, _) = Fight();
        Cmd.ApplyPower(c, p, new SpinnerPower(), 1, p);
        CombatManager.BeginPlayerTurn(c);
        Assert.Single(p.Orbs.OfType<GlassOrb>());
    }

    [Fact]
    public void Spinner_Upgraded_Channels_Glass_On_Play()
    {
        var (c, p, _) = Fight();
        Play(c, (Spinner)new Spinner().Upgraded(1));
        Assert.Single(p.Orbs.OfType<GlassOrb>());
    }

    [Fact]
    public void LightningRod_Channels_Lightning_For_Two_Turns()
    {
        var (c, p, _) = Fight();
        Play(c, new LightningRod());
        Assert.Equal(4, p.Block);
        CombatManager.BeginPlayerTurn(c);
        Assert.Single(p.Orbs.OfType<LightningOrb>());
        Assert.Equal(1, p.GetPowerAmount("LightningRod"));
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(2, p.Orbs.Count(o => o is LightningOrb));
        Assert.False(p.HasPower("LightningRod"));
    }

    [Fact]
    public void BiasedCognition_Grants_Focus_Then_Decays_It()
    {
        var (c, p, _) = Fight();
        Play(c, new BiasedCognition());
        Assert.Equal(4, p.GetPowerAmount("Focus"));
        CombatManager.BeginPlayerTurn(c);         // -1 Focus
        Assert.Equal(3, p.GetPowerAmount("Focus"));
    }

    [Fact]
    public void ConsumingShadow_Channels_Dark_And_Evokes_At_Turn_End()
    {
        var (c, p, m) = Fight(monsterHp: 60, slots: 5);
        Play(c, new ConsumingShadow());           // channel 2 Dark, apply power
        Assert.Equal(2, p.Orbs.OfType<DarkOrb>().Count());
        CombatManager.EndPlayerTurn(c);           // Dark passives (each +6 → evokeVal 12) then evoke newest (12)
        Assert.Equal(60 - 12, m.CurrentHp);
        Assert.Single(p.Orbs.OfType<DarkOrb>());  // one Dark evoked away
    }

    [Fact]
    public void Buffer_Prevents_One_Instance_Of_Damage()
    {
        var (c, p, m) = Fight(playerHp: 80);
        Cmd.ApplyPower(c, p, new BufferPower(), 1, p);
        Cmd.Attack(c, m, p, 10, ValueProp.Unpowered, null);   // absorbed fully
        Assert.Equal(80, p.CurrentHp);
        Assert.False(p.HasPower("Buffer"));
        Cmd.Attack(c, m, p, 10, ValueProp.Unpowered, null);   // no Buffer left
        Assert.Equal(70, p.CurrentHp);
    }

    [Fact]
    public void Hotfix_Grants_Temporary_Focus_That_Expires()
    {
        var (c, p, _) = Fight();
        Play(c, new Hotfix());
        Assert.Equal(2, p.GetPowerAmount("Focus"));
        CombatManager.EndPlayerTurn(c);
        Assert.Equal(0, p.GetPowerAmount("Focus"));
    }

    [Fact]
    public void FocusedStrike_Deals_Damage_And_Temp_Focus()
    {
        var (c, p, m) = Fight();
        Play(c, new FocusedStrike(), m);
        Assert.Equal(60 - 9, m.CurrentHp);
        Assert.Equal(1, p.GetPowerAmount("Focus"));
        CombatManager.EndPlayerTurn(c);
        Assert.Equal(0, p.GetPowerAmount("Focus"));
    }

    [Fact]
    public void Synchronize_Grants_Focus_Per_Distinct_Orb()
    {
        var (c, p, _) = Fight();
        OrbOps.Channel(c, new LightningOrb());
        OrbOps.Channel(c, new FrostOrb());        // 2 distinct
        Play(c, new Synchronize());
        Assert.Equal(2 * 2, p.GetPowerAmount("Focus"));
    }

    [Fact]
    public void Synthesis_Makes_Next_Power_Free()
    {
        var (c, p, m) = Fight();
        Play(c, new Synthesis(), m);
        Assert.Equal(60 - 14, m.CurrentHp);
        var fp = p.GetPower("FreePower");
        Assert.NotNull(fp);
        Assert.Equal(0, fp!.ModifyCardCost(new Defragment(), 1));   // next Power costs 0
    }

    [Fact]
    public void SignalBoost_Plays_Next_Power_Twice()
    {
        var (c, p, _) = Fight();
        Cmd.ApplyPower(c, p, new SignalBoostPower(), 1, p);
        Play(c, new Defragment());                // +1 Focus, played twice → +2
        Assert.Equal(2, p.GetPowerAmount("Focus"));
        Assert.False(p.HasPower("SignalBoost"));
    }

    [Fact]
    public void EchoForm_Plays_First_Card_Each_Turn_Twice()
    {
        var (c, p, m) = Fight();
        Cmd.ApplyPower(c, p, new EchoFormPower(), 1, p);
        Play(c, new StrikeDefect(), m);           // first card → played twice = 12
        Assert.Equal(60 - 12, m.CurrentHp);
        Play(c, new StrikeDefect(), m);           // second card → once = 6
        Assert.Equal(60 - 12 - 6, m.CurrentHp);
    }

    [Fact]
    public void Inert_Power_Cards_Apply_Their_Marker()
    {
        var (c, p, _) = Fight();
        Play(c, new MachineLearning());
        Play(c, new TrashToTreasure());
        Play(c, new CreativeAi());
        Play(c, new Feral());
        Assert.Equal(1, p.GetPowerAmount("MachineLearning"));
        Assert.Equal(1, p.GetPowerAmount("TrashToTreasure"));
        Assert.Equal(1, p.GetPowerAmount("CreativeAi"));
        Assert.Equal(1, p.GetPowerAmount("Feral"));
    }

    // ---- Batch 4: status-gen, scalers, X-cost, deck-manipulation ----

    private static (CombatState combat, Player p, Monster m) FightWithDeck(int slots, params CardModel[] deck)
    {
        var player = Catalog.BuildPlayer(deck.ToList(), currentHp: 80, maxHp: 80);
        player.OrbSlots = slots;
        var m = Monsters.CalcifiedCultist(hp: 60);
        var combat = Catalog.SetupCombat(player, new[] { m });
        player.ResetEnergy();
        return (combat, player, m);
    }

    [Fact]
    public void BoostAway_Blocks_And_Generates_Dazed()
    {
        var (c, p, _) = Fight();
        Play(c, new BoostAway());
        Assert.Equal(6, p.Block);
        Assert.Single(p.DiscardPile.OfType<Dazed>());
    }

    [Fact]
    public void FightThrough_Blocks_And_Generates_Two_Wounds()
    {
        var (c, p, _) = Fight();
        Play(c, new FightThrough());
        Assert.Equal(13, p.Block);
        Assert.Equal(2, p.DiscardPile.OfType<Wound>().Count());
    }

    [Fact]
    public void GunkUp_Hits_Thrice_And_Generates_Slimed()
    {
        var (c, p, m) = Fight();
        Play(c, new GunkUp(), m);
        Assert.Equal(60 - 12, m.CurrentHp);
        Assert.Single(p.DiscardPile.OfType<Slimed>());
    }

    [Fact]
    public void Turbo_Gains_Energy_And_Generates_Void()
    {
        var (c, p, _) = Fight();
        int before = p.Energy;
        Play(c, new Turbo());
        Assert.Equal(before + 2, p.Energy);
        Assert.Single(p.DiscardPile.OfType<Sts2Solver.Content.Void>());
    }

    [Fact]
    public void Smokestack_Fires_When_A_Status_Card_Is_Generated_By_A_Card()
    {
        var (c, p, m) = Fight();
        Cmd.ApplyPower(c, p, new SmokestackPower(), 5, p);
        Play(c, new Turbo());                 // generates Void → Smokestack 5
        Assert.Equal(60 - 5, m.CurrentHp);
    }

    [Fact]
    public void GoForTheEyes_Applies_Weak_Only_When_Enemy_Intends_To_Attack()
    {
        var (c, p, m) = Fight();
        CombatManager.RollInitialMoves(c, new Rng(0));
        bool attacks = m.IntendsToAttack;
        Play(c, new GoForTheEyes(), m);
        Assert.Equal(60 - 3, m.CurrentHp);
        Assert.Equal(attacks ? 1 : 0, m.GetPowerAmount("Weak"));
    }

    [Fact]
    public void Hyperbeam_Hits_All_And_Loses_Focus()
    {
        var (c, p, a, b) = Fight2(hp: 60);
        Cmd.ApplyPower(c, p, new FocusPower(), 5, p);
        Play(c, new Hyperbeam());
        Assert.Equal(60 - 28, a.CurrentHp);
        Assert.Equal(60 - 28, b.CurrentHp);
        Assert.Equal(2, p.GetPowerAmount("Focus"));      // 5 - 3
    }

    [Fact]
    public void Sunder_Gains_Energy_On_Kill()
    {
        var (c, p, m) = Fight(monsterHp: 20);
        p.Energy = 5;
        Play(c, new Sunder(), m);                         // 24 ≥ 20 → kill → +3
        Assert.False(m.IsAlive);
        Assert.Equal(5 - 3 + 3, p.Energy);
    }

    [Fact]
    public void DoubleEnergy_Doubles_Current_Energy()
    {
        var (c, p, _) = Fight();
        p.Energy = 3;
        Play(c, new DoubleEnergy());                      // costs 1 → 2 left, then +2 = 4
        Assert.Equal(4, p.Energy);
    }

    [Fact]
    public void BulkUp_Removes_Slot_And_Grants_Str_Dex()
    {
        var (c, p, _) = Fight(slots: 3);
        Play(c, new BulkUp());
        Assert.Equal(2, p.OrbSlots);
        Assert.Equal(2, p.GetPowerAmount("Strength"));
        Assert.Equal(2, p.GetPowerAmount("Dexterity"));
    }

    [Fact]
    public void Compact_Transforms_Hand_Status_To_Fuel()
    {
        var (c, p, _) = Fight();
        p.Hand.Add(new Wound());
        p.Hand.Add(new Dazed());
        Play(c, new Compact());
        Assert.Equal(6, p.Block);
        Assert.Equal(2, p.Hand.OfType<Fuel>().Count());
        Assert.Empty(p.Hand.Where(x => x.Type == CardType.Status));
    }

    [Fact]
    public void CompileDriver_Draws_Per_Distinct_Orb()
    {
        var (c, p, m) = Fight();
        c.Rng = new Rng(0);
        for (int i = 0; i < 5; i++) p.DrawPile.Add(new StrikeDefect());
        OrbOps.Channel(c, new LightningOrb());
        OrbOps.Channel(c, new FrostOrb());                // 2 distinct
        Play(c, new CompileDriver(), m);
        Assert.Equal(60 - 7, m.CurrentHp);
        Assert.Equal(2, p.Hand.Count(x => x is StrikeDefect));
    }

    [Fact]
    public void AdaptiveStrike_Adds_A_Free_Copy_To_Discard()
    {
        var (c, p, m) = Fight();
        Play(c, new AdaptiveStrike(), m);
        Assert.Equal(60 - 18, m.CurrentHp);
        // Discard holds the played original (cost 2) plus the generated free copy (cost 0).
        Assert.Equal(2, p.DiscardPile.OfType<AdaptiveStrike>().Count());
        Assert.Single(p.DiscardPile.OfType<AdaptiveStrike>().Where(a => a.Cost == 0));
    }

    [Fact]
    public void AllForOne_Returns_Zero_Cost_NonAttacks_From_Discard()
    {
        var (c, p, m) = Fight();
        p.DiscardPile.Add((CardModel)new Zap().Upgraded(1));   // upgraded Zap = 0-cost Skill → returned
        p.DiscardPile.Add(new StrikeDefect());                 // Attack → stays
        p.DiscardPile.Add(new Leap());                         // cost-1 Skill → stays
        Play(c, new AllForOne(), m);
        Assert.Equal(60 - 10, m.CurrentHp);
        Assert.Single(p.Hand.OfType<Zap>());                   // 0-cost Skill returned
        Assert.DoesNotContain(p.Hand, x => x is StrikeDefect); // Attack stays
        Assert.DoesNotContain(p.Hand, x => x is Leap);         // cost-1 Skill stays
    }

    [Fact]
    public void Hologram_Returns_A_Discard_Card_To_Hand()
    {
        var (c, p, _) = Fight();
        p.DiscardPile.Add(new StrikeDefect());
        Play(c, new Hologram());
        Assert.Equal(3, p.Block);
        Assert.Single(p.Hand.OfType<StrikeDefect>());
        Assert.Empty(p.DiscardPile.OfType<StrikeDefect>());
    }

    [Fact]
    public void Scavenge_Exhausts_A_Hand_Card_And_Grants_Energy_Next_Turn()
    {
        var (c, p, _) = Fight();
        var victim = new StrikeDefect();
        p.Hand.Add(victim);
        Play(c, new Scavenge());
        Assert.Contains(victim, p.ExhaustPile);
        int expected = p.EffectiveMaxEnergy + 2;
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(expected, p.Energy);
    }

    [Fact]
    public void MomentumStrike_Sets_Its_Own_Cost_To_Zero()
    {
        var (c, p, m) = Fight();
        var card = new MomentumStrike();
        Play(c, card, m);
        Assert.Equal(60 - 10, m.CurrentHp);
        Assert.Equal(0, card.Cost);
    }

    [Fact]
    public void Modded_Adds_Slot_Draws_And_Raises_Own_Cost()
    {
        var (c, p, _) = Fight(slots: 3);
        c.Rng = new Rng(0);
        p.DrawPile.Add(new StrikeDefect());
        var card = new Modded();
        Play(c, card);
        Assert.Equal(4, p.OrbSlots);
        Assert.Single(p.Hand.OfType<StrikeDefect>());
        Assert.Equal(1, card.Cost);
    }

    [Fact]
    public void Claw_Scales_With_Each_Claw_Play()
    {
        var (c, p, m) = Fight();
        var claw2 = new Claw();
        p.DrawPile.Add(claw2);                 // a second Claw in the deck
        Play(c, new Claw(), m);                // deals 3, buffs all Claws (+2)
        Assert.Equal(60 - 3, m.CurrentHp);
        Assert.Equal(5, claw2.Damage);         // 3 + 2
        Play(c, claw2, m);                     // the buffed Claw deals 5
        Assert.Equal(60 - 3 - 5, m.CurrentHp);
    }

    [Fact]
    public void HelixDrill_Hits_Per_Energy_Spent_This_Turn()
    {
        var (c, p, m) = FightWithDeck(3, new HelixDrill());
        p.Energy = 5;
        Play(c, new ColdSnap(), m);            // costs 1 → EnergySpentThisTurn = 1; deals 6
        Assert.Equal(60 - 6, m.CurrentHp);
        Play(c, new HelixDrill(), m);          // hits 1× (1 energy spent so far) for 3
        Assert.Equal(60 - 6 - 3, m.CurrentHp);
    }

    [Fact]
    public void Ftl_Draws_Only_Under_The_Play_Threshold()
    {
        var (c, p, m) = FightWithDeck(3, new Ftl());
        c.Rng = new Rng(0);
        p.DrawPile.Clear();                    // remove the deck's own Ftl so the draw is unambiguous
        for (int i = 0; i < 6; i++) p.DrawPile.Add(new StrikeDefect());
        Play(c, new Ftl(), m);                 // 0 plays so far (<3) → draw 1
        Assert.Equal(60 - 5, m.CurrentHp);
        Assert.Single(p.Hand.OfType<StrikeDefect>());
    }

    [Fact]
    public void TeslaCoil_Triggers_Lightning_Passives()
    {
        var (c, p, m) = Fight();
        OrbOps.Channel(c, new LightningOrb());
        OrbOps.Channel(c, new LightningOrb());
        Play(c, new TeslaCoil(), m);           // 3 (card) + 2 Lightning passives × 3 = 3 + 6
        Assert.Equal(60 - 3 - 6, m.CurrentHp);
    }

    [Fact]
    public void Quadcast_Evokes_Front_Orb_Four_Times()
    {
        var (c, p, m) = Fight();
        OrbOps.Channel(c, new LightningOrb());
        Play(c, new Quadcast());               // evoke 8 × 4 = 32
        Assert.Equal(60 - 32, m.CurrentHp);
        Assert.Empty(p.Orbs);
    }

    [Fact]
    public void Shatter_Hits_All_And_Evokes_Every_Orb_Twice()
    {
        var (c, p, a, b) = Fight2(hp: 60);
        OrbOps.Channel(c, new FrostOrb());     // evoke 5 block each
        Play(c, new Shatter());                // 7 to all + Frost evoked twice = 10 block
        Assert.Equal(60 - 7, a.CurrentHp);
        Assert.Equal(60 - 7, b.CurrentHp);
        Assert.Equal(10, p.Block);
        Assert.Empty(p.Orbs);
    }

    [Fact]
    public void MultiCast_Evokes_Front_X_Times()
    {
        var (c, p, m) = Fight();
        OrbOps.Channel(c, new LightningOrb());
        p.Energy = 2;
        Play(c, new MultiCast());              // X=2 → evoke 8 twice = 16
        Assert.Equal(60 - 16, m.CurrentHp);
    }

    [Fact]
    public void Tempest_Channels_X_Lightning()
    {
        var (c, p, _) = Fight(slots: 5);
        p.Energy = 3;
        Play(c, new Tempest());                // X=3 → 3 Lightning
        Assert.Equal(3, p.Orbs.OfType<LightningOrb>().Count());
    }

    [Fact]
    public void Voltaic_Channels_Per_Lightning_Channeled_This_Combat()
    {
        var (c, p, _) = FightWithDeck(6, new Voltaic());
        OrbOps.Channel(c, new LightningOrb());
        OrbOps.Channel(c, new LightningOrb());   // 2 lightning channeled this combat
        Play(c, new Voltaic());                  // channels 2 more
        Assert.Equal(4, p.Orbs.OfType<LightningOrb>().Count());
    }

    [Fact]
    public void Uproar_Hits_Twice()
    {
        var (c, p, m) = Fight();
        Play(c, new Uproar(), m);
        Assert.Equal(60 - 12, m.CurrentHp);
    }

    [Fact]
    public void Slimed_Draws_When_Played()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        p.DrawPile.Add(new StrikeDefect());
        var s = new Slimed();
        Play(c, s);
        Assert.Single(p.Hand.OfType<StrikeDefect>());
        Assert.Contains(s, p.ExhaustPile);
    }

    [Fact]
    public void Fuel_Gains_Energy_And_Draws()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        p.DrawPile.Add(new StrikeDefect());
        int before = p.Energy;
        Play(c, new Fuel());
        Assert.Equal(before + 1, p.Energy);
        Assert.Single(p.Hand.OfType<StrikeDefect>());
    }

    [Fact]
    public void Defect_Starter_Deck_Solves_Through_The_Orb_Subsystem()
    {
        // Exercises the EXACT search across the orb queue (CrackedCore channels Lightning at combat start, its
        // turn-end passive damages, Dualcast evokes) — the direct-PlayCard unit tests don't run the solver.
        var player = Catalog.BuildPlayer(Catalog.DefectStarterDeck(), 75, 75, relics: new[] { "CrackedCore" });
        var setup = Catalog.SetupCombat(player, new[] { Monsters.CalcifiedCultist(hp: 30) });
        var solver = new Solver();
        var value = solver.Solve(setup);
        Assert.Equal(1.0, value.Win, 6);                 // the starter handily beats a 30-HP cultist
        Assert.InRange(value.Loss, 0.0, 75.0);
    }

    [Fact]
    public void Defect_Pool_Is_Complete_88_Cards()
    {
        string[] names =
        {
            "AdaptiveStrike","AllForOne","BallLightning","Barrage","BeamCell","BiasedCognition","BoostAway",
            "BootSequence","Buffer","BulkUp","Capacitor","Chaos","ChargeBattery","Chill","Claw","ColdSnap",
            "Compact","CompileDriver","ConsumingShadow","Coolant","Coolheaded","CreativeAi","Darkness",
            "DefendDefect","Defragment","DoubleEnergy","Dualcast","EchoForm","EnergySurge","Feral","FightThrough",
            "FlakCannon","FocusedStrike","Ftl","Fusion","GeneticAlgorithm","Glacier","Glasswork","GoForTheEyes",
            "GunkUp","Hailstorm","HelixDrill","Hologram","Hotfix","Hyperbeam","IceLance","Ignition","Iteration",
            "Leap","LightningRod","Loop","MachineLearning","MeteorStrike","Modded","MomentumStrike","MultiCast",
            "Null","Overclock","Quadcast","Rainbow","Reboot","Refract","RocketPunch","Scavenge","Scrape",
            "ShadowShield","Shatter","SignalBoost","Skim","Smokestack","Spinner","Storm","StrikeDefect",
            "Subroutine","Sunder","Supercritical","SweepingBeam","Synchronize","Synthesis","Tempest","TeslaCoil",
            "Thunder","TrashToTreasure","Turbo","Uproar","Voltaic","WhiteNoise","Zap",
        };
        Assert.Equal(88, names.Length);
        Assert.Equal(88, names.Distinct().Count());
        foreach (var n in names)
        {
            Assert.Contains(n, Catalog.CardPool, StringComparer.OrdinalIgnoreCase);
            Assert.NotNull(Catalog.BuildCard(n));   // every name constructs
        }
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

    // ---- Void: −1 energy when drawn (Turbo's downside), via the per-card OnDraw hook ----

    [Fact]
    public void Void_Loses_1_Energy_On_MidTurn_Draw()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        p.MaxEnergy = 3; p.ResetEnergy();                 // energy 3
        p.DrawPile.Add(new Sts2Solver.Content.Void());
        Cmd.Draw(c, 1);                                   // drawing Void costs 1 energy
        Assert.Equal(2, p.Energy);
        Assert.Contains(p.Hand, h => h is Sts2Solver.Content.Void);   // and it sits (Unplayable) in hand
    }

    [Fact]
    public void Void_Energy_Loss_Floors_At_Zero()
    {
        var (c, p, _) = Fight();
        c.Rng = new Rng(0);
        p.MaxEnergy = 3; p.Energy = 0;
        p.DrawPile.Add(new Sts2Solver.Content.Void());
        Cmd.Draw(c, 1);
        Assert.Equal(0, p.Energy);                        // never negative
    }

    [Fact]
    public void Void_Loses_Energy_On_Turn_Start_Hand_Draw_Too()
    {
        var (c, p, _) = Fight();
        p.MaxEnergy = 3; p.ResetEnergy();
        p.DrawPile.Add(new Sts2Solver.Content.Void());
        p.DrawPile.Add(new StrikeDefect());
        CombatManager.DrawCards(c, 2, new Rng(0), fromHandDraw: true);   // opening-hand-style draw
        Assert.Equal(2, p.Energy);                        // the drawn Void still costs 1 energy
    }

    // ---- MachineLearning: +1 card on the turn-start hand draw (ModifyHandDraw) ----

    [Fact]
    public void MachineLearning_Adds_To_TurnStart_Draw_Count()
    {
        var (c, p, _) = Fight();
        Assert.Equal(Player.CardsDrawnPerTurn, CombatManager.TurnStartDrawCount(c));   // baseline 5
        Cmd.ApplyPower(c, p, new MachineLearningPower(), 1, p);
        Assert.Equal(Player.CardsDrawnPerTurn + 1, CombatManager.TurnStartDrawCount(c));
        Cmd.ApplyPower(c, p, new MachineLearningPower(), 1, p);                         // stacks
        Assert.Equal(Player.CardsDrawnPerTurn + 2, CombatManager.TurnStartDrawCount(c));
    }

    [Fact]
    public void MachineLearning_Upgraded_Is_Innate()
    {
        Assert.False(new MachineLearning().Innate);
        var ml = (MachineLearning)new MachineLearning().Upgraded();
        Assert.True(ml.Innate);
    }

    [Fact]
    public void FlakCannon_Is_RandomEnemy_And_Concentrates_On_The_First_Enemy()
    {
        var fc = new FlakCannon();
        Assert.Equal(TargetType.RandomEnemy, fc.Target);   // game: RandomEnemy, not AnyEnemy
        Assert.False(fc.NeedsTarget);                      // so the search cannot pick the best target
        var (c, p, a, b) = Fight2(hp: 60);
        p.Hand.Add(new Dazed()); p.Hand.Add(new Dazed());  // 2 Status cards => 2 hits of 8
        Play(c, fc, null);
        Assert.Equal(60 - 16, a.CurrentHp);                // both hits land on the first living enemy
        Assert.Equal(60, b.CurrentHp);                     // the other enemy is untouched (no best-target choice)
    }
}
