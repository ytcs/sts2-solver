using System.Collections.Concurrent;
using System.Net;
using System.Net.Sockets;
using System.Text;
using AgentBridge;
using HarmonyLib;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.CardRewardAlternatives;
using MegaCrit.Sts2.Core.Entities.TreasureRelicPicking;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Random;
using MegaCrit.Sts2.Core.Rewards;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.TestSupport;
using MegaCrit.Sts2.Core.Unlocks;

namespace OracleCombat;

// Headless game server speaking the in-game bridge's line protocol (mods/AgentBridge: Server.cs, Commands.cs).
// One game per process; the game thread runs the pump and executes every command.
public static class Serve
{
    sealed class Req
    {
        public string Line;
        public readonly TaskCompletionSource<string> Done = new(TaskCreationOptions.RunContinuationsAsynchronously);
    }

    static readonly BlockingCollection<Req> _cmds = new();
    static Pump _pump;
    static Req _inflight;
    static readonly Stack<Func<bool>> _nested = new();
    static bool _quit;
    public static int DefaultAscension = 10;

    public static int Main(Pump pump, Dictionary<string, string> kv)
    {
        _pump = pump;
        int port = int.Parse(kv.GetValueOrDefault("port", "15600"));
        DefaultAscension = int.Parse(kv.GetValueOrDefault("ascension", "10"));
        if (kv.TryGetValue("state-dir", out var sd)) { Directory.CreateDirectory(sd); SaveIsolation.Dir = Path.GetFullPath(sd); }
        Install(kv.GetValueOrDefault("pck") ?? Loc.DefaultPck());
        var listener = new TcpListener(IPAddress.Loopback, port);
        listener.Start();
        new Thread(() => Accept(listener)) { IsBackground = true, Name = "serve-accept" }.Start();
        Console.Error.WriteLine($"OracleCombat serve listening on 127.0.0.1:{port}");
        if (kv.TryGetValue("seed", out var seed) || kv.ContainsKey("character"))
            Fire(Ui.StartRun(kv.GetValueOrDefault("character", "ironclad"), DefaultAscension, seed, null), "start run");
        Settle();
        while (!_quit)
        {
            if (_cmds.TryTake(out var r, 2)) Process(r);
            _pump.Drain();
            Ui.DrainFatal();
        }
        return 0;
    }

    static void Install(string pck)
    {
        Loc.Install(pck);
        Patches.ApplyAscensionEffects = true;
        Patches.RealRunRng = true;
        Fatal.Lenient = true;
        var h = new Harmony("oracle.serve");
        h.CreateClassProcessor(typeof(Snap.SetUpPatch)).Patch();
        h.CreateClassProcessor(typeof(PromptPatch)).Patch();
        foreach (var t in typeof(ServePatches).GetNestedTypes(System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Public))
            h.CreateClassProcessor(t).Patch();
        CardSelectCmd.PushSelector(new ServeSelector());
        RewardsSet.testSelector = Ui.OnRewards;
        CombatManager.Instance.CombatWon += room => SynchronizationContext.Current.Post(_ => Fire(Ui.AfterCombatWon(room), "combat won"), null);
    }

    static void Accept(TcpListener listener)
    {
        while (true)
        {
            TcpClient c;
            try { c = listener.AcceptTcpClient(); } catch { continue; }
            Task.Run(() => Handle(c));
        }
    }

    static async Task Handle(TcpClient c)
    {
        using (c)
        {
            var stream = c.GetStream();
            string reply;
            try
            {
                using var reader = new StreamReader(stream, Encoding.UTF8, false, 4096, leaveOpen: true);
                var r = new Req { Line = (await reader.ReadLineAsync())?.Trim() ?? "" };
                _cmds.Add(r);
                reply = await r.Done.Task;
            }
            catch (Exception e) { reply = "ERR " + e.GetBaseException().Message; }
            var bytes = Encoding.UTF8.GetBytes(reply.EndsWith('\n') ? reply : reply + "\n");
            try { await stream.WriteAsync(bytes); } catch { }
        }
    }

    public static void Fire(Task t, string what) =>
        t.ContinueWith(x => Console.Error.WriteLine($"serve: {what} failed: {x.Exception?.GetBaseException()}"), TaskContinuationOptions.OnlyOnFaulted);

    static void Reply(Req r, string text) => r.Done.TrySetResult(text);

    // Game code blocked on a synchronous prompt (card reward): answer the command in flight with the prompt screen,
    // then keep serving until the prompt is answered; the answering command's reply is sent once the game settles.
    public static void WaitNested(Func<bool> answered)
    {
        _nested.Push(answered);
        try
        {
            if (_inflight != null) { var r = _inflight; _inflight = null; Snap.Observe(); Reply(r, Ui.Build().Render()); }
            while (!answered())
            {
                if (_cmds.TryTake(out var c, 5)) Process(c);
                if (_quit) throw new OperationCanceledException("server shutting down");
            }
        }
        finally { _nested.Pop(); }
    }

    static void Process(Req r)
    {
        try
        {
            var parts = r.Line.Split(' ', StringSplitOptions.RemoveEmptyEntries);
            string cmd = parts.Length > 0 ? parts[0].ToLowerInvariant() : "s";
            var args = parts.Skip(1).ToArray();
            if (cmd is "a" or "do")
            {
                _inflight = r;
                string err = cmd == "a" ? Ui.Act(args) : Ui.DoJson(string.Join(' ', args));
                if (err != null)
                {
                    if (_inflight == r) { _inflight = null; Reply(r, "ERR " + err + "\n" + Ui.Build().Render()); }
                    return;
                }
                if (_nested.Count > 0 && _nested.Peek()()) return;
                Settle();
                if (_inflight != null) { var x = _inflight; _inflight = null; Snap.Observe(); Reply(x, Ui.Build().Render()); }
                return;
            }
            Reply(r, ReadOnly(cmd, args));
        }
        catch (Exception e)
        {
            Console.Error.WriteLine("serve: " + r.Line + ": " + e);
            if (_inflight == r) _inflight = null;
            Reply(r, "ERR " + e.GetBaseException().Message);
        }
    }

    static string ReadOnly(string cmd, string[] args)
    {
        switch (cmd)
        {
            case "s": Settle(); Snap.Observe(); return Ui.Build().Render();
            case "peek": return Ui.Build().Render();
            case "fight": Settle(); return (Snap.Fight() ?? "null") + "\n";
            case "snap": return (Snap.State() ?? "null") + "\n";
            case "deck.json": return (Snap.DeckJson() ?? "null") + "\n";
            case "d": return Ui.Deck();
            case "p": return Ui.Pile(args.FirstOrDefault() ?? "draw");
            case "m": return Ui.FullMap();
            case "mods":
                string T(ModifierModel m) { try { return Text.Loc(m.Title); } catch { return ""; } }
                return string.Join("\n", ModelDb.GoodModifiers.Select(m => "good " + m.Id.Entry + ": " + T(m))
                    .Concat(ModelDb.BadModifiers.Select(m => "bad " + m.Id.Entry + ": " + T(m)))) + "\n";
            case "f":
                return $"fast mode: {MegaCrit.Sts2.Core.Saves.SaveManager.Instance.PrefsSave.FastMode}\n";
            case "draw": return "no map screen\n";
            case "t": return "overlay: " + Ui.Build().Kind + "\n";
            case "x":
                if (!ConsoleOk.Contains(args.FirstOrDefault() ?? "help")) return "fail not allowed headless (model-only console commands: " + string.Join(" ", ConsoleOk.Order()) + ")\n";
                try
                {
                    var res = new MegaCrit.Sts2.Core.DevConsole.DevConsole(true).ProcessCommand(string.Join(' ', args));
                    Settle();
                    return $"{(res.success ? "ok" : "fail")} {res.msg}\n";
                }
                catch (Exception e) { return "fail dev console is not supported headless: " + e.GetBaseException().Message + "\n"; }
            case "menu": return Ui.Suspend();
            case "shutdown": _quit = true; return "bye\n";
            default:
                return "commands: s | a <i> [args] | a dp <slot> | d | p draw|discard|exhaust | m | x <console cmd>\n";
        }
    }

    // dev console commands that only touch the model (no saves, cloud, logs, OS, platform)
    static readonly HashSet<string> ConsoleOk = new() { "help", "act", "afflict", "ancient", "power", "block", "card", "damage", "die", "draw", "enchant", "energy", "event", "fight",
        "godmode", "gold", "heal", "instant", "kill", "potion", "relic", "remove_card", "room", "stars", "travel", "upgrade", "win" };

    // Run the pump until the game waits for input: a prompt is pending, or nothing is queued and no action runs.
    public static void Settle()
    {
        int idle = 0;
        var sw = System.Diagnostics.Stopwatch.StartNew();
        while (true)
        {
            if (_pump.Drain() > 0) { idle = 0; sw.Restart(); continue; }
            Ui.DrainFatal();
            if (Ui.Prompted()) return;
            var rm = RunManager.Instance;
            bool busy = rm?.ActionExecutor != null && (rm.ActionExecutor.IsRunning || !rm.ActionQueueSet.IsEmpty) && !rm.ActionExecutor.IsPaused;
            if (!busy && ++idle >= 3) return;
            if (!busy) { Thread.Yield(); continue; }
            if (_pump.RunOne(1)) { idle = 0; sw.Restart(); }
            if (sw.ElapsedMilliseconds > 5000) { Console.Error.WriteLine("serve: settle gave up after 5 s without progress"); return; }
        }
    }
}

// Card prompts: the bridge's AgentSelector for card picks (async), and a nested wait for the synchronous card reward.
public sealed class ServeSelector : ICardSelector
{
    public Task<IEnumerable<CardModel>> GetSelectedCards(IEnumerable<CardModel> options, int minSelect, int maxSelect)
        => AgentSelector.Instance.GetSelectedCards(options, minSelect, maxSelect);

    public CardRewardSelection GetSelectedCardReward(IReadOnlyList<CardCreationResult> options, IReadOnlyList<CardRewardAlternative> alternatives)
    {
        var p = new Ui.PendingCardReward { Options = options, Alternatives = alternatives };
        Ui.CardRew = p;
        try { Serve.WaitNested(() => p.Answer.HasValue); }
        finally { Ui.CardRew = null; }
        int a = p.Answer.Value;
        return a < options.Count ? new CardRewardSelection { card = options[a].Card } : new CardRewardSelection { alternative = alternatives[a - options.Count] };
    }
}

// Screens that the game only shows through Godot nodes become prompts.
public static class ServePatches
{
    [HarmonyPatch(typeof(RelicSelectCmd), nameof(RelicSelectCmd.FromChooseARelicScreen))]
    static class P_ChooseRelic
    {
        static bool Prefix(Player player, IReadOnlyList<RelicModel> relics, ref Task<RelicModel> __result) { __result = Ui.ChooseRelic(player, relics); return false; }
    }

    [HarmonyPatch(typeof(CardSelectCmd), nameof(CardSelectCmd.FromChooseABundleScreen))]
    static class P_ChooseBundle
    {
        static bool Prefix(Player player, IReadOnlyList<IReadOnlyList<CardModel>> bundles, ref Task<IEnumerable<CardModel>> __result) { __result = Ui.ChooseBundle(player, bundles); return false; }
    }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Nodes.Events.Custom.CrystalSphere.NCrystalSphereScreen), nameof(MegaCrit.Sts2.Core.Nodes.Events.Custom.CrystalSphere.NCrystalSphereScreen.ShowScreen))]
    static class P_CrystalSphere
    {
        static bool Prefix(MegaCrit.Sts2.Core.Events.Custom.CrystalSphereEvent.CrystalSphereMinigame grid) { Ui.Sphere = grid; grid.Finished += Ui.SphereFinished; return false; }
    }
}
