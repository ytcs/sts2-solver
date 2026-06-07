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

// --bridge: the accuracy+latency instrument for the regime Ranwid actually runs in. Walks a deck-size LADDER
// (CalibrationFixtures.Bridge) straddling the exact-tractability boundary and, per rung, reports:
//   • truth     — exact value if it finishes within --exact-budget, else an MCTS@--proxy-trials proxy ("40k").
//   • m@2k       — MCTS at the ADVICE trial budget (--trials), as mean±stddev of survival & loss over --seeds
//                  seeds. The survival stddev is the NOISE FLOOR — the empirical basis for Advisor.SurvivalBand.
//   • Δ vs truth — bias of the 2k advice estimate against the best available truth.
//   • eval-ms    — wall-clock of EncounterEvaluator.Evaluate (the REAL per-evaluation advice cost, incl. the
//                  doomed exact attempt) vs raw m@2k ms. Advice runs this 100s of times, so eval-ms IS the
//                  number; the gap eval−m2k is the speed prize a tractability gate would reclaim.
// Flags: --trials (2000) --proxy-trials (20000) --seeds (6) --exact-budget S (30) --maxturns (12) --sizes a,b,c
if (args.Contains("--bridge"))
{
    int adviceTrials = ArgInt("--trials", 2_000);
    int proxyTrials = ArgInt("--proxy-trials", 20_000);
    int nSeeds = ArgInt("--seeds", 6);
    double exactBudget = ArgInt("--exact-budget", 30);
    // Only override the size-derived (short-race-at-small-sizes) horizon when --maxturns is given explicitly,
    // so the small rungs stay exact-anchorable by default.
    int? brTurns = args.Contains("--maxturns") ? ArgInt("--maxturns", 12) : null;
    string? sizesArg = args.SkipWhile(a => a != "--sizes").Skip(1).FirstOrDefault();
    int[] sizes = sizesArg != null
        ? sizesArg.Split(',').Select(s => int.Parse(s.Trim())).ToArray()
        : CalibrationFixtures.BridgeSizes;
    var fixtures = sizes.Select(s => CalibrationFixtures.BridgeRung(s, brTurns)).ToList();
    // The size ladder is winnable by construction (its job is self-consistency + speed as size→real regime), so
    // its seed-noise is ~0 and uninformative for SurvivalBand. Append the already-tested CONTESTED fixtures —
    // the ~92% block fight and the windup-burst MechaKnight — whose survival is genuinely uncertain; THEIR seed
    // stddev is the noise floor that matters. They're exact-tractable, so they also carry a true Δ-vs-exact.
    if (!args.Contains("--no-contested"))
        fixtures.AddRange(new[]
        {
            CalibrationFixtures.All.First(f => f.Archetype == "block"),
            CalibrationFixtures.EliteSweep.First(f => f.Name.Contains("MechaKnight")),
            CalibrationFixtures.BridgeContestedLarge(),   // large + contested: the noise floor where it's worst
        });
    var seeds = Enumerable.Range(1, nSeeds).ToArray();

    Console.WriteLine($"Bridge instrument — advice budget {adviceTrials:N0} trials over {nSeeds} seeds, "
        + $"proxy-truth {proxyTrials:N0} trials, exact budget {exactBudget:F0}s/rung, "
        + $"horizon {(brTurns.HasValue ? brTurns.Value.ToString() : "size-derived")}.");
    Console.WriteLine("  truth = exact if it finishes, else MCTS proxy. eval-ms = real EncounterEvaluator cost "
        + "(incl. doomed exact attempt).\n");
    Console.WriteLine($"  {"rung",-20} {"truth",-14} {"m@2k surv",13} {"m@2k loss",13} {"Δsurv",6} {"Δloss",6} "
        + $"{"eval-ms",8} {"m2k-ms",7} {"exact",7}");
    Console.WriteLine("  " + new string('-', 110));

    double maxSurvStd = 0, maxSurvBias = 0; double evalMsSum = 0, m2kMsSum = 0; int rungs = 0;
    foreach (var f in fixtures)
    {
        var ex = CalibrationHarness.RunExactBudgeted(f.Setup(), f.MaxTurns, exactBudget);
        var proxy = CalibrationHarness.RunMcts(f.Setup(), f.MaxTurns, proxyTrials, seed: 1);
        var ss = CalibrationHarness.RunMctsSeeds(() => f.Setup(), f.MaxTurns, adviceTrials, seeds);

        // The real advice cost: EncounterEvaluator at the advice budget (8s exact attempt → MCTS → rollouts).
        var stats = EncounterEvaluator.Evaluate(f.Setup(), new EvalOptions
        {
            MaxTurns = f.MaxTurns, MctsTrials = adviceTrials, Seed = 1,
        });

        double truthSurv = ex?.Survival ?? proxy.Survival;
        double truthLoss = ex?.Loss ?? proxy.Loss;
        string truthSrc = ex != null ? "exact" : $"~{proxyTrials / 1000}k";
        double dSurv = ss.SurvMean - truthSurv;
        double dLoss = ss.LossMean - truthLoss;

        Console.WriteLine($"  {f.Name,-20} {truthSurv,7:P1}/{truthLoss,4:F0}({truthSrc,-5}) "
            + $"{ss.SurvMean,6:P1}±{ss.SurvStd,5:P1} {ss.LossMean,6:F1}±{ss.LossStd,5:F1} "
            + $"{dSurv,+6:P1} {dLoss,+6:F1} {stats.ElapsedMs,8:N0} {ss.MsMean,7:F0} "
            + $"{(ex != null ? $"{ex.Ms,5:N0}ms" : "  DNF")}");

        maxSurvStd = Math.Max(maxSurvStd, ss.SurvStd);
        maxSurvBias = Math.Max(maxSurvBias, Math.Abs(dSurv));
        evalMsSum += stats.ElapsedMs; m2kMsSum += ss.MsMean; rungs++;
    }

    Console.WriteLine();
    Console.WriteLine($"  SurvivalBand evidence (current band 5.0%):");
    Console.WriteLine($"    • noise (max seed stddev @ {adviceTrials:N0} trials): {maxSurvStd:P2}  "
        + "— survival is essentially seed-stable (DP-UCT backs up TRUE probabilities).");
    Console.WriteLine($"    • bias  (max |m@{adviceTrials / 1000}k − best-truth|):    {maxSurvBias:P2}  "
        + "— the convergence gap vs more trials (pessimistic direction); the band need only cover THIS.");
    Console.WriteLine($"  Mean advice cost: {evalMsSum / rungs:N0} ms/eval (EncounterEvaluator) vs "
        + $"{m2kMsSum / rungs:N0} ms (raw m@2k) ⇒ ~{(evalMsSum - m2kMsSum) / rungs:N0} ms/eval reclaimable "
        + "by skipping the doomed exact attempt.");
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
    Console.WriteLine($"  Clone microbench: {nsPerClone:F0} ns/clone ({size}-card deck state).");

    // 1b) decimal-vs-double microbench of the damage-pipeline arithmetic (Strength add → Vulnerable ×1.5 →
    //     Weak ×0.75 → floor). Bounds the CEILING of a decimal→double conversion: the per-Attack arithmetic
    //     saving × Attack calls per solve. (Faithfulness aside: 0.7m/0.1m multipliers aren't binary-exact, so a
    //     real conversion would need scaled-int — this only sizes the prize.)
    const int aiters = 20_000_000;
    decimal dsink = 0m; double fsink = 0;
    for (int i = 0; i < 1_000_000; i++) { decimal a = 6 + (i & 7); a += 3m; a *= 1.5m; a *= 0.75m; dsink += Math.Floor(a); }   // warm
    var swd = System.Diagnostics.Stopwatch.StartNew();
    for (int i = 0; i < aiters; i++) { decimal a = 6 + (i & 7); a += 3m; a *= 1.5m; a *= 0.75m; dsink += Math.Floor(a); }
    swd.Stop();
    var swf = System.Diagnostics.Stopwatch.StartNew();
    for (int i = 0; i < aiters; i++) { double a = 6 + (i & 7); a += 3; a *= 1.5; a *= 0.75; fsink += Math.Floor(a); }
    swf.Stop();
    double nsDec = swd.Elapsed.TotalMilliseconds * 1e6 / aiters, nsDbl = swf.Elapsed.TotalMilliseconds * 1e6 / aiters;
    Console.WriteLine($"  Damage-arith microbench: decimal {nsDec:F1} ns/op vs double {nsDbl:F1} ns/op "
        + $"(save ~{nsDec - nsDbl:F1} ns/Attack) [sink {dsink + (decimal)fsink:F0}]\n");

    // 2) Cost attribution at a fixed (small) trial count, all on the faithful greedy-rollout leaf (the only
    //    leaf). Vary the APW prior on/off and the chance-enumeration threshold to attribute time:
    //    (APW)−(UCT) gap = the APW prior's per-candidate Score(ApplyPlay) cost; (ch4096)−(ch64) gap = the
    //    chance-node draw-enumeration cost. Whatever's left is the rollout playout itself.
    int t = ArgInt("--trials", 1_500);
    Console.WriteLine($"  Cost matrix @ {t:N0} trials (faithful rollout leaf; vary the APW prior + chance):\n");
    Console.WriteLine($"  {"config",-18} {"ms",9} {"ms/trial",9} {"nodes",9} {"clones",13} {"clone%",7}  value");
    var leafConfigs = new (string name, Action<MctsOptions> set)[]
    {
        ("APW roll ch4096", _ => { }),                                  // production default
        ("UCT roll ch4096", o => o.ActionWidening = false),             // APW off → APW-prior cost
        ("APW roll ch64",   o => o.ExactChanceThreshold = 64),          // DPW sampling → chance-enum cost
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
    Console.WriteLine($"\n  (APW roll)−(UCT roll) gap = APW-prior cost; (ch4096)−(ch64) gap = chance-enumeration");
    Console.WriteLine($"  cost; the remainder is the rollout playout itself (the dominant term — ~72%).");
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
    bool regret = args.Contains("--regret");   // also measure greedy-leaf policy regret vs the oracle (per fixture)
    IEnumerable<CalibrationFixtures.Fixture> source =
        randomN > 0 ? CalibrationFixtures.RandomDecks(randomN, seed)
        : args.Contains("--percharacter") ? CalibrationFixtures.PerCharacter   // per-class mechanic fixtures (Osty/poison/orbs/…)
        : args.Contains("--elites") ? CalibrationFixtures.All.Concat(CalibrationFixtures.EliteSweep)
        : CalibrationFixtures.All;
    var fixtures = source
        .Where(f => onlyArch == null || f.Archetype == onlyArch).ToList();

    Console.WriteLine($"Calibration — exact (ground truth) vs heuristic-guided MCTS, {trials:N0} trials, "
        + $"exact budget {exactBudget:F0}s/fixture\n");
    Console.WriteLine($"  {"fixture",-28} {"engine",-10} {"survive",8} {"loss",7} {"Δsurv",7} {"Δloss",7} {"work",12} {"time",7}");
    Console.WriteLine("  " + new string('-', 96));

    double sumDSurvRoll = 0, sumDLossRoll = 0;
    int graded = 0;
    foreach (var f in fixtures)
    {
        var exact = CalibrationHarness.RunExactBudgeted(f.Setup(), f.MaxTurns, exactBudget);
        var roll = CalibrationHarness.RunMcts(f.Setup(), f.MaxTurns, trials, seed: 1);

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

        // Policy regret (greedy leaf vs oracle): how much the heuristic's OWN turn loses vs optimal. Isolates the
        // leaf policy (exact plays the real engine) — the per-character heuristic-quality signal the redesign drives down.
        if (regret)
        {
            var pr = CalibrationHarness.MeasurePolicyRegret(f.Setup(), f.MaxTurns, exactBudget);
            Console.WriteLine(pr == null
                ? $"  {f.Name,-28} {"regret",-10} (exact > budget — no oracle)"
                : $"  {f.Name,-28} {"regret",-10} {"",8} {"",7} {pr.WinRegret,7:P1} {pr.LossRegret,7:F2}   (greedy win {pr.GreedyWin:P0} loss {pr.GreedyLoss:F1} vs opt loss {pr.OptLoss:F1})");
        }

        if (exact != null)
        {
            graded++;
            sumDSurvRoll += Math.Abs(roll.Survival - exact.Survival);
            sumDLossRoll += Math.Abs(roll.Loss - exact.Loss);
        }
        Console.WriteLine();
    }

    if (graded > 0)
    {
        Console.WriteLine($"  Mean abs error over {graded} graded fixture(s):");
        Console.WriteLine($"    mcts : Δsurv {sumDSurvRoll / graded:P1}   Δloss {sumDLossRoll / graded:F2}   (faithful rollout leaf)");
    }
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
