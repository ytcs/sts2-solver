using Sts2Solver.Content;
using Sts2Solver.Engine;
using Xunit;

namespace Sts2Solver.Tests;

/// <summary>Literal-number checks of the ported Regent cards/powers against the decompiled game logic:
/// the Stars resource (gain / spend / gating), the Forge → Sovereign Blade engine and its modifiers
/// (Parry / Seeking Edge / Sword Sage / Conqueror), star-payback powers, temporary-Strength debuffs,
/// resource-next-turn powers, and the catalog wiring. Effects that depend on unported subsystems (RNG
/// card generation, hand-draw-count, gold, multiplayer) are documented-inert in RegentCards.cs and are
/// only checked to be HP-safe no-ops here.</summary>
public class RegentCardTests
{
    private static (CombatState combat, Player p, Monster m) Fight(int monsterHp = 60, int playerHp = 80, int stars = 0)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: playerHp, maxHp: playerHp);
        player.Stars = stars;
        var m = Monsters.CalcifiedCultist(hp: monsterHp);
        var combat = Catalog.SetupCombat(player, new[] { m });
        player.ResetEnergy();
        return (combat, player, m);
    }

    private static (CombatState combat, Player p, Monster a, Monster b) Fight2(int hp = 60, int playerHp = 80, int stars = 0)
    {
        var player = Catalog.BuildPlayer(new List<CardModel>(), currentHp: playerHp, maxHp: playerHp);
        player.Stars = stars;
        var a = Monsters.CalcifiedCultist(hp: hp);
        var b = Monsters.CalcifiedCultist(hp: hp);
        var combat = Catalog.SetupCombat(player, new[] { a, b });
        player.ResetEnergy();
        return (combat, player, a, b);
    }

    /// <summary>Play a card, ensuring the player can afford its energy (stars must be set by the test).</summary>
    private static void Play(CombatState combat, CardModel card, Creature? target)
    {
        combat.Player.Hand.Add(card);
        combat.Player.MaxEnergy = Math.Max(combat.Player.MaxEnergy, card.Cost);
        combat.Player.Energy = Math.Max(combat.Player.Energy, card.Cost);
        CombatManager.PlayCard(combat, card, target);
    }

    // ---- Catalog wiring ----

    private static readonly string[] PoolNames =
    {
        "StrikeRegent","DefendRegent","FallingStar","Venerate","AstralPulse","BeatIntoShape","Bombardment",
        "CelestialMight","CollisionCourse","Comet","CrashLanding","CrescentSpear","CrushUnder","Devastate",
        "DyingStar","GammaBlast","GuidingStar","HeavenlyDrill","Hegemony","HeirloomHammer","KinglyKick",
        "KinglyPunch","KnockoutBlow","LunarBlast","MakeItSo","MeteorShower","Radiate","Resonance","SevenStars",
        "ShiningStrike","SolarStrike","Stardust","Supermassive","WroughtInWar","Alignment","Bulwark","CloakOfStars",
        "Conqueror","Convergence","CosmicIndifference","DecisionsDecisions","GatherLight","Glimmer","Glitterstream",
        "Glow","HiddenCache","IAmInvincible","KnowThyPlace","ManifestAuthority","ParticleWall","Patter","PhotonCut",
        "Prophesize","Reflect","RefineBlade","RoyalGamble","SeekingEdge","SpoilsOfBattle","SummonForth","Terraforming",
        "TheSmith","Arsenal","BigBang","BlackHole","ChildOfTheStars","ForegoneConclusion","Furnace","Genesis",
        "MonarchsGaze","Monologue","NeutronAegis","Orbit","PaleBlueDot","Parry","PillarOfCreation","Royalties",
        "SpectrumShift","SwordSage","TheSealedThrone","Tyranny","VoidForm","Begone","BundleOfJoy","Charge","Guards",
        "HammerTime","Largesse","Quasar",
    };

    [Fact]
    public void All_88_Regent_Cards_Build_And_Upgrade()
    {
        Assert.Equal(88, PoolNames.Length);
        Assert.Equal(88, PoolNames.Distinct(StringComparer.OrdinalIgnoreCase).Count());
        foreach (var name in PoolNames)
        {
            Assert.NotNull(Catalog.BuildCard(name));
            Assert.NotNull(Catalog.BuildCard(name + "+1"));
        }
    }

    [Fact]
    public void Pool_Contains_Regent_Cards_And_Excludes_SovereignBlade()
    {
        foreach (var name in PoolNames)
            Assert.Contains(name, Catalog.CardPool, StringComparer.OrdinalIgnoreCase);
        Assert.DoesNotContain("SovereignBlade", Catalog.CardPool, StringComparer.OrdinalIgnoreCase);
        Assert.NotNull(Catalog.BuildCard("SovereignBlade"));   // still buildable by name
    }

    [Fact]
    public void Regent_Starter_Deck_Is_Faithful()
    {
        var deck = Catalog.RegentStarterDeck();
        Assert.Equal(4, deck.Count(c => c is StrikeRegent));
        Assert.Equal(4, deck.Count(c => c is DefendRegent));
        Assert.Equal(1, deck.Count(c => c is FallingStar));
        Assert.Equal(1, deck.Count(c => c is Venerate));
        Assert.Equal(10, deck.Count);
    }

    // ---- Stars resource ----

    [Fact]
    public void DivineRight_Grants_3_Stars_At_Combat_Start()
    {
        var player = Catalog.BuildPlayer(Catalog.RegentStarterDeck(), 75, 75, relics: new[] { "DivineRight" });
        var combat = Catalog.SetupCombat(player, new[] { Monsters.CalcifiedCultist(hp: 60) });
        Assert.Equal(3, player.Stars);
    }

    [Fact]
    public void Venerate_Gains_Stars()
    {
        var (c, p, _) = Fight();
        Play(c, new Venerate(), null);
        Assert.Equal(2, p.Stars);
        Play(c, new Venerate().Upgraded(), null);
        Assert.Equal(2 + 3, p.Stars);
    }

    [Fact]
    public void Star_Cost_Card_Spends_Stars()
    {
        var (c, p, m) = Fight(stars: 5);
        Play(c, new FallingStar(), m);     // 2★
        Assert.Equal(3, p.Stars);
        Assert.Equal(60 - 8, m.CurrentHp);
        Assert.Equal(1, m.GetPowerAmount("Weak"));
        Assert.Equal(1, m.GetPowerAmount("Vulnerable"));
    }

    [Fact]
    public void Star_Cost_Card_Cannot_Be_Played_Without_Stars()
    {
        var (c, p, m) = Fight(stars: 1);
        c.Player.Hand.Add(new FallingStar());
        c.Player.ResetEnergy();
        Assert.False(p.CanAffordStars(c.Player.Hand[0]));   // 2★ needed, 1 held
        Assert.Throws<InvalidOperationException>(() => CombatManager.PlayCard(c, c.Player.Hand[0], m));
    }

    [Fact]
    public void KnockoutBlow_Gains_5_Stars_On_Kill()
    {
        var (c, p, m) = Fight(monsterHp: 20);
        Play(c, new KnockoutBlow(), m);     // 30 dmg kills the 20-HP enemy
        Assert.False(m.IsAlive);
        Assert.Equal(5, p.Stars);
    }

    [Fact]
    public void KnockoutBlow_No_Stars_Without_Kill()
    {
        var (c, p, m) = Fight(monsterHp: 60);
        Play(c, new KnockoutBlow(), m);
        Assert.True(m.IsAlive);
        Assert.Equal(0, p.Stars);
    }

    // ---- Forge → Sovereign Blade ----

    [Fact]
    public void Forge_Creates_SovereignBlade_In_Hand_And_Grows_It()
    {
        var (c, p, _) = Fight(stars: 4);
        Assert.Empty(p.Hand.OfType<SovereignBlade>());
        Play(c, new TheSmith(), null);     // 4★, Forge 30 → creates a blade at 10+30
        Assert.Equal(0, p.Stars);          // 4★ spent
        Assert.Equal(10 + 30, p.Hand.OfType<SovereignBlade>().Single().Damage);
    }

    [Fact]
    public void WroughtInWar_Forges_And_Damages()
    {
        var (c, p, m) = Fight();
        Play(c, new WroughtInWar(), m);
        Assert.Equal(60 - 7, m.CurrentHp);
        var blade = p.Hand.OfType<SovereignBlade>().Single();
        Assert.Equal(10 + 7, blade.Damage);   // base 10 + forged 7
    }

    [Fact]
    public void Second_Forge_Grows_The_Same_Blade()
    {
        var (c, p, m) = Fight();
        Play(c, new WroughtInWar(), m);      // Forge 7 → blade 17
        Play(c, new RefineBlade(), null);    // Forge 9 → blade 26
        var blade = p.Hand.OfType<SovereignBlade>().Single();
        Assert.Equal(10 + 7 + 9, blade.Damage);
    }

    [Fact]
    public void SovereignBlade_Deals_Accumulated_Damage()
    {
        var (c, p, m) = Fight();
        Play(c, new WroughtInWar(), m);      // 60-7=53, blade=17
        var blade = p.Hand.OfType<SovereignBlade>().Single();
        Play(c, blade, m);                   // 53-17=36
        Assert.Equal(53 - 17, m.CurrentHp);
    }

    /// <summary>A fresh BeatIntoShape (no prior powered hits on the target this turn) forges its base 5
    /// (= CalcBase), building a 10+5 blade, and its own hit then registers on the target's per-turn counter.</summary>
    [Fact]
    public void BeatIntoShape_With_No_Prior_Hits_Forges_Base()
    {
        // The deck must contain BeatIntoShape for the engine to track per-target powered hits.
        var player = Catalog.BuildPlayer(new List<CardModel> { new BeatIntoShape() }, currentHp: 80, maxHp: 80);
        var m = Monsters.CalcifiedCultist(hp: 200);
        var combat = Catalog.SetupCombat(player, new[] { m });
        player.ResetEnergy();
        Assert.True(combat.TracksPoweredHits);

        Play(combat, new BeatIntoShape(), m);
        Assert.Equal(10 + 5, player.Hand.OfType<SovereignBlade>().Single().Damage);   // priorHits=0 → Forge 5
        Assert.Equal(1, m.PlayerPoweredHitsThisTurn);                                 // its own hit counted
    }

    /// <summary>BeatIntoShape forges 5 × (1 + prior powered hits the player dealt the target this turn).
    /// Two StrikeRegents first ⇒ Forge 5 × 3 = 15 ⇒ a 10+15 blade.</summary>
    [Fact]
    public void BeatIntoShape_Forge_Scales_With_Prior_Powered_Hits()
    {
        var player = Catalog.BuildPlayer(new List<CardModel> { new BeatIntoShape() }, currentHp: 80, maxHp: 80);
        var m = Monsters.CalcifiedCultist(hp: 200);
        var combat = Catalog.SetupCombat(player, new[] { m });
        player.ResetEnergy();

        Play(combat, new StrikeRegent(), m);
        Play(combat, new StrikeRegent(), m);
        Assert.Equal(2, m.PlayerPoweredHitsThisTurn);

        Play(combat, new BeatIntoShape(), m);   // priorHits=2 → Forge 5 × 3 = 15
        Assert.Equal(10 + 15, player.Hand.OfType<SovereignBlade>().Single().Damage);
    }

    /// <summary>The per-target powered-hit counter is per-turn: it resets at the next player-turn start, so a
    /// BeatIntoShape played on a fresh turn (despite hits on a prior turn) forges only its base.</summary>
    [Fact]
    public void BeatIntoShape_PoweredHit_Counter_Resets_Each_Turn()
    {
        var player = Catalog.BuildPlayer(new List<CardModel> { new BeatIntoShape() }, currentHp: 80, maxHp: 80);
        var m = Monsters.CalcifiedCultist(hp: 200);
        var combat = Catalog.SetupCombat(player, new[] { m });
        player.ResetEnergy();

        Play(combat, new StrikeRegent(), m);
        Assert.Equal(1, m.PlayerPoweredHitsThisTurn);
        CombatManager.BeginPlayerTurn(combat);
        Assert.Equal(0, m.PlayerPoweredHitsThisTurn);
    }

    [Fact]
    public void SovereignBlade_Is_Retained_At_Turn_End()
    {
        var (c, p, m) = Fight();
        Play(c, new WroughtInWar(), m);
        Assert.Single(p.Hand.OfType<SovereignBlade>());
        CombatManager.EndPlayerTurn(c);
        Assert.Single(p.Hand.OfType<SovereignBlade>());   // retained, not discarded
        Assert.Empty(p.DiscardPile.OfType<SovereignBlade>());
    }

    [Fact]
    public void Parry_Gives_SovereignBlade_Block_On_Play()
    {
        var (c, p, m) = Fight();
        Play(c, new Parry(), null);          // ParryPower 10
        Play(c, new WroughtInWar(), m);      // forge a blade
        var blade = p.Hand.OfType<SovereignBlade>().Single();
        Play(c, blade, m);
        Assert.Equal(10, p.Block);           // gained Parry-worth of block
    }

    [Fact]
    public void SeekingEdge_Makes_SovereignBlade_Hit_All()
    {
        var (c, p, a, b) = Fight2();
        Play(c, new SeekingEdge(), null);    // SeekingEdge + Forge 7 → blade 17
        var blade = p.Hand.OfType<SovereignBlade>().Single();
        Play(c, blade, a);
        Assert.Equal(60 - 17, a.CurrentHp);
        Assert.Equal(60 - 17, b.CurrentHp);  // both enemies hit
    }

    [Fact]
    public void SwordSage_Adds_A_SovereignBlade_Replay()
    {
        var (c, p, m) = Fight(monsterHp: 100);
        Play(c, new WroughtInWar(), m);      // 7 dmg → m=93, blade 17
        Play(c, new SwordSage(), null);      // +1 replay
        var blade = p.Hand.OfType<SovereignBlade>().Single();
        Play(c, blade, m);                   // hits twice: 17+17 = 34
        Assert.Equal(100 - 7 - 34, m.CurrentHp);
    }

    [Fact]
    public void Conqueror_Doubles_SovereignBlade_Damage_To_Target()
    {
        var (c, p, m) = Fight(monsterHp: 100);
        Play(c, new WroughtInWar(), m);      // 7 dmg → m=93, blade 17
        Play(c, new Conqueror(), m);         // Forge 3 (blade 20) + ConquerorPower on m
        var blade = p.Hand.OfType<SovereignBlade>().Single();
        Play(c, blade, m);                   // 20 × 2 = 40
        Assert.Equal(100 - 7 - 40, m.CurrentHp);
    }

    [Fact]
    public void SummonForth_Pulls_Blades_Into_Hand()
    {
        var (c, p, m) = Fight();
        Play(c, new WroughtInWar(), m);
        var blade = p.Hand.OfType<SovereignBlade>().Single();
        p.Hand.Remove(blade);
        p.DiscardPile.Add(blade);            // blade now in discard
        Play(c, new SummonForth(), null);    // pulls it back + Forge 8
        Assert.Contains(blade, p.Hand);
        Assert.Equal(10 + 7 + 8, blade.Damage);
    }

    // ---- Star-payback powers ----

    [Fact]
    public void ChildOfTheStars_Blocks_Per_Star_Spent()
    {
        var (c, p, m) = Fight(stars: 5);
        Play(c, new ChildOfTheStars(), null);   // 2 block per star spent
        Play(c, new FallingStar(), m);           // spends 2★ → 4 block
        Assert.Equal(2 * 2, p.Block);
    }

    [Fact]
    public void BlackHole_Damages_All_On_Star_Gain_And_Spend()
    {
        var (c, p, a, b) = Fight2(stars: 0);
        Play(c, new BlackHole(), null);          // 3 dmg to all on any star event
        Play(c, new Venerate(), null);           // gains 2 stars → 3 to each enemy
        Assert.Equal(60 - 3, a.CurrentHp);
        Assert.Equal(60 - 3, b.CurrentHp);
    }

    // ---- Resource-per-turn / next-turn ----

    [Fact]
    public void Genesis_Gains_Stars_At_Next_Turn_Start()
    {
        var (c, p, _) = Fight();
        Play(c, new Genesis(), null);
        Assert.Equal(0, p.Stars);                // not this turn
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(2, p.Stars);                // 2 at the next turn start
    }

    [Fact]
    public void Furnace_Forges_At_Next_Turn_Start()
    {
        var (c, p, _) = Fight();
        Play(c, new Furnace(), null);
        Assert.Empty(p.Hand.OfType<SovereignBlade>());
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(10 + 5, p.Hand.OfType<SovereignBlade>().Single().Damage);
    }

    [Fact]
    public void Hegemony_Grants_Energy_Next_Turn()
    {
        var (c, p, m) = Fight();
        p.MaxEnergy = 3;
        Play(c, new Hegemony(), m);
        Assert.Equal(60 - 15, m.CurrentHp);
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(3 + 2, p.Energy);           // reset to 3 then +2
        Assert.False(p.HasPower("EnergyNextTurn"));
    }

    [Fact]
    public void HiddenCache_Gives_Star_Now_And_More_Next_Turn()
    {
        var (c, p, _) = Fight();
        Play(c, new HiddenCache(), null);
        Assert.Equal(1, p.Stars);
        CombatManager.BeginPlayerTurn(c);
        Assert.Equal(1 + 3, p.Stars);
    }

    [Fact]
    public void Orbit_Grants_Energy_Per_4_Spent()
    {
        var (c, p, m) = Fight();
        p.MaxEnergy = 9;
        p.ResetEnergy();
        Play(c, new Orbit(), null);              // 2 energy spent so far → progress 2
        Play(c, new KinglyKick(), m);            // 4 energy spent → progress 6 → +1 energy, progress 2
        // After Orbit(2)+KinglyKick(4)=6 spent: one 4-threshold crossed → +1 energy granted.
        Assert.Equal(1, p.GetPowerAmount("Orbit"));
    }

    // ---- Temporary-Strength debuffs ----

    [Fact]
    public void CrushUnder_Lowers_Enemy_Strength_Until_Their_Turn_End()
    {
        var (c, p, m) = Fight();
        Play(c, new CrushUnder(), m);
        Assert.Equal(60 - 7, m.CurrentHp);
        Assert.Equal(-1, m.GetPowerAmount("Strength"));   // temporary -1 Strength applied
        CombatManager.EndPlayerTurn(c);
        CombatManager.RunEnemyTurn(c);
        Assert.Equal(0, m.GetPowerAmount("Strength"));    // restored at enemy turn end
    }

    // ---- Strength / Monologue ----

    [Fact]
    public void Resonance_Buffs_Self_And_Debuffs_Enemies()
    {
        var (c, p, a, b) = Fight2(stars: 3);
        Play(c, new Resonance(), null);
        Assert.Equal(1, p.GetPowerAmount("Strength"));
        Assert.Equal(-1, a.GetPowerAmount("Strength"));
        Assert.Equal(-1, b.GetPowerAmount("Strength"));
    }

    [Fact]
    public void Monologue_Gains_Strength_Per_Card_Played()
    {
        var (c, p, m) = Fight();
        Play(c, new Monologue(), null);          // power applied; the Monologue card itself doesn't trigger
        Assert.Equal(0, p.GetPowerAmount("Strength"));
        Play(c, new StrikeRegent(), m);          // +1 Strength
        Assert.Equal(1, p.GetPowerAmount("Strength"));
        Play(c, new StrikeRegent(), m);          // +1 Strength
        Assert.Equal(2, p.GetPowerAmount("Strength"));
        CombatManager.EndPlayerTurn(c);
        Assert.Equal(0, p.GetPowerAmount("Strength"));   // all gained Strength lost at turn end
    }

    [Fact]
    public void MonarchsGaze_Lowers_Enemy_Strength_On_Powered_Attack()
    {
        var (c, p, m) = Fight();
        Play(c, new MonarchsGaze(), null);
        Play(c, new StrikeRegent(), m);
        Assert.Equal(-1, m.GetPowerAmount("Strength"));
    }

    // ---- Damage / block / scaling attacks ----

    [Fact]
    public void StrikeRegent_And_DefendRegent_Faithful()
    {
        var (c, p, m) = Fight();
        Play(c, new StrikeRegent(), m);
        Assert.Equal(60 - 6, m.CurrentHp);
        Play(c, new DefendRegent(), null);
        Assert.Equal(5, p.Block);
    }

    [Fact]
    public void SevenStars_Hits_All_Seven_Times()
    {
        var (c, p, a, b) = Fight2(hp: 100);
        Play(c, new SevenStars(), null);
        Assert.Equal(100 - 49, a.CurrentHp);
        Assert.Equal(100 - 49, b.CurrentHp);
    }

    [Fact]
    public void HeavenlyDrill_Hits_Per_Energy_And_Doubles_At_4()
    {
        var (c, p, m) = Fight(monsterHp: 200);
        p.MaxEnergy = 4; p.ResetEnergy();
        Play(c, new HeavenlyDrill(), m);     // X=4 energy → 4 hits, doubled to 8 → 8×8=64
        Assert.Equal(200 - 64, m.CurrentHp);
    }

    [Fact]
    public void Stardust_Hits_Per_Star_Spent()
    {
        var (c, p, m) = Fight(monsterHp: 200, stars: 4);
        Play(c, new Stardust(), m);          // X-star: spends all 4 stars → 4 hits × 5
        Assert.Equal(0, p.Stars);
        Assert.Equal(200 - 20, m.CurrentHp);
    }

    [Fact]
    public void LunarBlast_Scales_With_Skills_Played_This_Turn()
    {
        var (c, p, m) = Fight(monsterHp: 100);
        CombatManager.BeginPlayerTurn(c);    // turn 1 (resets the per-turn skill counter)
        Play(c, new DefendRegent(), null);   // 1 skill
        Play(c, new GatherLight(), null);    // 2 skills (also gains a star)
        Play(c, new LunarBlast(), m);        // 4 dmg × 2 skills = 8
        Assert.Equal(100 - 8, m.CurrentHp);
    }

    [Fact]
    public void Radiate_Scales_With_Stars_Gained_This_Turn()
    {
        var (c, p, a, b) = Fight2(hp: 100);
        CombatManager.BeginPlayerTurn(c);
        Play(c, new Venerate(), null);       // +2 stars this turn
        Play(c, new Radiate(), null);        // 3 dmg × 2 to all
        Assert.Equal(100 - 6, a.CurrentHp);
        Assert.Equal(100 - 6, b.CurrentHp);
    }

    [Fact]
    public void CrescentSpear_Scales_With_StarCost_Cards_Owned()
    {
        var (c, p, m) = Fight(stars: 1);
        // Hand: the CrescentSpear (1★) + two more star-cost cards in draw pile.
        p.DrawPile.Add(new FallingStar());   // 2★
        p.DrawPile.Add(new Comet());         // 5★
        var spear = new CrescentSpear();     // itself 1★ — counts
        Play(c, spear, m);                   // 8 + 2 × (3 star-cost cards) = 14
        Assert.Equal(60 - 14, m.CurrentHp);
    }

    // ---- Repeatable / pile-returning ----

    [Fact]
    public void ShiningStrike_Returns_To_Draw_Pile()
    {
        var (c, p, m) = Fight();
        var ss = new ShiningStrike();
        Play(c, ss, m);
        Assert.Equal(60 - 8, m.CurrentHp);
        Assert.Equal(2, p.Stars);
        Assert.Contains(ss, p.DrawPile);
        Assert.DoesNotContain(ss, p.DiscardPile);
    }

    [Fact]
    public void ParticleWall_Returns_To_Hand()
    {
        var (c, p, _) = Fight(stars: 4);
        var pw = new ParticleWall();
        Play(c, pw, null);
        Assert.Equal(9, p.Block);
        Assert.Equal(2, p.Stars);            // spent 2★
        Assert.Contains(pw, p.Hand);
    }

    // ---- NeutronAegis / Plating reuse ----

    [Fact]
    public void NeutronAegis_Grants_Plating_Block_At_Turn_End()
    {
        var (c, p, _) = Fight(stars: 5);
        Play(c, new NeutronAegis(), null);   // PlatingPower 8
        Assert.Equal(8, p.GetPowerAmount("Plating"));
        CombatManager.EndPlayerTurn(c);
        Assert.Equal(8, p.Block);            // Plating grants block at the owner's turn end
    }

    // ---- Inert effects are HP-safe no-ops ----

    [Fact]
    public void Inert_Generation_Cards_Are_Safe_NoOps()
    {
        var (c, p, m) = Fight(stars: 4);
        foreach (CardModel card in new CardModel[] { new Begone(), new Charge(), new Guards(), new BundleOfJoy(),
                                                     new Quasar(), new Largesse(), new HammerTime() })
            Play(c, card, null);
        Assert.Equal(80, p.CurrentHp);       // no self-harm
        Assert.Equal(60, m.CurrentHp);       // no stray damage
    }

    // ---- Ancient (Stars-pool) cards: MeteorShower / TheSealedThrone ----
    // (these are the Stars-powered Ancient cards; the simplified star-free copies that once lived in the
    // Special pool were removed in favour of these canonical, StarCost-modelling definitions.)

    [Fact]
    public void MeteorShower_Hits_All_For_14_With_Weak_And_Vulnerable()
    {
        var (c, _, a, b) = Fight2(40, stars: 2);
        Play(c, new MeteorShower(), null);   // 2★, 0 energy
        Assert.Equal(40 - 14, a.CurrentHp);
        Assert.Equal(40 - 14, b.CurrentHp);
        Assert.Equal(2, a.GetPowerAmount("Weak"));
        Assert.Equal(2, a.GetPowerAmount("Vulnerable"));
        Assert.Equal(2, b.GetPowerAmount("Vulnerable"));
    }

    [Fact]
    public void MeteorShower_Upgraded_Deals_21()
    {
        var (c, _, m) = Fight(stars: 2);
        Play(c, (CardModel)new MeteorShower().Upgraded(1), null);   // StarCost stays 2★
        Assert.Equal(60 - 21, m.CurrentHp);
    }

    [Fact]
    public void TheSealedThrone_Gains_A_Star_Per_Card_Played()
    {
        var (c, p, m) = Fight(stars: 3);
        Play(c, new TheSealedThrone(), null);   // spends 3★; the game fires on BeforeCardPlayed, so the throne
        Assert.True(p.HasPower("TheSealedThrone"));
        Assert.Equal(0, p.Stars);               // card itself does NOT gain a star (power didn't exist yet)
        Play(c, new StrikeRegent(), m);         // each SUBSEQUENT card play gains a star
        Assert.Equal(1, p.Stars);
    }
}
