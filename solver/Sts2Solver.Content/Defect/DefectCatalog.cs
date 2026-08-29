using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>Defect card + relic registration and starter deck. A partial of <see cref="Catalog"/>;
/// <c>BuildCard</c> aggregates this table via <c>CardTables()</c>. The orb subsystem lives in the engine
/// (Orbs.cs); these cards channel/evoke through <see cref="OrbOps"/>. First batch — expanded as the rest of
/// the 88-card pool is ported.</summary>
public static partial class Catalog
{
    internal static readonly Dictionary<string, Func<CardModel>> DefectCardFactories = new(StringComparer.OrdinalIgnoreCase)
    {
        ["StrikeDefect"] = () => new StrikeDefect(),
        ["DefendDefect"] = () => new DefendDefect(),
        ["Zap"] = () => new Zap(),
        ["Dualcast"] = () => new Dualcast(),
        ["BallLightning"] = () => new BallLightning(),
        ["Coolheaded"] = () => new Coolheaded(),
        ["ColdSnap"] = () => new ColdSnap(),
        ["BeamCell"] = () => new BeamCell(),
        ["Barrage"] = () => new Barrage(),
        ["Chill"] = () => new Chill(),
        ["Glacier"] = () => new Glacier(),
        ["Capacitor"] = () => new Capacitor(),
        ["Darkness"] = () => new Darkness(),
        ["Defragment"] = () => new Defragment(),
        ["BootSequence"] = () => new BootSequence(),
        ["Leap"] = () => new Leap(),
        ["Glasswork"] = () => new Glasswork(),
        ["ShadowShield"] = () => new ShadowShield(),
        ["ChargeBattery"] = () => new ChargeBattery(),
        ["Skim"] = () => new Skim(),
        ["Supercritical"] = () => new Supercritical(),
        ["Fusion"] = () => new Fusion(),
        ["Rainbow"] = () => new Rainbow(),
        ["Refract"] = () => new Refract(),
        ["IceLance"] = () => new IceLance(),
        ["MeteorStrike"] = () => new MeteorStrike(),
        ["SweepingBeam"] = () => new SweepingBeam(),
        ["Null"] = () => new Null(),
        ["Thunder"] = () => new Thunder(),
        ["Hailstorm"] = () => new Hailstorm(),
        ["Storm"] = () => new Storm(),
        ["Subroutine"] = () => new Subroutine(),
        ["Coolant"] = () => new Coolant(),
        ["Smokestack"] = () => new Smokestack(),
        ["Loop"] = () => new Loop(),
        ["Spinner"] = () => new Spinner(),
        ["LightningRod"] = () => new LightningRod(),
        ["BiasedCognition"] = () => new BiasedCognition(),
        ["ConsumingShadow"] = () => new ConsumingShadow(),
        ["Buffer"] = () => new Buffer(),
        ["Iteration"] = () => new Iteration(),
        ["Hotfix"] = () => new Hotfix(),
        ["FocusedStrike"] = () => new FocusedStrike(),
        ["Synchronize"] = () => new Synchronize(),
        ["Synthesis"] = () => new Synthesis(),
        ["SignalBoost"] = () => new SignalBoost(),
        ["EchoForm"] = () => new EchoForm(),
        ["MachineLearning"] = () => new MachineLearning(),
        ["TrashToTreasure"] = () => new TrashToTreasure(),
        ["CreativeAi"] = () => new CreativeAi(),
        ["Feral"] = () => new Feral(),
        ["BoostAway"] = () => new BoostAway(),
        ["FightThrough"] = () => new FightThrough(),
        ["GunkUp"] = () => new GunkUp(),
        ["Overclock"] = () => new Overclock(),
        ["Turbo"] = () => new Turbo(),
        ["GoForTheEyes"] = () => new GoForTheEyes(),
        ["Hyperbeam"] = () => new Hyperbeam(),
        ["Sunder"] = () => new Sunder(),
        ["RocketPunch"] = () => new RocketPunch(),
        ["DoubleEnergy"] = () => new DoubleEnergy(),
        ["EnergySurge"] = () => new EnergySurge(),
        ["Ignition"] = () => new Ignition(),
        ["BulkUp"] = () => new BulkUp(),
        ["Compact"] = () => new Compact(),
        ["CompileDriver"] = () => new CompileDriver(),
        ["AdaptiveStrike"] = () => new AdaptiveStrike(),
        ["AllForOne"] = () => new AllForOne(),
        ["Hologram"] = () => new Hologram(),
        ["Scavenge"] = () => new Scavenge(),
        ["Scrape"] = () => new Scrape(),
        ["Reboot"] = () => new Reboot(),
        ["MomentumStrike"] = () => new MomentumStrike(),
        ["Modded"] = () => new Modded(),
        ["GeneticAlgorithm"] = () => new GeneticAlgorithm(),
        ["Claw"] = () => new Claw(),
        ["HelixDrill"] = () => new HelixDrill(),
        ["FlakCannon"] = () => new FlakCannon(),
        ["Ftl"] = () => new Ftl(),
        ["TeslaCoil"] = () => new TeslaCoil(),
        ["Quadcast"] = () => new Quadcast(),
        ["Shatter"] = () => new Shatter(),
        ["MultiCast"] = () => new MultiCast(),
        ["Tempest"] = () => new Tempest(),
        ["Voltaic"] = () => new Voltaic(),
        ["Uproar"] = () => new Uproar(),
        ["Chaos"] = () => new Chaos(),
        ["WhiteNoise"] = () => new WhiteNoise(),
        ["Hibernate"] = () => new Hibernate(),
        ["OneForAll"] = () => new OneForAll(),
        ["ImitationLearning"] = () => new ImitationLearning(),
    };

    /// <summary>The Defect starting deck: 4 Strike, 4 Defend, 1 Zap, 1 Dualcast. Starts at 75 HP with the
    /// Cracked Core relic (3 orb slots + a Lightning orb channeled on turn 1).</summary>
    public static List<CardModel> DefectStarterDeck()
    {
        var deck = new List<CardModel>();
        for (int i = 0; i < 4; i++) deck.Add(new StrikeDefect());
        for (int i = 0; i < 4; i++) deck.Add(new DefendDefect());
        deck.Add(new Zap());
        deck.Add(new Dualcast());
        return deck;
    }
}

/// <summary>Defect starting relic. Grants the Defect's 3 orb slots and channels a Lightning orb at combat
/// start (the game channels it on turn 1; our combat-start channel is equivalent — the orb is present for
/// turn 1 either way). The 3-slot capacity is the character's BaseOrbSlotCount, delivered here as the always-
/// present starter (mirroring how BoundPhylactery delivers the Necrobinder's Osty). (Game: CrackedCore.)</summary>
public sealed class CrackedCore : RelicModel
{
    public override string Id => "CrackedCore";
    public override void OnCombatStart(CombatState combat)
    {
        combat.Player.OrbSlots = 3;
        OrbOps.Channel(combat, new LightningOrb());
    }
}
