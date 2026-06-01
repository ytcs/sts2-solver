namespace Sts2Solver.Engine;

/// <summary>An enemy creature plus its AI state machine.</summary>
public sealed class Monster : Creature
{
    public MonsterMoveStateMachine Ai = null!;

    /// <summary>Stable per-combat identity, assigned in encounter order (1-based) and continued for
    /// summoned monsters. Mirrors the game's creature id; used to disambiguate same-named monsters
    /// (e.g. multiple Wrigglers) when replaying recorded plays. Identity only — not part of the hash.</summary>
    public int Id;

    /// <summary>Behavioural discriminator for monsters that share a Name but differ in AI (e.g. a
    /// bite-first vs wriggle-first Wriggler). Folded into the structural hash so the solver never
    /// conflates two variants whose telegraphed move/log momentarily coincide.</summary>
    public string Variant = "";

    /// <summary>Set when summoned mid-combat with an unknown rolled HP; the trace validator fills the
    /// observed HP in on the monster's first compared snapshot, then clears it. Never hashed.</summary>
    public bool NeedsSpawnHpSync;

    public Monster() { Side = CombatSide.Enemy; }

    /// <summary>Execute the currently telegraphed move and record it in the move log. A move id that
    /// isn't a known MoveState (e.g. an injected "STUNNED") is treated as a no-op turn.</summary>
    public void PerformCurrentMove(CombatState combat)
    {
        if (!Ai.States.TryGetValue(Ai.CurrentMoveId, out var state) || state is not MoveState move)
            return; // stunned / unknown -> do nothing this turn
        move.Perform(combat, this);
        Ai.MoveLog.Add(move.Id);
    }

    public override Creature Clone()
    {
        var m = new Monster();
        CopyCreatureBaseTo(m);
        m.Ai = Ai.CloneState();
        m.Id = Id;
        m.Variant = Variant;
        m.NeedsSpawnHpSync = NeedsSpawnHpSync;
        return m;
    }

    public override void Hash(ref StateHasher h)
    {
        base.Hash(ref h);
        h.Add(Name.GetHashCode());
        h.Add(Variant.GetHashCode());
        h.Add(Ai.CurrentMoveId.GetHashCode());
        h.Add(Ai.MoveLog.Count);
        foreach (var id in Ai.MoveLog) h.Add(id.GetHashCode());   // ordered
    }

    public override string StateKey()
    {
        var v = Variant.Length > 0 ? $":{Variant}" : "";
        return $"M({base.StateKey()}{v}|{Ai.StateKey()})";
    }
}
