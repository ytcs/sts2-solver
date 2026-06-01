using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Event/Ancient special-pool card registration. A partial of Catalog; Core's
// CardTables() aggregates SpecialCardFactories alongside the characters' and
// Colorless tables, and CardPool includes them (they ARE deck-buildable —
// acquired from events / special rewards). Curses register separately in
// Catalog's CommonCardFactories (they are not deck-built). New powers live in
// Special/SpecialPowers.cs.
// ===========================================================================
public static partial class Catalog
{
    internal static readonly Dictionary<string, Func<CardModel>> SpecialCardFactories = new(StringComparer.OrdinalIgnoreCase)
    {
        // Event cards.
        ["ByrdSwoop"] = () => new ByrdSwoop(),
        ["Exterminate"] = () => new Exterminate(),
        ["Peck"] = () => new Peck(),
        ["Squash"] = () => new Squash(),
        ["RipAndTear"] = () => new RipAndTear(),
        ["Entrench"] = () => new Entrench(),
        ["Stack"] = () => new Stack(),
        ["Outmaneuver"] = () => new Outmaneuver(),
        ["FeedingFrenzy"] = () => new FeedingFrenzy(),
        ["Rebound"] = () => new Rebound(),
        ["HelloWorld"] = () => new HelloWorld(),
        ["ToricToughness"] = () => new ToricToughness(),
        ["Distraction"] = () => new Distraction(),
        ["DualWield"] = () => new DualWield(),
        // Ancient cards.
        ["MeteorShower"] = () => new MeteorShower(),
        ["Maul"] = () => new Maul(),
        ["NeowsFury"] = () => new NeowsFury(),
        ["Whistle"] = () => new Whistle(),
        ["BrightestFlame"] = () => new BrightestFlame(),
        ["Relax"] = () => new Relax(),
        ["Apparition"] = () => new Apparition(),
        ["WraithForm"] = () => new WraithForm(),
        ["ForbiddenGrimoire"] = () => new ForbiddenGrimoire(),
        ["TheSealedThrone"] = () => new TheSealedThrone(),
    };
}
