using Sts2Solver.Engine;

namespace Sts2Solver.Search;

/// <summary>
/// Lexicographic value of a state: maximise win probability first, then minimise expected forward
/// (additional) HP loss. Loss is measured from the state onward, independent of HP already lost.
/// </summary>
public readonly record struct Value(double Win, double Loss)
{
    private const double Eps = 1e-9;

    /// <summary>True if this value is strictly preferred to <paramref name="other"/>.</summary>
    public bool BetterThan(Value other) =>
        Win > other.Win + Eps || (Math.Abs(Win - other.Win) <= Eps && Loss < other.Loss - Eps);

    public override string ToString() => $"win={Win:P2}, E[HP loss]={Loss:F2}";
}

/// <summary>An action available at a player decision node. <see cref="ChoiceKey"/> is set (to an option's
/// <see cref="CardModel.StateKey"/>) only for cards that require an in-play choice (Headbutt, Armaments, …);
/// it is null for the common case and is what distinguishes the per-choice plays the search branches on.</summary>
public readonly record struct PlayerAction(string Label, string? CardKey, int TargetMonsterIndex, string? ChoiceKey = null)
{
    public static readonly PlayerAction EndTurn = new("End turn", null, -1);
}

public sealed class Solver
{
    public int MaxTurns { get; init; } = 30;

    /// <summary>Cooperative cancellation, checked periodically inside the recursion. Lets a caller bound
    /// exact search by wall-clock and fall back to sampling on timeout. Settable so a caller can clear it
    /// (back to <see cref="CancellationToken.None"/>) once <see cref="Solve"/> has completed, leaving the
    /// populated memo usable for policy extraction without further interruption.</summary>
    public CancellationToken Ct { get; set; } = CancellationToken.None;

    /// <summary>Optional admissible early-loss certificate. When set, decision nodes it proves lost are
    /// returned as their exact value <c>(0, CurrentHp)</c> without expansion — pruning that never changes the
    /// computed value (see <see cref="LossCertificate"/>), only the work. Null ⇒ no pruning.</summary>
    public LossCertificate? LossProof { get; set; }

    /// <summary>Optional hook fired once per distinct decision state when its exact forward value is
    /// finalised — used to harvest (state, value) training pairs for the learned value function
    /// (see <see cref="VfTrainer"/>). Null in normal solving.</summary>
    public Action<CombatState, Value>? OnSolved { get; set; }

    private readonly Dictionary<(ulong, ulong), Value> _memo = new();
    public int StatesEvaluated { get; private set; }

    // ---------- Top-level ----------

    /// <summary>
    /// Solve a combat from its setup state (deck in draw pile, TurnNumber 0). Averages over the
    /// stochastic opening (initial move roll + opening draw).
    /// </summary>
    public Value Solve(CombatState setup, int openingDraw = Player.CardsDrawnPerTurn)
    {
        double win = 0, loss = 0;
        foreach (var (prob, state) in OpeningStates(setup, openingDraw))
        {
            var v = SolvePlayerTurn(state);
            win += prob * v.Win;
            loss += prob * v.Loss;
        }
        return new Value(win, loss);
    }

    /// <summary>Distinct opening player-decision states with probabilities.</summary>
    public IEnumerable<(double prob, CombatState state)> OpeningStates(CombatState setup, int openingDraw)
    {
        foreach (var (probM, afterRoll) in EnumerateInitialMoveRolls(setup))
        {
            BeginPlayerTurnInPlace(afterRoll);
            afterRoll.PendingDraw = 0;   // discard any turn-start power draw (inert at the boundary, as before)
            foreach (var (probD, afterDraw) in DrawEnumerator.EnumerateDraw(afterRoll, openingDraw))
                yield return (probM * probD, afterDraw);
        }
    }

    // ---------- Player decision node ----------

    public Value SolvePlayerTurn(CombatState s)
    {
        if (s.AllMonstersDead) return new Value(1, 0);
        if (s.PlayerDead) return new Value(0, 0);
        if (s.TurnNumber > MaxTurns) return new Value(0, 0); // failed to win within the horizon

        var key = s.HashKey();
        if (_memo.TryGetValue(key, out var cached)) return cached;

        // Cooperative cancellation for a wall-clock budget (checked every 16k new states — cheap).
        if ((StatesEvaluated & 0x3FFF) == 0) Ct.ThrowIfCancellationRequested();
        StatesEvaluated++;

        // Post-draw discard-of-choice (Acrobatics / Prepared): the player picks which card(s) to drop before
        // normal play resumes — a MAX over the distinct hand cards. Memoised like any decision state.
        if (s.PendingDiscard > 0)
        {
            var dv = DiscardChoiceValue(s);
            _memo[key] = dv;
            return dv;
        }

        // Admissible early-loss prune: a provably-lost node has exact value (0, forward loss = current HP),
        // so we can skip expanding its subtree entirely without changing the computed value.
        if (LossProof != null && LossProof.IsProvablyLost(s))
        {
            var lost = new Value(0, s.Player.CurrentHp);
            _memo[key] = lost;
            return lost;
        }

        // Default action: end the turn.
        var best = EndTurnTransition(s);

        // Play any distinct playable card at any valid target.
        foreach (var action in LegalPlays(s))
        {
            var c = ApplyPlay(s, action);
            var v = ContinuePlay(c);   // playing a card costs the player no HP in scope
            if (v.BetterThan(best)) best = v;
        }

        _memo[key] = best;
        OnSolved?.Invoke(s, best);
        return best;
    }

    /// <summary>Continue the player's turn after a play has resolved. If the play deferred a mid-turn draw
    /// (<see cref="CombatState.PendingDraw"/> &gt; 0), open an explicit draw chance node — averaging the same
    /// turn's value over the exact draw distribution (drawing costs no HP, so no loss term is added here) —
    /// before recursing. Otherwise recurse directly. Each draw outcome is itself a clean decision state
    /// (PendingDraw drained), so the memo key never needs the counter.</summary>
    private Value ContinuePlay(CombatState c)
    {
        if (c.PendingDraw <= 0 || c.IsCombatOver) return SolvePlayerTurn(c);

        int n = c.PendingDraw;
        c.PendingDraw = 0;   // drained before the (cloning) enumerator, so each drawn child starts clean
        double win = 0, loss = 0;
        foreach (var (probD, afterDraw) in DrawEnumerator.EnumerateDraw(c, n))
        {
            CombatManager.ApplyPostDraw(afterDraw);   // EscapePlan block / set PendingDiscard (no-op for plain draws)
            var v = SolvePlayerTurn(afterDraw);
            win += probD * v.Win;
            loss += probD * v.Loss;
        }
        return new Value(win, loss);
    }

    /// <summary>Resolve a post-draw discard-of-choice (s.PendingDiscard &gt; 0): the player MAXes over which
    /// distinct hand card to drop (identical cards collapse by StateKey), one card per step, until the count is
    /// exhausted or the hand empties — then normal play resumes. Drawing/discarding cost no HP, so this is a
    /// pure lexicographic MAX over the resulting decision states.</summary>
    private Value DiscardChoiceValue(CombatState s)
    {
        var hand = s.Player.Hand;
        if (hand.Count == 0)   // nothing left to discard: clear the obligation, run any continuation, resume play
        {
            var cleared = s.Clone();
            cleared.PendingDiscard = 0;
            CombatManager.ApplyPostDiscard(cleared);   // HiddenDaggers adds its Shivs here (no-op for Acrobatics/Prepared)
            return SolvePlayerTurn(cleared);
        }

        Value best = default;
        bool any = false;
        var seen = new HashSet<string>();
        foreach (var card in hand)
        {
            var key = card.StateKey();
            if (!seen.Add(key)) continue;   // symmetric discards collapse
            var c = s.Clone();
            Cmd.DiscardFromHand(c, c.Player.Hand.First(h => h.StateKey() == key));
            c.PendingDiscard--;
            if (c.PendingDiscard == 0) CombatManager.ApplyPostDiscard(c);   // last discard resolved → run the continuation
            var v = SolvePlayerTurn(c);
            if (!any || v.BetterThan(best)) { best = v; any = true; }
        }
        return best;
    }

    /// <summary>Distinct (card, target) plays available at a decision node, deduplicated by card key.</summary>
    public IEnumerable<PlayerAction> LegalPlays(CombatState s)
    {
        // Cap plays per turn (unconditional safety net): a cost-0 replayable draw cantrip could otherwise build
        // an unbounded play chain and blow the stack. The cap sits far above any real line, so it never changes
        // the optimal value; flagged loop-risk decks additionally HASH PlaysThisTurn so the cap memoises soundly.
        if (s.PlaysThisTurn >= CombatState.MaxPlaysPerTurn) yield break;
        var seen = new HashSet<string>();
        foreach (var card in s.Player.Hand)
        {
            if (card.Unplayable) continue;
            if (!card.IsXCost && card.EffectiveCost(s) > s.Player.Energy) continue;   // EffectiveCost: in-combat cost reductions
            if (!s.Player.CanAffordStars(card)) continue;   // Regent star cost gates the play
            var ck = card.StateKey();
            var choices = card.Choices(s).Distinct().ToList();   // empty for the common no-choice card
            if (card.NeedsTarget)
            {
                for (int i = 0; i < s.Monsters.Count; i++)
                {
                    if (!s.Monsters[i].IsAlive) continue;
                    if (choices.Count == 0)
                    {
                        if (seen.Add($"{ck}@{i}")) yield return new PlayerAction($"Play {ck} -> M{i}", ck, i);
                    }
                    else foreach (var choice in choices)
                        if (seen.Add($"{ck}@{i}#{choice}"))
                            yield return new PlayerAction($"Play {ck} -> M{i} [{choice}]", ck, i, choice);
                }
            }
            else if (choices.Count == 0)
            {
                if (seen.Add(ck)) yield return new PlayerAction($"Play {ck}", ck, -1);
            }
            else foreach (var choice in choices)
                if (seen.Add($"{ck}#{choice}"))
                    yield return new PlayerAction($"Play {ck} [{choice}]", ck, -1, choice);
        }
    }

    /// <summary>Apply a play to a clone of <paramref name="s"/>. Internal so the sampling solver and other
    /// callers drive the SAME transition as the exact oracle.</summary>
    internal static CombatState ApplyPlay(CombatState s, PlayerAction action)
    {
        var c = s.Clone();
        var card = c.Player.Hand.First(h => h.StateKey() == action.CardKey);
        Creature? target = action.TargetMonsterIndex >= 0 ? c.Monsters[action.TargetMonsterIndex] : null;
        CombatManager.PlayCard(c, card, target, action.ChoiceKey);
        return c;
    }

    // ---------- End-of-turn transition (enemy turn + chance nodes) ----------

    private Value EndTurnTransition(CombatState s)
    {
        var afterEnemy = s.Clone();
        CombatManager.EndPlayerTurn(afterEnemy);
        CombatManager.RunEnemyTurn(afterEnemy);   // deterministic: telegraphed moves
        int enemyLoss = afterEnemy.PlayerHpLost - s.PlayerHpLost;

        if (afterEnemy.PlayerDead) return new Value(0, enemyLoss);
        if (afterEnemy.AllMonstersDead) return new Value(1, enemyLoss);

        double win = 0, leafLoss = 0;
        foreach (var (probM, afterRoll) in EnumerateNextMoveRolls(afterEnemy))
        {
            BeginPlayerTurnInPlace(afterRoll);
            afterRoll.PendingDraw = 0;   // discard any turn-start power draw (inert at the boundary, as before)
            int startLoss = afterRoll.PlayerHpLost - afterEnemy.PlayerHpLost; // start-of-turn (e.g. poison)
            foreach (var (probD, afterDraw) in DrawEnumerator.EnumerateDraw(afterRoll, Player.CardsDrawnPerTurn))
            {
                var v = SolvePlayerTurn(afterDraw);
                double w = probM * probD;
                win += w * v.Win;
                leafLoss += w * (startLoss + v.Loss);
            }
        }
        return new Value(win, enemyLoss + leafLoss);
    }

    internal static void BeginPlayerTurnInPlace(CombatState s) => CombatManager.BeginPlayerTurn(s);

    // ---------- Chance: monster move rolls (joint over living monsters) ----------

    private IEnumerable<(double prob, CombatState state)> EnumerateInitialMoveRolls(CombatState setup)
        => EnumerateMoveRolls(setup, initial: true);

    internal IEnumerable<(double prob, CombatState state)> EnumerateNextMoveRolls(CombatState s)
        => EnumerateMoveRolls(s, initial: false);

    private IEnumerable<(double prob, CombatState state)> EnumerateMoveRolls(CombatState s, bool initial)
    {
        // Collect each living monster's move distribution, then take the cartesian product.
        var perMonster = new List<(int index, List<(double prob, string moveId)> dist)>();
        for (int i = 0; i < s.Monsters.Count; i++)
        {
            var m = s.Monsters[i];
            if (!m.IsAlive) continue;
            var dist = initial ? m.Ai.EnumerateInitial(m) : m.Ai.EnumerateNext(m);
            perMonster.Add((i, dist));
        }

        foreach (var combo in CartesianProduct(perMonster))
        {
            var c = s.Clone();
            foreach (var (index, moveId) in combo.assignment)
                c.Monsters[index].Ai.CurrentMoveId = moveId;
            yield return (combo.prob, c);
        }
    }

    private static IEnumerable<(double prob, List<(int index, string moveId)> assignment)> CartesianProduct(
        List<(int index, List<(double prob, string moveId)> dist)> perMonster)
    {
        var acc = new List<(double, List<(int, string)>)> { (1.0, new List<(int, string)>()) };
        foreach (var (index, dist) in perMonster)
        {
            var next = new List<(double, List<(int, string)>)>();
            foreach (var (accProb, accList) in acc)
                foreach (var (prob, moveId) in dist)
                {
                    var list = new List<(int, string)>(accList) { (index, moveId) };
                    next.Add((accProb * prob, list));
                }
            acc = next;
        }
        return acc.Select(a => (a.Item1, a.Item2));
    }

    // ---------- Policy extraction ----------

    /// <summary>
    /// The single best action at a player decision node under the lexicographic objective: the play whose
    /// resulting state beats ending the turn, or <c>null</c> to end the turn (the baseline). Values come
    /// from the (possibly on-demand-extended) memo. Does not mutate <paramref name="s"/>.
    /// </summary>
    public PlayerAction? BestAction(CombatState s)
    {
        if (s.IsCombatOver) return null;
        var bestValue = EndTurnTransition(s);   // baseline: end the turn now
        PlayerAction? bestAction = null;
        foreach (var action in LegalPlays(s))
        {
            var child = ApplyPlay(s, action);
            var v = ContinuePlay(child);
            if (v.BetterThan(bestValue)) { bestValue = v; bestAction = action; }
        }
        return bestAction;   // null ⇒ end the turn is optimal
    }

    /// <summary>
    /// Greedily extract the optimal play sequence for the player's current turn from a decision state
    /// (the cards to play, in order, until ending the turn). Values are taken from the solved memo.
    /// </summary>
    public List<string> BestTurnPlan(CombatState s)
    {
        var plan = new List<string>();
        var cur = s.Clone();
        while (!cur.IsCombatOver)
        {
            var action = BestAction(cur);
            if (action == null) { plan.Add("End turn"); break; }
            plan.Add(action.Value.Label);
            cur = ApplyPlay(cur, action.Value);
        }
        return plan;
    }
}
