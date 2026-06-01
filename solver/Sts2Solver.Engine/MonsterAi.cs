namespace Sts2Solver.Engine;

public enum MoveRepeatType { CanRepeatForever, CanRepeatXTimes, CannotRepeat, UseOnlyOnce }

/// <summary>Base node in a monster's move state machine (Appendix D).</summary>
public abstract class MonsterState
{
    public abstract string Id { get; }
    public virtual bool IsMove => false;
}

/// <summary>A node that performs a concrete monster action and telegraphs an intent.</summary>
public sealed class MoveState : MonsterState
{
    private readonly string _id;
    public override string Id => _id;
    public override bool IsMove => true;

    /// <summary>The effect, executed on the monster's turn.</summary>
    public Action<CombatState, Monster> Perform;

    /// <summary>Informational telegraph (base attack damage if this is an attack, else null).</summary>
    public int? IntentDamage;
    public int IntentHits = 1;

    /// <summary>Where the machine goes after this move (resolved to enumerate the next move).</summary>
    public MonsterState? FollowUp;

    public MoveState(string id, Action<CombatState, Monster> perform, int? intentDamage = null, int intentHits = 1)
    {
        _id = id;
        Perform = perform;
        IntentDamage = intentDamage;
        IntentHits = intentHits;
    }
}

/// <summary>A weighted branch point. Selection follows the recovered RandomBranchState algorithm.</summary>
public sealed class RandomBranchState : MonsterState
{
    private readonly string _id;
    public override string Id => _id;

    public sealed class Branch
    {
        public required string TargetId;
        public MoveRepeatType Repeat = MoveRepeatType.CanRepeatForever;
        public int MaxTimes = 1;        // only for CanRepeatXTimes
        public int Cooldown = 0;        // 0 = no cooldown
        public Func<Monster, float> Weight = _ => 1f;
    }

    public readonly List<Branch> Branches = new();

    public RandomBranchState(string id) { _id = id; }

    public RandomBranchState Add(string targetId, float weight = 1f,
        MoveRepeatType repeat = MoveRepeatType.CanRepeatForever, int maxTimes = 1, int cooldown = 0)
        => Add(targetId, _ => weight, repeat, maxTimes, cooldown);

    public RandomBranchState Add(string targetId, Func<Monster, float> weight,
        MoveRepeatType repeat = MoveRepeatType.CanRepeatForever, int maxTimes = 1, int cooldown = 0)
    {
        Branches.Add(new Branch { TargetId = targetId, Weight = weight, Repeat = repeat, MaxTimes = maxTimes, Cooldown = cooldown });
        return this;
    }

    /// <summary>Effective weight of a branch given the monster's move history (0 ⇒ unavailable).</summary>
    public float EffectiveWeight(Branch b, Monster m)
    {
        var log = m.Ai.MoveLog;
        float repeatMult = 1f;
        switch (b.Repeat)
        {
            case MoveRepeatType.UseOnlyOnce:
                if (log.Contains(b.TargetId)) repeatMult = 0f;
                break;
            case MoveRepeatType.CannotRepeat:
                if (log.Count >= 1 && log[^1] == b.TargetId) repeatMult = 0f;
                break;
            case MoveRepeatType.CanRepeatXTimes:
                int n = b.MaxTimes;
                if (log.Count >= n && log.Skip(log.Count - n).All(id => id == b.TargetId)) repeatMult = 0f;
                break;
            case MoveRepeatType.CanRepeatForever:
            default:
                break;
        }
        if (repeatMult == 0f) return 0f;

        if (b.Cooldown > 0)
        {
            // Used within the last `Cooldown` executed moves?
            int from = Math.Max(0, log.Count - b.Cooldown);
            for (int i = from; i < log.Count; i++)
                if (log[i] == b.TargetId) return 0f;
        }
        return Math.Max(0f, b.Weight(m)) * repeatMult;
    }
}

/// <summary>
/// A monster's move state machine. Tracks the committed (telegraphed) move and the executed-move log
/// used by repeat/cooldown constraints.
/// </summary>
public sealed class MonsterMoveStateMachine
{
    public readonly Dictionary<string, MonsterState> States = new();
    public readonly string InitialStateId;
    public string CurrentMoveId = "";   // the telegraphed move for the upcoming enemy turn
    public readonly List<string> MoveLog = new();

    public MonsterMoveStateMachine(IEnumerable<MonsterState> states, string initialStateId)
    {
        foreach (var s in states) States[s.Id] = s;
        InitialStateId = initialStateId;
    }

    // Shares the (immutable) States dictionary; used by CloneState to avoid rebuilding it each clone.
    private MonsterMoveStateMachine(Dictionary<string, MonsterState> states, string initialStateId)
    {
        States = states;
        InitialStateId = initialStateId;
    }

    public MoveState CurrentMove => (MoveState)States[CurrentMoveId];

    /// <summary>
    /// Enumerate the distribution of move ids reachable from <paramref name="startStateId"/>,
    /// resolving RandomBranchStates against the current MoveLog. Deterministic chains yield one outcome.
    /// </summary>
    public List<(double prob, string moveId)> Enumerate(string startStateId, Monster m)
    {
        var outcomes = new List<(double, string)>();
        Walk(startStateId, 1.0, m, outcomes);
        // Merge duplicate move ids.
        return outcomes
            .GroupBy(o => o.Item2)
            .Select(g => (g.Sum(x => x.Item1), g.Key))
            .ToList();
    }

    private void Walk(string stateId, double prob, Monster m, List<(double, string)> outcomes)
    {
        var state = States[stateId];
        if (state is MoveState ms) { outcomes.Add((prob, ms.Id)); return; }
        if (state is RandomBranchState rb)
        {
            double total = rb.Branches.Sum(b => rb.EffectiveWeight(b, m));
            if (total <= 0)
                throw new InvalidOperationException($"RandomBranchState '{rb.Id}' has no available branches.");
            foreach (var b in rb.Branches)
            {
                double w = rb.EffectiveWeight(b, m);
                if (w <= 0) continue;
                Walk(b.TargetId, prob * (w / total), m, outcomes);
            }
            return;
        }
        throw new InvalidOperationException($"Unknown state type for '{stateId}'.");
    }

    public List<(double prob, string moveId)> EnumerateInitial(Monster m) => Enumerate(InitialStateId, m);

    public List<(double prob, string moveId)> EnumerateNext(Monster m)
    {
        var follow = CurrentMove.FollowUp
            ?? throw new InvalidOperationException($"Move '{CurrentMoveId}' has no FollowUp.");
        return Enumerate(follow.Id, m);
    }

    public MonsterMoveStateMachine CloneState()
    {
        // States are immutable definitions (shared by reference); only CurrentMoveId + MoveLog are per-combat.
        var copy = new MonsterMoveStateMachine(States, InitialStateId) { CurrentMoveId = CurrentMoveId };
        copy.MoveLog.AddRange(MoveLog);
        return copy;
    }

    // Full log is needed for exact constraint evaluation (UseOnlyOnce inspects the whole history).
    // Combats are short, so this stays small.
    public string StateKey() => $"{CurrentMoveId}:{string.Join(">", MoveLog)}";
}
