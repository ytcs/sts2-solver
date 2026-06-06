using Sts2Solver.Engine;

namespace Sts2Solver.Content;

public static partial class Catalog
{
    /// <summary>Every relic the engine models, keyed by the game's relic class name — which is also the
    /// <c>Catalog.BuildRelic</c> key, the decompile file name, and the PascalCased run-save relic id (so
    /// ranwid's generic <c>GameIds.ModelledRelicName</c> picks any entry up automatically). The four character
    /// starter relics are defined in their character files; the combat pool lives in <c>Relics/</c>.
    /// <para>Scope is the to-do's "combat-affecting relics" (in-combat HP/block/damage/energy hooks), NOT the
    /// full cosmetic/economy/map/reward catalog. Every entry is deterministic ⇒ exact for the objective.</para></summary>
    private static readonly Dictionary<string, Func<RelicModel>> RelicFactories = new(StringComparer.OrdinalIgnoreCase)
    {
        // Character starter relics (combat-start mechanics: heal / Stars / Osty / orbs).
        ["BurningBlood"]     = () => new BurningBlood(),       // Ironclad   (Ironclad/IroncladRelics.cs)
        ["DivineRight"]      = () => new DivineRight(),        // Regent     (Regent/RegentCatalog.cs)
        ["BoundPhylactery"]  = () => new BoundPhylactery(),    // Necrobinder(Necrobinder/NecrobinderCatalog.cs)
        ["CrackedCore"]      = () => new CrackedCore(),        // Defect     (Defect/DefectCatalog.cs)

        // --- Combat pool, batch 1 (Relics/CombatRelics.cs) ---
        // Combat-start stat/block/power grants:
        ["Anchor"]           = () => new Anchor(),
        ["BronzeScales"]     = () => new BronzeScales(),
        ["Vajra"]            = () => new Vajra(),
        ["OddlySmoothStone"] = () => new OddlySmoothStone(),
        ["DataDisk"]         = () => new DataDisk(),
        ["Gorget"]           = () => new Gorget(),
        ["Akabeko"]          = () => new Akabeko(),
        ["BagOfMarbles"]     = () => new BagOfMarbles(),
        ["RedMask"]          = () => new RedMask(),
        ["BloodVial"]        = () => new BloodVial(),
        // Recurring / turn-numbered turn-start effects:
        ["Sai"]              = () => new Sai(),
        ["Brimstone"]        = () => new Brimstone(),
        ["Lantern"]          = () => new Lantern(),
        ["VeryHotCocoa"]     = () => new VeryHotCocoa(),
        ["Candelabra"]       = () => new Candelabra(),
        ["Chandelier"]       = () => new Chandelier(),
        ["FestivePopper"]    = () => new FestivePopper(),
        // Passive modifiers (install a hidden relic power):
        ["Ectoplasm"]        = () => new Ectoplasm(),
        ["StrikeDummy"]      = () => new StrikeDummy(),
        ["FakeStrikeDummy"]  = () => new FakeStrikeDummy(),
        ["MiniatureCannon"]  = () => new MiniatureCannon(),

        // --- Combat pool, batch 2 (HP-loss reducers + passive modifiers) ---
        ["TungstenRod"]      = () => new TungstenRod(),
        ["TheBoot"]          = () => new TheBoot(),
        ["SpikedGauntlets"]  = () => new SpikedGauntlets(),
        ["PaelsBlood"]       = () => new PaelsBlood(),
        ["BlessedAntler"]    = () => new BlessedAntler(),

        // --- Combat pool, batch 3 (stateless event-hook relics: combat-start / turn / on-play / on-exhaust / end-of-turn) ---
        ["FakeAnchor"]         = () => new FakeAnchor(),
        ["TwistedFunnel"]      = () => new TwistedFunnel(),
        ["BloodSoakedRose"]    = () => new BloodSoakedRose(),
        ["PrismaticGem"]       = () => new PrismaticGem(),
        ["Sozu"]               = () => new Sozu(),
        ["Fiddle"]             = () => new Fiddle(),
        ["FakeBloodVial"]      = () => new FakeBloodVial(),
        ["DivineDestiny"]      = () => new DivineDestiny(),
        ["Bread"]              = () => new Bread(),
        ["PaelsFlesh"]         = () => new PaelsFlesh(),
        ["CaptainsWheel"]      = () => new CaptainsWheel(),
        ["HornCleat"]          = () => new HornCleat(),
        ["SparklingRouge"]     = () => new SparklingRouge(),
        ["MercuryHourglass"]   = () => new MercuryHourglass(),
        ["MrStruggles"]        = () => new MrStruggles(),
        ["RoyalPoison"]        = () => new RoyalPoison(),
        ["RunicCapacitor"]     = () => new RunicCapacitor(),
        ["BagOfPreparation"]   = () => new BagOfPreparation(),
        ["RingOfTheSnake"]     = () => new RingOfTheSnake(),
        ["RingOfTheDrake"]     = () => new RingOfTheDrake(),
        ["BigMushroom"]        = () => new BigMushroom(),
        ["Orichalcum"]         = () => new Orichalcum(),
        ["FakeOrichalcum"]     = () => new FakeOrichalcum(),
        ["CloakClasp"]         = () => new CloakClasp(),
        ["RippleBasin"]        = () => new RippleBasin(),
        ["ScreamingFlagon"]    = () => new ScreamingFlagon(),
        ["StoneCalendar"]      = () => new StoneCalendar(),
        ["LunarPastry"]        = () => new LunarPastry(),
        ["IntimidatingHelmet"] = () => new IntimidatingHelmet(),
        ["IvoryTile"]          = () => new IvoryTile(),
        ["DaughterOfTheWind"]  = () => new DaughterOfTheWind(),
        ["LostWisp"]           = () => new LostWisp(),
        ["GamePiece"]          = () => new GamePiece(),
        ["CharonsAshes"]       = () => new CharonsAshes(),
        ["SneckoSkull"]        = () => new SneckoSkull(),

        // --- Combat pool, batch 4 (stateful counter relics: hashed per-turn/per-combat counters via relic powers) ---
        ["Kunai"]              = () => new Kunai(),
        ["Shuriken"]           = () => new Shuriken(),
        ["OrnamentalFan"]      = () => new OrnamentalFan(),
        ["LetterOpener"]       = () => new LetterOpener(),
        ["Nunchaku"]           = () => new Nunchaku(),
        ["TuningFork"]         = () => new TuningFork(),
        ["IronClub"]           = () => new IronClub(),
        ["Permafrost"]         = () => new Permafrost(),
        ["RainbowRing"]        = () => new RainbowRing(),

        // --- Combat pool, batch 5 (every-N-turns + damage/stars/play-count reactors) ---
        ["HappyFlower"]        = () => new HappyFlower(),
        ["FakeHappyFlower"]    = () => new FakeHappyFlower(),
        ["Pendulum"]           = () => new Pendulum(),
        ["PollinousCore"]      = () => new PollinousCore(),
        ["CentennialPuzzle"]   = () => new CentennialPuzzle(),
        ["DemonTongue"]        = () => new DemonTongue(),
        ["GalacticDust"]       = () => new GalacticDust(),
        ["MiniRegent"]         = () => new MiniRegent(),
        ["BeatingRemnant"]     = () => new BeatingRemnant(),
        ["Vambrace"]           = () => new Vambrace(),
        ["ThrowingAxe"]        = () => new ThrowingAxe(),
        ["RuinedHelmet"]       = () => new RuinedHelmet(),
    };
}
