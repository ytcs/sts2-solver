using System.Text.Json;
using HarmonyLib;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Saves;

namespace OracleCombat;

// Run snapshots for forks: the game's own save (`RunManager.ToSave`) at every save point (room entry, combat won, event done) into
// <state-dir>/history, `snapshot` on demand, `load` = the main menu's Continue path without nodes. `mapgen` builds a map library from
// library seeds only: a run's unrevealed acts are hidden state and are never generated from that run's seed.
public static class Fork
{
    public static string HistoryDir;
    static int _n;
    static string _pendingScreen;

    static string Write(SerializableRun save, string tag)
    {
        var rs = RunManager.Instance.DebugOnlyGetState();
        string path = Path.Combine(HistoryDir, $"{_n++:D4}_A{rs.CurrentActIndex + 1}F{rs.TotalFloor:D2}_{tag}.json");
        File.WriteAllBytes(path, JsonSerializer.SerializeToUtf8Bytes(save, JsonSerializationUtility.GetTypeInfo<SerializableRun>()));
        _pendingScreen = path;
        return path;
    }

    [HarmonyPatch(typeof(SaveManager), nameof(SaveManager.SaveRun))]
    static class P_SaveRun
    {
        static void Prefix(AbstractRoom preFinishedRoom)
        {
            if (HistoryDir == null || RunManager.Instance.DebugOnlyGetState() == null) return;
            try { Write(RunManager.Instance.ToSave(preFinishedRoom), preFinishedRoom == null ? "enter" : "done"); }
            catch (Exception e) { Console.Error.WriteLine("serve: snapshot failed: " + e.GetBaseException().Message); }
        }
    }

    // the first settled screen after a snapshot, kept beside it for the reload check
    public static void Screen(string text)
    {
        if (_pendingScreen == null) return;
        File.WriteAllText(_pendingScreen + ".screen.txt", text);
        _pendingScreen = null;
    }

    public static string Snapshot(string tag)
    {
        var rs = RunManager.Instance.DebugOnlyGetState();
        if (rs == null) return "no run\n";
        if (HistoryDir == null) return "ERR start the server with --state-dir and --history\n";
        var room = rs.CurrentRoom is { IsPreFinished: true } r ? r : null;
        string path = Write(RunManager.Instance.ToSave(room), tag ?? "manual");
        _pendingScreen = null;
        return path + "\n";
    }

    public static async Task Load(string path)
    {
        var save = JsonSerializer.Deserialize(File.ReadAllBytes(path), JsonSerializationUtility.GetTypeInfo<SerializableRun>())
                   ?? throw new OracleException("unreadable snapshot " + path);
        Ui.EndRunForFork();
        var rs = RunState.FromSerializable(save);
        await RunManager.Instance.SetUpSavedSingleplayer(rs, save);
        Ui.WatchRelics();
        RunManager.Instance.Launch();
        await RunManager.Instance.GenerateMap();
        await RunManager.Instance.LoadIntoLatestMapCoord(AbstractRoom.FromSerializable(save.PreFinishedRoom, rs));
    }

    // one JSON line per (library seed, act): act id, map text (Ui.FullMap: rows, types, edges, bosses)
    public static async Task MapGen(string prefix, int n, string outPath)
    {
        using var w = new StreamWriter(outPath);
        for (int i = 0; i < n; i++)
        {
            string seed = $"{prefix}{i:D6}";
            await Ui.StartRun("ironclad", Serve.DefaultAscension, seed, null);
            var rs = RunManager.Instance.DebugOnlyGetState();
            for (int a = 0; a < rs.Acts.Count; a++)
            {
                if (a > 0) await RunManager.Instance.SetActInternal(a);
                w.WriteLine(JsonSerializer.Serialize(new Dictionary<string, object> { ["seed"] = seed, ["act"] = a, ["act_id"] = rs.Act.Id.Entry, ["map"] = Ui.FullMap() }));
            }
            w.Flush();
        }
        Ui.EndRunForFork();
    }
}
