using Sts2Solver.Engine;

namespace Sts2Solver.Search;

/// <summary>
/// An admissible in-search early-loss certificate: proves, for individual decision states, that the player
/// cannot win and is dead within the horizon — so the searcher can return the exact value of that subtree
/// without expanding it. Complements <see cref="HorizonBound"/> (a single global depth cut) with per-node
/// pruning that uses each node's <i>actual</i> remaining HP.
///
/// Why returning a concrete value here is exact (not just a survival floor): if the player is guaranteed
/// dead by enemy turn <c>Kdie ≤ MaxTurns</c>, then EVERY descendant line ends in death within the horizon
/// (none is horizon-truncated to (0,0)), and — with no mid-combat healing — the forward HP lost on a death
/// is exactly the player's current HP (HP runs from its current value to 0). So the subtree's lexicographic
/// value is exactly <c>(win=0, loss=CurrentHp)</c>, independent of how the doomed player plays. The exact
/// solver would compute the same; this just skips the work. Gated by oracle-equality tests.
///
/// Soundness rests on three sound bounds for a SINGLE, DETERMINISTIC, non-summoning enemy facing a deck with
/// no Power cards, no player-power gains, no damage growth (Strength/Vulnerable), and no healing:
///  • incoming is LOWER-bounded by the forced-min-move, permanently-Weak trajectory (<see cref="HorizonBound"/>),
///  • player block is UPPER-bounded per turn (so Kdie, the proven death turn, is pushed as late as possible),
///  • player damage output is UPPER-bounded per cycle (so we never under-state the player's ability to win).
/// Any disqualifying deck/enemy ⇒ <see cref="TryBuild"/> returns null and no pruning happens.
/// </summary>
public sealed class LossCertificate
{
    private readonly long[] _dmgCum;   // cumulative min incoming by absolute enemy turn t (max-Weak); index 0..cap
    private readonly DeckProfile _deck;
    private readonly int _cap;

    private LossCertificate(long[] dmgCum, DeckProfile deck, int cap)
    {
        _dmgCum = dmgCum; _deck = deck; _cap = cap;
    }

    /// <summary>Build a certificate for this combat, or null if it doesn't qualify (the searcher then runs
    /// unpruned). <paramref name="maxTurns"/> is the search horizon the certificate is evaluated against.</summary>
    public static LossCertificate? TryBuild(CombatState setup, int maxTurns)
    {
        if (setup.Monsters.Count(m => m.IsAlive) != 1) return null;

        var deck = DeckProfile.Build(setup.Player);
        if (deck == null || !deck.DamageBoundable || deck.HealsPlayer) return null;

        // The trajectory both lower-bounds incoming and (via its summon guard) certifies the enemy stays a
        // lone, deterministic attacker for the whole horizon.
        var dmg = HorizonBound.MinDamageTrajectory(setup, maxTurns, deck.WeakProto);
        if (dmg == null) return null;

        return new LossCertificate(dmg, deck, maxTurns);
    }

    /// <summary>True if the player is provably unable to win from <paramref name="s"/> (a guaranteed loss).
    /// When true, the subtree's exact value is <c>(0, s.Player.CurrentHp)</c>.</summary>
    public bool IsProvablyLost(CombatState s)
    {
        Monster? enemy = null;
        int living = 0;
        foreach (var m in s.Monsters) if (m.IsAlive) { living++; enemy = m; }
        if (living != 1) return false;                 // already won, or summoned adds — out of scope

        int t0 = s.TurnNumber;
        if (t0 < 1 || t0 > _cap) return false;
        long prior = _dmgCum[t0 - 1];
        long hpPool = s.Player.CurrentHp + s.Player.Block;

        // Earliest absolute enemy turn the player is guaranteed dead by: even max future block can't cover the
        // min future incoming. (Both bounds taken in the player-favourable direction ⇒ a sound death turn.)
        int kdie = -1;
        for (int t = t0; t <= _cap && t < _dmgCum.Length; t++)
        {
            long incForward = _dmgCum[t] - prior;
            long blockForward = HorizonBound.BlockUpperBound(t - t0 + 1, _deck);
            if (hpPool + blockForward < incForward) { kdie = t; break; }
        }
        if (kdie < 0) return false;                    // no death provable within the horizon ⇒ can't prune

        // The player has player-turns t0..kdie to deal the enemy's remaining HP (player turn k precedes enemy
        // turn k). If even the upper-bound damage over those turns falls short, the fight is unwinnable.
        int turns = kdie - t0 + 1;
        long damageUb = _deck.DamageUpperBound(turns);
        return damageUb < enemy!.CurrentHp;            // current HP under-states kill cost (ignores block/heal) ⇒ sound
    }
}
