namespace Sts2Solver.Engine;

/// <summary>
/// Allocation-free 128-bit structural hash used as the memoisation key. Two independent FNV-1a lanes
/// make collisions negligible (~10⁻¹⁸ at the state counts we deal with) without building strings.
/// </summary>
public struct StateHasher
{
    private ulong _a = 14695981039346656037UL;
    private ulong _b = 1469598103934665603UL;

    public StateHasher() { }

    public void Add(long value)
    {
        ulong u = (ulong)value;
        _a = (_a ^ u) * 1099511628211UL;
        _b = (_b ^ (u + 0x9E3779B97F4A7C15UL)) * 1000000007UL;
    }

    /// <summary>Add a set of longs order-independently (sorted in place).</summary>
    public void AddSorted(Span<long> values)
    {
        values.Sort();
        foreach (var v in values) Add(v);
    }

    public readonly (ulong, ulong) Result => (_a, _b);
}

public static class HashingExtensions
{
    public static (ulong, ulong) HashKey(this CombatState s)
    {
        var h = new StateHasher();
        h.Add(s.TurnNumber);
        h.Add((long)s.CurrentSide);
        h.Add(s.CardExhaustedThisTurn ? 1 : 0);
        h.Add(s.PlayerLostHpThisTurn ? 1 : 0);
        h.Add(s.PlayerUnblockedHitsCount);
        h.Add(s.SkillsPlayedThisTurn);    // Regent Lunar Blast — must live in the MEMO key, not just StateKey,
        h.Add(s.StarsGainedThisTurn);     // or two states differing only in these collide → stale memoised value.
        h.Add(s.AttacksPlayedThisTurn);
        h.Add(s.CardsDiscardedThisTurn);
        h.Add(s.EtherealPlayedThisCombat);   // Necrobinder Pull from Below / Banshee's Cry — memo key, not just
        h.Add(s.OstyAttacksThisTurn);        // StateKey, or states differing only in these collide → stale value.
        h.Add(s.DoomAppliedThisTurn ? 1 : 0);
        h.Add(s.PendingDiscard);   // post-draw discard-of-choice in flight (Acrobatics / Prepared)
        h.Add(s.PendingDiscardCard?.KeyHash ?? 0);   // its continuation (HiddenDaggers' shiv creation), or 0 — keeps an
                                                     // in-flight HiddenDaggers discard distinct from a continuation-less one
        if (s.BoundsPlays) h.Add(s.PlaysThisTurn);               // only when a cost-0 cantrip risks a play loop
        if (s.TracksCardsDrawn) h.Add(s.CardsDrawnThisCombat);   // only when a card (Murder) reads it
        // (Skills/Stars/Attacks/Discarded are added unconditionally, not gated on non-zero: conditional
        //  untagged adds would let e.g. Skills=0,Stars=3 collide with Skills=3,Stars=0 — position must stay fixed.)
        s.Player.Hash(ref h);
        h.Add(s.Monsters.Count);
        foreach (var m in s.Monsters) m.Hash(ref h);
        return h.Result;
    }
}
