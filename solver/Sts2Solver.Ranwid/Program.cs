using Sts2Solver.Search;
using Sts2Solver.Ranwid;

// ranwid — the seer. A LIVE COMPANION for an ongoing Slay the Spire 2 run: it watches your save, shows the
// current deck's per-elite stats + the best card to remove, and lets you type the cards a reward screen
// offers (Tab auto-completes, typos auto-correct) to get a take-vs-skip verdict against the Act's elites.
//
//   ranwid                         live companion: watch the run, interactive prompt (default)
//   ranwid --once                  evaluate once and exit (non-interactive)
//   ranwid --save <current_run.save>   watch/read a specific save file (e.g. a modded profile)
//   --rewards A,B,C                (with --once) vet these reward options; in live mode just type them
//   --no-advice                    skip the (slower) removal/pick advice; just the deck + elite stats
//   --player <net_id>              pick a multiplayer slot
//   --budget-seconds <s>           exact-search cap before MCTS fallback (default 8; raise for precision)
//   --rollouts <n> (2000)  --trials <n> (40000)  --seed <n>

string? ArgVal(string flag) { int i = Array.IndexOf(args, flag); return i >= 0 && i + 1 < args.Length ? args[i + 1] : null; }
int? ArgInt(string flag) => int.TryParse(ArgVal(flag), out var v) ? v : null;
double? ArgDouble(string flag) => double.TryParse(ArgVal(flag), out var v) ? v : null;

bool once = args.Contains("--once");
bool noAdvice = args.Contains("--no-advice");
string? rewardsArg = ArgVal("--rewards");
string? saveArg = ArgVal("--save");
int? playerNetId = ArgInt("--player");
var opts = new EvalOptions
{
    BudgetSeconds = ArgDouble("--budget-seconds") ?? 8.0,
    Rollouts = ArgInt("--rollouts") ?? 2000,
    MctsTrials = ArgInt("--trials") ?? 40_000,
    Seed = ArgInt("--seed") ?? 1,
};

if (once)
{
    var path = saveArg ?? SaveLocator.FindNewest();
    if (path == null) { Console.Error.WriteLine("ranwid: no ongoing unmodded run found (start a run, or pass --save <file>)."); return 1; }
    var ctx = Companion.Load(path, playerNetId);
    if (ctx == null) return 0;                       // non-Ironclad / unreadable (message already printed)
    Companion.ReportDeck(ctx, opts);
    if (!noAdvice)
    {
        var tokens = (rewardsArg ?? "").Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        if (tokens.Length > 0) Companion.ReportPick(ctx, tokens, opts);
        Companion.ReportCuts(ctx, opts);
    }
    return 0;
}

// Default: the live companion (interactive prompt + auto-refresh on save change).
return new Companion(saveArg, playerNetId, opts).Run();
