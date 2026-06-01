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
        h.Add(s.AttacksPlayedThisTurn);
        h.Add(s.CardsDiscardedThisTurn);
        if (s.TracksCardsDrawn) h.Add(s.CardsDrawnThisCombat);   // only when a card (Murder) reads it
        s.Player.Hash(ref h);
        h.Add(s.Monsters.Count);
        foreach (var m in s.Monsters) m.Hash(ref h);
        return h.Result;
    }
}
