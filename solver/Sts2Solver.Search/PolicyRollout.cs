using Sts2Solver.Engine;

namespace Sts2Solver.Search;

/// <summary>
/// A deterministic player policy: given a decision state, the action to take (or <c>null</c> to end the
/// turn). Used to drive faithful Monte-Carlo playouts so we can recover the *distribution* of HP loss
/// (min / max / percentiles) that the expectation-only solver value can't express.
/// </summary>
public interface IPlayoutPolicy
{
    /// <summary>The action to take at this player decision node, or <c>null</c> to end the turn.</summary>
    PlayerAction? NextAction(CombatState s);
}

/// <summary>Optimal policy backed by a solved <see cref="Solver"/>'s memo (exact, survival-first then
/// min expected HP loss). Make sure the solver's <see cref="Solver.Ct"/> is cleared before sampling so a
/// stale budget token can't interrupt rollouts.</summary>
public sealed class ExactMemoPolicy : IPlayoutPolicy
{
    private readonly Solver _solver;
    public ExactMemoPolicy(Solver solver) => _solver = solver;
    public PlayerAction? NextAction(CombatState s) => _solver.BestAction(s);
}

// The MCTS-path distribution policy is the shared CombatHeuristic.HeuristicPolicy (intent-aware,
// survive-then-race), defined in CombatHeuristic.cs so MCTS rollouts and this sampler stay in lock-step.

/// <summary>Summary of an outcome distribution over many playouts of a fixed policy. Loss percentiles are
/// over *raw* forward HP loss (matching the solver's <see cref="Value.Loss"/> semantics);
/// <see cref="NetMeanLoss"/> is the mean net of post-combat healing (e.g. Burning Blood) on winning runs.</summary>
public readonly record struct OutcomeStats(
    int Samples, double Survival,
    int MinLoss, int MaxLoss, double MeanLoss, int P10Loss, int P50Loss, int P90Loss,
    double NetMeanLoss);

/// <summary>Faithful engine playouts of a policy, for recovering the HP-loss distribution.</summary>
public static class PolicyRollout
{
    /// <summary>One faithful combat playout from a setup state (deck in draw pile, TurnNumber 0),
    /// resolving every stochastic step (initial + per-turn move rolls, draws) with <paramref name="rng"/>
    /// and choosing player actions with <paramref name="policy"/>. Returns whether the player won and the
    /// raw + net HP lost. Net loss includes any on-victory heal (Burning Blood) the engine actually
    /// applied, capped at max HP, clamped to ≥ 0.</summary>
    public static (bool win, int rawLoss, int netLoss) Playout(
        CombatState setup, IPlayoutPolicy policy, Rng rng, int maxTurns)
    {
        int startHp = setup.Player.CurrentHp;
        var combat = setup.Clone();
        combat.Rng = rng;   // ambient RNG so mid-turn draws (Shrug It Off, …) are real

        CombatManager.RollInitialMoves(combat, rng);
        CombatManager.BeginPlayerTurn(combat);
        CombatManager.DrawCards(combat, Player.CardsDrawnPerTurn, rng);

        while (!combat.IsCombatOver)
        {
            // Player turn: follow the policy until it ends the turn (guard against pathological loops).
            for (int guard = 0; guard < 500 && !combat.IsCombatOver; guard++)
            {
                var action = policy.NextAction(combat);
                if (action is not { CardKey: not null } a) break;
                var card = combat.Player.Hand.FirstOrDefault(h => h.StateKey() == a.CardKey);
                if (card == null) break;   // policy referenced a card no longer in hand — end the turn
                Creature? target = a.TargetMonsterIndex >= 0 && a.TargetMonsterIndex < combat.Monsters.Count
                    ? combat.Monsters[a.TargetMonsterIndex] : null;
                CombatManager.PlayCard(combat, card, target);
            }
            if (combat.IsCombatOver) break;

            CombatManager.EndPlayerTurn(combat);     // end-of-turn-in-hand (Infection/Burn) may kill here
            if (combat.IsCombatOver) break;
            CombatManager.RunEnemyTurn(combat);
            if (combat.IsCombatOver) break;

            CombatManager.RollNextMoves(combat, rng);
            CombatManager.BeginPlayerTurn(combat);
            CombatManager.DrawCards(combat, Player.CardsDrawnPerTurn, rng);
            if (combat.TurnNumber > maxTurns) break;   // failed to win within the horizon ⇒ loss
        }

        bool win = combat.AllMonstersDead;
        if (win) CombatManager.OnVictory(combat);      // post-combat relic heal (Burning Blood +6)
        int netLoss = Math.Max(0, startHp - combat.Player.CurrentHp);
        return (win, combat.PlayerHpLost, netLoss);
    }

    /// <summary>Run <paramref name="samples"/> seeded playouts and summarise survival + the HP-loss
    /// distribution. Seeds are derived from <paramref name="baseSeed"/> for reproducibility.</summary>
    public static OutcomeStats Sample(CombatState setup, IPlayoutPolicy policy, int samples, int maxTurns, int baseSeed = 1)
    {
        if (samples <= 0) throw new ArgumentOutOfRangeException(nameof(samples));
        var rawLosses = new int[samples];
        int wins = 0;
        double netSum = 0;
        for (int i = 0; i < samples; i++)
        {
            var (win, raw, net) = Playout(setup, policy, new Rng(baseSeed + i), maxTurns);
            rawLosses[i] = raw;
            if (win) wins++;
            netSum += net;
        }
        Array.Sort(rawLosses);
        int Pct(double q) => rawLosses[Math.Clamp((int)(q * (samples - 1) + 0.5), 0, samples - 1)];
        return new OutcomeStats(
            Samples: samples,
            Survival: (double)wins / samples,
            MinLoss: rawLosses[0],
            MaxLoss: rawLosses[^1],
            MeanLoss: rawLosses.Average(),
            P10Loss: Pct(0.10), P50Loss: Pct(0.50), P90Loss: Pct(0.90),
            NetMeanLoss: netSum / samples);
    }
}
