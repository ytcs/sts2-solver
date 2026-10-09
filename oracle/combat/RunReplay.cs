using System.Text;
using System.Text.Json.Nodes;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Random;
using MegaCrit.Sts2.Core.Rewards;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Unlocks;

namespace OracleCombat;

// Whole-run replay from a seed via the game's own run-level code (NGame.StartNewSingleplayerRun path, TestMode UI-less).
public sealed class RunReplay
{
    private readonly Pump _pump;
    private readonly TextWriter _out;
    private readonly JsonObject _head;
    private readonly List<JsonObject> _steps;
    private Player _player;
    private RunState _run;
    private readonly ChoiceSelector _sel = new();
    private readonly Dictionary<string, string> _alias = new();

    public RunReplay(string compactPath, TextWriter @out, Pump pump)
    {
        _pump = pump; _out = @out;
        var lines = File.ReadLines(compactPath).Where(l => l.Trim().Length > 0).Select(l => JsonNode.Parse(l).AsObject()).ToList();
        _head = lines[0]; _steps = lines.Skip(1).ToList();
    }

    private void Emit(string ev, JsonObject o)
    {
        o["event"] = ev;
        _out.WriteLine(o.ToJsonString());
        _out.Flush();
    }

    private void Wait(Task t, string what)
    {
        _pump.RunUntil(() => t.IsCompleted || (Fatal.IsSet && !Fatal.Lenient), () => what + ": " + Describe());
        if (Fatal.IsSet && !Fatal.Lenient) throw new OracleException(what + ": " + Fatal.Message);
        if (t.IsFaulted) throw t.Exception.GetBaseException();
        _pump.Drain();
    }

    private string Describe() =>
        $"room={_run?.CurrentRoom?.GetType().Name} execRunning={RunManager.Instance.ActionExecutor?.IsRunning} execPaused={RunManager.Instance.ActionExecutor?.IsPaused} cur={RunManager.Instance.ActionExecutor?.CurrentlyRunningAction}";

    public void Run()
    {
        string seed = (string)_head["seed"];
        string charName = ((string)_head["character"]).ToUpperInvariant();
        int asc = (int)_head["ascension"];
        Patches.ApplyAscensionEffects = true;
        var unlock = UnlockState.all;
        var rng = new Rng(StringHelper.GetDeterministicHashCode(seed), "act_selection");
        var acts = ActModel.GetRandomList(rng, unlock, isMultiplayer: false).ToList();
        var character = ModelDb.GetById<CharacterModel>(new ModelId("CHARACTER", charName));
        _player = Player.CreateForNewRun(character, unlock, 1UL);
        _run = RunState.CreateForNewRun(new List<Player> { _player }, acts.Select(a => a.ToMutable()).ToList(), Array.Empty<ModifierModel>(), GameMode.Standard, asc, seed);
        RunManager.Instance.SetUpNewSingleplayer(_run, shouldSave: false);
        Wait(Start(), "start run");
        Emit("start", new JsonObject
        {
            ["seed"] = seed, ["character"] = charName, ["ascension"] = asc,
            ["acts"] = new JsonArray(_run.Acts.Select(a => (JsonNode)a.Id.Entry).ToArray()),
            ["started_with_neow"] = _run.ExtraFields.StartedWithNeow,
            ["boss"] = _run.Act.BossEncounter?.Id.Entry,
            ["map"] = FullMap(),
            ["room"] = RoomInfo(),
            ["state"] = Dump.State(_player, _run, null),
        });
        foreach (var kv in _head["aliases"]?.AsObject() ?? new JsonObject()) _alias[kv.Key] = (string)kv.Value;
        using var selScope = CardSelectCmd.PushSelector(_sel);
        int i = 0;
        while (i < _steps.Count)
        {
            var st = _steps[i];
            string screen = (string)st["screen"];
            switch (screen)
            {
                case "EVENT": DoEvent(st); i++; break;
                case "MAP": DoMap(st); i++; break;
                case "COMBAT": DoCombat(st); i++; break;
                case "REWARDS": case "CARD_REWARD": i = DoRewards(i); break;
                default:
                    Emit("blocked", new JsonObject { ["step"] = i, ["screen"] = screen, ["record"] = st.DeepClone(), ["room"] = RoomInfo() });
                    return;
            }
        }
        Emit("end", new JsonObject { ["steps_done"] = i, ["room"] = RoomInfo(), ["state"] = Dump.State(_player, _run, null) });
    }

    public static string NameToId(string name)
    {
        var sb = new StringBuilder();
        foreach (char ch in name.Trim().TrimEnd('+'))
            if (char.IsLetterOrDigit(ch)) sb.Append(char.ToUpperInvariant(ch));
            else if (ch == ' ' || ch == '-' || ch == '_') sb.Append('_');
        return sb.ToString();
    }

    private string CardId(string aliasOrId) => _alias.TryGetValue(aliasOrId, out var id) ? id : NameToId(aliasOrId);

    private JsonObject State() => Dump.State(_player, _run, CombatManager.Instance.DebugOnlyGetState());

    private void DoEvent(JsonObject st)
    {
        var ev = RunManager.Instance.EventSynchronizer.GetLocalEvent();
        var before = RoomInfo();
        string want = NameToId((string)st["pick"]);
        var opts = ev.CurrentOptions;
        int idx = -1;
        for (int k = 0; k < opts.Count; k++)
            if (opts[k].TextKey.Split('.').Last().ToUpperInvariant() == want) { idx = k; break; }
        if (idx < 0) throw new OracleException($"event pick {want} not among options: " + string.Join(", ", opts.Select(o => o.TextKey)));
        RunManager.Instance.EventSynchronizer.ChooseLocalOption(idx);
        _pump.Drain();
        Wait(RunManager.Instance.EventSynchronizer.AwaitPendingOptionTasks(), "event option");
        Emit("event", new JsonObject { ["floor"] = (int)st["floor"], ["pick"] = want, ["before"] = before, ["after"] = RoomInfo(), ["choices"] = _sel.Prompts.DeepClone(), ["state"] = State() });
        _sel.Prompts.Clear();
    }

    private void DoMap(JsonObject st)
    {
        var pick = st["pick"];
        if (pick is not JsonValue) throw new OracleException("map pick without coordinate: " + pick.ToJsonString());
        var m = System.Text.RegularExpressions.Regex.Match((string)pick, @"^r(\d+)c(\d+)$");
        if (!m.Success) throw new OracleException("bad map pick " + pick);
        var coord = new MapCoord(int.Parse(m.Groups[2].Value), int.Parse(m.Groups[1].Value));
        Wait(RunManager.Instance.EnterMapCoord(coord), "enter map coord " + pick);
        Emit("map", new JsonObject { ["floor"] = (int)st["floor"], ["pick"] = (string)pick, ["total_floor"] = _run.TotalFloor, ["room"] = RoomInfo(), ["state"] = State() });
    }

    private void DoCombat(JsonObject st)
    {
        var fight = st["fight"].AsObject();
        var enc = (_run.CurrentRoom as CombatRoom)?.Encounter?.Id.Entry;
        if (enc != (string)fight["encounter"]) throw new OracleException($"combat: room encounter {enc} != record {(string)fight["encounter"]}");
        var d = new Driver(_out, _pump, _player, _run) { Sel = _sel, Tag = "combat" };
        d.Settle();
        d.Record(null);
        foreach (var turn in fight["turns"].AsArray())
        {
            var acts = turn["acts"].AsArray();
            for (int k = 0; k < acts.Count; k++)
            {
                var a = acts[k].AsArray();
                string kind = (string)a[0];
                _sel.ScriptedIds.Clear();
                while (k + 1 < acts.Count && (string)acts[k + 1][0] == "c")
                {
                    k++;
                    _sel.ScriptedIds.Enqueue(acts[k].AsArray().Skip(1).Select(x => CardId((string)x)).ToArray());
                }
                ActionSpec spec;
                switch (kind)
                {
                    case "p":
                    {
                        string id = CardId((string)a[1]);
                        var hand = _player.PlayerCombatState.Hand.Cards;
                        int pos = Enumerable.Range(0, hand.Count).FirstOrDefault(j => hand[j].Id.Entry == id && hand[j].CanPlay(), -1);
                        if (pos < 0) throw new OracleException($"combat {fight["id"]}: {id} not playable in hand: " + string.Join(", ", hand.Select(c => c.Id.Entry)));
                        spec = new ActionSpec { Kind = "play", HandPos = pos, Target = a.Count > 2 && a[2] != null ? (int)a[2] : null };
                        break;
                    }
                    case "pot":
                        spec = new ActionSpec { Kind = "use_potion", Slot = (int)a[1], Target = a.Count > 2 && a[2] != null ? (int)a[2] : null };
                        break;
                    case "e":
                        spec = new ActionSpec { Kind = "end_turn" };
                        break;
                    default: throw new OracleException("unknown combat act " + kind);
                }
                d.Exec(spec);
                if (_sel.ScriptedIds.Count > 0) throw new OracleException($"combat {fight["id"]}: unused choices");
            }
        }
        if (CombatManager.Instance.IsInProgress) throw new OracleException($"combat {fight["id"]}: still in progress after the recorded actions");
    }

    private int DoRewards(int i)
    {
        var picks = new List<string>();
        var cardPicks = new Queue<string>();
        int j = i;
        for (; j < _steps.Count; j++)
        {
            var st = _steps[j];
            string screen = (string)st["screen"];
            if (screen == "CARD_REWARD") { var p = st["pick"]; cardPicks.Enqueue(p == null ? null : CardId((string)p)); continue; }
            if (screen != "REWARDS") break;
            var pk = st["pick"];
            if (pk is JsonArray arr) picks.AddRange(arr.Select(x => (string)x));
            else if (pk is JsonValue) picks.Add((string)pk);
            else throw new OracleException("unsupported rewards pick " + pk.ToJsonString());
        }
        var room = _run.CurrentRoom as CombatRoom ?? throw new OracleException("rewards outside a combat room: " + _run.CurrentRoom?.GetType().Name);
        var done = new TaskCompletionSource();
        JsonObject offered = null;
        RewardsSet.testSelector = async set =>
        {
            set.ThrowInTestIfRewardsNotTaken = false;
            offered = new JsonObject { ["rewards"] = new JsonArray(set.Rewards.Select(r => (JsonNode)RewardJson(r)).ToArray()) };
            var taken = new HashSet<Reward>();
            foreach (var pk in picks)
            {
                if (pk == "proceed") continue;
                var r = set.Rewards.FirstOrDefault(x => !taken.Contains(x) && Matches(x, pk));
                if (r == null) { done.TrySetException(new OracleException($"reward pick {pk} not offered: " + offered.ToJsonString())); return; }
                taken.Add(r);
                if (r is CardReward) _sel.ScriptedCardReward.Enqueue(cardPicks.Count > 0 ? cardPicks.Dequeue() : null);
                await RunManager.Instance.RewardsSetSynchronizer.SelectLocalReward(r);
            }
            done.TrySetResult();
        };
        try
        {
            Wait(room.OfferRoomEndRewards(), "offer rewards");
            Wait(done.Task, "select rewards");
        }
        finally { RewardsSet.testSelector = null; }
        Emit("rewards", new JsonObject { ["floor"] = (int)_steps[i]["floor"], ["offered"] = offered, ["picks"] = new JsonArray(picks.Select(x => (JsonNode)x).ToArray()), ["choices"] = _sel.Prompts.DeepClone(), ["state"] = State() });
        _sel.Prompts.Clear();
        return j;
    }

    private static bool Matches(Reward r, string pick)
    {
        string p = pick.ToLowerInvariant();
        return r switch
        {
            GoldReward g => p == "gold" || p == g.Amount + " gold",
            PotionReward pr => p == "potion" || NameToId(pick) == pr.Potion?.Id.Entry,
            CardReward => p == "card",
            RelicReward rr => p == "relic" || NameToId(pick) == rr.Relic?.Id.Entry,
            _ => false,
        };
    }

    private static JsonObject RewardJson(Reward r) => r switch
    {
        GoldReward g => new JsonObject { ["type"] = "gold", ["amount"] = g.Amount },
        PotionReward p => new JsonObject { ["type"] = "potion", ["id"] = p.Potion?.Id.Entry },
        CardReward c => new JsonObject { ["type"] = "card", ["cards"] = new JsonArray(c.Cards.Select(x => (JsonNode)Dump.CardBrief(x)).ToArray()) },
        RelicReward rr => new JsonObject { ["type"] = "relic", ["id"] = rr.Relic?.Id.Entry },
        _ => new JsonObject { ["type"] = r.GetType().Name },
    };

    private async Task Start()
    {
        await RunManager.Instance.FinalizeStartingRelics();
        RunManager.Instance.Launch();
        await RunManager.Instance.EnterAct(0, doTransition: false);
    }

    private JsonObject RoomInfo()
    {
        var room = _run.CurrentRoom;
        var o = new JsonObject { ["type"] = room?.GetType().Name, ["room_type"] = room?.RoomType.ToString() };
        if (room is EventRoom)
        {
            var ev = RunManager.Instance.EventSynchronizer.GetLocalEvent();
            o["event"] = ev.Id.Entry;
            o["finished"] = ev.IsFinished;
            o["options"] = new JsonArray(ev.CurrentOptions.Select(op => (JsonNode)new JsonObject
            {
                ["key"] = op.TextKey, ["title"] = SafeText(() => op.Title?.GetFormattedText()), ["locked"] = op.IsLocked, ["proceed"] = op.IsProceed,
            }).ToArray());
        }
        if (room is CombatRoom cr) o["encounter"] = cr.Encounter?.Id.Entry;
        return o;
    }

    private static string SafeText(Func<string> f) { try { return f(); } catch (Exception e) { return "<" + e.GetType().Name + ">"; } }

    public static string Code(MapPointType t) => t switch
    {
        MapPointType.Monster => "M", MapPointType.Elite => "E", MapPointType.RestSite => "R", MapPointType.Shop => "$",
        MapPointType.Treasure => "T", MapPointType.Unknown => "?", MapPointType.Boss => "B", MapPointType.Ancient => "A", _ => "."
    };

    // Same text as mods/AgentBridge Decisions.FullMap (the live replay log's `map` event).
    public string FullMap()
    {
        var rs = _run;
        if (rs?.Map == null) return "no map\n";
        var sb = new StringBuilder("rows bottom->top; point = <type>c<col>><child cols>; * = visited\n");
        var visited = rs.VisitedMapCoords.ToHashSet();
        int rows = rs.Map.GetRowCount();
        for (int r = 0; r < rows; r++)
        {
            var row = rs.Map.GetPointsInRow(r).Where(p => p != null).OrderBy(p => p.coord.col).ToList();
            if (row.Count == 0) continue;
            sb.Append('r').Append(r).Append(':');
            foreach (var p in row)
                sb.Append(' ').Append(visited.Contains(p.coord) ? "*" : "").Append(Code(p.PointType)).Append('c').Append(p.coord.col)
                  .Append('>').Append(string.Join(",", p.Children.OrderBy(c => c.coord.col).Select(c => c.coord.col)));
            sb.Append('\n');
        }
        sb.Append($"boss: {rs.Map.BossMapPoint?.coord.row} {rs.Act.BossEncounter?.Id.Entry}");
        if (rs.Act.HasSecondBoss) sb.Append($" + {rs.Act.SecondBossEncounter?.Id.Entry}");
        sb.Append('\n');
        return sb.ToString();
    }
}
