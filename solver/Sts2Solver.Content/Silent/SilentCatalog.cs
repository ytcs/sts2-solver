using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Silent card registration + starter deck. A partial of Catalog; Core's
// CardTables() aggregates SilentCardFactories alongside the other characters'.
// ===========================================================================
public static partial class Catalog
{
    internal static readonly Dictionary<string, Func<CardModel>> SilentCardFactories = new(StringComparer.OrdinalIgnoreCase)
    {
        // Batch 1 — primitive-only cards (Attack / Block / Weak / Poison / Dexterity).
        ["StrikeSilent"] = () => new StrikeSilent(),
        ["DefendSilent"] = () => new DefendSilent(),
        ["Neutralize"] = () => new Neutralize(),
        ["Slice"] = () => new Slice(),
        ["Deflect"] = () => new Deflect(),
        ["Dash"] = () => new Dash(),
        ["SuckerPunch"] = () => new SuckerPunch(),
        ["LegSweep"] = () => new LegSweep(),
        ["DaggerSpray"] = () => new DaggerSpray(),
        ["PoisonedStab"] = () => new PoisonedStab(),
        ["DeadlyPoison"] = () => new DeadlyPoison(),
        ["Footwork"] = () => new Footwork(),
        ["Haze"] = () => new Haze(),
        ["Flechettes"] = () => new Flechettes(),
        // Batch 2 — Shivs, poison-synergy powers, and more primitive attacks/skills.
        ["Shiv"] = () => new Shiv(),
        ["CloakAndDagger"] = () => new CloakAndDagger(),
        ["BladeDance"] = () => new BladeDance(),
        ["Pinpoint"] = () => new Pinpoint(),
        ["FlickFlack"] = () => new FlickFlack(),
        ["Scare"] = () => new Scare(),
        ["Snakebite"] = () => new Snakebite(),
        ["BubbleBubble"] = () => new BubbleBubble(),
        ["Suppress"] = () => new Suppress(),
        ["DodgeAndRoll"] = () => new DodgeAndRoll(),
        ["Blur"] = () => new Blur(),
        ["Afterimage"] = () => new Afterimage(),
        ["NoxiousFumes"] = () => new NoxiousFumes(),
        ["Envenom"] = () => new Envenom(),
        ["PiercingWail"] = () => new PiercingWail(),
        // Batch 3 — remaining Silent cards (primitive attacks/skills, X-cost, thorns, draw-next-turn).
        ["Survivor"] = () => new Survivor(),
        ["Backstab"] = () => new Backstab(),
        ["DaggerThrow"] = () => new DaggerThrow(),
        ["Predator"] = () => new Predator(),
        ["BouncingFlask"] = () => new BouncingFlask(),
        ["Caltrops"] = () => new Caltrops(),
        ["GrandFinale"] = () => new GrandFinale(),
        ["Skewer"] = () => new Skewer(),
        ["Adrenaline"] = () => new Adrenaline(),
        ["Backflip"] = () => new Backflip(),
        ["Expertise"] = () => new Expertise(),
    };

    /// <summary>The Silent starting deck: 5 Strike, 5 Defend, 1 Neutralize, 1 Survivor.
    /// (Survivor's discard half is not modelled — see SilentCards.cs — so it contributes its block only;
    /// the deck composition is faithful.)</summary>
    public static List<CardModel> SilentStarterDeck()
    {
        var deck = new List<CardModel>();
        for (int i = 0; i < 5; i++) deck.Add(new StrikeSilent());
        for (int i = 0; i < 5; i++) deck.Add(new DefendSilent());
        deck.Add(new Neutralize());
        deck.Add(new Survivor());
        return deck;
    }
}
