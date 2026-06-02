using System.Collections.Generic;
using System.Linq;

namespace Sts2Solver.Engine;

/// <summary>Where a card goes after being played.</summary>
public enum CardResultPile { Discard, Exhaust, Removed }

/// <summary>A single resolution of a card play (the card + its chosen target).</summary>
public sealed class CardPlay
{
    public required CardModel Card;
    public Creature? Target;

    /// <summary>For X-cost cards (Whirlwind), the energy actually spent — i.e. the X value.</summary>
    public int XValue;

    /// <summary>Stars actually spent on this play (Regent star-cost cards). For X-star cards (Stardust) this
    /// is all the player's stars; the card reads it to scale its effect (Stardust hits StarsSpent times).</summary>
    public int StarsSpent;

    /// <summary>For cards that require an in-play CHOICE (Headbutt's topdeck target, Armaments' upgrade target,
    /// an exhaust/discard selection, …): the <see cref="CardModel.StateKey"/> of the chosen option. The search
    /// enumerates these via <see cref="CardModel.Choices"/> and promotes each to its own decision; trace replay
    /// passes the recorded choice. Null = no choice was made (the card has none, or a caller left it to the
    /// card's default).</summary>
    public string? ChoiceKey;
}

/// <summary>
/// Base card. Mirrors MegaCrit.Sts2.Core.Models.CardModel: cost/type/rarity/target metadata plus an
/// imperative OnPlay. We model the game's DynamicVars as plain values computed from <see cref="Upgrades"/>
/// in each concrete card, which is faithful for the fixed-value cards in scope.
/// </summary>
public abstract class CardModel
{
    public abstract string Name { get; }
    public abstract int BaseCost { get; }
    public abstract CardType Type { get; }
    public abstract CardRarity Rarity { get; }
    public abstract TargetType Target { get; }

    public int Upgrades { get; set; }

    public virtual int Cost => BaseCost;

    /// <summary>Combat-dependent cost (Necrobinder Banshee's Cry drops 2 per Ethereal played; Flatten is
    /// free after an Osty attack). Defaults to the static <see cref="Cost"/>. The solver's playability
    /// check and <see cref="CombatManager.PlayCard"/> both consult this.</summary>
    public virtual int EffectiveCost(CombatState combat) => Cost;

    /// <summary>X-cost cards (Whirlwind) spend ALL the player's current energy; the spent amount is passed
    /// to OnPlay via <see cref="CardPlay.XValue"/>. Their <see cref="Cost"/> stays 0 for the playability check.</summary>
    public virtual bool IsXCost => false;

    /// <summary>Stars required and consumed to play this card, on top of its energy cost (Regent cards like
    /// FallingStar = 2★). 0 for everything else. (Game: CardModel.CanonicalStarCost.) The play is gated on
    /// the player having at least this many Stars, and they are spent on play.</summary>
    public virtual int StarCost => 0;

    /// <summary>X-star cards (Stardust, RoyalGamble's spend-all forms) consume ALL the player's stars; the
    /// spent amount is passed to OnPlay via <see cref="CardPlay.StarsSpent"/>. (Game: CardModel.HasStarCostX.)</summary>
    public virtual bool IsXStarCost => false;

    /// <summary>Retained cards are NOT discarded at end of the player's turn — they stay in hand (game:
    /// CardKeyword.Retain). The Regent's Sovereign Blade token retains so its forged damage carries across
    /// turns; the Necrobinder's Eradicate/Reap/Sow/Spur/… retain too. Some cards gain Retain dynamically and
    /// override this with a mutable backing flag.</summary>
    public virtual bool Retain => false;

    public virtual CardResultPile ResultPile => Type == CardType.Power ? CardResultPile.Removed : CardResultPile.Discard;

    public bool NeedsTarget => Target == TargetType.AnyEnemy;

    /// <summary>True for cards carrying the game's Strike tag (Strike, Pommel Strike, Twin Strike, …).
    /// Perfected Strike counts these across the deck. (Game: CardTag.Strike.)</summary>
    public virtual bool IsStrike => false;

    /// <summary>True for cards carrying the game's OstyAttack tag (the Necrobinder's Osty-powered attacks).
    /// Squeeze counts these across the deck; the cards fizzle while Osty is missing. (Game: CardTag.OstyAttack.)</summary>
    public virtual bool IsOstyAttack => false;

    /// <summary>Cards that cannot be played from hand (e.g. Status/Curse like Infection). The solver
    /// skips them and <see cref="CombatManager.PlayCard"/> rejects them.</summary>
    public virtual bool Unplayable => false;

    /// <summary>True if this card does something when held in hand at end of the player's turn.</summary>
    public virtual bool HasTurnEndInHandEffect => false;

    /// <summary>Ethereal cards still in hand at end of the player's turn are exhausted, not discarded
    /// (e.g. Dazed from Entomancer's Personal Hive).</summary>
    public virtual bool Ethereal => false;

    /// <summary>The card's effect. Concrete cards call into <see cref="Cmd"/>. A card that requires an in-play
    /// choice reads <see cref="CardPlay.ChoiceKey"/> (and should apply a sensible default when it is null, for
    /// trace replay / callers that don't choose).</summary>
    public abstract void OnPlay(CombatState combat, CardPlay play);

    /// <summary>The DISTINCT options a player must choose among when playing this card (each an option's
    /// <see cref="StateKey"/>) — e.g. the distinct discard-pile cards Headbutt could topdeck. The search
    /// promotes each to its own decision; symmetric options collapse because identical cards share a StateKey.
    /// Empty (the default) means the card has no choice and is enumerated/played exactly as before.</summary>
    public virtual IEnumerable<string> Choices(CombatState combat) => System.Array.Empty<string>();

    /// <summary>Distinct StateKeys of the player's hand cards EXCLUDING this played instance — the card is
    /// removed from hand before OnPlay (CombatManager.PlayCard), so it can never be its own pick. The common
    /// <see cref="Choices"/> body for a "choose a card from hand" effect (exhaust/discard selection).</summary>
    protected IEnumerable<string> HandChoices(CombatState combat) =>
        combat.Player.Hand.Where(c => !ReferenceEquals(c, this)).Select(c => c.StateKey()).Distinct();

    /// <summary>Resolve a hand-card choice at OnPlay time: the hand card whose StateKey == <paramref name="choiceKey"/>,
    /// else the first hand card (the trace-replay / no-choice default). Null only when the hand is empty.</summary>
    protected static CardModel? ChosenHandCard(CombatState combat, string? choiceKey) =>
        choiceKey == null ? combat.Player.Hand.FirstOrDefault()
                          : combat.Player.Hand.FirstOrDefault(c => c.StateKey() == choiceKey)
                            ?? combat.Player.Hand.FirstOrDefault();

    /// <summary>Fires for each copy still in hand at end of the player's turn (before the hand is
    /// discarded). Infection deals 3 to the player here.</summary>
    public virtual void OnTurnEndInHand(CombatState combat) { }

    /// <summary>True for a cost-0 draw card that returns to a pile and so can be REPLAYED (EscapePlan,
    /// Prepared) — playing it is free and draws, and reshuffle recirculates it, so the search could otherwise
    /// build an unbounded play chain in one turn. Decks holding one set <see cref="CombatState.BoundsPlays"/>
    /// at setup, capping plays per turn (cost-≥1 draw cards are energy-bounded, so they never set this).</summary>
    public virtual bool LoopRiskDraw => false;

    /// <summary>True for a card that DRAWS and then acts on the resulting hand (EscapePlan, Acrobatics,
    /// Prepared). In search the draw is deferred (a chance node); this card registers via
    /// <see cref="Cmd.DeferDrawThenResolve"/> so the solver runs its post-draw step after the draw resolves.
    /// The common card returns false and resolves entirely in <see cref="OnPlay"/>.</summary>
    public virtual bool HasPostDraw => false;

    /// <summary>For a post-draw DISCARD-of-choice card: how many cards the player then discards of their choice
    /// from the post-draw hand (Acrobatics 1, Prepared = cards drawn). 0 ⇒ the post-draw step is the
    /// deterministic/conditional <see cref="OnPostDraw"/> instead (EscapePlan).</summary>
    public virtual int PostDrawDiscardCount => 0;

    /// <summary>The deterministic / conditional post-draw effect, run on the post-draw hand once the deferred
    /// draw has resolved (EscapePlan: +Block iff the just-drawn card is a Skill). <paramref name="drawn"/> is how
    /// many cards the draw ACTUALLY produced — 0 when both piles were empty, so a card that reads the drawn card
    /// (the hand's last) must guard on <c>drawn &gt; 0</c> (the game's drawn-card is null there). Only consulted
    /// when <see cref="PostDrawDiscardCount"/> is 0. Default no-op.</summary>
    public virtual void OnPostDraw(CombatState combat, int drawn) { }

    /// <summary>Apply one upgrade level's stat changes. Called <see cref="Upgrades"/> times during construction helpers.</summary>
    protected virtual void OnUpgrade() { }

    public CardModel Upgraded(int levels = 1)
    {
        for (int i = 0; i < levels; i++) { Upgrades++; OnUpgrade(); }
        _key = null;
        _keyHash = null;   // identity changed — drop the cached hash too (it would otherwise go stale)
        return this;
    }

    public virtual CardModel Clone() => (CardModel)MemberwiseClone();   // copies cached _key (Upgrades unchanged)

    /// <summary>True for cards that carry MUTABLE per-combat state — e.g. <c>Rampage</c>, whose damage
    /// escalates with each play — so their identity (<see cref="StateKey"/>) changes as the fight progresses.
    /// Such cards MUST be deep-cloned when a <see cref="CombatState"/> is cloned (see <c>Player.Clone</c>), or
    /// sibling search branches would share one instance and corrupt each other's value; and their
    /// <see cref="KeyHash"/> must not be cached. The immutable majority safely share instances across clones.</summary>
    public virtual bool Stateful => false;

    // Card identity is immutable in combat for all but Stateful cards, so cache the key + its hash; Stateful
    // cards (whose StateKey overrides recompute from mutable fields) must NOT cache the hash.
    private string? _key;
    private int? _keyHash;
    public virtual string StateKey() => _key ??= (Upgrades > 0 ? $"{Name}+{Upgrades}" : Name);
    public int KeyHash => Stateful ? StateKey().GetHashCode() : (_keyHash ??= StateKey().GetHashCode());
}
