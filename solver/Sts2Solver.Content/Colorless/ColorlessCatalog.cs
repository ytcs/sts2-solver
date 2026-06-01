using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Colorless card registration. A partial of Catalog; Core's CardTables()
// aggregates ColorlessCardFactories alongside the other characters'. Colorless
// cards belong to no single character (they appear via card-reward/relic
// effects), so there is no starter deck here.
// ===========================================================================
public static partial class Catalog
{
    internal static readonly Dictionary<string, Func<CardModel>> ColorlessCardFactories = new(StringComparer.OrdinalIgnoreCase)
    {
        // Attacks (damage-faithful).
        ["FlashOfSteel"] = () => new FlashOfSteel(),
        ["DramaticEntrance"] = () => new DramaticEntrance(),
        ["MindBlast"] = () => new MindBlast(),
        ["HandOfGreed"] = () => new HandOfGreed(),
        ["Clash"] = () => new Clash(),
        // Skills (block / draw / debuff — HP-faithful).
        ["Finesse"] = () => new Finesse(),
        ["DarkShackles"] = () => new DarkShackles(),
        ["MasterOfStrategy"] = () => new MasterOfStrategy(),
        ["ThinkingAhead"] = () => new ThinkingAhead(),
        ["Impatience"] = () => new Impatience(),
        ["PanicButton"] = () => new PanicButton(),
        // Powers.
        ["Panache"] = () => new Panache(),
        ["TheBomb"] = () => new TheBomb(),
        ["Mayhem"] = () => new Mayhem(),
        // HP-neutral / RNG cards ported as their faithful subset (documented inert).
        ["Apotheosis"] = () => new Apotheosis(),
        ["Metamorphosis"] = () => new Metamorphosis(),
        ["Enlightenment"] = () => new Enlightenment(),
        ["Purity"] = () => new Purity(),
    };
}
