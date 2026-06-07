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
        List<Advisor.Encounter> StrengthPool, List<string> Warnings, string DeckSummary,
        Advisor.Encounter? Boss = null, string BossComp = "", string? UnmodelledBoss = null);

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
            var spec = GameIds.CardSpec(e.Id, e.Upgrade);
            // Fold a modelled enchantment into the build spec ("Name+U@Sharp:3"); warn on an unmodelled one so
            // the player knows that card is scored as if plain.
            if (e.EnchantId != null)
            {
                var ench = GameIds.ClassName(e.EnchantId);
                if (Catalog.IsModelledEnchant(ench)) spec += $"@{ench}:{e.EnchantAmount}";
                else warnings.Add($"enchantment {ench} on {GameIds.ClassName(e.Id)} ignored (not modelled)");
            }
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

        // The dashboard's per-elite rows show the run's ACTUAL elites (at current HP — "can I take this fight").
        var runEliteClasses = run.EliteEncounterIds
            .Select(GameIds.EncounterClassName)
            .Where(Catalog.IsKnownEliteEncounter)
            .ToList();

        string deckSummary = SummarizeSpecs(run.Deck.Select(e => GameIds.CardSpec(e.Id, e.Upgrade)));
        return BuildContext(run, path, deckSpecs, relicNames, runEliteClasses, warnings, deckSummary);
    }

    /// <summary>Assemble a <see cref="Context"/> from already-resolved deck specs / relic names / elite classes.
    /// The dashboard's per-elite rows are <paramref name="runEliteClasses"/> (the run's actual elites — or, in
    /// custom mode, the whole act pool); deck strength + advice evaluate over the Act's FULL representative pool
    /// (<see cref="Catalog.RepresentativeElites"/>), so a single-target-heavy deck still values AoE. Shared by the
    /// save-file loader and the custom-deck mode.</summary>
    public static Context BuildContext(
        RunState run, string path, List<string> deckSpecs, List<string> relicNames,
        IReadOnlyList<string> runEliteClasses, List<string> warnings, string deckSummary)
    {
        int asc = run.Ascension;
        Advisor.Encounter MakeEnc(string cls)
        {
            string disp = cls.EndsWith("Elite", StringComparison.Ordinal) ? cls[..^5]
                        : cls.EndsWith("Boss", StringComparison.Ordinal) ? cls[..^4]
                        : cls;
            return new Advisor.Encounter(disp, () => Catalog.BuildEliteEncounter(cls, asc));
        }

        var encounters = new List<Advisor.Encounter>();
        var eliteInfo = new List<(string, string)>();
        foreach (var cls in runEliteClasses)
        {
            if (!Catalog.IsKnownEliteEncounter(cls)) continue;
            encounters.Add(MakeEnc(cls));
            string comp = string.Join(" + ", Catalog.BuildEliteEncounter(cls, asc)
                .GroupBy(m => m.Name).Select(g => g.Count() > 1 ? $"{g.Count()}× {g.Key}" : g.Key));
            eliteInfo.Add((MakeEnc(cls).Name, comp));
        }

        var poolClasses = Catalog.RepresentativeElites(runEliteClasses);
        var strengthPool = (poolClasses.Count > 0 ? poolClasses : runEliteClasses)
            .Where(Catalog.IsKnownEliteEncounter).Select(MakeEnc).ToList();

        // Fold the run's act BOSS into the strength pool so the index reflects boss-readiness, not just elites.
        // One boss keeps the extra (slower, tankier) eval bounded — it becomes the long pole of the parallel pool.
        // Keep it separately too (Boss/BossComp) so the dashboard can headline it as a row above the elites.
        Advisor.Encounter? boss = null;
        string bossComp = "";
        var bossCls = BossEncounterForRun(run, out string? unmodelledBoss);
        if (bossCls != null)
        {
            var bEnc = MakeEnc(bossCls);
            strengthPool.Add(bEnc);
            boss = bEnc;
            string comp = string.Join(" + ", Catalog.BuildEliteEncounter(bossCls, asc)
                .GroupBy(m => m.Name).Select(g => g.Count() > 1 ? $"{g.Count()}× {g.Key}" : g.Key));
            bossComp = comp == bEnc.Name ? "" : comp;   // hide a single-monster comp that just repeats the name
        }
        else if (unmodelledBoss != null)
        {
            // The run's ACTUAL boss is known but not yet ported (TheKin/Queen/KaiserCrab/TestSubject). Surface it
            // honestly as a "not modelled" row and keep it OUT of deck strength — never substitute a different boss.
            warnings.Add($"act boss {unmodelledBoss} not modelled (excluded from deck strength)");
        }

        return new Context(run, path, deckSpecs, relicNames, encounters, eliteInfo, strengthPool,
            warnings, deckSummary, boss, bossComp, unmodelledBoss);
    }

    /// <summary>Resolve the run's act BOSS. Returns the ported boss encounter class name (folded into deck
    /// strength + headlined) when the run's ACTUAL boss is modelled; otherwise sets <paramref name="unmodelled"/>
    /// to the boss's display name when the run has a known-but-unported boss (TheKin/Queen/KaiserCrab/TestSubject)
    /// so it can be shown as a "not modelled" row rather than silently swapped for a different boss. Only when the
    /// save carries NO boss id does it fall back to the act's first pool boss (e.g. custom mode, where ActIndex is
    /// a theme index). The save's boss id already encodes the encounter, e.g.
    /// <c>ENCOUNTER.AEONGLASS_BOSS → AeonglassBoss</c> (matching the catalog key directly — do NOT re-append
    /// "Boss", which produced the never-matching "AeonglassBossBoss" and forced the wrong-boss fallback).</summary>
    private static string? BossEncounterForRun(RunState run, out string? unmodelled)
    {
        unmodelled = null;
        if (!string.IsNullOrWhiteSpace(run.BossId))
        {
            var name = GameIds.EncounterClassName(run.BossId);
            if (!name.EndsWith("Boss", StringComparison.Ordinal)) name += "Boss";   // tolerate an id without _BOSS
            if (Catalog.IsKnownEliteEncounter(name)) return name;
            unmodelled = name[..^4];   // strip the "Boss" suffix for display (e.g. "TheKin")
            return null;
        }
        var pool = Catalog.ActBossPool(run.ActIndex);
        return pool.Count > 0 ? pool[0] : null;
    }

    /// <summary>Build benchmark <see cref="Advisor.Encounter"/>s from an act-pool of encounter class names (elites
    /// or bosses), keeping only the modelled ones. Shared by the next-act deck-strength projection, which scores
    /// against a whole upcoming-act pool. Display name strips the Elite/Boss suffix (matching <c>BuildContext</c>).</summary>
    public static List<Advisor.Encounter> EncountersForClasses(IEnumerable<string> classes, int ascension)
    {
        Advisor.Encounter Make(string cls)
        {
            string disp = cls.EndsWith("Elite", StringComparison.Ordinal) ? cls[..^5]
                        : cls.EndsWith("Boss", StringComparison.Ordinal) ? cls[..^4]
                        : cls;
            return new Advisor.Encounter(disp, () => Catalog.BuildEliteEncounter(cls, ascension));
        }
        return classes.Where(Catalog.IsKnownEliteEncounter).Select(Make).ToList();
    }

    /// <summary>"4x StrikeIronclad, 1x Bash" — group a spec list by spec, most-frequent first. "(empty)" when none.</summary>
    public static string SummarizeSpecs(IEnumerable<string> specs)
    {
        var list = specs.ToList();
        return list.Count == 0 ? "(empty)" : string.Join(", ", list
            .GroupBy(s => s)
            .OrderByDescending(g => g.Count()).ThenBy(g => g.Key, StringComparer.Ordinal)
            .Select(g => $"{g.Count()}x {g.Key}"));
    }

    // ── Reporting ───────────────────────────────────────────────────────────────

    /// <summary>One evaluable dashboard row: an encounter to solve, or (null <see cref="Enc"/>) a row that is
    /// skipped unevaluated with a <see cref="Skipped"/> reason (e.g. an unported boss).</summary>
    public readonly record struct RowSpec(string Label, string Comp, Advisor.Encounter? Enc, string? Skipped);

    /// <summary>The run's actual elites as rows, in stable table order (no boss).</summary>
    public static List<RowSpec> EliteRowSpecs(Context c) =>
        c.Encounters.Zip(c.EliteInfo).Select(p => new RowSpec(p.Second.name, p.Second.comp, p.First, null)).ToList();

    /// <summary>The act-boss row (its own panel in the TUI), or null when the run has no boss. A known-but-unported
    /// boss becomes a skipped "not modelled" row — never substituted for a different boss.</summary>
    public static RowSpec? BossRowSpec(Context c) =>
        c.Boss is { } b ? new RowSpec($"Boss: {b.Name}", c.BossComp, b, null)
        : c.UnmodelledBoss is { } ub ? new RowSpec($"Boss: {ub}", "", null, "not modelled")
        : null;

    /// <summary>Evaluate a single row (one encounter). The streaming entry point the TUI calls per row so numbers
    /// fill in as each finishes; a null-encounter row returns its skipped reason unevaluated. Pure compute.</summary>
    public static EliteResult EvaluateRow(Context c, RowSpec row, EvalOptions opts)
    {
        if (row.Enc == null) return new EliteResult(row.Label, row.Comp, null, row.Skipped ?? "—");
        if (c.DeckSpecs.Count == 0) return new EliteResult(row.Label, row.Comp, null, "no playable deck cards");
        var deck = c.DeckSpecs.Select(Catalog.BuildCard).ToList();
        var player = Catalog.BuildPlayer(deck, c.Run.PlayerHp, c.Run.PlayerMaxHp, c.Run.MaxEnergy, c.RelicNames);
        var stats = EncounterEvaluator.Evaluate(Catalog.SetupCombat(player, row.Enc.Value.Build()), opts);
        return new EliteResult(row.Label, row.Comp, stats, null);
    }

    /// <summary>Evaluate the current deck against each Act elite (the data behind both the text report and the
    /// live dashboard). Pure compute — no output. The boss headlines as the first row, at the run's actual HP.</summary>
    public static List<EliteResult> EvaluateElites(Context c, EvalOptions opts)
    {
        // The Act's elites are independent solves → run them across cores (AsOrdered keeps the table order
        // stable). Each eval builds its own deck/monsters and MCTS tree, so there's no shared mutable state.
        var rows = EliteRowSpecs(c).AsParallel().AsOrdered().Select(s => EvaluateRow(c, s, opts)).ToList();

        // The act boss headlines the table as the first row ("Boss: <name>"), evaluated after the parallel
        // elites (one extra, tankier fight).
        if (BossRowSpec(c) is { } bs) rows.Insert(0, EvaluateRow(c, bs, opts));
        return rows;
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
                case ConsoleKey.U: ShowUpgrades(ctx); RenderCurrent(ctx); break;
                case ConsoleKey.C: ShowRewardCheck(ctx); RenderCurrent(ctx); break;
                case ConsoleKey.D or ConsoleKey.F5: ReEvaluate(ctx, force: true); break;
            }
        }
        return 0;
    }

    // ── Custom deck mode (no save file) ──────────────────────────────────────────
    //
    // `ranwid --custom [character]` — start from a character's STARTER deck + relic + HP and edit it by hand:
    // add/remove cards, switch the act you're evaluating against, and watch the deck-strength index + per-elite
    // numbers update. For multiplayer GUESTS (whose run isn't saved locally) this is the way to mirror a run; it's
    // also a deck-building sandbox. Reuses the same Context / Dashboard / Advisor machinery as the live mode.

    private const string CustomFooter =
        "  [grey]([/][white]+card[/][grey]) add   ([/][white]-card[/][grey]) remove   "
        + "([/][white]act[/][grey] N) act   ([/][white]r[/][grey]) cuts   ([/][white]u[/][grey]) upgrades   ([/][white]c[/][grey]) reward   "
        + "([/][white]help[/][grey]) more   ([/][white]q[/][grey]) quit[/]";

    /// <summary>Run the interactive custom-deck session for <paramref name="initialChar"/> (default Ironclad).</summary>
    public int RunCustom(string? initialChar)
    {
        _footer = CustomFooter;
        var profile = (initialChar != null ? Catalog.FindCharacter(initialChar) : null) ?? Catalog.CharacterProfiles[0];
        int actIndex = 0;                 // which Act's elite pool to score against (0-based; [act] to change)
        int asc = 0;
        int hp = profile.StartingHp;
        var deck = profile.StarterDeckSpecs();

        Context Build() => BuildCustom(profile, actIndex, asc, hp, deck);
        var ctx = Build();
        ReEvaluate(ctx, force: true);

        while (true)
        {
            var line = LineEditor.ReadLine("deck> ", CompleteToken)?.Trim();
            if (line == null) return 0;                               // EOF (piped/closed input)
            if (line.Length == 0) { ReEvaluate(ctx, force: true); continue; }

            // +card / -card shorthand (no space needed).
            string verb, arg;
            if (line[0] is '+' or '-') { verb = line[0] == '+' ? "add" : "rm"; arg = line[1..].Trim(); }
            else { var sp = line.IndexOf(' '); verb = (sp < 0 ? line : line[..sp]).ToLowerInvariant(); arg = sp < 0 ? "" : line[(sp + 1)..].Trim(); }

            switch (verb)
            {
                case "q" or "quit" or "exit": return 0;
                case "help" or "?": ShowCustomHelp(); RenderCurrent(ctx); break;
                case "deck" or "list": ShowDeckList(deck); RenderCurrent(ctx); break;

                case "add":
                    if (CustomAdd(deck, arg)) { ctx = Build(); ReEvaluate(ctx, force: true); }
                    else RenderCurrent(ctx);
                    break;
                case "rm" or "remove" or "del" or "delete":
                    if (CustomRemove(deck, arg)) { ctx = Build(); ReEvaluate(ctx, force: true); }
                    else RenderCurrent(ctx);
                    break;

                case "act":
                    if (TryParseAct(arg, out int ai)) { actIndex = ai; ctx = Build(); ReEvaluate(ctx, force: true); }
                    else RenderCurrent(ctx);
                    break;
                case "char" or "character":
                    if (Catalog.FindCharacter(arg) is { } np)
                    { profile = np; deck = profile.StarterDeckSpecs(); hp = profile.StartingHp; ctx = Build(); ReEvaluate(ctx, force: true); }
                    else { Note($"unknown character '{arg}' (try: {string.Join(", ", Catalog.CharacterProfiles.Select(p => p.Key))})"); RenderCurrent(ctx); }
                    break;
                case "hp":
                    if (int.TryParse(arg, out int h) && h > 0) { hp = h; ctx = Build(); ReEvaluate(ctx, force: true); }
                    else RenderCurrent(ctx);
                    break;
                case "asc" or "ascension":
                    if (int.TryParse(arg, out int a) && a >= 0) { asc = a; ctx = Build(); ReEvaluate(ctx, force: true); }
                    else RenderCurrent(ctx);
                    break;
                case "reset":
                    deck = profile.StarterDeckSpecs(); hp = profile.StartingHp; ctx = Build(); ReEvaluate(ctx, force: true);
                    break;

                case "r" or "cuts" or "advise" or "removals": ShowRemovals(ctx); RenderCurrent(ctx); break;
                case "u" or "upgrade" or "upgrades": ShowUpgrades(ctx); RenderCurrent(ctx); break;
                case "c" or "check" or "reward":
                    if (arg.Length > 0) { ShowRewardCheckFor(ctx, arg); } else ShowRewardCheck(ctx);
                    RenderCurrent(ctx);
                    break;
                case "d" or "refresh": ReEvaluate(ctx, force: true); break;

                default:
                    // Bare card name with no verb → treat as add (the common case while drafting).
                    if (CustomAdd(deck, line)) { ctx = Build(); ReEvaluate(ctx, force: true); }
                    else RenderCurrent(ctx);
                    break;
            }
        }
    }

    /// <summary>Build a custom Context: a synthesized RunState (display only) + the chosen act's elite pool as
    /// both the dashboard rows and the strength pool.</summary>
    private static Context BuildCustom(CharacterProfile profile, int actIndex, int asc, int hp, List<string> deck)
    {
        var theme = Catalog.ActThemes[Math.Clamp(actIndex, 0, Catalog.ActThemes.Count - 1)];
        var relicNames = profile.StarterRelic is { } r && Catalog.IsModelledRelic(r) ? new List<string> { r } : new List<string>();
        // RunState here is for DISPLAY + the shared eval params only — Deck/RelicIds carry just enough for the header
        // (card count, "relics: …"); the real deck/relics/pool are passed to BuildContext explicitly.
        var displayDeck = deck.Select(SpecToEntry).ToList();
        var run = new RunState(
            Ascension: asc, ActIndex: actIndex, ActId: $"ACT.{theme.ToUpperInvariant()}",
            Character: profile.CharacterId, PlayerHp: hp, PlayerMaxHp: profile.StartingHp, MaxEnergy: profile.MaxEnergy,
            Deck: displayDeck, RelicIds: relicNames, EliteEncounterIds: Array.Empty<string>(),
            BossId: null, PlayerCount: 1, PlayerNetId: 0);

        var warnings = new List<string>();
        return BuildContext(run, "(custom deck)", deck.ToList(), relicNames,
            Catalog.ActElitePool(actIndex), warnings, SummarizeSpecs(deck));
    }

    private static CardEntry SpecToEntry(string spec)
    {
        int plus = spec.IndexOf('+');
        return plus < 0 ? new CardEntry(spec, 0, null, 0)
                        : new CardEntry(spec[..plus], int.TryParse(spec[(plus + 1)..], out var u) ? u : 0, null, 0);
    }

    /// <summary>Resolve "card [xN] | card N" → add N copies (default 1). Returns true if the deck changed.</summary>
    private bool CustomAdd(List<string> deck, string arg)
    {
        if (string.IsNullOrWhiteSpace(arg)) { Note("usage: add <card> [xN]"); return false; }
        // Trailing count: "Strike x3" or "Strike 3".
        int count = 1;
        var toks = arg.Split(' ', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        if (toks.Length >= 2)
        {
            var last = toks[^1].TrimStart('x', 'X');
            if (int.TryParse(last, out int n) && n > 0) { count = Math.Min(n, 100); arg = string.Join(' ', toks[..^1]); }
        }
        var m = CardNameMatcher.Resolve(arg);
        if (m.Canonical == null) { Note(m.Suggestions.Count > 0 ? $"'{arg}'? did you mean: {string.Join(", ", m.Suggestions)}" : $"unknown card '{arg}'"); return false; }
        try { Catalog.BuildCard(m.Canonical); } catch (ArgumentException) { Note($"'{m.Canonical}' isn't a buildable card"); return false; }
        for (int i = 0; i < count; i++) deck.Add(m.Canonical);
        return true;
    }

    /// <summary>Resolve a card and remove ONE matching copy (exact spec, else base-name). Returns true if changed.</summary>
    private bool CustomRemove(List<string> deck, string arg)
    {
        if (string.IsNullOrWhiteSpace(arg)) { Note("usage: rm <card>"); return false; }
        var m = CardNameMatcher.Resolve(arg);
        var target = m.Canonical;
        if (target == null) { Note(m.Suggestions.Count > 0 ? $"'{arg}'? did you mean: {string.Join(", ", m.Suggestions)}" : $"unknown card '{arg}'"); return false; }
        int idx = deck.FindIndex(s => string.Equals(s, target, StringComparison.OrdinalIgnoreCase));
        if (idx < 0)
        {
            // Fall back to base-name (ignore upgrade) so "rm bash" removes "Bash+1".
            string baseName = target.Split('+')[0];
            idx = deck.FindIndex(s => string.Equals(s.Split('+')[0], baseName, StringComparison.OrdinalIgnoreCase));
        }
        if (idx < 0) { Note($"'{target}' isn't in the deck"); return false; }
        deck.RemoveAt(idx);
        return true;
    }

    private static bool TryParseAct(string arg, out int actIndex)
    {
        actIndex = 0;
        if (string.IsNullOrWhiteSpace(arg)) return false;
        if (int.TryParse(arg, out int n) && n >= 1 && n <= Catalog.ActThemes.Count) { actIndex = n - 1; return true; }
        for (int i = 0; i < Catalog.ActThemes.Count; i++)
            if (Catalog.ActThemes[i].StartsWith(arg, StringComparison.OrdinalIgnoreCase)) { actIndex = i; return true; }
        return false;
    }

    /// <summary>One-shot reward check for cards given inline (e.g. "c Whirlwind Inflame").</summary>
    private void ShowRewardCheckFor(Context ctx, string arg)
    {
        Spectre.Console.AnsiConsole.Clear();
        var cards = new List<string>();
        foreach (var tok in arg.Split(' ', StringSplitOptions.RemoveEmptyEntries))
        {
            var m = CardNameMatcher.Resolve(tok);
            if (m.Canonical != null) cards.Add(m.Canonical);
        }
        if (cards.Count == 0 || ctx.StrengthPool.Count == 0) { Note("nothing to check"); return; }
        (double, List<Advisor.PickItem>) advice = default;
        Spectre.Console.AnsiConsole.Status().Start("checking the reward…",
            _ => advice = Advisor.PickAdvice(ctx.DeckSpecs, cards, ctx.StrengthPool, ctx.Run.MaxEnergy, ctx.RelicNames, _opts));
        Dashboard.RenderPick(advice);
        WaitForKey();
    }

    private static void ShowDeckList(List<string> deck)
    {
        Spectre.Console.AnsiConsole.Clear();
        Spectre.Console.AnsiConsole.MarkupLine($"[grey]Deck ({deck.Count} cards):[/] {Spectre.Console.Markup.Escape(SummarizeSpecs(deck))}");
        WaitForKey();
    }

    private static void ShowCustomHelp()
    {
        Spectre.Console.AnsiConsole.Clear();
        var lines = new[]
        {
            "[bold]Custom deck mode[/] — build a deck by hand and watch its strength.",
            "",
            "  [white]+<card>[/] / [white]add <card> [[xN]][/]   add a card (Tab completes, typos auto-correct)",
            "  [white]-<card>[/] / [white]rm <card>[/]          remove one copy",
            "  [white]<card>[/]                       (bare name) add a card",
            "  [white]deck[/]                         list the current deck",
            "  [white]act <1-4 | name>[/]             choose which Act's elites to score against",
            "  [white]char <name>[/]                  switch character (resets to its starter deck)",
            "  [white]hp <n>[/]   [white]asc <n>[/]            set HP / ascension",
            "  [white]reset[/]                        back to the starter deck",
            "  [white]r[/]                            best cards to remove (cuts)",
            "  [white]c <card…>[/]                    check reward card(s) take-vs-skip",
            "  [white]q[/]                            quit",
            "",
            $"[grey]characters: {string.Join(", ", Catalog.CharacterProfiles.Select(p => p.Key))}[/]",
            $"[grey]acts: {string.Join(", ", Catalog.ActThemes.Select((t, i) => $"{i + 1}={t}"))}[/]",
        };
        foreach (var l in lines) Spectre.Console.AnsiConsole.MarkupLine(l);
        WaitForKey();
    }

    private static void Note(string msg)
    {
        Spectre.Console.AnsiConsole.MarkupLine($"  [yellow]{Spectre.Console.Markup.Escape(msg)}[/]");
        Thread.Sleep(900);
    }

    private List<EliteResult> _elites = new();
    private double _strength = double.NaN;
    private string? _footer;     // null = the default live-watch footer; set by custom mode to its own command hint.

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

        Dashboard.Render(ctx, _elites, _strength, evaluating: true, _footer);
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

    private void RenderCurrent(Context ctx) => Dashboard.Render(ctx, _elites, _strength, evaluating: false, _footer);

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

    /// <summary>[u] best cards to upgrade — the slow per-card sweep, run only on demand.</summary>
    private void ShowUpgrades(Context ctx)
    {
        Spectre.Console.AnsiConsole.Clear();
        if (ctx.StrengthPool.Count == 0 || ctx.DeckSpecs.Count == 0)
        {
            Spectre.Console.AnsiConsole.MarkupLine("[grey]Upgrade advice needs at least one elite and one card.[/]");
        }
        else
        {
            (double, List<Advisor.UpgradeItem>) advice = default;
            Spectre.Console.AnsiConsole.Status().Start("finding the best cards to upgrade…",
                _ => advice = Advisor.UpgradeAdvice(ctx.DeckSpecs, ctx.StrengthPool,
                    ctx.Run.MaxEnergy, ctx.RelicNames, _opts));
            Dashboard.RenderUpgrades(advice);
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
        if (Console.IsInputRedirected) { Console.ReadLine(); return; }   // piped input: no key events
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
