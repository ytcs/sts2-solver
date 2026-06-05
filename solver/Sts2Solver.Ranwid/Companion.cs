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
    private readonly string? _saveDir;            // when started with --save-dir, scan this root
    private volatile bool _dirty;
    private string? _loadedKey;                   // path|mtime of the currently-loaded save

    public Companion(string? fixedPath, string? saveDir, int? netId, EvalOptions opts)
    {
        _fixedPath = fixedPath; _saveDir = saveDir; _netId = netId; _opts = opts;
    }

    // ── Loaded run context ─────────────────────────────────────────────────────

    public sealed record Context(
        RunState Run, string Path, List<string> DeckSpecs, List<string> RelicNames,
        List<Advisor.Encounter> Encounters, List<(string name, string comp)> EliteInfo,
        List<Advisor.Encounter> StrengthPool, List<string> Warnings, string DeckSummary);

    /// <summary>Parse a save and resolve the deck/relics/elite encounters the advisor needs. Returns null for
    /// non-Ironclad or unreadable runs. <paramref name="quiet"/> suppresses the failure messages — the live
    /// watcher uses it, because a run ending (the game clears/deletes the save) would otherwise spam errors;
    /// the caller keeps the last good dashboard instead.</summary>
    public static Context? Load(string path, int? netId, bool quiet = false)
    {
        RunState run;
        try { run = RunSaveReader.Parse(path, netId); }
        catch (Exception ex) { if (!quiet) Console.Error.WriteLine($"ranwid: could not parse '{path}': {ex.Message}"); return null; }

        var warnings = new List<string>();
        if (!GameIds.IsSupportedCharacter(run.Character))
        {
            if (!quiet) Console.WriteLine($"ranwid: unsupported character {GameIds.CharacterName(run.Character)} "
                + "(supported: Ironclad, Silent, Regent, Necrobinder, Defect).");
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
            else warnings.Add($"relic {GameIds.ClassName(rid)} ignored (not modelled)");
        }

        int asc = run.Ascension;
        Advisor.Encounter MakeEnc(string cls)
        {
            string disp = cls.EndsWith("Elite", StringComparison.Ordinal) ? cls[..^5] : cls;
            return new Advisor.Encounter(disp, () => Catalog.BuildEliteEncounter(cls, asc));
        }

        // The dashboard's per-elite rows show the run's ACTUAL elites (at current HP — "can I take this fight").
        var encounters = new List<Advisor.Encounter>();
        var eliteInfo = new List<(string, string)>();
        var runEliteClasses = new List<string>();
        foreach (var eid in run.EliteEncounterIds)
        {
            var cls = GameIds.EncounterClassName(eid);
            if (!Catalog.IsKnownEliteEncounter(cls)) continue;
            runEliteClasses.Add(cls);
            encounters.Add(MakeEnc(cls));
            string comp = string.Join(" + ", Catalog.BuildEliteEncounter(cls, asc)
                .GroupBy(m => m.Name).Select(g => g.Count() > 1 ? $"{g.Count()}× {g.Key}" : g.Key));
            eliteInfo.Add((MakeEnc(cls).Name, comp));
        }

        // Deck strength + advice evaluate over the Act's FULL elite pool (representative + AoE-balanced), so a
        // single-target-heavy run still values multi-enemy cards (Whirlwind). Falls back to the run's elites.
        var poolClasses = Catalog.RepresentativeElites(runEliteClasses);
        var strengthPool = (poolClasses.Count > 0 ? poolClasses : runEliteClasses).Select(MakeEnc).ToList();

        string deckSummary = run.Deck.Count == 0 ? "(empty)" : string.Join(", ", run.Deck
            .GroupBy(e => GameIds.CardSpec(e.Id, e.Upgrade))
            .OrderByDescending(g => g.Count()).ThenBy(g => g.Key, StringComparer.Ordinal)
            .Select(g => $"{g.Count()}x {g.Key}"));

        return new Context(run, path, deckSpecs, relicNames, encounters, eliteInfo, strengthPool, warnings, deckSummary);
    }

    // ── Reporting ───────────────────────────────────────────────────────────────

    /// <summary>Evaluate the current deck against each Act elite (the data behind both the text report and the
    /// live dashboard). Pure compute — no output.</summary>
    public static List<EliteResult> EvaluateElites(Context c, EvalOptions opts)
    {
        // The Act's elites are independent solves → run them across cores (AsOrdered keeps the table order
        // stable). Each eval builds its own deck/monsters and MCTS tree, so there's no shared mutable state.
        return c.Encounters.Zip(c.EliteInfo).AsParallel().AsOrdered().Select(pair =>
        {
            var (enc, info) = pair;
            if (c.DeckSpecs.Count == 0) return new EliteResult(info.name, info.comp, null, "no playable deck cards");
            var deck = c.DeckSpecs.Select(Catalog.BuildCard).ToList();
            var player = Catalog.BuildPlayer(deck, c.Run.PlayerHp, c.Run.PlayerMaxHp, c.Run.MaxEnergy, c.RelicNames);
            var stats = EncounterEvaluator.Evaluate(Catalog.SetupCombat(player, enc.Build()), opts);
            return new EliteResult(info.name, info.comp, stats, null);
        }).ToList();
    }

    /// <summary>Resolve reward tokens (auto-correct), then compute take-vs-skip advice — or null if no card
    /// resolves / there are no elites. Shared by the one-shot path and the live "check a reward" command.</summary>
    public static (double skip, List<Advisor.PickItem> ranked)? PickFromTokens(
        Context c, IEnumerable<string> tokens, EvalOptions opts)
    {
        if (c.StrengthPool.Count == 0) return null;
        var cards = new List<string>();
        foreach (var tok in tokens)
        {
            var m = CardNameMatcher.Resolve(tok);
            if (m.Canonical != null) cards.Add(m.Canonical);
        }
        if (cards.Count == 0) return null;
        return Advisor.PickAdvice(c.DeckSpecs, cards, c.StrengthPool,
            c.Run.MaxEnergy, c.RelicNames, opts);
    }

    // ── Live loop (Spectre dashboard) ─────────────────────────────────────────────
    //
    // The default screen is the always-on deck + per-elite dashboard. It repaints automatically whenever the
    // save changes (a FileSystemWatcher sets _dirty). Removal advice is OFF by default (it re-evaluates the
    // whole deck per card — too slow to run unprompted) and lives behind the [r] key. Keys: r removals,
    // c check-a-reward, d refresh, q quit.

    public int Run()
    {
        var path = _fixedPath ?? SaveSource.ResolveSaveFileInteractive(_saveDir);
        if (path == null) { Console.Error.WriteLine("ranwid: no ongoing unmodded run found (start a run, or pass --save <file> / --save-dir <folder>)."); return 1; }

        var ctx = Reload(path, out path);
        StartWatcher(path);

        while (true)
        {
            // A newer save was written (you advanced a screen / played a card) → reload + re-evaluate + repaint.
            // When the RUN ENDS the game clears/deletes current_run.save: FindNewest returns null, or the file is
            // momentarily unreadable. In that case keep the LAST good dashboard on screen (so post-game analysis
            // still works) and stay silent — never blank the screen or spam errors.
            if (_dirty)
            {
                _dirty = false;
                var newPath = _fixedPath ?? SaveSource.FindNewest(_saveDir);
                if (newPath != null && Key(newPath) != _loadedKey)
                {
                    _loadedKey = Key(newPath);
                    var refreshed = Load(newPath, _netId, quiet: true);
                    if (refreshed != null) { ctx = refreshed; path = newPath; ReEvaluate(ctx); }
                    // refreshed == null → unreadable/unsupported right now: keep the last dashboard, silently.
                }
            }

            // No keyboard (redirected/piped input) → still auto-refresh, just no commands.
            if (Console.IsInputRedirected || !Console.KeyAvailable) { Thread.Sleep(50); continue; }
            var key = Console.ReadKey(intercept: true).Key;

            if (key is ConsoleKey.Q or ConsoleKey.Escape) break;
            if (ctx == null) continue;
            switch (key)
            {
                case ConsoleKey.R: ShowRemovals(ctx); RenderCurrent(ctx); break;
                case ConsoleKey.C: ShowRewardCheck(ctx); RenderCurrent(ctx); break;
                case ConsoleKey.D or ConsoleKey.F5: ReEvaluate(ctx, force: true); break;
            }
        }
        return 0;
    }

    private List<EliteResult> _elites = new();
    private double _strength = double.NaN;

    /// <summary>Load the save at <paramref name="path"/>, evaluate the elites (with a spinner), and paint the
    /// dashboard. Returns the loaded context (null if unreadable/unsupported) and the resolved path.</summary>
    private Context? Reload(string path, out string resolvedPath)
    {
        resolvedPath = path;
        _loadedKey = Key(path);
        var ctx = Load(path, _netId, quiet: true);
        if (ctx == null)
        {
            Spectre.Console.AnsiConsole.MarkupLine("[grey]ranwid: waiting for a readable run… (start or load one)[/]");
            _elites = new(); _eliteKey = _strengthKey = null;
            return null;
        }
        ReEvaluate(ctx);
        return ctx;
    }

    private string? _eliteKey, _strengthKey;

    /// <summary>Recompute only what's gone stale, and repaint only if something changed — so a save write that
    /// doesn't affect the numbers (gold, map move, an HP tick) doesn't burn a full ~6-eval refresh. The per-elite
    /// rows depend on deck + relics + CURRENT HP + energy + ascension + the elite set; the deck-strength index is
    /// HP-INDEPENDENT (fixed 100 HP), so an HP-only change recomputes the rows but reuses the cached strength.
    /// <paramref name="force"/> = the manual [d] refresh (recompute regardless).</summary>
    private void ReEvaluate(Context ctx, bool force = false)
    {
        string deckKey = string.Join(",", ctx.DeckSpecs.OrderBy(s => s, StringComparer.Ordinal))
            + "|R" + string.Join(",", ctx.RelicNames.OrderBy(s => s, StringComparer.Ordinal))
            + "|E" + ctx.Run.MaxEnergy + "|A" + ctx.Run.Ascension
            + "|" + string.Join(",", ctx.EliteInfo.Select(i => i.name));
        string eliteKey = deckKey + "|HP" + ctx.Run.PlayerHp + "/" + ctx.Run.PlayerMaxHp;

        bool elitesStale = force || eliteKey != _eliteKey;
        bool strengthStale = force || deckKey != _strengthKey;
        if (!elitesStale && !strengthStale) return;   // nothing the eval depends on changed → no recompute, no repaint

        Dashboard.Render(ctx, _elites, _strength, evaluating: true);
        Spectre.Console.AnsiConsole.Status().Start("evaluating…", _ =>
        {
            if (elitesStale) { _elites = EvaluateElites(ctx, _opts); _eliteKey = eliteKey; }
            if (strengthStale)
            {
                _strength = Advisor.DeckStrength(ctx.DeckSpecs, ctx.StrengthPool, ctx.Run.MaxEnergy, ctx.RelicNames, _opts);
                _strengthKey = deckKey;
            }
        });
        RenderCurrent(ctx);
    }

    private void RenderCurrent(Context ctx) => Dashboard.Render(ctx, _elites, _strength, evaluating: false);

    /// <summary>[r] best cards to remove — the slow per-card sweep, run only on demand.</summary>
    private void ShowRemovals(Context ctx)
    {
        Spectre.Console.AnsiConsole.Clear();
        if (ctx.StrengthPool.Count == 0 || ctx.DeckSpecs.Count <= 1)
        {
            Spectre.Console.AnsiConsole.MarkupLine("[grey]Removal advice needs at least one elite and two cards.[/]");
        }
        else
        {
            (double, List<Advisor.AdviceItem>) advice = default;
            Spectre.Console.AnsiConsole.Status().Start("finding the best cards to remove…",
                _ => advice = Advisor.RemovalAdvice(ctx.DeckSpecs, ctx.StrengthPool,
                    ctx.Run.MaxEnergy, ctx.RelicNames, _opts));
            Dashboard.RenderRemovals(advice);
        }
        WaitForKey();
    }

    /// <summary>[c] vet one or more reward cards (take-vs-skip). Types auto-complete + auto-correct.</summary>
    private void ShowRewardCheck(Context ctx)
    {
        Spectre.Console.AnsiConsole.Clear();
        if (ctx.StrengthPool.Count == 0) { Spectre.Console.AnsiConsole.MarkupLine("[grey]No elites to compare against.[/]"); WaitForKey(); return; }
        Spectre.Console.AnsiConsole.MarkupLine("[grey]Type the reward card(s), space-separated (Tab completes). Blank to cancel.[/]");
        var line = LineEditor.ReadLine("cards> ", CompleteToken)?.Trim();
        if (string.IsNullOrEmpty(line)) return;

        var cards = new List<string>();
        foreach (var tok in line.Split(' ', StringSplitOptions.RemoveEmptyEntries))
        {
            var m = CardNameMatcher.Resolve(tok);
            if (m.Canonical != null) cards.Add(m.Canonical);
            else Spectre.Console.AnsiConsole.MarkupLine($"[grey]  '{Spectre.Console.Markup.Escape(tok)}' — not a known card.[/]");
        }
        if (cards.Count == 0) { WaitForKey(); return; }

        (double, List<Advisor.PickItem>) advice = default;
        Spectre.Console.AnsiConsole.Status().Start("checking the reward…",
            _ => advice = Advisor.PickAdvice(ctx.DeckSpecs, cards, ctx.StrengthPool,
                ctx.Run.MaxEnergy, ctx.RelicNames, _opts));
        Dashboard.RenderPick(advice);
        WaitForKey();
    }

    private static void WaitForKey()
    {
        Spectre.Console.AnsiConsole.Markup("\n  [grey]press any key…[/]");
        Console.ReadKey(intercept: true);
    }

    /// <summary>Auto-refresh on save change: a FileSystemWatcher on the save's directory (primary) plus a slow
    /// poll fallback (some filesystems don't surface change events reliably). Both just flip <c>_dirty</c>; the
    /// main loop debounces by comparing the path|mtime key.</summary>
    private void StartWatcher(string path)
    {
        try
        {
            var dir = _saveDir ?? Path.GetDirectoryName(path);
            if (dir != null && Directory.Exists(dir))
            {
                var fsw = new FileSystemWatcher(dir, "*.save")
                {
                    NotifyFilter = NotifyFilters.LastWrite | NotifyFilters.Size | NotifyFilters.FileName | NotifyFilters.CreationTime,
                    IncludeSubdirectories = _saveDir != null,   // --save-dir scans recursively, so watch deep
                    EnableRaisingEvents = true,
                };
                void Touch(object? _, FileSystemEventArgs __) => _dirty = true;
                fsw.Changed += Touch; fsw.Created += Touch; fsw.Renamed += (_, __) => _dirty = true;
                _watcher = fsw;   // keep alive for the process lifetime
            }
        }
        catch { /* watcher unavailable → rely on the poll fallback below */ }

        var t = new Thread(() =>
        {
            while (true)
            {
                Thread.Sleep(2000);
                try
                {
                    var p = _fixedPath ?? SaveSource.FindNewest(_saveDir);
                    if (p != null && Key(p) != _loadedKey) _dirty = true;
                }
                catch { /* transient FS errors: ignore, retry next tick */ }
            }
        }) { IsBackground = true, Name = "ranwid-watch" };
        t.Start();
    }

    private FileSystemWatcher? _watcher;

    private static string Key(string path)
    {
        try { return $"{path}|{File.GetLastWriteTimeUtc(path).Ticks}"; } catch { return path; }
    }

    private static IReadOnlyList<string> CompleteToken(string token) => CardNameMatcher.Complete(token);
}
