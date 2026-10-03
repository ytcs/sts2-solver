using System.Text.Json;
using System.Text.Json.Nodes;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.CardRewardAlternatives;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.Potions;
using MegaCrit.Sts2.Core.GameActions;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.TestSupport;

namespace OracleCombat;

/// <summary>ICardSelector fed by the script's `choose` entries (or the random policy); records every prompt.</summary>
public sealed class ChoiceSelector : ICardSelector
{
    public Queue<int[]> Scripted = new();
    public Random RandomPolicy;           // non-null in random mode
    public JsonArray Prompts = new();
    public List<ActionSpec> RecordedChoices = new();

    /// <summary>Set by a Harmony prefix around `FromChooseACardScreen(canSkip: false)`: the prompt must pick exactly one card.</summary>
    public static bool ChooseACardMustPick;

    public Task<IEnumerable<CardModel>> GetSelectedCards(IEnumerable<CardModel> options, int minSelect, int maxSelect)
    {
        if (ChooseACardMustPick && minSelect == 0 && maxSelect == 1) minSelect = 1;
        var opts = options.ToList();
        int n = opts.Count;
        int hi = Math.Min(maxSelect, n);
        int lo = Math.Min(minSelect, hi);
        int[] picked;
        if (Scripted.Count > 0)
        {
            picked = Scripted.Dequeue();
            if (picked.Length < lo || picked.Length > hi || picked.Distinct().Count() != picked.Length || picked.Any(i => i < 0 || i >= n))
                throw new OracleException($"invalid scripted choice [{string.Join(",", picked)}] for prompt with {n} options, min {minSelect}, max {maxSelect}");
        }
        else if (RandomPolicy != null)
        {
            int cnt = lo == hi ? lo : RandomPolicy.Next(lo, hi + 1);
            picked = Enumerable.Range(0, n).OrderBy(_ => RandomPolicy.Next()).Take(cnt).OrderBy(i => i).ToArray();
        }
        else
        {
            throw new OracleException($"script has no `choose` entry for a prompt with {n} options (min {minSelect}, max {maxSelect}): " +
                string.Join(", ", opts.Select(c => c.Id.Entry + (c.CurrentUpgradeLevel > 0 ? "+" : ""))));
        }
        var po = new JsonObject
        {
            ["min"] = minSelect, ["max"] = maxSelect,
            ["options"] = new JsonArray(opts.Select(c => (JsonNode)new JsonObject { ["id"] = c.Id.Entry, ["upgrade"] = c.CurrentUpgradeLevel, ["pile"] = c.Pile?.Type.ToString() }).ToArray()),
            ["picked"] = new JsonArray(picked.Select(i => (JsonNode)i).ToArray()),
        };
        Prompts.Add(po);
        RecordedChoices.Add(new ActionSpec { Kind = "choose", Choose = picked });
        return Task.FromResult<IEnumerable<CardModel>>(picked.Select(i => opts[i]).ToList());
    }

    public CardRewardSelection GetSelectedCardReward(IReadOnlyList<CardCreationResult> options, IReadOnlyList<CardRewardAlternative> alternatives)
        => throw new OracleException("card reward selection requested during combat (unsupported)");
}

public sealed class Driver
{
    private readonly Scenario _sc;
    private readonly TextWriter _out;
    private readonly Pump _pump;
    private Player _player;
    private RunState _run;
    private readonly ChoiceSelector _sel = new();
    private int _step;
    private int _logIdx;
    public readonly List<ActionSpec> Recorded = new();
    public int MaxSteps = 400;
    public int MaxRounds = 60;
    public Random RandomDriver;          // random mode
    // Policy weights for the random driver (relative to 1.0 per legal action): `end_turn`, attack-card plays, potion uses.
    // endw = 0 means "never end the turn while anything else is legal" (long fights); atkw < 1 stalls (prefers skills/powers).
    public double EndWeight = 1, AttackWeight = 1, PotionWeight = 1;
    public string Result = "unfinished";

    public Driver(Scenario sc, TextWriter @out, Pump pump) { _sc = sc; _out = @out; _pump = pump; }

    private CombatState St => CombatManager.Instance.DebugOnlyGetState();
    private PlayerCombatState Pcs => _player.PlayerCombatState;

    private string Stuck() =>
        $"inProgress={CombatManager.Instance.IsInProgress} side={St?.CurrentSide} phase={Pcs?.Phase} turn={Pcs?.TurnNumber} " +
        $"execRunning={RunManager.Instance.ActionExecutor.IsRunning} execPaused={RunManager.Instance.ActionExecutor.IsPaused} " +
        $"curAction={RunManager.Instance.ActionExecutor.CurrentlyRunningAction}";

    private bool WaitingForInput()
    {
        var st = St;
        if (st == null || Pcs == null) return false;
        return CombatManager.Instance.IsInProgress && st.CurrentSide == CombatSide.Player && Pcs.Phase == PlayerTurnPhase.Play
               && !RunManager.Instance.ActionExecutor.IsRunning && !CombatManager.Instance.PlayerActionsDisabled;
    }

    private void Check()
    {
        if (Fatal.IsSet && !Fatal.Lenient) throw new OracleException(Fatal.Message);
    }

    /// <summary>Pump until the game is waiting for player input again (or combat ended).</summary>
    private void Settle(int? turnMustExceed = null)
    {
        _pump.RunUntil(() =>
        {
            if (Fatal.IsSet && !Fatal.Lenient) return true;
            if (!CombatManager.Instance.IsInProgress && _pump.Pending == 0 && !RunManager.Instance.ActionExecutor.IsRunning) return true;
            if (!WaitingForInput()) return false;
            if (turnMustExceed.HasValue && Pcs.TurnNumber <= turnMustExceed.Value) return false;
            return true;
        }, Stuck);
        _pump.Drain();
        Check();
    }

    private JsonObject Record(ActionSpec action)
    {
        var rec = new JsonObject();
        rec["step"] = _step++;
        rec["action"] = action == null ? null : Scenario.ActionToJson(action);
        rec["choices"] = _sel.Prompts.DeepClone();
        _sel.Prompts.Clear();
        var s = Dump.State(_player, _run, St);
        foreach (var kv in s) rec[kv.Key] = kv.Value?.DeepClone();
        rec["log"] = Dump.Log(ref _logIdx);
        bool over = !CombatManager.Instance.IsInProgress;
        rec["combat_over"] = over;
        if (over)
        {
            var st = St;
            bool anyEnemy = st != null && st.Enemies.Any(e => e.IsAlive);
            Result = _player.Creature.IsAlive && !anyEnemy ? "win" : (_player.Creature.IsDead ? "loss" : "over");
            rec["result"] = Result;
        }
        _out.WriteLine(rec.ToJsonString());
        return rec;
    }

    private IDisposable _selScope;

    /// <summary>Tear down the run so another scenario can be executed in the same process.</summary>
    public void Dispose()
    {
        _selScope?.Dispose(); _selScope = null;
        _pump.Drain();
        RunManager.Instance.CleanUp(graceful: true);
        _pump.Drain();
        MegaCrit.Sts2.Core.Context.LocalContext.NetId = 1UL;
    }

    public void Run()
    {
        var (player, run) = Setup.BuildRun(_sc);
        _player = player; _run = run;
        _selScope = CardSelectCmd.PushSelector(_sel);
        // leading `choose` entries answer prompts raised during combat setup (e.g. Toolbox at turn 1)
        int start = 0;
        while (start < _sc.Script.Count && _sc.Script[start].Kind == "choose") _sel.Scripted.Enqueue(_sc.Script[start++].Choose);
        _sel.RandomPolicy = RandomDriver;
        var task = Setup.EnterCombat(_sc, player, run);
        _pump.RunUntil(() => task.IsCompleted, Stuck);
        if (task.IsFaulted) throw task.Exception.GetBaseException();
        Settle();
        if (_sel.Scripted.Count > 0) throw new OracleException($"{_sel.Scripted.Count} unused leading `choose` entries");
        foreach (var c in _sel.RecordedChoices) Recorded.Add(c);
        _sel.RecordedChoices.Clear();
        _sel.RandomPolicy = null;
        Record(null);
        ScriptedLoop(start);
        if (RandomDriver != null) RandomLoop();
    }

    private void ScriptedLoop(int start)
    {
        var script = _sc.Script;
        for (int i = start; i < script.Count; i++)
        {
            var a = script[i];
            if (a.Kind == "choose") throw new OracleException($"script[{i}]: `choose` must follow an action");
            _sel.Scripted.Clear();
            int j = i + 1;
            while (j < script.Count && script[j].Kind == "choose") { _sel.Scripted.Enqueue(script[j].Choose); j++; }
            if (!CombatManager.Instance.IsInProgress) throw new OracleException($"script[{i}]: combat already over");
            Exec(a);
            if (_sel.Scripted.Count > 0) throw new OracleException($"script[{i}]: {_sel.Scripted.Count} unused `choose` entries");
            i = j - 1;
        }
        _sel.Scripted.Clear();
    }

    private void RandomLoop()
    {
        int steps = 0;
        while (CombatManager.Instance.IsInProgress && steps < MaxSteps && St.RoundNumber <= MaxRounds)
        {
            _sel.RandomPolicy = RandomDriver;
            var legal = Legal();
            ActionSpec a;
            if (EndWeight == 1 && AttackWeight == 1 && PotionWeight == 1) a = legal[RandomDriver.Next(legal.Count)];
            else
            {
                var hand = Pcs.Hand.Cards;
                double W(ActionSpec x) => x.Kind == "end_turn" ? EndWeight : x.Kind == "use_potion" ? PotionWeight
                    : hand[x.HandPos].Type == MegaCrit.Sts2.Core.Entities.Cards.CardType.Attack ? AttackWeight : 1.0;
                double tot = legal.Sum(W);
                a = legal[legal.Count - 1]; // end_turn is always last (the only action left when every other weight is 0)
                if (tot > 0)
                {
                    double r = RandomDriver.NextDouble() * tot;
                    foreach (var x in legal) { r -= W(x); if (r < 0) { a = x; break; } }
                }
            }
            Exec(a);
            steps++;
        }
        if (CombatManager.Instance.IsInProgress) Result = "truncated";
        _sel.RandomPolicy = null;
    }

    public List<ActionSpec> Legal()
    {
        var res = new List<ActionSpec>();
        var st = St;
        var hand = Pcs.Hand.Cards;
        for (int i = 0; i < hand.Count; i++)
        {
            var c = hand[i];
            if (!c.CanPlay()) continue;
            if (c.TargetType == TargetType.AnyEnemy)
            {
                for (int j = 0; j < st.Enemies.Count; j++)
                    if (c.CanPlayTargeting(st.Enemies[j])) res.Add(new ActionSpec { Kind = "play", HandPos = i, Target = j });
            }
            else if (c.TargetType == TargetType.AnyAlly)
            {
                for (int j = 0; j < st.Allies.Count; j++)
                    if (c.CanPlayTargeting(st.Allies[j])) res.Add(new ActionSpec { Kind = "play", HandPos = i, TargetAlly = j });
            }
            else if (c.CanPlayTargeting(null)) res.Add(new ActionSpec { Kind = "play", HandPos = i });
        }
        for (int s = 0; s < _player.PotionSlots.Count; s++)
        {
            var p = _player.PotionSlots[s];
            if (p == null || p.IsQueued) continue;
            if (p.Usage != PotionUsage.CombatOnly && p.Usage != PotionUsage.AnyTime) continue;
            if (!p.PassesCustomUsabilityCheck || !_player.CanUseOrRemovePotions) continue;
            if (p.IsValidTarget(null) || p.IsValidTarget(_player.Creature)) res.Add(new ActionSpec { Kind = "use_potion", Slot = s });
            for (int j = 0; j < st.Enemies.Count; j++)
                if (st.Enemies[j].IsAlive && p.TargetType == TargetType.AnyEnemy && p.IsValidTarget(st.Enemies[j])) res.Add(new ActionSpec { Kind = "use_potion", Slot = s, Target = j });
            for (int j = 1; j < st.Allies.Count; j++)
                if (p.TargetType == TargetType.AnyAlly && p.IsValidTarget(st.Allies[j])) res.Add(new ActionSpec { Kind = "use_potion", Slot = s, TargetAlly = j });
        }
        res.Add(new ActionSpec { Kind = "end_turn" });
        return res;
    }

    private Creature ResolveTarget(ActionSpec a)
    {
        if (a.Target.HasValue)
        {
            if (a.Target.Value < 0 || a.Target.Value >= St.Enemies.Count) throw new OracleException($"enemy target {a.Target} out of range ({St.Enemies.Count})");
            return St.Enemies[a.Target.Value];
        }
        if (a.TargetAlly.HasValue)
        {
            if (a.TargetAlly.Value < 0 || a.TargetAlly.Value >= St.Allies.Count) throw new OracleException($"ally target {a.TargetAlly} out of range ({St.Allies.Count})");
            return St.Allies[a.TargetAlly.Value];
        }
        return null;
    }

    private void Exec(ActionSpec a)
    {
        int turnBefore = Pcs.TurnNumber;
        _sel.RecordedChoices.Clear();
        switch (a.Kind)
        {
            case "play":
            {
                var hand = Pcs.Hand.Cards;
                if (a.HandPos < 0 || a.HandPos >= hand.Count) throw new OracleException($"play: hand_pos {a.HandPos} out of range (hand has {hand.Count})");
                var card = hand[a.HandPos];
                var target = ResolveTarget(a);
                if (!card.TryManualPlay(target))
                {
                    card.CanPlay(out var why, out _);
                    throw new OracleException($"illegal play: {card.Id.Entry} hand_pos {a.HandPos} target {target?.Monster?.Id.Entry ?? "none"} reason={why}");
                }
                Settle();
                break;
            }
            case "end_turn":
                RunManager.Instance.ActionQueueSynchronizer.RequestEnqueue(new EndPlayerTurnAction(_player, turnBefore));
                Settle(turnMustExceed: turnBefore);
                break;
            case "use_potion":
            {
                var p = a.Slot >= 0 && a.Slot < _player.PotionSlots.Count ? _player.PotionSlots[a.Slot] : null;
                if (p == null) throw new OracleException($"use_potion: slot {a.Slot} empty");
                p.EnqueueManualUse(ResolveTarget(a));
                Settle();
                break;
            }
            default: throw new OracleException("cannot execute action kind " + a.Kind);
        }
        Recorded.Add(a);
        foreach (var c in _sel.RecordedChoices) Recorded.Add(c);
        Record(a);
    }
}
