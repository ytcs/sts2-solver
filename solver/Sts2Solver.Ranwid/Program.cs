using Sts2Solver.Search;
using Sts2Solver.Ranwid;

// ranwid — the seer. A LIVE COMPANION for an ongoing Slay the Spire 2 run: it watches your save and shows a
// live dashboard of the current deck and how it fares against the Act's elites, auto-refreshing whenever the
// run changes. Keys: (r) best cards to remove · (c) check a reward card · (d) refresh · (q) quit.
//
//   ranwid                         live dashboard (default): watch the run, auto-refresh on save change
//   ranwid --once                  render the dashboard once and exit (non-interactive)
//   ranwid --preview               show the dashboard with sample data (no run needed)
//   ranwid --save <current_run.save>   watch/read a specific save file (e.g. a modded profile)
//   ranwid --save-dir <folder>         search this folder for the save (overrides auto-detect; RANWID_SAVE_DIR env also works)
//   --rewards A,B,C                (with --once) check these reward options
//   --player <net_id>              pick a multiplayer slot
//   --rollouts <n> (2000)  --trials <n> (2000)  --seed <n>

string? ArgVal(string flag) { int i = Array.IndexOf(args, flag); return i >= 0 && i + 1 < args.Length ? args[i + 1] : null; }
int? ArgInt(string flag) => int.TryParse(ArgVal(flag), out var v) ? v : null;

bool once = args.Contains("--once");
string? rewardsArg = ArgVal("--rewards");
string? saveArg = ArgVal("--save");
string? saveDirArg = ArgVal("--save-dir");
int? playerNetId = ArgInt("--player");
var opts = new EvalOptions
{
    // ranwid is MCTS-only: exact expectimax can't solve real run decks in time, and attempting it just adds
    // latency. BudgetSeconds = 0 skips the exact attempt entirely (exact remains a dev/calibration tool — the
    // CLI, --bridge, and tests still use it on small decks). MCTS gives survival + mean + HP-loss distribution.
    BudgetSeconds = 0.0,
    Rollouts = ArgInt("--rollouts") ?? 2000,
    MctsTrials = ArgInt("--trials") ?? 2_000,
    Seed = ArgInt("--seed") ?? 1,
};

if (args.Contains("--preview")) { Dashboard.RenderPreview(); return 0; }

if (once)
{
    var path = saveArg ?? SaveSource.FindNewest(saveDirArg);
    if (path == null) { Console.Error.WriteLine("ranwid: no ongoing unmodded run found (start a run, or pass --save <file> / --save-dir <folder>)."); return 1; }
    var ctx = Companion.Load(path, playerNetId);
    if (ctx == null) return 0;                       // non-Ironclad / unreadable (message already printed)
    Dashboard.Render(ctx, Companion.EvaluateElites(ctx, opts), evaluating: false);
    var tokens = (rewardsArg ?? "").Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
    if (tokens.Length > 0)
    {
        var pick = Companion.PickFromTokens(ctx, tokens, opts);
        if (pick is { } p) Dashboard.RenderPick(p);
    }
    return 0;
}

// Default: the live companion (interactive prompt + auto-refresh on save change).
return new Companion(saveArg, saveDirArg, playerNetId, opts).Run();
