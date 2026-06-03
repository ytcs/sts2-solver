namespace Sts2Solver.Engine;

/// <summary>
/// The player creature plus deck/energy state. Single-player for v1
/// (MegaCrit.Sts2.Core.Entities.Players.PlayerCombatState collapsed onto the creature).
/// </summary>
public sealed class Player : Creature
{
    public int Energy;
    public int MaxEnergy = 3;

    /// <summary>The Regent's secondary resource (game: PlayerCombatState.Stars). Gained by many cards and
    /// spent to play star-cost cards; persists across turns within a combat. 0 for other characters. Gained
    /// via <see cref="Cmd.GainStars"/> (which fires the AfterStarsGained hooks); spent in
    /// <see cref="CombatManager.PlayCard"/>.</summary>
    public int Stars;

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

    /// <summary>The Defect's orb queue (FIFO: front = oldest) and its slot capacity (game BaseOrbSlotCount = 3,
    /// set by the CrackedCore starter relic at combat start). Empty / 0 for every non-Defect player, so they
    /// contribute nothing to clone / hash / StateKey. See <see cref="OrbModel"/> / <see cref="OrbOps"/>.</summary>
    public readonly List<OrbModel> Orbs = new();
    public int OrbSlots;

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

    public void SpendStars(int amount) => Stars = Math.Max(0, Stars - amount);

    /// <summary>True if the player can pay this card's star cost (X-star cards are always affordable — they
    /// spend whatever stars are present). Energy is checked separately by the solver / PlayCard.</summary>
    public bool CanAffordStars(CardModel card) => card.IsXStarCost || card.StarCost <= Stars;

    public override Creature Clone()
    {
        var p = new Player();
        CopyCreatureBaseTo(p);
        p.Energy = Energy;
        p.MaxEnergy = MaxEnergy;
        p.Stars = Stars;
        // The immutable majority of cards safely share instances across clones (only the list copies); cards
        // with mutable per-combat state (CardModel.Stateful, e.g. Rampage) MUST be deep-cloned, or sibling
        // search branches would share — and corrupt — that state. See CardModel.Stateful.
        CopyPile(Hand, p.Hand);
        CopyPile(DrawPile, p.DrawPile);
        CopyPile(DiscardPile, p.DiscardPile);
        CopyPile(ExhaustPile, p.ExhaustPile);
        p.Relics.AddRange(Relics);   // relics are immutable definitions
        p.Osty = Osty != null ? (Osty)Osty.Clone() : null;
        // Orb queue is ORDERED (evoke order matters). Stateful orbs (Dark/Glass) carry mutable accumulators
        // and MUST be deep-cloned per branch; immutable orbs (Lightning/Frost/Plasma) can be shared by ref.
        p.OrbSlots = OrbSlots;
        foreach (var orb in Orbs) p.Orbs.Add(orb.Stateful ? orb.Clone() : orb);
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
        h.Add(Stars);
        HashPile(ref h, Hand, 0x11);
        HashPile(ref h, DrawPile, 0x22);
        HashPile(ref h, DiscardPile, 0x33);
        HashPile(ref h, ExhaustPile, 0x44);
        // Relics are fixed for the combat; a single marker per relic id keeps the key canonical.
        Span<long> relics = stackalloc long[Relics.Count];
        for (int i = 0; i < Relics.Count; i++) relics[i] = Relics[i].Id.GetHashCode();
        h.AddSorted(relics);
        if (Osty != null) { h.Add(0x5057); Osty.Hash(ref h); }
        // Orbs: ORDER-sensitive (evoke FIFO) and slot count both matter; contribute only when the player has
        // orb capacity (Defect), so other characters keep canonical keys.
        if (OrbSlots > 0)
        {
            h.Add(0x0B5);
            h.Add(OrbSlots);
            h.Add(Orbs.Count);
            foreach (var orb in Orbs) orb.Hash(ref h);   // ordered — NOT sorted
        }
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
        // Stars (Regent) and Osty (Necrobinder) contribute to the key only when present, so decks without them
        // keep their existing canonical keys.
        var stars = Stars != 0 ? $"|s{Stars}" : "";
        var osty = Osty != null ? $"|{Osty.StateKey()}" : "";
        // Orbs are ORDER-sensitive (evoke FIFO), so keep them as a list, not a sorted bag; present only for Defect.
        var orbs = OrbSlots > 0 ? $"|O{OrbSlots}[{string.Join(",", Orbs.Select(o => o.StateKey()))}]" : "";
        return $"P({base.StateKey()}|e{Energy}/{MaxEnergy}{stars}|H[{Bag(Hand)}]|D[{Bag(DrawPile)}]|X[{Bag(DiscardPile)}]|E[{Bag(ExhaustPile)}]|R[{relics}]{osty}{orbs})";
    }
}

/// <summary>Minimal relic model. v1 only needs a post-combat hook (Burning Blood heals 6).</summary>
public abstract class RelicModel
{
    public abstract string Id { get; }
    public virtual void AfterCombatVictory(CombatState combat) { }

    /// <summary>Fires once when combat is set up, before turn 1 (game: AfterRoomEntered for a CombatRoom).
    /// The Regent's DivineRight grants Stars here; the Necrobinder's Bound Phylactery summons Osty.</summary>
    public virtual void OnCombatStart(CombatState combat) { }

    /// <summary>Fires at the start of each player turn after energy resets (Bound Phylactery re-summons
    /// Osty each turn after the first).</summary>
    public virtual void OnPlayerTurnStart(CombatState combat) { }
}
