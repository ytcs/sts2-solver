namespace Sts2Solver.Engine;

/// <summary>
/// The player creature plus deck/energy state. Single-player for v1
/// (MegaCrit.Sts2.Core.Entities.Players.PlayerCombatState collapsed onto the creature).
/// </summary>
public sealed class Player : Creature
{
    public int Energy;
    public int MaxEnergy = 3;

    public const int MaxHandSize = 10;
    public const int CardsDrawnPerTurn = 5;

    public readonly List<CardModel> Hand = new();
    public readonly List<CardModel> DrawPile = new();
    public readonly List<CardModel> DiscardPile = new();
    public readonly List<CardModel> ExhaustPile = new();

    public readonly List<RelicModel> Relics = new();

    /// <summary>The Necrobinder's pet (null for every other character). Surfaced through
    /// <see cref="CombatState.AllCreatures"/> so its powers join the damage pipeline; see <see cref="Osty"/>.</summary>
    public Osty? Osty;

    public bool IsOstyAlive => Osty is { CurrentHp: > 0 };
    public bool IsOstyMissing => !IsOstyAlive;

    public Player() { Side = CombatSide.Player; }

    /// <summary>Max energy including power bonuses (Pyre). Each turn's reset uses this.</summary>
    public int EffectiveMaxEnergy
    {
        get
        {
            int bonus = 0;
            foreach (var p in Powers) bonus += p.ModifyMaxEnergy(this);
            return MaxEnergy + bonus;
        }
    }

    public void ResetEnergy() => Energy = EffectiveMaxEnergy;
    public void LoseEnergy(int amount) => Energy = Math.Max(0, Energy - amount);

    public override Creature Clone()
    {
        var p = new Player();
        CopyCreatureBaseTo(p);
        p.Energy = Energy;
        p.MaxEnergy = MaxEnergy;
        // The immutable majority of cards safely share instances across clones (only the list copies); cards
        // with mutable per-combat state (CardModel.Stateful, e.g. Rampage) MUST be deep-cloned, or sibling
        // search branches would share — and corrupt — that state. See CardModel.Stateful.
        CopyPile(Hand, p.Hand);
        CopyPile(DrawPile, p.DrawPile);
        CopyPile(DiscardPile, p.DiscardPile);
        CopyPile(ExhaustPile, p.ExhaustPile);
        p.Relics.AddRange(Relics);   // relics are immutable definitions
        p.Osty = Osty != null ? (Osty)Osty.Clone() : null;
        return p;
    }

    /// <summary>Copy a pile into a clone, deep-cloning only the cards that carry mutable per-combat state
    /// (<see cref="CardModel.Stateful"/>) so clones don't share — and corrupt — that state.</summary>
    private static void CopyPile(List<CardModel> from, List<CardModel> to)
    {
        for (int i = 0; i < from.Count; i++)
        {
            var c = from[i];
            to.Add(c.Stateful ? c.Clone() : c);
        }
    }

    public override void Hash(ref StateHasher h)
    {
        base.Hash(ref h);
        h.Add(Energy);
        h.Add(MaxEnergy);
        HashPile(ref h, Hand, 0x11);
        HashPile(ref h, DrawPile, 0x22);
        HashPile(ref h, DiscardPile, 0x33);
        HashPile(ref h, ExhaustPile, 0x44);
        // Relics are fixed for the combat; a single marker per relic id keeps the key canonical.
        Span<long> relics = stackalloc long[Relics.Count];
        for (int i = 0; i < Relics.Count; i++) relics[i] = Relics[i].Id.GetHashCode();
        h.AddSorted(relics);
        if (Osty != null) { h.Add(0x5057); Osty.Hash(ref h); }
    }

    private static void HashPile(ref StateHasher h, List<CardModel> pile, long tag)
    {
        h.Add(tag);
        h.Add(pile.Count);
        Span<long> buf = stackalloc long[pile.Count];
        for (int i = 0; i < pile.Count; i++) buf[i] = pile[i].KeyHash;
        h.AddSorted(buf);   // pile order is irrelevant
    }

    public override string StateKey()
    {
        // Draw + discard order is irrelevant to optimal play (draws are modelled as random over the
        // multiset), so canonicalise them as sorted multisets. Hand order is likewise irrelevant.
        string Bag(List<CardModel> pile) =>
            string.Join(",", pile.Select(c => c.StateKey()).OrderBy(s => s, StringComparer.Ordinal));
        var relics = string.Join(",", Relics.Select(r => r.Id).OrderBy(s => s, StringComparer.Ordinal));
        var osty = Osty != null ? $"|{Osty.StateKey()}" : "";
        return $"P({base.StateKey()}|e{Energy}/{MaxEnergy}|H[{Bag(Hand)}]|D[{Bag(DrawPile)}]|X[{Bag(DiscardPile)}]|E[{Bag(ExhaustPile)}]|R[{relics}]{osty})";
    }
}

/// <summary>Minimal relic model. v1 only needs a post-combat hook (Burning Blood heals 6).</summary>
public abstract class RelicModel
{
    public abstract string Id { get; }
    public virtual void AfterCombatVictory(CombatState combat) { }

    /// <summary>Fires once when combat is set up (Bound Phylactery summons Osty here).</summary>
    public virtual void OnCombatStart(CombatState combat) { }

    /// <summary>Fires at the start of each player turn after energy resets (Bound Phylactery re-summons
    /// Osty each turn after the first).</summary>
    public virtual void OnPlayerTurnStart(CombatState combat) { }
}
