using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>Registry + builders mapping names to engine objects. Each character contributes its own
/// card table in a partial of this class (Ironclad/, Silent/); shared status cards register here.
/// <c>CardTables()</c> aggregates them, so adding a character is one line.</summary>
public static partial class Catalog
{
    /// <summary>Status/curse cards shared across characters: status cards added to piles by monsters
    /// (Infection, Burn, Dazed) and the Ascension 5+ starting curse (AscendersBane). Defined in
    /// Core/StatusCards.cs.</summary>
    private static readonly Dictionary<string, Func<CardModel>> CommonCardFactories = new(StringComparer.OrdinalIgnoreCase)
    {
        ["Infection"] = () => new Infection(),
        ["Burn"] = () => new Burn(),
        ["Dazed"] = () => new Dazed(),
        ["AscendersBane"] = () => new AscendersBane(),
        // Curses (Core/Curses.cs) — buildable by name for a real run deck, excluded from the deck-buildable
        // CardPool (never deliberately added), like the status cards above.
        ["BadLuck"] = () => new BadLuck(),
        ["Decay"] = () => new Decay(),
        ["Regret"] = () => new Regret(),
        ["Doubt"] = () => new Doubt(),
        ["Shame"] = () => new Shame(),
        ["Clumsy"] = () => new Clumsy(),
        ["CurseOfTheBell"] = () => new CurseOfTheBell(),
        ["Folly"] = () => new Folly(),
        ["Greed"] = () => new Greed(),
        ["Injury"] = () => new Injury(),
        ["PoorSleep"] = () => new PoorSleep(),
        ["Writhe"] = () => new Writhe(),
        ["Guilty"] = () => new Guilty(),
        ["Debt"] = () => new Debt(),
        ["Normality"] = () => new Normality(),
        ["SporeMind"] = () => new SporeMind(),
        ["Enthralled"] = () => new Enthralled(),
    };

    /// <summary>Every registered card table, in lookup order. Add a character = add one line here.</summary>
    private static IEnumerable<IReadOnlyDictionary<string, Func<CardModel>>> CardTables()
    {
        yield return CommonCardFactories;
        yield return IroncladCardFactories;
        yield return SilentCardFactories;
        yield return ColorlessCardFactories;
        yield return SpecialCardFactories;
    }

    /// <summary>The deck-buildable card pool: every registered character + colorless card (Ironclad + Silent
    /// + Colorless), excluding the shared status/curse cards (Burn, Dazed, Infection, AscendersBane) which are
    /// never deliberately added to a deck. Used by <see cref="TrainingFixtures"/> to draw randomised decks for
    /// the Phase-C VF training corpus, and by the advisor's card-name auto-complete/auto-correct.</summary>
    public static IReadOnlyList<string> CardPool =>
        _cardPool ??= IroncladCardFactories.Keys
            .Concat(SilentCardFactories.Keys)
            .Concat(ColorlessCardFactories.Keys)
            .Concat(SpecialCardFactories.Keys)
            .ToArray();
    private static string[]? _cardPool;

    /// <summary>Parse a card spec like "Bash" or "Bash+1" (the +N suffix is upgrade level).</summary>
    public static CardModel BuildCard(string spec)
    {
        int upgrades = 0;
        var name = spec;
        int plus = spec.IndexOf('+');
        if (plus >= 0)
        {
            name = spec[..plus];
            upgrades = int.Parse(spec[(plus + 1)..]);
        }
        foreach (var table in CardTables())
            if (table.TryGetValue(name, out var f))
                return f().Upgraded(upgrades);
        var known = string.Join(", ", CardTables().SelectMany(t => t.Keys));
        throw new ArgumentException($"Unknown card '{name}'. Known: {known}");
    }

    public static RelicModel BuildRelic(string name) =>
        RelicFactories.TryGetValue(name, out var f) ? f()
            : throw new ArgumentException($"Unknown relic '{name}'.");

    /// <summary>Build a monster scaled to <paramref name="ascension"/> (HP via ToughEnemies, damage via
    /// DeadlyEnemies). Defaults to A10 — the solver targets max-ascension play; the trace validator passes
    /// each trace's recorded ascension so A0 traces stay A0.</summary>
    public static Monster BuildMonster(string name, int ascension = 10) =>
        MonsterFactories.TryGetValue(name, out var f) ? f(ascension)
            : throw new ArgumentException($"Unknown monster '{name}'.");

    /// <summary>Build a player with a deck placed in the draw pile, ready for combat setup.</summary>
    public static Player BuildPlayer(IEnumerable<CardModel> deck, int currentHp, int maxHp,
        int maxEnergy = 3, IEnumerable<string>? relics = null)
    {
        var p = new Player
        {
            Name = "Player",
            CurrentHp = currentHp,
            MaxHp = maxHp,
            MaxEnergy = maxEnergy,
        };
        p.DrawPile.AddRange(deck);
        if (relics != null) foreach (var r in relics) p.Relics.Add(BuildRelic(r));
        return p;
    }

    /// <summary>Assemble a CombatState (deck already in draw pile). Monsters are assigned encounter-order
    /// ids (1-based), mirroring the game so recorded plays can be matched to the right target.</summary>
    public static CombatState SetupCombat(Player player, IEnumerable<Monster> monsters)
    {
        var list = monsters.ToList();
        for (int i = 0; i < list.Count; i++) list[i].Id = i + 1;
        return new() { Player = player, Monsters = list, TurnNumber = 0, CurrentSide = CombatSide.Player };
    }
}
