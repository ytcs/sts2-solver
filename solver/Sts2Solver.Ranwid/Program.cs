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

// --advice-bench: time a full removal-advice run on a synthetic 30-card deck vs the Act-1 elites, to measure
// the parallel speedup end-to-end (the cost the player actually waits on). Reports the implied sequential time
// (one eval × the grid size) vs the measured parallel time.
if (args.Contains("--advice-bench"))
{
    var deck = new List<string>();
    string[] variety = { "StrikeIronclad","DefendIronclad","Bash","Inflame","DemonForm","Uppercut","TwinStrike",
        "Whirlwind","ShrugItOff","PommelStrike","IronWave","Headbutt","Hemokinesis","Armaments","Thunderclap" };
    for (int i = 0; i < 30; i++) deck.Add(variety[i % variety.Length]);
    int distinct = deck.Distinct().Count();
    int asc = ArgInt("--ascension") ?? 0;
    var elites = new[] { "TerrorEelElite", "ByrdonisElite", "MechaKnightElite" }
        .Select(n => new Advisor.Encounter(n, () => Sts2Solver.Content.Catalog.BuildEliteEncounter(n, asc))).ToList();
    var rankOpts = opts with { MctsTrials = Advisor.AdviceTrials };   // the production ranking budget

    Console.WriteLine($"advice-bench: {deck.Count}-card deck ({distinct} distinct) vs {elites.Count} Act-1 elites "
        + $"⇒ {(1 + distinct) * elites.Count} evals @ {Advisor.AdviceTrials} trials, {Environment.ProcessorCount} cores.");
    // One ScoreDeck (sequential over #elites) → per-eval baseline at the production ranking budget.
    var sw1 = System.Diagnostics.Stopwatch.StartNew();
    var baseScore = Advisor.ScoreDeck(deck, elites, 70, 80, 3, new[] { "BurningBlood" }, rankOpts);
    sw1.Stop();
    double perEvalMs = sw1.ElapsedMilliseconds / (double)elites.Count;

    var sw2 = System.Diagnostics.Stopwatch.StartNew();
    var (baseline, items) = Advisor.RemovalAdvice(deck, elites, 70, 80, 3, new[] { "BurningBlood" }, opts);
    sw2.Stop();

    double seqEstMs = (1 + distinct) * elites.Count * perEvalMs;
    Console.WriteLine($"  per-eval        : {perEvalMs:F0} ms");
    Console.WriteLine($"  removal advice  : {sw2.ElapsedMilliseconds / 1000.0:F1} s (parallel)");
    Console.WriteLine($"  implied serial  : {seqEstMs / 1000.0:F1} s  ⇒ ~{seqEstMs / Math.Max(1, sw2.ElapsedMilliseconds):F1}x speedup");
    Console.WriteLine($"  bottleneck survival {baseline.MinSurvival:P0}; top cut: "
        + (items.FirstOrDefault(i => i.IsImprovement) is { } it ? it.Card : "(none improves)"));
    var strength = Advisor.DeckStrength(deck, elites, 3, new[] { "BurningBlood" }, opts);
    Console.WriteLine($"  deck strength   : {strength:F0}/100 (from 100 HP)");
    return 0;
}

if (once)
{
    var path = saveArg ?? SaveSource.FindNewest(saveDirArg);
    if (path == null) { Console.Error.WriteLine("ranwid: no ongoing unmodded run found (start a run, or pass --save <file> / --save-dir <folder>)."); return 1; }
    var ctx = Companion.Load(path, playerNetId);
    if (ctx == null) return 0;                       // non-Ironclad / unreadable (message already printed)
    var strength = Advisor.DeckStrength(ctx.DeckSpecs, ctx.Encounters, ctx.Run.MaxEnergy, ctx.RelicNames, opts);
    Dashboard.Render(ctx, Companion.EvaluateElites(ctx, opts), strength, evaluating: false);
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
