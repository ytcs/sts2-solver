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
        // Batch 4 — remaining primitive-portable cards (attacks/skills/powers, X-cost, Shiv-generation,
        // Thorns/FreeSkill). Sly is HP-neutral (unmodelled discard trigger).
        ["Abrasive"] = () => new Abrasive(),
        ["Assassinate"] = () => new Assassinate(),
        ["Expose"] = () => new Expose(),
        ["LeadingStrike"] = () => new LeadingStrike(),
        ["Malaise"] = () => new Malaise(),
        ["Pounce"] = () => new Pounce(),
        ["Reflex"] = () => new Reflex(),
        ["Ricochet"] = () => new Ricochet(),
        ["Tactician"] = () => new Tactician(),
        ["Untouchable"] = () => new Untouchable(),
        ["StormOfSteel"] = () => new StormOfSteel(),
        ["CalculatedGamble"] = () => new CalculatedGamble(),
        // Batch 5 — Silent powers + Shiv-synergy cards (Wave 2). MP-only / Sly / Retain effects are inert
        // in single-player (documented in SilentCards.cs / SilentPowers.cs).
        ["Accelerant"] = () => new Accelerant(),
        ["Accuracy"] = () => new Accuracy(),
        ["Anticipate"] = () => new Anticipate(),
        ["Strangle"] = () => new Strangle(),
        ["InfiniteBlades"] = () => new InfiniteBlades(),
        ["PhantomBlades"] = () => new PhantomBlades(),
        ["Outbreak"] = () => new Outbreak(),
        ["SerpentForm"] = () => new SerpentForm(),
        ["Tracking"] = () => new Tracking(),
        ["MasterPlanner"] = () => new MasterPlanner(),
        ["Shadowmeld"] = () => new Shadowmeld(),
        ["Burst"] = () => new Burst(),
        ["FanOfKnives"] = () => new FanOfKnives(),
        ["WellLaidPlans"] = () => new WellLaidPlans(),
        ["Sneaky"] = () => new Sneaky(),
        ["Flanking"] = () => new Flanking(),
        ["ShadowStep"] = () => new ShadowStep(),
        ["BladeOfInk"] = () => new BladeOfInk(),
        // Batch 6 — counter/conditional attacks + Intangible (Wave 3).
        ["Finisher"] = () => new Finisher(),
        ["MementoMori"] = () => new MementoMori(),
        ["PreciseCut"] = () => new PreciseCut(),
        ["Mirage"] = () => new Mirage(),
        ["EchoingSlash"] = () => new EchoingSlash(),
        ["WraithForm"] = () => new WraithForm(),
        // Batch 7 — remaining feasible cards (Wave 4).
        ["TheHunt"] = () => new TheHunt(),
        ["HandTrick"] = () => new HandTrick(),
        ["BulletTime"] = () => new BulletTime(),
        ["UpMySleeve"] = () => new UpMySleeve(),
        // Batch 8 — the formerly-deferred cards: real player-MAX discard-of-choice (Acrobatics, Prepared,
        // HiddenDaggers — the last via a discard continuation that adds its Shivs after the discards resolve),
        // deterministic-default selection (ToolsOfTheTrade, Nightmare), deterministic exhaust-replay (KnifeTrap),
        // conditional-on-drawn-card (EscapePlan), and mid-turn-draw triggers (CorrosiveWave, Speedster, Murder).
        ["KnifeTrap"] = () => new KnifeTrap(),
        ["Nightmare"] = () => new Nightmare(),
        ["Acrobatics"] = () => new Acrobatics(),
        ["Prepared"] = () => new Prepared(),
        ["HiddenDaggers"] = () => new HiddenDaggers(),
        ["ToolsOfTheTrade"] = () => new ToolsOfTheTrade(),
        ["EscapePlan"] = () => new EscapePlan(),
        ["CorrosiveWave"] = () => new CorrosiveWave(),
        ["Speedster"] = () => new Speedster(),
        ["Murder"] = () => new Murder(),
    };

    // =======================================================================================================
    // All 88 cards of the SilentCardPool are now ported. The cards that need mechanics the exact solver does
    // not fully model (hand-discard / card SELECTION, and mid-turn-DRAW effects) follow the project's
    // established bar — faithful with a concrete driver / the trace validator, and degrading sensibly in pure
    // search (selection uses a deterministic default à la Armaments / Burning Pact; draws are no-ops without
    // an ambient Rng). The mechanic each leans on, for reference:
    //
    //   - Acrobatics / Prepared / HiddenDaggers                              : real player-MAX discard-of-choice
    //   - ToolsOfTheTrade / Nightmare                                        : default card SELECTION
    //   - KnifeTrap                                                           : deterministic exhaust replay (exact)
    //   - EscapePlan                                                          : conditional on the drawn card
    //   - CorrosiveWave / Speedster                                           : mid-turn-draw triggers
    //   - Murder                                                              : scales on cards drawn this combat
    //                                                                           (tracked via TracksCardsDrawn)
    // =======================================================================================================
    public static readonly string[] DeferredSilentCards = System.Array.Empty<string>();

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
