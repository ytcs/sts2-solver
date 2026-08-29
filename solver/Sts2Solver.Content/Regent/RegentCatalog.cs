using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Regent card registration + starter deck + the DivineRight starter relic.
// A partial of Catalog; Core's CardTables() aggregates RegentCardFactories
// alongside the other characters', and CardPool includes them. The Sovereign
// Blade token is registered in Core's CommonCardFactories (buildable by name,
// excluded from the deck-buildable pool — it is generated, never deck-built).
// ===========================================================================
public static partial class Catalog
{
    internal static readonly Dictionary<string, Func<CardModel>> RegentCardFactories = new(StringComparer.OrdinalIgnoreCase)
    {
        // Starters.
        ["StrikeRegent"] = () => new StrikeRegent(),
        ["DefendRegent"] = () => new DefendRegent(),
        ["FallingStar"] = () => new FallingStar(),
        ["Venerate"] = () => new Venerate(),
        // Attacks.
        ["AstralPulse"] = () => new AstralPulse(),
        ["BeatIntoShape"] = () => new BeatIntoShape(),
        ["Bombardment"] = () => new Bombardment(),
        ["CelestialMight"] = () => new CelestialMight(),
        ["CollisionCourse"] = () => new CollisionCourse(),
        ["Comet"] = () => new Comet(),
        ["CrashLanding"] = () => new CrashLanding(),
        ["CrescentSpear"] = () => new CrescentSpear(),
        ["CrushUnder"] = () => new CrushUnder(),
        ["Devastate"] = () => new Devastate(),
        ["DyingStar"] = () => new DyingStar(),
        ["GammaBlast"] = () => new GammaBlast(),
        ["GuidingStar"] = () => new GuidingStar(),
        ["HeavenlyDrill"] = () => new HeavenlyDrill(),
        ["Hegemony"] = () => new Hegemony(),
        ["HeirloomHammer"] = () => new HeirloomHammer(),
        ["KinglyKick"] = () => new KinglyKick(),
        ["KinglyPunch"] = () => new KinglyPunch(),
        ["KnockoutBlow"] = () => new KnockoutBlow(),
        ["LunarBlast"] = () => new LunarBlast(),
        ["MakeItSo"] = () => new MakeItSo(),
        ["MeteorShower"] = () => new MeteorShower(),
        ["Radiate"] = () => new Radiate(),
        ["Resonance"] = () => new Resonance(),
        ["SevenStars"] = () => new SevenStars(),
        ["ShiningStrike"] = () => new ShiningStrike(),
        ["SolarStrike"] = () => new SolarStrike(),
        ["Stardust"] = () => new Stardust(),
        ["Supermassive"] = () => new Supermassive(),
        ["WroughtInWar"] = () => new WroughtInWar(),
        // Block / skills.
        ["Alignment"] = () => new Alignment(),
        ["Bulwark"] = () => new Bulwark(),
        ["CloakOfStars"] = () => new CloakOfStars(),
        ["Conqueror"] = () => new Conqueror(),
        ["Convergence"] = () => new Convergence(),
        ["CosmicIndifference"] = () => new CosmicIndifference(),
        ["DecisionsDecisions"] = () => new DecisionsDecisions(),
        ["GatherLight"] = () => new GatherLight(),
        ["Glimmer"] = () => new Glimmer(),
        ["Glitterstream"] = () => new Glitterstream(),
        ["Glow"] = () => new Glow(),
        ["HiddenCache"] = () => new HiddenCache(),
        ["IAmInvincible"] = () => new IAmInvincible(),
        ["KnowThyPlace"] = () => new KnowThyPlace(),
        ["ManifestAuthority"] = () => new ManifestAuthority(),
        ["ParticleWall"] = () => new ParticleWall(),
        ["Patter"] = () => new Patter(),
        ["PhotonCut"] = () => new PhotonCut(),
        ["Prophesize"] = () => new Prophesize(),
        ["Reflect"] = () => new Reflect(),
        ["RefineBlade"] = () => new RefineBlade(),
        ["RoyalGamble"] = () => new RoyalGamble(),
        ["SeekingEdge"] = () => new SeekingEdge(),
        ["SpoilsOfBattle"] = () => new SpoilsOfBattle(),
        ["SummonForth"] = () => new SummonForth(),
        ["Terraforming"] = () => new Terraforming(),
        ["TheSmith"] = () => new TheSmith(),
        // Power cards.
        ["Arsenal"] = () => new Arsenal(),
        ["BigBang"] = () => new BigBang(),
        ["BlackHole"] = () => new BlackHole(),
        ["ChildOfTheStars"] = () => new ChildOfTheStars(),
        ["ForegoneConclusion"] = () => new ForegoneConclusion(),
        ["Furnace"] = () => new Furnace(),
        ["Genesis"] = () => new Genesis(),
        ["MonarchsGaze"] = () => new MonarchsGaze(),
        ["Monologue"] = () => new Monologue(),
        ["NeutronAegis"] = () => new NeutronAegis(),
        ["Orbit"] = () => new Orbit(),
        ["PaleBlueDot"] = () => new PaleBlueDot(),
        ["Parry"] = () => new Parry(),
        ["PillarOfCreation"] = () => new PillarOfCreation(),
        ["Royalties"] = () => new Royalties(),
        ["SpectrumShift"] = () => new SpectrumShift(),
        ["SwordSage"] = () => new SwordSage(),
        ["TheSealedThrone"] = () => new TheSealedThrone(),
        ["Tyranny"] = () => new Tyranny(),
        ["VoidForm"] = () => new VoidForm(),
        // Card-generation / selection / transform / multiplayer (HP-neutral; inert effects, see RegentCards.cs).
        ["Begone"] = () => new Begone(),
        ["BundleOfJoy"] = () => new BundleOfJoy(),
        ["Charge"] = () => new Charge(),
        ["Guards"] = () => new Guards(),
        ["HammerTime"] = () => new HammerTime(),
        ["Largesse"] = () => new Largesse(),
        ["Plot"] = () => new Plot(),
        ["Constellation"] = () => new Constellation(),
        ["Tutor"] = () => new Tutor(),
        ["Quasar"] = () => new Quasar(),
    };

    /// <summary>The Regent starting deck: 4 Strike, 4 Defend, 1 FallingStar, 1 Venerate. Starting relic
    /// DivineRight (grants 3 Stars at combat start). (MegaCrit Regent.StartingDeck — StartingHp 75.)</summary>
    public static List<CardModel> RegentStarterDeck()
    {
        var deck = new List<CardModel>();
        for (int i = 0; i < 4; i++) deck.Add(new StrikeRegent());
        for (int i = 0; i < 4; i++) deck.Add(new DefendRegent());
        deck.Add(new FallingStar());
        deck.Add(new Venerate());
        return deck;
    }
}

/// <summary>Regent starter relic: gain 3 Stars at the start of each combat. (MegaCrit DivineRight.)</summary>
public sealed class DivineRight : RelicModel
{
    public override string Id => "DivineRight";
    public override void OnCombatStart(CombatState combat) => Cmd.GainStars(combat, 3);
}
