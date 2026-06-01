using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>Necrobinder card + relic registration and starter deck. A partial of <see cref="Catalog"/>;
/// <c>BuildCard</c> aggregates this table via <c>CardTables()</c>. The two tokens (Soul, SweepingGaze)
/// are registered as buildable cards but kept out of the deck-building pool.</summary>
public static partial class Catalog
{
    internal static readonly Dictionary<string, Func<CardModel>> NecrobinderCardFactories = new(StringComparer.OrdinalIgnoreCase)
    {
        ["StrikeNecrobinder"] = () => new StrikeNecrobinder(),
        ["DefendNecrobinder"] = () => new DefendNecrobinder(),
        ["Bodyguard"] = () => new Bodyguard(),
        ["Unleash"] = () => new Unleash(),
        ["Afterlife"] = () => new Afterlife(),
        ["BansheesCry"] = () => new BansheesCry(),
        ["BlightStrike"] = () => new BlightStrike(),
        ["BoneShards"] = () => new BoneShards(),
        ["BorrowedTime"] = () => new BorrowedTime(),
        ["Bury"] = () => new Bury(),
        ["Calcify"] = () => new Calcify(),
        ["CallOfTheVoid"] = () => new CallOfTheVoid(),
        ["CaptureSpirit"] = () => new CaptureSpirit(),
        ["Cleanse"] = () => new Cleanse(),
        ["Countdown"] = () => new Countdown(),
        ["DanseMacabre"] = () => new DanseMacabre(),
        ["DeathMarch"] = () => new DeathMarch(),
        ["Deathbringer"] = () => new Deathbringer(),
        ["DeathsDoor"] = () => new DeathsDoor(),
        ["Debilitate"] = () => new Debilitate(),
        ["Defile"] = () => new Defile(),
        ["Defy"] = () => new Defy(),
        ["Delay"] = () => new Delay(),
        ["Demesne"] = () => new Demesne(),
        ["DevourLife"] = () => new DevourLife(),
        ["Dirge"] = () => new Dirge(),
        ["DrainPower"] = () => new DrainPower(),
        ["Dredge"] = () => new Dredge(),
        ["Eidolon"] = () => new Eidolon(),
        ["EndOfDays"] = () => new EndOfDays(),
        ["EnfeeblingTouch"] = () => new EnfeeblingTouch(),
        ["Eradicate"] = () => new Eradicate(),
        ["Fear"] = () => new Fear(),
        ["Fetch"] = () => new Fetch(),
        ["Flatten"] = () => new Flatten(),
        ["ForbiddenGrimoire"] = () => new ForbiddenGrimoire(),
        ["Friendship"] = () => new Friendship(),
        ["GlimpseBeyond"] = () => new GlimpseBeyond(),
        ["GraveWarden"] = () => new GraveWarden(),
        ["Graveblast"] = () => new Graveblast(),
        ["Hang"] = () => new Hang(),
        ["Haunt"] = () => new Haunt(),
        ["HighFive"] = () => new HighFive(),
        ["Invoke"] = () => new Invoke(),
        ["LegionOfBone"] = () => new LegionOfBone(),
        ["Lethality"] = () => new Lethality(),
        ["Melancholy"] = () => new Melancholy(),
        ["Misery"] = () => new Misery(),
        ["NecroMastery"] = () => new NecroMastery(),
        ["NegativePulse"] = () => new NegativePulse(),
        ["Neurosurge"] = () => new Neurosurge(),
        ["NoEscape"] = () => new NoEscape(),
        ["Oblivion"] = () => new Oblivion(),
        ["Pagestorm"] = () => new Pagestorm(),
        ["Parse"] = () => new Parse(),
        ["Poke"] = () => new Poke(),
        ["Protector"] = () => new Protector(),
        ["PullAggro"] = () => new PullAggro(),
        ["PullFromBelow"] = () => new PullFromBelow(),
        ["Putrefy"] = () => new Putrefy(),
        ["Rattle"] = () => new Rattle(),
        ["Reanimate"] = () => new Reanimate(),
        ["Reap"] = () => new Reap(),
        ["ReaperForm"] = () => new ReaperForm(),
        ["Reave"] = () => new Reave(),
        ["RightHandHand"] = () => new RightHandHand(),
        ["Sacrifice"] = () => new Sacrifice(),
        ["Scourge"] = () => new Scourge(),
        ["SculptingStrike"] = () => new SculptingStrike(),
        ["Seance"] = () => new Seance(),
        ["SentryMode"] = () => new SentryMode(),
        ["Severance"] = () => new Severance(),
        ["SharedFate"] = () => new SharedFate(),
        ["Shroud"] = () => new Shroud(),
        ["SicEm"] = () => new SicEm(),
        ["SleightOfFlesh"] = () => new SleightOfFlesh(),
        ["Snap"] = () => new Snap(),
        ["SoulStorm"] = () => new SoulStorm(),
        ["Sow"] = () => new Sow(),
        ["SpiritOfAsh"] = () => new SpiritOfAsh(),
        ["Spur"] = () => new Spur(),
        ["Squeeze"] = () => new Squeeze(),
        ["TheScythe"] = () => new TheScythe(),
        ["TimesUp"] = () => new TimesUp(),
        ["Transfigure"] = () => new Transfigure(),
        ["Undeath"] = () => new Undeath(),
        ["Veilpiercer"] = () => new Veilpiercer(),
        ["Wisp"] = () => new Wisp(),
    };

    /// <summary>The Necrobinder starting deck: 4 Strike, 4 Defend, 1 Bodyguard, 1 Unleash. Starts at 66 HP
    /// with the Bound Phylactery relic (summons Osty before combat and each turn after the first).</summary>
    public static List<CardModel> NecrobinderStarterDeck()
    {
        var deck = new List<CardModel>();
        for (int i = 0; i < 4; i++) deck.Add(new StrikeNecrobinder());
        for (int i = 0; i < 4; i++) deck.Add(new DefendNecrobinder());
        deck.Add(new Bodyguard());
        deck.Add(new Unleash());
        return deck;
    }
}

/// <summary>Necrobinder starting relic. Summons Osty (1 HP) at combat start, and re-summons 1 at the start
/// of every turn after the first (growing Osty's MaxHp). (Game: BoundPhylactery.)</summary>
public sealed class BoundPhylactery : RelicModel
{
    public override string Id => "BoundPhylactery";
    public override void OnCombatStart(CombatState combat) => NecroOsty.Summon(combat, 1);
    public override void OnPlayerTurnStart(CombatState combat)
    {
        if (combat.TurnNumber != 1) NecroOsty.Summon(combat, 1);
    }
}
