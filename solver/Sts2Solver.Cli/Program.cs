using System.Text.Json;
using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;

// sts2solve — compute HP lost under optimal play for a deck vs an encounter.
//   sts2solve --demo                 run the built-in Ironclad-starter vs CalcifiedCultist scenario
//   sts2solve <scenario.json>        solve a scenario file
//   (add --lines to print the optimal opening play for each possible opening hand)
//   --mcts [--trials N] [--anytime]  use the sampling/MCTS solver instead of exact expectimax
//   --hybrid N                       (with --mcts) defer subtrees of size ≤ N to the exact oracle

bool showLines = args.Contains("--lines");
bool useMcts = args.Contains("--mcts");
bool anytime = args.Contains("--anytime");
var positional = args.Where(a => !a.StartsWith("--")).ToList();

int ArgInt(string flag, int fallback)
{
    int i = Array.IndexOf(args, flag);
    return i >= 0 && i + 1 < args.Length && int.TryParse(args[i + 1], out var v) ? v : fallback;
}

// --perf-probe: measure MCTS cost (wall-clock + search-tree size NodesCreated) along the TWO scaling axes that
// matter for late-game usability, at fixed trials vs a fixed elite. Series A scales raw DECK SIZE at low card
// VARIETY (3 distinct types); series B scales distinct-card VARIETY at fixed size. Theory: because SelectEdge
// expands EVERY legal play once (no action progressive widening), per-decision branching ~ distinct legal plays,
// so B should blow up far faster than A — confirming action-PW/PUCT (not raw size) is the right lever.
if (args.Contains("--perf-probe"))
{
    int trials = ArgInt("--trials", 20_000);
    int maxTurns = ArgInt("--maxturns", 12);
    int hp = ArgInt("--hp", 60);

    string[] variety = {  // distinct-effect Ironclad cards, added one at a time for series B
        "StrikeIronclad", "DefendIronclad", "Bash", "Inflame", "Uppercut", "PommelStrike",
        "ShrugItOff", "Anger", "IronWave", "Hemokinesis", "TwinStrike", "Headbutt" };

    List<Sts2Solver.Engine.CardModel>? TryBuild(IEnumerable<string> specs)
    {
        try { return specs.Select(Catalog.BuildCard).ToList(); } catch (ArgumentException) { return null; }
    }
    (long ms, int nodes, double win, double loss) RunOne(List<Sts2Solver.Engine.CardModel> deck)
    {
        var player = Catalog.BuildPlayer(deck, hp, hp, 3, new[] { "BurningBlood" });
        var setup = Catalog.SetupCombat(player, new[] { Monsters.Byrdonis(hp: 60) });
        var sw = System.Diagnostics.Stopwatch.StartNew();
        var mcts = new MctsSolver(new MctsOptions { Trials = trials, MaxTurns = maxTurns, Seed = 1 });
        var v = mcts.Solve(setup);
        return (sw.ElapsedMilliseconds, mcts.NodesCreated, v.Win, v.Loss);
    }

    Console.WriteLine($"MCTS cost probe vs Byrdonis(60), {trials:N0} trials, horizon {maxTurns}, player {hp} HP.\n");

    Console.WriteLine("  A) scale DECK SIZE at low variety (Strike/Defend/Bash):");
    Console.WriteLine($"     {"size",4} {"distinct",8} {"nodes",12} {"ms",7}  {"ms/1k-trial",11}  value");
    foreach (int size in new[] { 10, 20, 30, 40, 50 })
    {
        var specs = new List<string>();
        for (int i = 0; i < size; i++) specs.Add(i % 3 == 2 ? "Bash" : (i % 3 == 0 ? "StrikeIronclad" : "DefendIronclad"));
        var deck = TryBuild(specs)!;
        int distinct = specs.Distinct().Count();
        var (ms, nodes, win, loss) = RunOne(deck);
        Console.WriteLine($"     {size,4} {distinct,8} {nodes,12:N0} {ms,7} {ms * 1000.0 / trials,11:F2}  {win:P0}/{loss:F1}");
    }

    Console.WriteLine("\n  B) scale distinct-card VARIETY at fixed size 12:");
    Console.WriteLine($"     {"size",4} {"distinct",8} {"nodes",12} {"ms",7}  {"ms/1k-trial",11}  value");
    foreach (int k in new[] { 3, 5, 7, 9, 11 })
    {
        var chosen = variety.Take(k).ToList();
        var specs = new List<string>();
        for (int i = 0; i < 12; i++) specs.Add(chosen[i % chosen.Count]);
        var deck = TryBuild(specs);
        if (deck == null) { Console.WriteLine($"     (k={k}: a card name failed to build — skipped)"); continue; }
        int distinct = specs.Distinct().Count();
        var (ms, nodes, win, loss) = RunOne(deck);
        Console.WriteLine($"     {12,4} {distinct,8} {nodes,12:N0} {ms,7} {ms * 1000.0 / trials,11:F2}  {win:P0}/{loss:F1}");
    }
    Console.WriteLine("\n  If B's nodes/ms climb steeply with distinct while A stays ~flat, action-branching (not deck");
    Console.WriteLine("  size) is the bottleneck → PUCT + action progressive widening is the right algorithmic fix.");
    return 0;
}

// --converge: 30-card-vs-elite self-convergence. No exact ground truth exists at 30 cards, so watch the root
// value SETTLE as trials grow (anytime), with cumulative wall-clock, to find the smallest budget that's
// advice-quality and confirm it lands in the 1–2 s target. Pair with --calibrate (which shows the rollout leaf
// is essentially exact by ~2–5k trials on solvable decks) to choose the production trial budget.
if (args.Contains("--converge"))
{
    int cvMax = ArgInt("--trials", 8_000);
    int cvEvery = ArgInt("--every", 500);
    int cvTurns = ArgInt("--maxturns", 12);
    int cvSize = ArgInt("--size", 30);
    int cvHp = ArgInt("--hp", 60);
    string[] cvVariety = { "StrikeIronclad", "DefendIronclad", "Bash", "Inflame", "Uppercut", "PommelStrike",
        "ShrugItOff", "Anger", "IronWave", "Hemokinesis", "TwinStrike", "Headbutt" };
    var cvSpecs = new List<string>();
    for (int i = 0; i < cvSize; i++) cvSpecs.Add(cvVariety[i % cvVariety.Length]);
    var cvPlayer = Catalog.BuildPlayer(cvSpecs.Select(Catalog.BuildCard).ToList(), cvHp, cvHp, 3, new[] { "BurningBlood" });
    var cvSetup = Catalog.SetupCombat(cvPlayer, new[] { Monsters.Byrdonis(hp: 60) });

    Console.WriteLine($"Converge — {cvSize}-card deck ({cvSpecs.Distinct().Count()} distinct) vs Byrdonis(60), "
        + $"APW+rollout default, horizon {cvTurns}. Watching the root value settle.\n");
    Console.WriteLine($"  {"trials",8} {"win",8} {"E[loss]",8} {"cum-s",7}");
    var cvMcts = new MctsSolver(new MctsOptions { Trials = cvMax, MaxTurns = cvTurns, Seed = 1 });
    var cvSw = System.Diagnostics.Stopwatch.StartNew();
    foreach (var (tr, val) in cvMcts.SolveAnytime(cvSetup, cvEvery))
        Console.WriteLine($"  {tr,8:N0} {val.Win,8:P1} {val.Loss,8:F1} {cvSw.Elapsed.TotalSeconds,7:F2}");
    return 0;
}

// --profile: attribute a representative 30-card-vs-elite MCTS solve's wall-clock to per-node state cloning,
// to decide the next perf lever (clone-elimination via make/undo). Reports the solve time vs the 1–2s target,
// an isolated ns/clone microbenchmark, the CombatState.Clone count, and clone's estimated share of total.
// Flags: --trials (top row, default 40000), --size (deck size, default 30), --maxturns (12), --hp (60).
if (args.Contains("--profile"))
{
    int trials = ArgInt("--trials", 40_000);
    int maxTurns = ArgInt("--maxturns", 12);
    int size = ArgInt("--size", 30);
    int hp = ArgInt("--hp", 60);

    string[] variety = {
        "StrikeIronclad", "DefendIronclad", "Bash", "Inflame", "Uppercut", "PommelStrike",
        "ShrugItOff", "Anger", "IronWave", "Hemokinesis", "TwinStrike", "Headbutt" };
    var specs = new List<string>();
    for (int i = 0; i < size; i++) specs.Add(variety[i % variety.Length]);
    int distinct = specs.Distinct().Count();

    CombatState MakeSetup()
    {
        var player = Catalog.BuildPlayer(specs.Select(Catalog.BuildCard).ToList(), hp, hp, 3, new[] { "BurningBlood" });
        return Catalog.SetupCombat(player, new[] { Monsters.Byrdonis(hp: 60) });
    }

    Console.WriteLine($"Profile — {size}-card Ironclad deck ({distinct} distinct) vs Byrdonis(60), APW default, "
        + $"horizon {maxTurns}.\n  Target: a 1–2 s solve.\n");

    // 1) Isolated ns/clone microbenchmark on a representative setup state (the static counter side-effect
    //    keeps the JIT from eliding the discarded clone).
    var probe = MakeSetup();
    const int warm = 20_000, iters = 300_000;
    for (int i = 0; i < warm; i++) { _ = probe.Clone(); }
    var swc = System.Diagnostics.Stopwatch.StartNew();
    for (int i = 0; i < iters; i++) { _ = probe.Clone(); }
    swc.Stop();
    double nsPerClone = swc.Elapsed.TotalMilliseconds * 1_000_000.0 / iters;
    Console.WriteLine($"  Clone microbench: {nsPerClone:F0} ns/clone ({size}-card deck state).\n");

    // 2) Leaf-cost comparison at a fixed (small) trial count. All three share the APW-default tree + prior;
    //    they differ ONLY in the leaf seed: greedy rollout (a full playout to terminal — faithful, expensive)
    //    vs the closed-form CombatHeuristic.Evaluate vs the Phase-C learned VF. If the closed-form leaves are
    //    dramatically faster, the rollout is the bottleneck (not cloning); if they're ALSO slow, the APW prior's
    //    per-candidate Score(ApplyPlay) dominates and the clone-free static prior is the lever.
    int t = ArgInt("--trials", 1_500);
    Console.WriteLine($"  Bottleneck matrix @ {t:N0} trials (cheap leaf throughout; vary the APW prior + chance):\n");
    Console.WriteLine($"  {"config",-18} {"ms",9} {"ms/trial",9} {"nodes",9} {"clones",13} {"clone%",7}  value");
    var leafConfigs = new (string name, Action<MctsOptions> set)[]
    {
        // Baseline = current production default (APW prior + greedy rollout + exact-chance ≤4096).
        ("APW roll ch4096", _ => { }),
        // Cheap leaf, APW prior ON vs OFF: the time gap = the APW prior's per-candidate Score(ApplyPlay) cost.
        ("APW heur ch4096", o => o.UseHeuristicLeaf = true),
        ("UCT heur ch4096", o => { o.UseHeuristicLeaf = true; o.ActionWidening = false; }),
        // APW prior ON, cheap leaf, but force DPW draw sampling instead of ≤4096-way exact enumeration: the gap
        // vs "APW heur ch4096" = the chance-node draw-enumeration cost.
        ("APW heur ch64", o => { o.UseHeuristicLeaf = true; o.ExactChanceThreshold = 64; }),
    };
    foreach (var (name, set) in leafConfigs)
    {
        var profSetup = MakeSetup();
        var opt = new MctsOptions { Trials = t, MaxTurns = maxTurns, Seed = 1 };
        set(opt);
        long cBefore = CombatState.ClonesCreated;
        var profSw = System.Diagnostics.Stopwatch.StartNew();
        var profMcts = new MctsSolver(opt);
        var profV = profMcts.Solve(profSetup);
        profSw.Stop();
        long clones = CombatState.ClonesCreated - cBefore;
        double cloneMs = clones * nsPerClone / 1_000_000.0;
        double pct = profSw.ElapsedMilliseconds > 0 ? 100.0 * cloneMs / profSw.ElapsedMilliseconds : 0;
        Console.WriteLine($"  {name,-18} {profSw.ElapsedMilliseconds,9} "
            + $"{profSw.ElapsedMilliseconds / (double)t,9:F2} {profMcts.NodesCreated,9:N0} {clones,13:N0} {pct,6:F0}%  "
            + $"{profV.Win:P0}/{profV.Loss:F1}");
    }
    Console.WriteLine($"\n  (APW heur)−(UCT heur) gap = APW-prior cost; (ch4096)−(ch64) gap = chance-enumeration");
    Console.WriteLine($"  cost. Whichever dominates is the real lever toward the 1–2 s target — not cloning (≈12%).");
    return 0;
}

// --calibrate: run exact (ground truth) vs MCTS (rollout-leaf and heuristic-leaf) over the DIVERSE fixture
// suite (CalibrationFixtures — block / strength / debuff / aggro / power archetypes, not just the starter)
// and report how closely sampling tracks exact. Used to tune the shared CombatHeuristic against the oracle.
// Flags: --trials N (default 40000), --exact-budget S (per-fixture exact cap, default 90s), --archetype X.
if (args.Contains("--calibrate"))
{
    int trials = ArgInt("--trials", 40_000);
    double exactBudget = ArgInt("--exact-budget", 90);
    string? onlyArch = args.SkipWhile(a => a != "--archetype").Skip(1).FirstOrDefault();

    // Fixture source: default = the six tuned archetypes; --elites appends the single-monster elite sweep;
    // --random N draws N deterministic random decks from the safe pool (--seed S, default 12345).
    int randomN = ArgInt("--random", 0);
    int seed = ArgInt("--seed", 12345);
    IEnumerable<CalibrationFixtures.Fixture> source =
        randomN > 0 ? CalibrationFixtures.RandomDecks(randomN, seed)
        : args.Contains("--elites") ? CalibrationFixtures.All.Concat(CalibrationFixtures.EliteSweep)
        : CalibrationFixtures.All;
    var fixtures = source
        .Where(f => onlyArch == null || f.Archetype == onlyArch).ToList();

    Console.WriteLine($"Calibration — exact (ground truth) vs heuristic-guided MCTS, {trials:N0} trials, "
        + $"exact budget {exactBudget:F0}s/fixture\n");
    Console.WriteLine($"  {"fixture",-28} {"engine",-10} {"survive",8} {"loss",7} {"Δsurv",7} {"Δloss",7} {"work",12} {"time",7}");
    Console.WriteLine("  " + new string('-', 96));

    double sumDSurvRoll = 0, sumDLossRoll = 0, sumDSurvHeur = 0, sumDLossHeur = 0, sumDSurvLearn = 0, sumDLossLearn = 0;
    int graded = 0;
    foreach (var f in fixtures)
    {
        var exact = CalibrationHarness.RunExactBudgeted(f.Setup(), f.MaxTurns, exactBudget);
        var roll = CalibrationHarness.RunMcts(f.Setup(), f.MaxTurns, trials, heuristicLeaf: false, seed: 1);
        var heur = CalibrationHarness.RunMcts(f.Setup(), f.MaxTurns, trials, heuristicLeaf: true, seed: 1);
        var learn = CalibrationHarness.RunMctsLearned(f.Setup(), f.MaxTurns, trials, seed: 1);

        void Row(string name, EngineResult r, bool isExact)
        {
            string ds = isExact || exact == null ? "-" : $"{Math.Abs(r.Survival - exact.Survival):P1}";
            string dl = isExact || exact == null ? "-" : $"{Math.Abs(r.Loss - exact.Loss):F1}";
            string work = isExact ? $"{r.Work:N0} st" : $"{r.Work:N0} tr";
            Console.WriteLine($"  {name,-28} {r.Label,-10} {r.Survival,8:P1} {r.Loss,7:F1} {ds,7} {dl,7} {work,12} {r.Ms / 1000.0,6:F1}s");
        }

        if (exact != null) Row(f.Name, exact, true);
        else Console.WriteLine($"  {f.Name,-28} {"exact",-10} {"(exact > budget — no ground truth)",-44}");
        Row(f.Name, roll, false);
        Row(f.Name, heur, false);
        Row(f.Name, learn, false);

        if (exact != null)
        {
            graded++;
            sumDSurvRoll += Math.Abs(roll.Survival - exact.Survival);
            sumDLossRoll += Math.Abs(roll.Loss - exact.Loss);
            sumDSurvHeur += Math.Abs(heur.Survival - exact.Survival);
            sumDLossHeur += Math.Abs(heur.Loss - exact.Loss);
            sumDSurvLearn += Math.Abs(learn.Survival - exact.Survival);
            sumDLossLearn += Math.Abs(learn.Loss - exact.Loss);
        }
        Console.WriteLine();
    }

    if (graded > 0)
    {
        Console.WriteLine($"  Mean abs error over {graded} graded fixture(s):");
        Console.WriteLine($"    mcts-roll  : Δsurv {sumDSurvRoll / graded:P1}   Δloss {sumDLossRoll / graded:F2}");
        Console.WriteLine($"    mcts-heur  : Δsurv {sumDSurvHeur / graded:P1}   Δloss {sumDLossHeur / graded:F2}   (static-leaf baseline)");
        Console.WriteLine($"    mcts-learn : Δsurv {sumDSurvLearn / graded:P1}   Δloss {sumDLossLearn / graded:F2}   (Phase-C learned leaf)");
    }
    return 0;
}

// --train-vf: harvest exact-solver labels over the broad TrainingFixtures grid and fit the Phase-C learned
// value function (LearnedValue). Prints a train-set report (learned vs static-baseline win MAE) and the
// fitted weights as a C# block to paste into LearnedValue.Weights, and writes them to /tmp/vf-weights.txt.
// Flags: --budget-seconds S (per-fixture exact cap), --epochs N, --sample-rate R, --maxturns T.
if (args.Contains("--train-vf"))
{
    double budget = ArgInt("--budget-seconds", 10);
    int epochs = ArgInt("--epochs", 4000);
    int maxTurns = ArgInt("--maxturns", 16);
    string? sr = args.SkipWhile(a => a != "--sample-rate").Skip(1).FirstOrDefault();
    double sampleRate = double.TryParse(sr, out var srv) ? srv : 0.12;

    var fixtures = TrainingFixtures.All(maxTurns)
        .Select(f => (f.Setup(), f.MaxTurns)).ToList();
    Console.WriteLine($"Training VF: {fixtures.Count} fixtures, budget {budget:F0}s/fixture, "
        + $"sampleRate {sampleRate}, epochs {epochs}\nCollecting exact-solve labels…");

    var examples = VfTrainer.Collect(fixtures, budget, sampleRate, seed: 12345);
    Console.WriteLine($"Collected {examples.Count:N0} labelled decision states. Fitting…");

    var model = VfTrainer.Fit(examples, epochs);
    Console.WriteLine(VfTrainer.Report(model, examples));

    string code = VfTrainer.Emit(model);
    File.WriteAllText("/tmp/vf-weights.txt", code);
    Console.WriteLine("\n--- LearnedValue.Weights (written to /tmp/vf-weights.txt) ---\n");
    Console.WriteLine(code);
    return 0;
}

// --validate <trace.jsonl | dir>: replay recorded game traces through the engine and diff.
if (args.Contains("--validate"))
{
    var target = positional.FirstOrDefault()
        ?? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
                        "Projects", "sts2-solver", "data", "combat_traces");
    var files = Directory.Exists(target)
        ? Directory.GetFiles(target, "*.jsonl").OrderBy(f => f).ToArray()
        : new[] { target };
    if (files.Length == 0) { Console.Error.WriteLine($"No trace files in {target}"); return 2; }

    bool allOk = true;
    foreach (var file in files)
    {
        var rep = TraceValidator.Validate(file);
        foreach (var line in rep.Lines) Console.WriteLine(line);
        Console.WriteLine();
        allOk &= rep.Ok;
    }
    return allOk ? 0 : 1;
}

Scenario scenario;
if (args.Contains("--demo") || positional.Count == 0)
{
    Console.Error.WriteLine("(no scenario given — running built-in demo: Ironclad starter vs CalcifiedCultist)\n");
    scenario = Scenario.Demo();
}
else
{
    var json = File.ReadAllText(positional[0]);
    scenario = Scenario.FromJson(json);
}

var setup = scenario.Build();

Console.WriteLine($"Deck ({setup.Player.DrawPile.Count} cards): {DescribeDeck(setup.Player.DrawPile)}");
Console.WriteLine($"Player: {setup.Player.CurrentHp}/{setup.Player.MaxHp} HP, {setup.Player.MaxEnergy} energy"
                  + (setup.Player.Relics.Count > 0 ? $", relics: {string.Join(", ", setup.Player.Relics.Select(r => r.Id))}" : ""));
Console.WriteLine($"Encounter: {string.Join(" + ", setup.Monsters.Select(m => $"{m.Name}({m.CurrentHp} HP)"))}");
Console.WriteLine();

// ---------- Sampling/MCTS mode ----------
if (useMcts)
{
    var opt = new MctsOptions
    {
        Trials = ArgInt("--trials", 500_000),
        MaxTurns = scenario.MaxTurns,
        HybridExactBelow = ArgInt("--hybrid", 0),
    };
    var mcts = new MctsSolver(opt);
    Console.WriteLine($"=== Sampling/MCTS solver (UCT*: DP-UCT + Partial Bellman backups) ===");
    Console.WriteLine($"  Trials: {opt.Trials:N0}"
        + (opt.HybridExactBelow > 0 ? $", hybrid exact below size {opt.HybridExactBelow:N0}" : "")
        + $", seed {opt.Seed}");

    var swm = System.Diagnostics.Stopwatch.StartNew();
    Value mvalue;
    if (anytime)
    {
        mvalue = default;
        Console.WriteLine("  anytime curve:");
        foreach (var (t, v) in mcts.SolveAnytime(setup, Math.Max(1, opt.Trials / 10)))
        {
            mvalue = v;
            Console.WriteLine($"    {t,9:N0} trials  ->  win {v.Win:P2},  E[HP loss] {v.Loss:F2}");
        }
    }
    else mvalue = mcts.Solve(setup);
    swm.Stop();

    int mHeal = scenario.HealOnWin(setup);
    Console.WriteLine();
    Console.WriteLine($"  Win probability     : {mvalue.Win:P2}");
    Console.WriteLine($"  Expected HP loss    : {mvalue.Loss:F2}"
        + (mHeal > 0 ? $"  (net of post-combat heal: {Math.Max(0, mvalue.Loss - mvalue.Win * mHeal):F2})" : ""));
    Console.WriteLine($"  Nodes created       : {mcts.NodesCreated:N0}  in {swm.ElapsedMilliseconds} ms");
    return 0;
}

var solver = new Solver { MaxTurns = scenario.MaxTurns };
var sw = System.Diagnostics.Stopwatch.StartNew();
var value = solver.Solve(setup);
sw.Stop();

// Per-opening-hand breakdown (also yields a worst-case figure across openings).
var openings = solver.OpeningStates(setup, Player.CardsDrawnPerTurn).ToList();
var perHand = openings
    .GroupBy(o => HandKey(o.state))
    .Select(g =>
    {
        var prob = g.Sum(x => x.prob);
        var state = g.First().state;
        var v = solver.SolvePlayerTurn(state);
        return (handKey: g.Key, prob, value: v, state);
    })
    .OrderByDescending(h => h.prob)
    .ToList();

double worstCase = perHand.Count == 0 ? value.Loss : perHand.Max(h => h.value.Loss);
int healOnWin = scenario.HealOnWin(setup);

Console.WriteLine("=== Result (optimal play: maximise survival, then minimise HP loss) ===");
Console.WriteLine($"  Win probability     : {value.Win:P2}");
Console.WriteLine($"  Expected HP loss    : {value.Loss:F2}" + (healOnWin > 0 ? $"  (net of post-combat heal: {Math.Max(0, value.Loss - value.Win * healOnWin):F2})" : ""));
Console.WriteLine($"  Worst opening hand  : {worstCase:F2} expected HP loss");
Console.WriteLine($"  States evaluated    : {solver.StatesEvaluated:N0}  in {sw.ElapsedMilliseconds} ms");
Console.WriteLine();

if (showLines)
{
    Console.WriteLine("=== Optimal opening line per opening hand ===");
    foreach (var h in perHand)
    {
        var plan = solver.BestTurnPlan(h.state);
        Console.WriteLine($"  [{h.prob:P1}] {h.handKey}");
        Console.WriteLine($"          -> {string.Join(" | ", plan)}   ({h.value})");
    }
}

return 0;

static string DescribeDeck(IEnumerable<CardModel> cards) =>
    string.Join(", ", cards.GroupBy(c => c.StateKey()).Select(g => $"{g.Count()}x {g.Key}"));

static string HandKey(CombatState s) =>
    string.Join(", ", s.Player.Hand.GroupBy(c => c.StateKey())
        .OrderBy(g => g.Key).Select(g => $"{g.Count()}x {g.Key}"));


// ---------- Scenario model ----------

sealed class Scenario
{
    public int PlayerHp { get; set; } = 80;
    public int PlayerMaxHp { get; set; } = 80;
    public int MaxEnergy { get; set; } = 3;
    public int MaxTurns { get; set; } = 30;
    public int Ascension { get; set; } = 10;   // default to max ascension; scales monster HP/damage
    public string? DeckPreset { get; set; }
    public List<string> Deck { get; set; } = new();
    public List<string> Relics { get; set; } = new();
    public List<string> Monsters { get; set; } = new();

    public static Scenario Demo() => new()
    {
        DeckPreset = "ironclad-starter",
        Relics = { "BurningBlood" },
        Monsters = { "CalcifiedCultist" },
    };

    public static Scenario FromJson(string json) =>
        JsonSerializer.Deserialize<Scenario>(json, new JsonSerializerOptions { PropertyNameCaseInsensitive = true })
        ?? throw new ArgumentException("Could not parse scenario JSON.");

    public CombatState Build()
    {
        var deck = DeckPreset?.ToLowerInvariant() == "ironclad-starter"
            ? Catalog.IroncladStarterDeck()
            : Deck.Select(Catalog.BuildCard).ToList();
        if (deck.Count == 0) throw new ArgumentException("Scenario has an empty deck.");
        var player = Catalog.BuildPlayer(deck, PlayerHp, PlayerMaxHp, MaxEnergy, Relics);
        var monsters = Monsters.Select(m => Catalog.BuildMonster(m, Ascension)).ToList();
        if (monsters.Count == 0) throw new ArgumentException("Scenario has no monsters.");
        return Catalog.SetupCombat(player, monsters);
    }

    public int HealOnWin(CombatState setup) => setup.Player.Relics.Any(r => r.Id == "BurningBlood") ? 6 : 0;
}
