using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;
using Sts2Solver.Ranwid;

// ranwid — the seer. Watches your live (unmodded) Slay the Spire 2 run and, for each elite that can
// appear in the current Act, reports the solver's survival probability + HP-loss distribution.
//
//   ranwid                         watch the newest ongoing unmodded run, refresh on save change
//   ranwid --once                  evaluate once and exit
//   ranwid --save <current_run.save>   read a specific save file (offline)
//   --advise                       also rank single-card removals by bottleneck-elite survival (slower)
//   --player <net_id>  pick a multiplayer slot
//   --budget-seconds <s>  exact-search cap before MCTS fallback (default 8; raise for exact precision)
//   --rollouts <n>  playouts for the distribution (default 2000)   --trials <n>  MCTS fallback trials (default 40000)
//   --seed <n>

string? ArgVal(string flag) { int i = Array.IndexOf(args, flag); return i >= 0 && i + 1 < args.Length ? args[i + 1] : null; }
int? ArgInt(string flag) => int.TryParse(ArgVal(flag), out var v) ? v : null;
double? ArgDouble(string flag) => double.TryParse(ArgVal(flag), out var v) ? v : null;

bool once = args.Contains("--once");
bool advise = args.Contains("--advise");
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
    return EvaluateAndPrint(path, playerNetId, opts);
}

// Watch mode: re-locate the newest unmodded save each tick and re-evaluate when it changes.
Console.WriteLine("ranwid: watching for the newest ongoing unmodded run… (Ctrl-C to stop)");
string? lastKey = null;
while (true)
{
    var path = saveArg ?? SaveLocator.FindNewest();
    if (path == null)
    {
        if (lastKey != "none") { Console.WriteLine("ranwid: no ongoing unmodded run found — waiting…"); lastKey = "none"; }
    }
    else
    {
        var key = $"{path}|{File.GetLastWriteTimeUtc(path).Ticks}";
        if (key != lastKey)
        {
            lastKey = key;
            try { EvaluateAndPrint(path, playerNetId, opts); }
            catch (Exception ex) { Console.Error.WriteLine($"ranwid: failed on '{path}': {ex.Message}"); }
        }
    }
    Thread.Sleep(1500);
}

// ---------- core ----------

int EvaluateAndPrint(string path, int? netId, EvalOptions evalOpts)
{
    RunState run;
    try { run = RunSaveReader.Parse(path, netId); }
    catch (Exception ex) { Console.Error.WriteLine($"ranwid: could not parse '{path}': {ex.Message}"); return 1; }

    var warnings = new List<string>();
    string deckSummary = DeckSummary(run.Deck);

    if (!GameIds.IsIroncladCharacter(run.Character))
    {
        warnings.Add($"only Ironclad is supported for now — character is {GameIds.CharacterName(run.Character)}; no fight evaluated.");
        Reporting.Print(run, path, Array.Empty<EliteResult>(), warnings, deckSummary);
        return 0;
    }

    // Build the deck (cards are immutable, so one list is safely reused across encounters). Also collect the
    // ported card specs (name+upgrade strings) for the advice engine, which rebuilds decks per variant.
    var cards = new List<CardModel>();
    var deckSpecs = new List<string>();
    foreach (var e in run.Deck)
    {
        if (e.HasEnchant) warnings.Add($"enchantment on {GameIds.ClassName(e.Id)} ignored (not modelled)");
        var spec = GameIds.CardSpec(e.Id, e.Upgrade);
        try { cards.Add(Catalog.BuildCard(spec)); deckSpecs.Add(spec); }
        catch (ArgumentException) { warnings.Add($"card {GameIds.ClassName(e.Id)} skipped (not ported)"); }
    }

    var relicNames = new List<string>();
    foreach (var rid in run.RelicIds)
    {
        var name = GameIds.ModelledRelicName(rid);
        if (name != null) relicNames.Add(name);
        else warnings.Add($"relic {GameIds.ClassName(rid)} ignored (only Burning Blood is modelled)");
    }

    var results = new List<EliteResult>();
    foreach (var eid in run.EliteEncounterIds)
    {
        var cls = GameIds.EncounterClassName(eid);
        string disp = cls.EndsWith("Elite", StringComparison.Ordinal) ? cls[..^5] : cls;
        if (!Catalog.IsKnownEliteEncounter(cls))
        {
            results.Add(new EliteResult(disp, "", null, "encounter not ported"));
            continue;
        }
        var monsters = Catalog.BuildEliteEncounter(cls, run.Ascension);
        string comp = string.Join(" + ", monsters.GroupBy(m => m.Name)
            .Select(g => g.Count() > 1 ? $"{g.Count()}× {g.Key}" : g.Key));
        if (cards.Count == 0)
        {
            results.Add(new EliteResult(disp, comp, null, "no playable deck cards"));
            continue;
        }
        var player = Catalog.BuildPlayer(cards, run.PlayerHp, run.PlayerMaxHp, run.MaxEnergy, relicNames);
        var setup = Catalog.SetupCombat(player, monsters);
        var stats = EncounterEvaluator.Evaluate(setup, evalOpts);
        results.Add(new EliteResult(disp, comp, stats, null));
    }

    Reporting.Print(run, path, results, warnings, deckSummary);

    if (advise)
    {
        var encounters = new List<Advisor.Encounter>();
        foreach (var eid in run.EliteEncounterIds)
        {
            var cls = GameIds.EncounterClassName(eid);
            if (!Catalog.IsKnownEliteEncounter(cls)) continue;
            string disp = cls.EndsWith("Elite", StringComparison.Ordinal) ? cls[..^5] : cls;
            int asc = run.Ascension;
            encounters.Add(new Advisor.Encounter(disp, () => Catalog.BuildEliteEncounter(cls, asc)));
        }
        if (encounters.Count == 0 || deckSpecs.Count <= 1)
            Console.WriteLine("\n(advice needs ≥1 ported elite and ≥2 ported deck cards — skipped.)");
        else
        {
            Console.WriteLine($"\nComputing card-removal advice over {encounters.Count} elite(s)…");
            var advice = Advisor.RemovalAdvice(deckSpecs, encounters,
                run.PlayerHp, run.PlayerMaxHp, run.MaxEnergy, relicNames, evalOpts);
            Console.WriteLine(Advisor.Format(advice));
        }
    }
    return 0;
}

static string DeckSummary(IReadOnlyList<CardEntry> deck) =>
    deck.Count == 0 ? "(empty)" :
    string.Join(", ", deck
        .GroupBy(e => GameIds.CardSpec(e.Id, e.Upgrade))
        .OrderByDescending(g => g.Count()).ThenBy(g => g.Key, StringComparer.Ordinal)
        .Select(g => $"{g.Count()}x {g.Key}"));
