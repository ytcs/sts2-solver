using System.Text;
using MegaCrit.Sts2.Core.DevConsole;

namespace AgentBridge;

public static class Commands
{
    public static async Task<string> Execute(string line)
    {
        var parts = line.Split(' ', StringSplitOptions.RemoveEmptyEntries);
        string cmd = parts.Length > 0 ? parts[0].ToLowerInvariant() : "s";
        var args = parts.Skip(1).ToArray();
        switch (cmd)
        {
            case "s":
                await Settle(0, null);
                return await MainThread.Run(() => { Main.EnsureSelector(); Snap.Observe(); return Decisions.Build().Render(); });
            case "peek":   // the state as it is now, without waiting for the screen to settle (for reading the screen right after an action that already settled)
                return await MainThread.Run(() => { Main.EnsureSelector(); return Decisions.Build().Render(); });
            case "a":
            {
                string before = await MainThread.Run(() => Decisions.Build().Sig());
                string? err = await MainThread.Run(() => Act(args));
                if (err != null) return "ERR " + err + "\n" + await MainThread.Run(() => Decisions.Build().Render());
                await Settle(4, before);
                return await MainThread.Run(() => { Snap.Observe(); return Decisions.Build().Render(); });
            }
            case "do":   // one action in the oracle script vocabulary (JSON), then the new state like `a`
            {
                string before = await MainThread.Run(() => Decisions.Build().Sig());
                string? err = await MainThread.Run(() => Decisions.DoJson(string.Join(' ', args)));
                if (err != null) return "ERR " + err + "\n" + await MainThread.Run(() => Decisions.Build().Render());
                await Settle(4, before);
                return await MainThread.Run(() => { Snap.Observe(); return Decisions.Build().Render(); });
            }
            case "draw":   // draw a route on the in-game map: draw r1c6 r2c6 r3c6 ... | draw clear
                return await MainThread.Run(() => DrawRoute(args));
            case "mods":   // run modifiers selectable on the Custom Run screen
                return await MainThread.Run(() =>
                {
                    string T(ModifierModel m) { try { return Text.Loc(m.Title); } catch { return ""; } }
                    return string.Join("\n", ModelDb.GoodModifiers.Select(m => "good " + m.Id.Entry + ": " + T(m))
                        .Concat(ModelDb.BadModifiers.Select(m => "bad " + m.Id.Entry + ": " + T(m)))) + "\n";
                });
            case "deck.json": return await MainThread.Run(() => (Snap.DeckJson() ?? "null") + "\n");
            case "fight":   // JSON {scenario, log, state} of the current combat for the solver (visible information only)
                await Settle(0, null);
                return await MainThread.Run(() => (Snap.Fight() ?? "null") + "\n");
            case "snap":
                return await MainThread.Run(() => (Snap.State() ?? "null") + "\n");
            case "d": return await MainThread.Run(Deck);
            case "p": return await MainThread.Run(() => Pile(args.FirstOrDefault() ?? "draw"));
            case "m": return await MainThread.Run(Decisions.FullMap);
            case "f":
                return await MainThread.Run(() =>
                {
                    var prefs = MegaCrit.Sts2.Core.Saves.SaveManager.Instance.PrefsSave;
                    if (args.Length > 0 && Enum.TryParse<MegaCrit.Sts2.Core.Settings.FastModeType>(args[0], true, out var m)) prefs.FastMode = m;
                    return $"fast mode: {prefs.FastMode}\n";
                });
            case "t":
                return await MainThread.Run(() =>
                {
                    var root = ((Godot.SceneTree)Godot.Engine.GetMainLoop()).Root;
                    var sb = new StringBuilder();
                    foreach (var b in UiHelper.FindAll<NClickableControl>(root).Where(b => b.IsVisibleInTree()))
                        sb.Append(b.IsEnabled ? "" : "(off) ").Append(b.GetPath()).Append(' ').Append(Text.NodeLabel(b)).Append('\n');
                    sb.Append("overlay: ").Append(NOverlayStack.Instance?.Peek()?.GetType().Name ?? "-").Append('\n');
                    return sb.ToString();
                });
            case "x":
                return await MainThread.Run(() =>
                {
                    var r = new DevConsole(true).ProcessCommand(string.Join(' ', args));
                    return $"{(r.success ? "ok" : "fail")} {r.msg}\n";
                });
            default:
                return "commands: s | a <i> [args] | a dp <slot> | d | p draw|discard|exhaust | m | x <console cmd>\n";
        }
    }

    /// <summary>Draws a polyline through the given map points (r&lt;row&gt;c&lt;col&gt;) on the map screen's drawing layer, so a human reviewer sees the planned route.</summary>
    private static string DrawRoute(string[] args)
    {
        var screen = NMapScreen.Instance;
        if (screen == null) return "no map screen\n";
        var dr = screen.Drawings;
        if (args.Length == 1 && args[0] == "clear") { dr.ClearDrawnLinesLocal(); return "cleared\n"; }
        var pts = UiHelper.FindAll<NMapPoint>(screen).ToList();
        var centers = new List<Godot.Vector2>();
        foreach (var a in args)
        {
            var m = System.Text.RegularExpressions.Regex.Match(a, @"^r(\d+)c(\d+)$");
            if (!m.Success) return "bad point " + a + " (use r<row>c<col>)\n";
            int row = int.Parse(m.Groups[1].Value), col = int.Parse(m.Groups[2].Value);
            var n = pts.FirstOrDefault(p => p.Point.coord.row == row && p.Point.coord.col == col);
            if (n == null) return $"no map point {a}\n";
            centers.Add(dr.GetGlobalTransform().AffineInverse() * n.GetGlobalRect().GetCenter());
        }
        if (centers.Count < 2) return "need at least two points\n";
        dr.SetDrawingModeLocal(DrawingMode.Drawing);
        dr.BeginLineLocal(centers[0], DrawingMode.Drawing);
        for (int i = 1; i < centers.Count; i++)
            for (int s = 1; s <= 12; s++)
                dr.UpdateCurrentLinePositionLocal(centers[i - 1].Lerp(centers[i], s / 12f));
        dr.StopLineLocal();
        dr.SetDrawingModeLocal(DrawingMode.None);
        return $"drew {centers.Count} points\n";
    }

    private static string? Act(string[] args)
    {
        Main.EnsureSelector();
        if (args.Length == 0) return "usage: a <i> [args]";
        if (AgentSelector.Current != null)
        {
            if (args[0] == "-") return AgentSelector.Answer(Array.Empty<int>());
            var idx = new List<int>();
            foreach (var a in args)
            {
                if (!int.TryParse(a, out var i)) return "bad index " + a;
                idx.Add(i);
            }
            return AgentSelector.Answer(idx);
        }
        if (args[0] == "dp") return Decisions.DiscardPotion(args.Skip(1).FirstOrDefault());
        var d = Decisions.Build();
        if (d.Busy && d.Opts.Count == 0) return "game is busy";
        if (!int.TryParse(args[0], out int n) || n < 0 || n >= d.Opts.Count) return $"bad option {args[0]} (0..{d.Opts.Count - 1})";
        return d.Opts[n].Run(args.Skip(1).ToArray());
    }

    /// <summary>
    /// Waits (main thread, frame by frame) until the game is at a decision: a pending card choice, a ready combat turn, or any other screen
    /// whose options stay unchanged for ~8 frames. Gives up after 30 s and returns whatever is there.
    /// </summary>
    private static Task Settle(int minFrames, string? before)
    {
        return MainThread.Run(() =>
        {
            long start = MainThread.Frame;
            long changeDeadline = Environment.TickCount64 + 8000;
            bool changed = before == null;
            string last = "";
            int stable = 0;
            return MainThread.Until(() =>
            {
                if (MainThread.Frame - start < minFrames) return false;
                if (AgentSelector.Current != null) return true;
                var d = Decisions.Build();
                if (!changed)
                {
                    // the action must visibly change the screen first (loads and transitions keep the old screen up for a while)
                    if (d.Sig() == before && Environment.TickCount64 < changeDeadline) return false;
                    changed = true;
                }
                if (d.Busy || d.Opts.Count == 0) { stable = 0; last = ""; return false; }
                if (d.Kind == "COMBAT") return true;
                string sig = d.Sig();
                if (sig == last) stable++;
                else { stable = 0; last = sig; }
                return stable >= 8;
            }, 30000);
        }).Unwrap();
    }

    private static string Deck()
    {
        var rs = RunManager.Instance.DebugOnlyGetState();
        var me = rs != null ? LocalContext.GetMe(rs) : null;
        if (me == null) return "no run\n";
        var sb = new StringBuilder();
        sb.Append($"deck ({me.Deck.Cards.Count}):\n");
        foreach (var g in me.Deck.Cards.GroupBy(c => Text.Card(c)).OrderBy(g => g.Key))
            sb.Append(g.Count() > 1 ? $"{g.Count()}x " : "").Append(g.Key).Append('\n');
        sb.Append("relics:\n");
        foreach (var r in me.Relics) sb.Append(Decisions.Relic(r)).Append('\n');
        sb.Append("potions:\n");
        foreach (var p in me.PotionSlots) if (p != null) sb.Append($"{Text.Loc(p.Title)}: {Text.Loc(p.DynamicDescription)}\n");
        return sb.ToString();
    }

    private static string Pile(string which)
    {
        var rs = RunManager.Instance.DebugOnlyGetState();
        var me = rs != null ? LocalContext.GetMe(rs) : null;
        var pcs = me?.PlayerCombatState;
        if (pcs == null) return "not in combat\n";
        var pile = which.StartsWith("disc") ? pcs.DiscardPile : which.StartsWith("ex") ? pcs.ExhaustPile : pcs.DrawPile;
        var sb = new StringBuilder($"{which} ({pile.Cards.Count}, unordered):\n");
        foreach (var g in pile.Cards.GroupBy(c => Text.Card(c, false)).OrderBy(g => g.Key))
            sb.Append(g.Count() > 1 ? $"{g.Count()}x " : "").Append(g.Key).Append('\n');
        return sb.ToString();
    }
}
