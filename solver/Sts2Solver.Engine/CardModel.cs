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

    /// <summary>X-cost cards (Whirlwind) spend ALL the player's current energy; the spent amount is passed
    /// to OnPlay via <see cref="CardPlay.XValue"/>. Their <see cref="Cost"/> stays 0 for the playability check.</summary>
    public virtual bool IsXCost => false;

    public virtual CardResultPile ResultPile => Type == CardType.Power ? CardResultPile.Removed : CardResultPile.Discard;

    public bool NeedsTarget => Target == TargetType.AnyEnemy;

    /// <summary>True for cards carrying the game's Strike tag (Strike, Pommel Strike, Twin Strike, …).
    /// Perfected Strike counts these across the deck. (Game: CardTag.Strike.)</summary>
    public virtual bool IsStrike => false;

    /// <summary>Cards that cannot be played from hand (e.g. Status/Curse like Infection). The solver
    /// skips them and <see cref="CombatManager.PlayCard"/> rejects them.</summary>
    public virtual bool Unplayable => false;

    /// <summary>True if this card does something when held in hand at end of the player's turn.</summary>
    public virtual bool HasTurnEndInHandEffect => false;

    /// <summary>Ethereal cards still in hand at end of the player's turn are exhausted, not discarded
    /// (e.g. Dazed from Entomancer's Personal Hive).</summary>
    public virtual bool Ethereal => false;

    /// <summary>The card's effect. Concrete cards call into <see cref="Cmd"/>.</summary>
    public abstract void OnPlay(CombatState combat, CardPlay play);

    /// <summary>Fires for each copy still in hand at end of the player's turn (before the hand is
    /// discarded). Infection deals 3 to the player here.</summary>
    public virtual void OnTurnEndInHand(CombatState combat) { }

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
