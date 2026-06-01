using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;

namespace Sts2Solver.Ranwid;

/// <summary>
/// The live companion: ranwid as a persistent session, not a one-shot command. It loads the ongoing run,
/// prints the deck + per-elite stats and the best card to remove, then drops into a prompt where — as you
/// play — you can type the cards a reward screen offers (with Tab auto-complete and typo auto-correct, via
/// <see cref="CardNameMatcher"/>) and get a take-vs-skip verdict against the current Act's elites, re-run cut
/// advice, or refresh. A background watcher notices when the save changes (you advanced a screen) and the next
/// prompt refreshes against the new state — no restart.
/// </summary>
public sealed class Companion
{
    private readonly int? _netId;
    private readonly EvalOptions _opts;
    private readonly string? _fixedPath;          // when started with --save, watch this exact file
    private volatile bool _dirty;
    private string? _loadedKey;                   // path|mtime of the currently-loaded save

    public Companion(string? fixedPath, int? netId, EvalOptions opts)
    {
        _fixedPath = fixedPath; _netId = netId; _opts = opts;
    }

    // ── Loaded run context ─────────────────────────────────────────────────────

    public sealed record Context(
        RunState Run, string Path, List<string> DeckSpecs, List<string> RelicNames,
        List<Advisor.Encounter> Encounters, List<(string name, string comp)> EliteInfo,
        List<string> Warnings, string DeckSummary);

    /// <summary>Parse a save and resolve the deck/relics/elite encounters the advisor needs. Returns null
    /// (with a console message) for non-Ironclad or unreadable runs.</summary>
    public static Context? Load(string path, int? netId)
    {
        RunState run;
        try { run = RunSaveReader.Parse(path, netId); }
        catch (Exception ex) { Console.Error.WriteLine($"ranwid: could not parse '{path}': {ex.Message}"); return null; }

        var warnings = new List<string>();
        if (!GameIds.IsIroncladCharacter(run.Character))
        {
            Console.WriteLine($"ranwid: only Ironclad is supported for now (run is {GameIds.CharacterName(run.Character)}).");
            return null;
        }

        var deckSpecs = new List<string>();
        foreach (var e in run.Deck)
        {
            if (e.HasEnchant) warnings.Add($"enchantment on {GameIds.ClassName(e.Id)} ignored (not modelled)");
            var spec = GameIds.CardSpec(e.Id, e.Upgrade);
            try { Catalog.BuildCard(spec); deckSpecs.Add(spec); }
            catch (ArgumentException) { warnings.Add($"card {GameIds.ClassName(e.Id)} skipped (not ported)"); }
        }

        var relicNames = new List<string>();
        foreach (var rid in run.RelicIds)
        {
            var name = GameIds.ModelledRelicName(rid);
            if (name != null) relicNames.Add(name);
            else warnings.Add($"relic {GameIds.ClassName(rid)} ignored (only Burning Blood is modelled)");
        }

        var encounters = new List<Advisor.Encounter>();
        var eliteInfo = new List<(string, string)>();
        foreach (var eid in run.EliteEncounterIds)
        {
            var cls = GameIds.EncounterClassName(eid);
            if (!Catalog.IsKnownEliteEncounter(cls)) continue;
            string disp = cls.EndsWith("Elite", StringComparison.Ordinal) ? cls[..^5] : cls;
            int asc = run.Ascension;
            encounters.Add(new Advisor.Encounter(disp, () => Catalog.BuildEliteEncounter(cls, asc)));
            string comp = string.Join(" + ", Catalog.BuildEliteEncounter(cls, asc)
                .GroupBy(m => m.Name).Select(g => g.Count() > 1 ? $"{g.Count()}× {g.Key}" : g.Key));
            eliteInfo.Add((disp, comp));
        }

        string deckSummary = run.Deck.Count == 0 ? "(empty)" : string.Join(", ", run.Deck
            .GroupBy(e => GameIds.CardSpec(e.Id, e.Upgrade))
            .OrderByDescending(g => g.Count()).ThenBy(g => g.Key, StringComparer.Ordinal)
            .Select(g => $"{g.Count()}x {g.Key}"));

        return new Context(run, path, deckSpecs, relicNames, encounters, eliteInfo, warnings, deckSummary);
    }

    // ── Reporting ───────────────────────────────────────────────────────────────

    /// <summary>Print the per-elite stats for the current deck (the "all stats for the current deck" view).</summary>
    public static void ReportDeck(Context c, EvalOptions opts)
    {
        var results = new List<EliteResult>();
        foreach (var (enc, info) in c.Encounters.Zip(c.EliteInfo))
        {
            if (c.DeckSpecs.Count == 0) { results.Add(new EliteResult(info.name, info.comp, null, "no playable deck cards")); continue; }
            var deck = c.DeckSpecs.Select(Catalog.BuildCard).ToList();
            var player = Catalog.BuildPlayer(deck, c.Run.PlayerHp, c.Run.PlayerMaxHp, c.Run.MaxEnergy, c.RelicNames);
            var stats = EncounterEvaluator.Evaluate(Catalog.SetupCombat(player, enc.Build()), opts);
            results.Add(new EliteResult(info.name, info.comp, stats, null));
        }
        Reporting.Print(c.Run, c.Path, results, c.Warnings, c.DeckSummary);
    }

    /// <summary>Print the best single-card removals for the current deck (the "best card to remove next" view).</summary>
    public static void ReportCuts(Context c, EvalOptions opts)
    {
        if (c.Encounters.Count == 0 || c.DeckSpecs.Count <= 1)
        {
            Console.WriteLine("(removal advice needs ≥1 ported elite and ≥2 ported deck cards.)");
            return;
        }
        Console.WriteLine($"Evaluating removals over {c.Encounters.Count} elite(s)…");
        Console.WriteLine(Advisor.Format(Advisor.RemovalAdvice(
            c.DeckSpecs, c.Encounters, c.Run.PlayerHp, c.Run.PlayerMaxHp, c.Run.MaxEnergy, c.RelicNames, opts)));
    }

    /// <summary>Resolve a list of typed reward tokens (auto-correct) then print take-vs-skip advice.</summary>
    public static void ReportPick(Context c, IEnumerable<string> tokens, EvalOptions opts)
    {
        if (c.Encounters.Count == 0) { Console.WriteLine("(pick advice needs ≥1 ported elite.)"); return; }
        var cards = new List<string>();
        foreach (var tok in tokens)
        {
            var m = CardNameMatcher.Resolve(tok);
            if (m.Canonical == null)
            {
                Console.WriteLine($"  '{tok}' — unknown card. Did you mean: {string.Join(", ", m.Suggestions)}?");
                continue;
            }
            if (m.Corrected) Console.WriteLine($"  interpreting '{tok}' as {m.Canonical}");
            cards.Add(m.Canonical);
        }
        if (cards.Count == 0) { Console.WriteLine("(no resolvable reward cards.)"); return; }
        Console.WriteLine($"Evaluating {cards.Count} reward option(s) vs {c.Encounters.Count} elite(s)…");
        Console.WriteLine(Advisor.FormatPick(Advisor.PickAdvice(
            c.DeckSpecs, cards, c.Encounters, c.Run.PlayerHp, c.Run.PlayerMaxHp, c.Run.MaxEnergy, c.RelicNames, opts)));
    }

    // ── Live loop ────────────────────────────────────────────────────────────────

    public int Run()
    {
        var path = _fixedPath ?? SaveLocator.FindNewest();
        if (path == null) { Console.Error.WriteLine("ranwid: no ongoing unmodded run found (start a run, or pass --save <file>)."); return 1; }

        var ctx = LoadAndReport(path);
        StartWatcher();

        Console.WriteLine("\nLive companion ready. Type reward cards to vet (e.g. `Bludgeon Inflame Whirlwind`), "
            + "or: cuts · deck · help · quit.  (Tab completes, typos auto-correct.)");
        while (true)
        {
            // Refresh against a newer save before prompting (you advanced a screen).
            if (_dirty)
            {
                _dirty = false;
                var newPath = _fixedPath ?? SaveLocator.FindNewest() ?? path;
                Console.WriteLine($"\n[run updated]");
                var refreshed = LoadAndReport(newPath);
                if (refreshed != null) { ctx = refreshed; path = newPath; }
            }

            string? line = LineEditor.ReadLine("\nranwid> ", CompleteToken);
            if (line == null) { Console.WriteLine(); break; }     // EOF / Ctrl-D
            line = line.Trim();
            if (line.Length == 0) continue;

            var parts = line.Split(' ', StringSplitOptions.RemoveEmptyEntries);
            var cmd = parts[0].ToLowerInvariant();
            var rest = parts.Skip(1).ToArray();

            if (cmd is "quit" or "exit" or "q") break;
            else if (cmd is "help" or "?") PrintHelp();
            else if (cmd is "deck" or "stats") { if (ctx != null) ReportDeck(ctx, _opts); }
            else if (cmd is "cuts" or "remove" or "cut") { if (ctx != null) ReportCuts(ctx, _opts); }
            else if (cmd is "pick" or "reward" or "rewards") { if (ctx != null) ReportPick(ctx, rest, _opts); }
            else if (ctx != null) ReportPick(ctx, parts, _opts);   // bare card list = pick advice
        }
        return 0;
    }

    private Context? LoadAndReport(string path)
    {
        _loadedKey = Key(path);
        var ctx = Load(path, _netId);
        if (ctx == null) return null;
        ReportDeck(ctx, _opts);
        ReportCuts(ctx, _opts);
        return ctx;
    }

    private void StartWatcher()
    {
        var t = new Thread(() =>
        {
            while (true)
            {
                Thread.Sleep(1500);
                try
                {
                    var p = _fixedPath ?? SaveLocator.FindNewest();
                    if (p != null && Key(p) != _loadedKey) _dirty = true;
                }
                catch { /* transient FS errors: ignore, retry next tick */ }
            }
        }) { IsBackground = true, Name = "ranwid-watch" };
        t.Start();
    }

    private static string Key(string path)
    {
        try { return $"{path}|{File.GetLastWriteTimeUtc(path).Ticks}"; } catch { return path; }
    }

    private static IReadOnlyList<string> CompleteToken(string token) => CardNameMatcher.Complete(token);

    private static void PrintHelp()
    {
        Console.WriteLine(
            "  <card> [<card> …]   vet reward options (take-vs-skip) vs the Act's elites; typos auto-correct\n" +
            "  pick <cards…>       same, explicit\n" +
            "  cuts                best card to remove from the current deck\n" +
            "  deck | stats        per-elite stats for the current deck\n" +
            "  help | quit         this help / exit\n" +
            "  (Tab auto-completes card names; the run auto-refreshes when you change screens.)");
    }
}
