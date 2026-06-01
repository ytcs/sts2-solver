using Sts2Solver.Engine;

namespace Sts2Solver.Search;

/// <summary>
/// Derives a SOUND upper bound on the search horizon (player turns) for a fight, from the player's
/// intuition that a fight is bounded when the deck can't out-block the enemy's (ramping) telegraphed damage
/// per deck-cycle. If by some turn T the player's HP plus the most block the deck could possibly have
/// produced still can't cover the least damage the enemy could possibly have dealt, the player is dead by T
/// under ANY play — so any line not decided by T is a loss, and T is a sufficient horizon (it captures every
/// win, since a win must happen before the player would die). "Killing is blocking" is consistent here: the
/// bound assumes the player never kills (the longest possible defensive fight), so the real fight ends no
/// later.
///
/// Soundness is the whole point — a horizon cut too short would silently drop winning lines and
/// under-report survival. So this is deliberately conservative: it OVER-estimates the player's block (the
/// safe direction) and UNDER-estimates incoming, and BAILS to <c>defaultMax</c> in every case it can't prove.
/// It only ever returns a value ≤ <c>defaultMax</c>. Gated by oracle-equality tests (HorizonBoundTests).
///
/// v2 extends the single-enemy bound two ways while preserving soundness:
///  • <b>Weak-bearing decks</b> no longer bail. A deck that can apply Weak only ever <i>reduces</i> incoming,
///    so the min-damage trajectory now assumes the enemy is permanently Weak (×0.75) — the maximum player
///    benefit, which pushes the proven death turn <i>later</i> (a longer, still-sound horizon).
///  • <b>Multi-enemy</b> fights are bounded by <see cref="MultiEnemy"/> kill-order reasoning: a sound lower
///    bound on cumulative incoming over undecided lines (≥1 enemy alive), giving the player a generous
///    upper-bound damage budget to kill the highest-rate attackers first.
/// </summary>
public static class HorizonBound
{
    // Conservative tunables. The block slack absorbs mitigation not captured by on-play probing (e.g. a
    // relic, an unseen interaction); the turn margin pads the proven death turn. Generous on purpose: for a
    // ramping enemy the bound still triggers (growth outpaces any fixed block); for a non-ramping enemy the
    // player can genuinely turtle forever and we correctly fall back to defaultMax.
    internal const int UnaccountedBlockPerTurn = 8;
    internal const int MarginTurns = 2;
    internal const int FloorTurns = 4;

    public static int Compute(CombatState setup, int defaultMax)
    {
        var living = setup.Monsters.Where(m => m.IsAlive).ToList();
        if (living.Count == 0) return Math.Min(FloorTurns, defaultMax);

        // Characterise the deck's block/Weak. Bail on anything that could push real incoming BELOW the
        // min-move trajectory in a way we don't model: a Power card, or a card that grants the player a power
        // (a damage-reducing power would invalidate the trajectory). Weak is now modelled, not bailed on.
        var deck = DeckProfile.Build(setup.Player);
        if (deck == null) return defaultMax;

        if (living.Count > 1)
            return MultiEnemy.Compute(setup, deck, defaultMax);

        // ---- Single living enemy: the original block-deficit bound, now Weak-aware. ----
        var dmg = MinDamageTrajectory(setup, defaultMax, deck.WeakProto);
        if (dmg == null) return defaultMax;              // branching / unresolvable enemy AI

        int hp = setup.Player.CurrentHp;
        for (int t = 1; t <= defaultMax && t < dmg.Length; t++)
        {
            long blockUb = BlockUpperBound(t, deck);
            if (hp + blockUb <= dmg[t])
                return Math.Clamp(t + MarginTurns, FloorTurns, defaultMax);
        }
        return defaultMax;                               // no finite bound provable ⇒ keep the default
    }

    /// <summary>Most block the deck could have produced by the end of turn <paramref name="t"/>: a full
    /// deck's block per cycle (front-loaded — generous), plus the per-turn unaccounted slack.</summary>
    internal static long BlockUpperBound(int t, DeckProfile deck) =>
        (long)((t + deck.CycleTurns - 1) / deck.CycleTurns) * deck.BlockPerCycle
        + (long)UnaccountedBlockPerTurn * t;

    // ---------- enemy min-damage trajectory (sound lower bound on cumulative incoming) ----------

    /// <summary>Cumulative gross incoming by the end of each turn 1..cap, with each (single, deterministic)
    /// enemy forced to its minimum-damage move and the player doing nothing at huge HP (so the sim runs the
    /// full horizon and block never reduces the gross figure). When <paramref name="weakProto"/> is non-null
    /// the deck can apply Weak, so each enemy is kept permanently Weak (the maximum mitigation the player
    /// could sustain — generous, keeps the bound a sound LOWER bound on incoming). Returns null if any enemy
    /// turn is non-deterministic (a branch) or can't be resolved — the caller then bails.</summary>
    internal static long[]? MinDamageTrajectory(CombatState setup, int cap, PowerModel? weakProto)
    {
        var sim = setup.Clone();
        sim.Player.MaxHp = 1_000_000_000;
        sim.Player.CurrentHp = 1_000_000_000;            // effectively invincible: the fight runs to `cap`
        int baseLost = sim.PlayerHpLost;
        int initialLiving = sim.Monsters.Count(m => m.IsAlive);

        foreach (var m in sim.Monsters)
            if (m.IsAlive && !SetMinMove(m, initial: true)) return null;

        var traj = new long[cap + 1];
        for (int t = 1; t <= cap; t++)
        {
            // A mid-fight summon (more living enemies than we started with) breaks the lower bound: a real
            // player could kill the adds, pushing incoming below this idle trajectory. Bail to the caller.
            if (sim.Monsters.Count(m => m.IsAlive) > initialLiving) return null;

            // Keep every living enemy permanently Weak (×0.75) when the deck can apply Weak: the maximum
            // incoming reduction the player could sustain, so the trajectory stays a LOWER bound on incoming.
            if (weakProto != null) KeepWeak(sim, weakProto);

            CombatManager.BeginPlayerTurn(sim);          // player plays nothing this turn
            CombatManager.EndPlayerTurn(sim);
            CombatManager.RunEnemyTurn(sim);
            traj[t] = sim.PlayerHpLost - baseLost;
            foreach (var m in sim.Monsters)
                if (m.IsAlive && !SetMinMove(m, initial: false)) return null;
        }
        return traj;
    }

    /// <summary>Top every living enemy back up to a high Weak stack so the ×0.75 multiplier never lapses
    /// over the bounded horizon (Weak ticks down 1 per enemy turn end).</summary>
    private static void KeepWeak(CombatState sim, PowerModel weakProto)
    {
        const int topUp = 1_000;
        foreach (var m in sim.Monsters)
        {
            if (!m.IsAlive) continue;
            int have = m.GetPowerAmount("Weak");
            if (have < topUp) m.AddPower(weakProto.Clone(), topUp - have);
        }
    }

    /// <summary>Force <paramref name="m"/> to its minimum-damage telegraphed move. Returns false if the move
    /// choice is non-deterministic (the AI branches) — we only bound deterministic enemies soundly.</summary>
    internal static bool SetMinMove(Monster m, bool initial)
    {
        List<(double prob, string moveId)> dist;
        try { dist = initial ? m.Ai.EnumerateInitial(m) : m.Ai.EnumerateNext(m); }
        catch { return false; }
        if (dist.Count != 1) return false;               // branch ⇒ not soundly minimisable here
        m.Ai.CurrentMoveId = dist[0].moveId;
        return true;
    }
}
