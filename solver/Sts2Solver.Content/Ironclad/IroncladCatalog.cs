using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>Ironclad card + relic registration and starter deck. A partial of <see cref="Catalog"/>;
/// <c>BuildCard</c> aggregates this table via <c>CardTables()</c>. Shared status cards live in Core;
/// monster factories live in Monsters/.</summary>
public static partial class Catalog
{
    internal static readonly Dictionary<string, Func<CardModel>> IroncladCardFactories = new(StringComparer.OrdinalIgnoreCase)
    {
        ["StrikeIronclad"] = () => new StrikeIronclad(),
        ["DefendIronclad"] = () => new DefendIronclad(),
        ["Bash"] = () => new Bash(),
        ["Bludgeon"] = () => new Bludgeon(),
        ["Inflame"] = () => new Inflame(),
        ["IronWave"] = () => new IronWave(),
        ["TwinStrike"] = () => new TwinStrike(),
        ["Uppercut"] = () => new Uppercut(),
        ["Hemokinesis"] = () => new Hemokinesis(),
        ["Thunderclap"] = () => new Thunderclap(),
        ["Stomp"] = () => new Stomp(),
        ["MoltenFist"] = () => new MoltenFist(),
        ["HowlFromBeyond"] = () => new HowlFromBeyond(),
        ["Break"] = () => new Break(),
        ["Breakthrough"] = () => new Breakthrough(),
        ["Taunt"] = () => new Taunt(),
        ["Tremble"] = () => new Tremble(),
        ["Rage"] = () => new Rage(),
        ["DemonForm"] = () => new DemonForm(),
        ["BodySlam"] = () => new BodySlam(),
        ["Bully"] = () => new Bully(),
        ["AshenStrike"] = () => new AshenStrike(),
        ["FlameBarrier"] = () => new FlameBarrier(),
        ["Mangle"] = () => new Mangle(),
        ["SetupStrike"] = () => new SetupStrike(),
        ["Impervious"] = () => new Impervious(),
        ["BloodWall"] = () => new BloodWall(),
        ["Colossus"] = () => new Colossus(),
        ["StoneArmor"] = () => new StoneArmor(),
        ["Dominate"] = () => new Dominate(),
        ["NotYet"] = () => new NotYet(),
        ["Conflagration"] = () => new Conflagration(),
        ["Rupture"] = () => new Rupture(),
        ["Juggernaut"] = () => new Juggernaut(),
        ["Barricade"] = () => new Barricade(),
        ["Feed"] = () => new Feed(),
        ["ShrugItOff"] = () => new ShrugItOff(),
        ["PommelStrike"] = () => new PommelStrike(),
        ["BattleTrance"] = () => new BattleTrance(),
        ["FeelNoPain"] = () => new FeelNoPain(),
        ["DarkEmbrace"] = () => new DarkEmbrace(),
        ["Bloodletting"] = () => new Bloodletting(),
        ["Offering"] = () => new Offering(),
        ["Whirlwind"] = () => new Whirlwind(),
        ["Anger"] = () => new Anger(),
        ["Headbutt"] = () => new Headbutt(),
        ["SwordBoomerang"] = () => new SwordBoomerang(),
        ["Pyre"] = () => new Pyre(),
        ["EvilEye"] = () => new EvilEye(),
        ["ForgottenRitual"] = () => new ForgottenRitual(),
        ["OneTwoPunch"] = () => new OneTwoPunch(),
        ["Unrelenting"] = () => new Unrelenting(),
        ["Corruption"] = () => new Corruption(),
        ["Spite"] = () => new Spite(),
        ["Cruelty"] = () => new Cruelty(),
        ["CrimsonMantle"] = () => new CrimsonMantle(),
        ["Unmovable"] = () => new Unmovable(),
        ["Juggling"] = () => new Juggling(),
        ["FiendFire"] = () => new FiendFire(),
        ["SecondWind"] = () => new SecondWind(),
        ["TrueGrit"] = () => new TrueGrit(),
        ["Cinder"] = () => new Cinder(),
        ["BurningPact"] = () => new BurningPact(),
        ["Brand"] = () => new Brand(),
        ["Inferno"] = () => new Inferno(),
        ["DrumOfBattle"] = () => new DrumOfBattle(),
        ["Pillage"] = () => new Pillage(),
        ["ExpectAFight"] = () => new ExpectAFight(),
        ["Vicious"] = () => new Vicious(),
        ["Dismantle"] = () => new Dismantle(),
        ["PerfectedStrike"] = () => new PerfectedStrike(),
        ["Rampage"] = () => new Rampage(),
        ["InfernalBlade"] = () => new InfernalBlade(),
        ["Stoke"] = () => new Stoke(),
        ["Armaments"] = () => new Armaments(),
        ["PactsEnd"] = () => new PactsEnd(),
        ["Thrash"] = () => new Thrash(),
        ["Aggression"] = () => new Aggression(),
        ["Stampede"] = () => new Stampede(),
        ["Hellraiser"] = () => new Hellraiser(),
        ["PrimalForce"] = () => new PrimalForce(),
        ["Havoc"] = () => new Havoc(),
        ["Cascade"] = () => new Cascade(),
        ["DemonicShield"] = () => new DemonicShield(),
        ["Tank"] = () => new Tank(),
        ["Midnight"] = () => new Midnight(),
        ["Blaze"] = () => new Blaze(),
        ["Outrage"] = () => new Outrage(),
        ["FightMe"] = () => new FightMe(),
        ["TearAsunder"] = () => new TearAsunder(),
    };

    // The relic registry lives centrally in Core/RelicCatalog.cs (Catalog.RelicFactories), so the combat
    // relic pool can grow without editing this character file.

    /// <summary>The Ironclad starting deck: 5 Strike, 4 Defend, 1 Bash.</summary>
    public static List<CardModel> IroncladStarterDeck()
    {
        var deck = new List<CardModel>();
        for (int i = 0; i < 5; i++) deck.Add(new StrikeIronclad());
        for (int i = 0; i < 4; i++) deck.Add(new DefendIronclad());
        deck.Add(new Bash());
        return deck;
    }
}
