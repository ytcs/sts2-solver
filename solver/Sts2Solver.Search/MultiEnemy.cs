using Sts2Solver.Engine;

namespace Sts2Solver.Search;

/// <summary>
/// Sound horizon bound for multi-enemy fights via kill-order reasoning. A line is "undecided" while ≥1 enemy
/// is alive; the player can kill enemies to reduce future incoming, but killing costs damage output over
/// time. We compute a sound LOWER bound on cumulative incoming over undecided lines, then find the turn the
/// player is guaranteed dead by (vs the max deck block).
///
/// The incoming lower bound combines, per turn t:
///  • each enemy's solo forced-min, permanently-Weak damage trajectory (a lower bound on what it deals while
///    alive — allies can only buff it, so isolation under-states), and
///  • the most damage the player could have "saved" by killing attackers before t. To keep the saving an
///    OVER-estimate (so incoming stays a sound LOWER bound), every non-survivor is treated as killed as early
///    as if the player's <i>entire</i> upper-bound damage budget were dedicated to it alone — impossible to
///    beat with a shared budget — and we take the survivor choice that maximises the saving (min over which
///    single enemy stays alive forever, since the line must stay undecided). When the deck's damage can't be
///    bounded, kills are treated as free/instant (budget = ∞), giving the loosest-but-sound min-single-enemy
///    bound.
/// Bails to <paramref name="defaultMax"/> on any enemy with a branching AI or a mid-fight summon.
/// </summary>
public static class MultiEnemy
{
    public static int Compute(CombatState setup, DeckProfile deck, int defaultMax)
    {
        var livingIdx = Enumerable.Range(0, setup.Monsters.Count)
            .Where(i => setup.Monsters[i].IsAlive).ToList();
        int n = livingIdx.Count;
        if (n < 2) return defaultMax;   // single-enemy path handles n==1

        // Per-enemy solo cumulative min-incoming trajectory (with permanent Weak) and HP-to-kill.
        var e = new long[n][];
        var killTurn = new int[n];
        for (int k = 0; k < n; k++)
        {
            var solo = SoloState(setup, livingIdx[k]);
            var traj = HorizonBound.MinDamageTrajectory(solo, defaultMax, deck.WeakProto);
            if (traj == null) return defaultMax;            // branching / summoning enemy ⇒ unbounded here
            e[k] = traj;

            int hp = setup.Monsters[livingIdx[k]].CurrentHp; // ignores block ⇒ kills modelled as fast as possible
            killTurn[k] = deck.DamageBoundable ? EarliestKill(hp, deck, defaultMax) : 1;
        }

        int playerHp = setup.Player.CurrentHp;
        for (int t = 1; t <= defaultMax; t++)
        {
            long incLb = MinIncoming(e, killTurn, t, n);
            long blockUb = HorizonBound.BlockUpperBound(t, deck);
            if (playerHp + blockUb <= incLb)
                return Math.Clamp(t + HorizonBound.MarginTurns, HorizonBound.FloorTurns, defaultMax);
        }
        return defaultMax;
    }

    /// <summary>Sound lower bound on cumulative incoming by turn t: pick the single survivor that maximises
    /// the player's saving (every other enemy killed as early as the budget alone could manage).</summary>
    private static long MinIncoming(long[][] e, int[] killTurn, int t, int n)
    {
        long best = long.MaxValue;
        for (int s = 0; s < n; s++)                          // s = the enemy that stays alive the whole time
        {
            long inc = At(e[s], t);
            for (int i = 0; i < n; i++)
            {
                if (i == s) continue;
                inc += At(e[i], Math.Min(t, killTurn[i] - 1));  // i contributes only until it could be killed
            }
            if (inc < best) best = inc;
        }
        return best;
    }

    private static long At(long[] cum, int t) => t <= 0 ? 0 : cum[Math.Min(t, cum.Length - 1)];

    /// <summary>Earliest player turn whose cumulative damage UPPER bound covers <paramref name="hp"/>, or
    /// cap+1 if the player provably can't kill the enemy within the horizon even dedicating the whole budget.</summary>
    private static int EarliestKill(int hp, DeckProfile deck, int cap)
    {
        for (int p = 1; p <= cap; p++)
            if (deck.DamageUpperBound(p) >= hp) return p;
        return cap + 1;
    }

    /// <summary>A clone of the setup keeping only the one enemy at <paramref name="idx"/> (the rest removed),
    /// so its damage can be trajected in isolation.</summary>
    private static CombatState SoloState(CombatState setup, int idx)
    {
        var c = setup.Clone();
        var keep = c.Monsters[idx];
        c.Monsters.Clear();
        c.Monsters.Add(keep);
        return c;
    }
}
