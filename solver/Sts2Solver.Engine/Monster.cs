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

    /// <summary>Turns this monster's move is skipped (Whistle stun = 1). Each enemy turn a stunned monster
    /// takes no action and the counter decrements; its telegraphed move is simply DELAYED to its next turn
    /// (the AI transition lives inside the move's Perform, which is skipped, so CurrentMoveId is preserved).
    /// Bounded to the stun count — never a permanent disable — so it can't over-credit the player.</summary>
    public int StunnedTurns;

    /// <summary>Decimillipede reattach phase. 0 = not downed. When a Reattach segment is brought to 0 HP while
    /// another segment still lives it is DOWNED, not killed: its non-Reattach powers are stripped and this is set
    /// to 2 (the game's DEAD_MOVE → REATTACH_MOVE delay). Each enemy turn it decrements; at 0 the segment
    /// reattaches (heals to its Reattach amount) if another segment is still alive. While &gt; 0 the segment sits
    /// at 0 HP — untargetable (IsAlive false) and out of the move-roll — so the revival is a deterministic
    /// <see cref="CombatManager.RunEnemyTurn"/> step, never a chance node (convergence-preserving). Gated:
    /// 0 for every non-Decimillipede monster, so it never enters their hash/state key.</summary>
    public int ReattachIn;

    /// <summary>The AI move a "death-phase" monster jumps to when brought to 0 HP INSTEAD of dying — a survive-
    /// at-0 → telegraph → final-blow → die sequence (e.g. WaterfallGiant's Steam Eruption). null = no death
    /// phase (the monster dies normally). Set once at construction; identity only, never hashed.</summary>
    public string? DeathPhaseEntryMove;

    /// <summary>True while the monster is in its death phase: it was reduced to 0 HP but survives (untargetable,
    /// further damage ignored) to run its final-blow sequence, then truly dies. Keeps it <see cref="IsAlive"/>
    /// so combat doesn't end before the blow lands. Gated: false for every non-death-phase monster.</summary>
    public bool InDeathPhase;

    /// <summary>A death-phase monster stays alive at 0 HP until its final blow resolves; it is then untargetable
    /// (the player can't hit a 0-HP monster — its damage is a no-op) but still acts, so the explosion is
    /// guaranteed. Otherwise the standard <c>CurrentHp &gt; 0</c>.</summary>
    public override bool IsAlive => CurrentHp > 0 || InDeathPhase;

    public Monster() { Side = CombatSide.Enemy; }

    /// <summary>True when the monster's currently telegraphed move is an attack (its <see cref="MoveState"/>
    /// carries IntentDamage). Read by Defect GoForTheEyes to conditionally apply Weak. Safe before the first
    /// move is rolled (returns false).</summary>
    public bool IntendsToAttack
        => Ai != null && Ai.States.TryGetValue(Ai.CurrentMoveId, out var s) && s is MoveState m && m.IntentDamage.HasValue;

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
        m.StunnedTurns = StunnedTurns;
        m.ReattachIn = ReattachIn;
        m.DeathPhaseEntryMove = DeathPhaseEntryMove;
        m.InDeathPhase = InDeathPhase;
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
        if (StunnedTurns > 0) h.Add(StunnedTurns * 0x9E3779B1);   // gated: 0 for every un-stunned monster
        if (ReattachIn > 0) h.Add(ReattachIn * 0x85EBCA77);       // gated: 0 for every non-downed monster
        if (InDeathPhase) h.Add(0x27D4EB2F);                      // gated: false for every non-death-phase monster
    }

    public override string StateKey()
    {
        var v = Variant.Length > 0 ? $":{Variant}" : "";
        var stun = StunnedTurns > 0 ? $"!{StunnedTurns}" : "";   // appended only when stunned (no fragmentation otherwise)
        var down = ReattachIn > 0 ? $"~{ReattachIn}" : "";       // appended only while downed (Decimillipede)
        var boom = InDeathPhase ? "*" : "";                      // appended only during a death phase (explode-on-death)
        return $"M({base.StateKey()}{v}{stun}{down}{boom}|{Ai.StateKey()})";
    }
}
