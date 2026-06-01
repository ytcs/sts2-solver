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

// --horizon: print the sound horizon bound derived for each calibration fixture (defaultMax 40), and
// validate exact-value equality between the auto-horizon and the large default (soundness sanity).
if (args.Contains("--horizon"))
{
    int big = ArgInt("--maxturns", 40);
    Console.WriteLine($"Horizon bound vs default {big} (exact value must be identical when bounded)\n");
    Console.WriteLine($"  {"fixture",-28} {"deck",4} {"bound",6}  {"exact@bound",-22} {"exact@big",-22} ok");
    Console.WriteLine("  " + new string('-', 92));
    foreach (var f in CalibrationFixtures.All)
    {
        int deck = f.Setup().Player.DrawPile.Count;
        int bound = HorizonBound.Compute(f.Setup(), big);
        var atBound = CalibrationHarness.RunExactBudgeted(f.Setup(), bound, 90);
        var atBig = CalibrationHarness.RunExactBudgeted(f.Setup(), big, 90);
        string vb = atBound == null ? "(timeout)" : $"{atBound.Survival:P1}/{atBound.Loss:F1} {atBound.Ms}ms";
        string vg = atBig == null ? "(timeout)" : $"{atBig.Survival:P1}/{atBig.Loss:F1} {atBig.Ms}ms";
        string ok = (atBound != null && atBig != null)
            ? (Math.Abs(atBound.Survival - atBig.Survival) < 1e-9 && Math.Abs(atBound.Loss - atBig.Loss) < 1e-6 ? "YES" : "*** NO ***")
            : "?";
        Console.WriteLine($"  {f.Name,-28} {deck,4} {bound,6}  {vb,-22} {vg,-22} {ok}");
    }
    return 0;
}

// --objective: the survival-vs-scalar objective experiment. For each calibration fixture, solve under BOTH
// the lexicographic objective (survival-first, the oracle) and the single-scalar objective (minimise E[HP loss],
// death = full remaining HP), then cross-evaluate each policy under the other metric. Decisive, noise-free data
// on the open question "drop survival probability?": ScalarRegret (HP the survival-first policy wastes) and
// SurvivalSacrifice (wins the loss-minimising policy throws away). Flags: --exact-budget S (default 90).
if (args.Contains("--objective"))
{
    double budget = ArgInt("--exact-budget", 90);
    Console.WriteLine("Objective experiment — lexicographic (survival-first) vs single scalar (min E[HP loss], "
        + "death = full HP), exact oracle.\n");
    Console.WriteLine($"  {"fixture",-28} {"lex win/loss",-16} {"scalarOpt",9} {"lexPolScal",10} "
        + $"{"regret",7} {"scalPolWin",10} {"sacrifice",9}  {"states L/S",-18} {"ms L/S"}");
    Console.WriteLine("  " + new string('-', 124));

    double sumRegret = 0, sumSacrifice = 0, maxRegret = 0, maxSacrifice = 0;
    foreach (var f in CalibrationFixtures.All)
    {
        ObjectiveRow r;
        try
        {
            using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(budget * 3));
            r = ObjectiveExperiment.Run(f.Name, f.Setup(), f.MaxTurns);
        }
        catch (OperationCanceledException) { Console.WriteLine($"  {f.Name,-28} (exceeded budget — skipped)"); continue; }

        Console.WriteLine(
            $"  {r.Fixture,-28} {r.WinLex,6:P1}/{r.LossLex,6:F1}   {r.ScalarOpt,9:F2} {r.ScalarOfLexPolicy,10:F2} "
            + $"{r.ScalarRegret,7:F2} {r.WinOfScalarPolicy,10:P1} {r.SurvivalSacrifice,9:P1}  "
            + $"{r.StatesLex,8:N0}/{r.StatesScalar,-8:N0} {r.MsLex}/{r.MsScalar}");
        sumRegret += r.ScalarRegret; sumSacrifice += r.SurvivalSacrifice;
        maxRegret = Math.Max(maxRegret, r.ScalarRegret); maxSacrifice = Math.Max(maxSacrifice, r.SurvivalSacrifice);
    }
    Console.WriteLine("  " + new string('-', 124));
    Console.WriteLine($"  Σ scalar-regret {sumRegret:F2} HP (max {maxRegret:F2})  |  "
        + $"Σ survival-sacrifice {sumSacrifice:P1} (max {maxSacrifice:P1})");
    Console.WriteLine("\n  regret≈0 ⇒ survival-first policy is already loss-optimal (scalar changes no HP outcomes);");
    Console.WriteLine("  sacrifice≈0 ⇒ dropping survival is SAFE (loss-min policy keeps the same wins).");
    return 0;
}

// --objective-random N: the objective experiment over a broad RANDOM draw of fixtures (decks of varying size /
// composition from the whole card pool × random monster × HP). The decisive test: the lex vs scalar objectives
// can only diverge in the PARTIAL-SURVIVAL regime (0 < win < 1), so we report the regret/sacrifice distribution
// conditioned on that band. Flags: --count N (default 200), --seed, --maxturns, --budget S (per-fixture, default 8).
if (args.Contains("--objective-random"))
{
    int count = ArgInt("--count", 200);
    int seed = ArgInt("--seed", 20260601);
    int maxTurns = ArgInt("--maxturns", 14);
    double budget = ArgInt("--budget", 8);
    Console.WriteLine($"Objective experiment — RANDOM corpus ({count} draws, seed {seed}, {budget:F0}s/fixture). "
        + "Divergence is only possible at partial survival (0<win<1).\n");

    int solved = 0, timedOut = 0, partial = 0, diverged = 0, engineFail = 0;
    double sumRegret = 0, sumSacrifice = 0, maxRegret = 0, maxSacrifice = 0;
    string worstRegretFx = "-", worstSacrificeFx = "-";
    foreach (var f in TrainingFixtures.Random(count, seed, maxTurns))
    {
        ObjectiveRow r;
        try
        {
            using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(budget));
            r = ObjectiveExperiment.Run(f.Name, f.Setup(), f.MaxTurns, cts.Token);
        }
        catch (OperationCanceledException) { timedOut++; continue; }
        catch (InvalidOperationException ex)   // engine can't model this deck soundly (e.g. clone/StateKey bug)
        {
            engineFail++;
            if (engineFail <= 5) Console.WriteLine($"  ENGINE-FAIL {f.Name,-34} {ex.Message}");
            continue;
        }
        solved++;

        bool isPartial = r.WinLex > 1e-6 && r.WinLex < 1 - 1e-6;
        if (!isPartial) continue;
        partial++;
        sumRegret += r.ScalarRegret; sumSacrifice += r.SurvivalSacrifice;
        if (r.ScalarRegret > maxRegret) { maxRegret = r.ScalarRegret; worstRegretFx = f.Name; }
        if (r.SurvivalSacrifice > maxSacrifice) { maxSacrifice = r.SurvivalSacrifice; worstSacrificeFx = f.Name; }
        // A meaningful divergence: the scalar policy gives up >1% survival, OR the survival-first policy wastes >0.5 HP.
        if (r.SurvivalSacrifice > 0.01 || r.ScalarRegret > 0.5)
        {
            diverged++;
            Console.WriteLine($"  DIVERGE {f.Name,-34} lexWin {r.WinLex,6:P1} loss {r.LossLex,6:F1} | "
                + $"scalarOpt {r.ScalarOpt,6:F2} regret {r.ScalarRegret,6:F2} | scalPolWin {r.WinOfScalarPolicy,6:P1} "
                + $"sacrifice {r.SurvivalSacrifice,6:P1}");
        }
    }
    Console.WriteLine("\n  " + new string('-', 80));
    Console.WriteLine($"  solved {solved}/{count} ({timedOut} timed out @ {budget:F0}s, {engineFail} engine-fail), "
        + $"partial-survival (0<win<1): {partial}");
    if (partial > 0)
    {
        Console.WriteLine($"  divergences (sacrifice>1% or regret>0.5HP): {diverged}/{partial}");
        Console.WriteLine($"  mean scalar-regret {sumRegret / partial:F3} HP (max {maxRegret:F2} @ {worstRegretFx})");
        Console.WriteLine($"  mean survival-sacrifice {sumSacrifice / partial:P2} (max {maxSacrifice:P1} @ {worstSacrificeFx})");
    }
    return 0;
}

// --objective-penalty: on PARTIAL-SURVIVAL fixtures (where the objectives can diverge), sweep the scalar
// objective's death penalty P (charged on top of the lost bar) as multiples of max HP, and measure how the
// scalar-P policy's survival recovers toward the lexicographic optimum — and at what extra-HP-loss cost. P=0 is
// the pure "maximise expected final HP" scalar; a huge P must reproduce lexicographic survival (validation).
// This is the constrained-MDP penalty / big-M view of survival-first. Flags: --count, --seed, --budget, --want K.
if (args.Contains("--objective-penalty"))
{
    int count = ArgInt("--count", 400);
    int seed = ArgInt("--seed", 4242);
    int maxTurns = ArgInt("--maxturns", 14);
    double budget = ArgInt("--budget", 10);
    int want = ArgInt("--want", 10);                 // stop after this many partial-survival fixtures
    double[] mults = { 0.0, 0.5, 1.0, 2.0, 5.0, 1e6 };

    Console.WriteLine($"Objective death-penalty sweep on partial-survival fixtures (seed {seed}, {budget:F0}s/solve). "
        + "P as ×maxHP; P=0 ⇒ max-E[final HP]; P→∞ ⇒ lexicographic.\n");

    int found = 0, examined = 0;
    foreach (var f in TrainingFixtures.Random(count, seed, maxTurns))
    {
        if (found >= want) break;
        examined++;
        int maxHp = f.Setup().Player.MaxHp;

        // Lex baseline first; only fixtures with 0<win<1 can show divergence.
        Value vLex; int lexStates;
        try
        {
            using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(budget));
            var lex0 = new Solver { MaxTurns = f.MaxTurns, Ct = cts.Token };
            vLex = lex0.Solve(f.Setup());
            lexStates = lex0.StatesEvaluated;
        }
        catch (OperationCanceledException) { continue; }
        catch (InvalidOperationException) { continue; }
        if (vLex.Win <= 1e-6 || vLex.Win >= 1 - 1e-6) continue;
        found++;

        Console.WriteLine($"  {f.Name}   (maxHP {maxHp}, {lexStates:N0} states)");
        Console.WriteLine($"    lexicographic:  survive {vLex.Win,6:P1}   E[HP loss] {vLex.Loss,6:F2}");
        foreach (double mult in mults)
        {
            double P = mult * maxHp;
            try
            {
                using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(budget));
                var scal = new ScalarSolver { MaxTurns = f.MaxTurns, DeathPenalty = P, Ct = cts.Token };
                scal.Solve(f.Setup());
                scal.Ct = CancellationToken.None;
                var pv = ObjectiveExperiment.EvaluatePolicy(f.Setup(), f.MaxTurns, scal.BestAction);
                string tag = mult >= 1e5 ? "P=∞ " : $"P={mult:0.0}×";
                Console.WriteLine($"    scalar {tag,-6} survive {pv.Win,6:P1}   E[HP loss] {pv.LexLoss,6:F2}   "
                    + $"Δsurv {pv.Win - vLex.Win,+6:P1}   Δloss {pv.LexLoss - vLex.Loss,+6:F2}");
            }
            catch (OperationCanceledException) { Console.WriteLine($"    scalar P={mult:0.0}× (timeout)"); }
        }
        Console.WriteLine();
    }
    Console.WriteLine($"  examined {examined} draws, {found} partial-survival fixtures swept.");
    return 0;
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

// --calibrate: run exact (ground truth) vs MCTS (rollout-leaf and heuristic-leaf) over the DIVERSE fixture
// suite (CalibrationFixtures — block / strength / debuff / aggro / power archetypes, not just the starter)
// and report how closely sampling tracks exact. Used to tune the shared CombatHeuristic against the oracle.
// Flags: --trials N (default 40000), --exact-budget S (per-fixture exact cap, default 90s), --archetype X.
if (args.Contains("--calibrate"))
{
    int trials = ArgInt("--trials", 40_000);
    double exactBudget = ArgInt("--exact-budget", 90);
    string? onlyArch = args.SkipWhile(a => a != "--archetype").Skip(1).FirstOrDefault();

    var fixtures = CalibrationFixtures.All
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
