using Sts2Solver.Engine;

namespace Sts2Solver.Search;

/// <summary>
/// One shared intent-aware heuristic, used as (a) the rollout/leaf policy for both <see cref="MctsSolver"/>
/// and <see cref="PolicyRollout"/> (so there is a single definition of "reasonable play"), and (b) an
/// optional static leaf-value estimator (a survive-the-telegraphed-hit + race model, per
/// <c>docs/mcts-solver-design.md §5</c>). Weights live as constants here so they can be calibrated against
/// the exact oracle (see CalibrationHarness). Non-admissible is fine — this only guides search.
/// </summary>
public static class CombatHeuristic
{
    // Position-score weights (lower score = better for the player). The score is *survival-first*: dying to
    // the telegraphed hit is a near-infinite cliff, being left with too little HP to survive the next hit is
    // steeply penalised, and only then does racing the enemy's HP down (lightly sped by Strength/Vulnerable)
    // matter. This mirrors the solver's lexicographic objective (survive, then minimise loss) — a plain
    // linear "enemyHp ≫ unblocked" score under-blocks because enemy HP dwarfs incoming, so the greedy
    // rollout eats lethal hits and throws away winnable low-survival fights. Calibrate against exact.
    // Weights are env-overridable (STS2_W*) so they can be swept against the exact oracle without a rebuild;
    // the defaults are the calibrated values.
    private static double Env(string k, double dflt) =>
        double.TryParse(Environment.GetEnvironmentVariable(k), out var v) ? v : dflt;
    private static readonly double WEnemyHp = Env("STS2_WENEMYHP", 100.0);   // race: per point of remaining enemy HP
    private static readonly double WLoss = Env("STS2_WLOSS", 50.0);          // per HP lost to the telegraphed hit this turn
    private static readonly double WDanger = Env("STS2_WDANGER", 0.0);       // per HP left below the safety buffer (next-hit risk)
    private static readonly double BufferFrac = Env("STS2_BUFFERFRAC", 0.0); // safety buffer as a fraction of the incoming hit
    private static readonly double LethalPenalty = Env("STS2_WLETHAL", 1e9); // dying to this turn's hit — avoid at almost any cost
    private static readonly double WStrength = Env("STS2_WSTRENGTH", 60.0);  // per point of player Strength (×living enemies)
    private static readonly double WVuln = Env("STS2_WVULN", 25.0);          // per stack of Vulnerable on enemies
    private static readonly double WOverblock = Env("STS2_WOVERBLOCK", 2.0); // mild discipline against wasting block
    private const double SurvivalSlope = 1.25;                               // logistic slope per turn of race margin (leaf)

    // ---------- Faithful damage prediction (mirrors Cmd.Attack: additive → multiplicative → floor) ----------

    /// <summary>The post-modifier damage a single hit of <paramref name="baseDamage"/> would deal
    /// (Strength/Vulnerable/Weak etc.), without applying it. Mirrors <see cref="Cmd.Attack"/>'s pipeline.</summary>
    public static int PredictHit(CombatState combat, Creature dealer, Creature target, int baseDamage, ValueProp props)
    {
        decimal amount = baseDamage;
        foreach (var c in combat.AllCreatures)
            foreach (var p in c.Powers)
                amount += p.ModifyDamageAdditive(target, amount, props, dealer, null);
        foreach (var c in combat.AllCreatures)
            foreach (var p in c.Powers)
                amount *= p.ModifyDamageMultiplicative(target, amount, props, dealer, null);
        return (int)Math.Floor(Math.Max(0m, amount));
    }

    /// <summary>Total telegraphed incoming damage to the player this enemy turn, AFTER modifiers and
    /// multi-hit (gross — before the player's own block). Replaces the base-intent estimate.</summary>
    public static int IncomingDamage(CombatState s)
    {
        int total = 0;
        foreach (var m in s.Monsters)
        {
            if (!m.IsAlive) continue;
            if (m.Ai.States.TryGetValue(m.Ai.CurrentMoveId, out var st) && st is MoveState mv && mv.IntentDamage is int dmg)
                total += PredictHit(s, m, s.Player, dmg, ValueProp.Move) * mv.IntentHits;
        }
        return total;
    }

    public static int EnemyHpTotal(CombatState s) => s.Monsters.Where(m => m.IsAlive).Sum(m => m.CurrentHp + m.Block);

    // ---------- Policy score ----------

    /// <summary>Default interpolation point for the static (non-rollout) uses of the score.</summary>
    private static readonly double DefaultAggression = Env("STS2_AGGRO", 0.5);

    /// <summary>Position score at the balanced default aggression. Used where a single fixed score is needed
    /// (MCTS node ordering, the leaf race model).</summary>
    public static double Score(CombatState s) => Score(s, DefaultAggression);

    /// <summary>
    /// Position score (lower = better for the player) at an <paramref name="aggression"/> λ ∈ [0,1] that
    /// interpolates between the two ends of the block↔damage spectrum (the player's intuition): at λ=1 the
    /// score is pure race (spend everything on damage, ignore block), at λ=0 it is pure survival (block — or
    /// kill the attacker, since <see cref="IncomingDamage"/> only counts living monsters, so a lethal blow
    /// removes incoming = "killing is blocking"). The optimal turn lives at some λ in between; rollouts
    /// sample λ so leaf seeds average over the whole spectrum instead of one biased extreme. Strength and
    /// enemy Vulnerable speed the race; a small over-block term discourages wasting energy on unneeded block.
    /// </summary>
    public static double Score(CombatState s, double aggression)
    {
        var p = s.Player;
        int incoming = IncomingDamage(s);
        int unblocked = Math.Max(0, incoming - p.Block);
        int overblock = Math.Max(0, p.Block - incoming);
        int hpAfter = p.CurrentHp - unblocked;

        // Survival end of the spectrum: every HP lost to the telegraphed hit is bad; dying is a cliff.
        double survival = unblocked * WLoss;
        if (hpAfter <= 0) survival += LethalPenalty + (-hpAfter) * WLoss;        // monotone: more overkill = worse
        else survival += WDanger * Math.Max(0, SafetyBuffer(s, incoming) - hpAfter);

        // Race end of the spectrum: enemy HP down, sped by our offensive setup.
        int nLiving = s.Monsters.Count(m => m.IsAlive);
        double enemyHp = s.Monsters.Where(m => m.IsAlive).Sum(m => m.CurrentHp);
        double playerStr = p.GetPowerAmount("Strength");
        double enemyVuln = s.Monsters.Where(m => m.IsAlive).Sum(m => m.GetPowerAmount("Vulnerable"));
        double race = enemyHp * WEnemyHp - playerStr * WStrength * nLiving - enemyVuln * WVuln;

        double a = Math.Clamp(aggression, 0.0, 1.0);
        return a * race + (1.0 - a) * survival + overblock * WOverblock;
    }

    /// <summary>HP reserve we'd like to keep after this turn's hit, so the *next* telegraphed hit isn't
    /// lethal. A fraction of the current incoming hit (enemies tend to repeat/escalate), capped at max HP.
    /// Default fraction 0 ⇒ buffer disabled (death-cliff only); raise to make the policy keep reserve.</summary>
    private static int SafetyBuffer(CombatState s, int incoming) =>
        Math.Min((int)(BufferFrac * incoming), s.Player.MaxHp);

    // ---------- Static leaf value (optional, UseHeuristicLeaf) ----------

    /// <summary>A cheap static estimate of <c>(survival, expected HP loss)</c> from the current decision
    /// state: race turns-to-kill vs turns-to-die, seeded by one simulated heuristic player turn for the
    /// player's per-turn output and block. Used as the MCTS leaf value when full rollouts are too slow.</summary>
    public static Value Evaluate(CombatState s, int maxTurns)
    {
        if (s.AllMonstersDead) return new Value(1, 0);
        if (s.PlayerDead) return new Value(0, 0);

        int enemyHp = EnemyHpTotal(s);

        // Gauge the player's per-turn damage + standing block by simulating one heuristic turn on a clone.
        var (dmgPerTurn, blockPerTurn) = SimulateTurnOutput(s);

        int incoming = IncomingDamage(s);
        int netPerTurn = Math.Max(0, incoming - blockPerTurn);

        double turnsToKill = dmgPerTurn <= 0 ? maxTurns + 1 : Math.Ceiling((double)enemyHp / dmgPerTurn);
        double turnsToDie = netPerTurn <= 0 ? maxTurns + 1 : (double)s.Player.CurrentHp / netPerTurn;

        // Survival: smooth logistic on the race margin (turns-to-die minus turns-to-kill).
        double survival = turnsToKill > maxTurns ? 0.0 : 1.0 / (1.0 + Math.Exp(-SurvivalSlope * (turnsToDie - turnsToKill)));
        double loss = Math.Min(s.Player.CurrentHp, netPerTurn * Math.Min(turnsToKill, maxTurns));
        return new Value(survival, loss);
    }

    /// <summary>Gauge the player's per-turn output from a decision state by simulating one greedy
    /// (balanced-λ) heuristic turn on a clone: returns the damage dealt to enemies and the standing block
    /// left at turn end. Shared by the static leaf <see cref="Evaluate"/> and the learned value function's
    /// feature extractor so both read the same per-turn gauge.</summary>
    public static (int dmgPerTurn, int blockPerTurn) SimulateTurnOutput(CombatState s)
    {
        var sim = s.Clone();
        int enemyBefore = EnemyHpTotal(sim);
        var policy = new HeuristicPolicy();
        for (int g = 0; g < 60 && !sim.IsCombatOver; g++)
        {
            var action = policy.NextAction(sim);
            if (action is not { CardKey: not null } a) break;
            var card = sim.Player.Hand.FirstOrDefault(h => h.StateKey() == a.CardKey);
            if (card == null) break;
            Creature? target = a.TargetMonsterIndex >= 0 && a.TargetMonsterIndex < sim.Monsters.Count
                ? sim.Monsters[a.TargetMonsterIndex] : null;
            CombatManager.PlayCard(sim, card, target);
        }
        return (Math.Max(0, enemyBefore - EnemyHpTotal(sim)), sim.Player.Block);
    }

    // ---------- Shared play enumeration (kept in step with the solvers' LegalPlays/ApplyPlay) ----------

    /// <summary>Distinct (card, target) plays at a decision node, deduplicated by card key.</summary>
    public static IEnumerable<PlayerAction> LegalPlays(CombatState s)
    {
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

    /// <summary>Clone the state and play the chosen action (cost/target already validated by LegalPlays).</summary>
    public static CombatState ApplyPlay(CombatState s, PlayerAction action)
    {
        var c = s.Clone();
        var card = c.Player.Hand.First(h => h.StateKey() == action.CardKey);
        Creature? target = action.TargetMonsterIndex >= 0 ? c.Monsters[action.TargetMonsterIndex] : null;
        CombatManager.PlayCard(c, card, target, action.ChoiceKey);
        return c;
    }
}

/// <summary>Greedy policy on <see cref="CombatHeuristic.Score"/> at a fixed aggression λ: repeatedly play
/// the card that most improves the position; end the turn when no play strictly improves it. λ interpolates
/// the play between all-block (0) and all-damage (1); sampling λ per rollout spreads leaf seeds across the
/// block↔race spectrum. Shared by MCTS rollouts and the distribution sampler so both reflect one definition
/// of "reasonable play".</summary>
public sealed class HeuristicPolicy : IPlayoutPolicy
{
    private readonly double _aggression;
    public HeuristicPolicy(double aggression = 0.5) => _aggression = aggression;

    public PlayerAction? NextAction(CombatState s)
    {
        double current = CombatHeuristic.Score(s, _aggression);
        PlayerAction? best = null;
        double bestScore = current;
        foreach (var action in CombatHeuristic.LegalPlays(s))
        {
            double sc = CombatHeuristic.Score(CombatHeuristic.ApplyPlay(s, action), _aggression);
            if (sc < bestScore - 1e-9) { bestScore = sc; best = action; }
        }
        return best;   // null ⇒ end the turn (nothing improves the position)
    }
}
