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
        ["Wound"] = () => new Wound(),
        ["Slimed"] = () => new Slimed(),
        ["Void"] = () => new Void(),
        ["Fuel"] = () => new Fuel(),
        // Remaining Status pool cards.
        ["Beckon"] = () => new Beckon(),
        ["Debris"] = () => new Debris(),
        ["FranticEscape"] = () => new FranticEscape(),
        ["Soot"] = () => new Soot(),
        ["Toxic"] = () => new Toxic(),
        ["Wither"] = () => new Wither(),
        ["Disintegration"] = () => new Disintegration(),
        ["MindRot"] = () => new MindRot(),
        ["Sloth"] = () => new Sloth(),
        ["WasteAway"] = () => new WasteAway(),
        // Token pool cards (minion summons + GiantRock + Luminesce).
        ["GiantRock"] = () => new GiantRock(),
        ["Luminesce"] = () => new Luminesce(),
        ["MinionDiveBomb"] = () => new MinionDiveBomb(),
        ["MinionSacrifice"] = () => new MinionSacrifice(),
        ["MinionStrike"] = () => new MinionStrike(),
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
        // The Regent's Sovereign Blade token: buildable by name (for tests / trace replay), generated in
        // combat by Forge, never deck-built — so registered here (excluded from CardPool) like status cards.
        ["SovereignBlade"] = () => new SovereignBlade(),
        // Necrobinder tokens: buildable (so traces/tests can construct them) but excluded from the deck pool.
        ["Soul"] = () => new Soul(),
        ["SweepingGaze"] = () => new SweepingGaze(),
    };

    /// <summary>Every registered card table, in lookup order. Add a character = add one line here.</summary>
    private static IEnumerable<IReadOnlyDictionary<string, Func<CardModel>>> CardTables()
    {
        yield return CommonCardFactories;
        yield return IroncladCardFactories;
        yield return SilentCardFactories;
        yield return RegentCardFactories;
        yield return ColorlessCardFactories;
        yield return SpecialCardFactories;
        yield return NecrobinderCardFactories;
        yield return DefectCardFactories;
    }

    /// <summary>The deck-buildable card pool: every registered character + colorless card (Ironclad + Silent
    /// + Colorless), excluding the shared status/curse cards (Burn, Dazed, Infection, AscendersBane) which are
    /// never deliberately added to a deck. Used by <see cref="TrainingFixtures"/> to draw randomised decks for
    /// the Phase-C VF training corpus, and by the advisor's card-name auto-complete/auto-correct.</summary>
    public static IReadOnlyList<string> CardPool =>
        _cardPool ??= IroncladCardFactories.Keys
            .Concat(SilentCardFactories.Keys)
            .Concat(RegentCardFactories.Keys)
            .Concat(ColorlessCardFactories.Keys)
            .Concat(SpecialCardFactories.Keys)
            .Concat(NecrobinderCardFactories.Keys)
            .Concat(DefectCardFactories.Keys)
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

    /// <summary>True if <paramref name="name"/> is a relic the engine models (i.e. <see cref="BuildRelic"/>
    /// can build it). Used by the advisor to map a run's relics to modelled ones and ignore the rest.</summary>
    public static bool IsModelledRelic(string name) => RelicFactories.ContainsKey(name);

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
        // Murder's damage scales with cumulative cards drawn this combat — only then do we track + hash that
        // ever-growing counter (it would otherwise needlessly fragment every other deck's state space).
        var allCards = player.DrawPile.Concat(player.Hand).Concat(player.DiscardPile).ToList();
        bool tracksDrawn = allCards.Any(c => c.Name == "Murder");
        // DeathMarch scales on cards drawn mid-turn (by effects) this turn — gated counter, tracked only when present.
        bool tracksMidTurnDraws = allCards.Any(c => c.TracksMidTurnDrawScaling);
        // A cost-0 replayable draw cantrip (EscapePlan / Prepared) could loop the per-turn play chain in search;
        // such decks cap plays per turn (and hash the counter). Cost-≥1 draws are energy-bounded ⇒ no cap.
        // Also bound (and hash) plays when the deck holds a card that caps plays-per-turn while in hand
        // (Normality ≤3) — the cap depends on PlaysThisTurn, which must then memoise soundly.
        bool boundsPlays = allCards.Any(c => c.LoopRiskDraw)
                        || allCards.Any(c => c.PlayCapWhileInHand < CombatState.MaxPlaysPerTurn);
        // BeatIntoShape's forge scales on the target's prior powered hits this turn — only then track that
        // per-creature counter (it would otherwise fragment every other deck's state space).
        bool tracksPoweredHits = allCards.Any(c => c.TracksTargetPoweredHits);
        // Defect Voltaic (lightnings channeled this combat) / HelixDrill (energy spent this turn) — gated
        // counters tracked + hashed only when a deck that reads them is present.
        bool tracksLightning = allCards.Any(c => c.TracksLightningChanneledThisCombat);
        bool tracksEnergySpent = allCards.Any(c => c.TracksEnergySpentThisTurn);
        // Colorless GoldAxe scales on cards played this combat — gated counter, tracked only when present.
        bool tracksCardsPlayed = allCards.Any(c => c.TracksCardsPlayedThisCombat);
        var combat = new CombatState { Player = player, Monsters = list, TurnNumber = 0,
                                       CurrentSide = CombatSide.Player, TracksCardsDrawn = tracksDrawn,
                                       TracksMidTurnDraws = tracksMidTurnDraws,
                                       BoundsPlays = boundsPlays, TracksPoweredHits = tracksPoweredHits,
                                       TracksLightningChanneled = tracksLightning, TracksEnergySpent = tracksEnergySpent,
                                       TracksCardsPlayed = tracksCardsPlayed };
        foreach (var r in player.Relics) r.OnCombatStart(combat);   // e.g. DivineRight grants Stars, Bound Phylactery summons Osty
        return combat;
    }
}
