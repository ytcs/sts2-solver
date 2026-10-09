using System.Text;
using System.Text.Json.Nodes;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Random;
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
    }

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
