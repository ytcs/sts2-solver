using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;

namespace Sts2Solver.Ranwid;

/// <summary>
/// The shared data + compute layer behind the ranwid dashboard. It parses an ongoing run's save into a scorable
/// <see cref="Context"/> (deck specs, relics, the Act's elite/boss encounters, and the representative strength
/// pool), and evaluates rows against it. Pure functions only — no UI: the Terminal.Gui front-end
/// (<see cref="Tui.RanwidApp"/>) drives the presentation, streaming each row's result in as it finishes, and the
/// custom-deck sandbox reuses <see cref="BuildCustom"/> + the deck-edit helpers.
/// </summary>
public static class Companion
{
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

    // ── Row evaluation ────────────────────────────────────────────────────────────

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

    /// <summary>Evaluate the current deck against each Act elite (the data behind the dashboard's elite table).
    /// Pure compute — no output. The boss headlines as the first row, at the run's actual HP.</summary>
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

    // ── Custom-deck sandbox helpers (no save file) ────────────────────────────────
    //
    // ranwid --custom [character] starts from a character's STARTER deck + relic + HP and edits it by hand. These
    // pure helpers (Spectre-free) back the TUI's deck editor: build a scorable Context from the hand-built inputs,
    // and add/remove cards by resolved name. For multiplayer GUESTS (whose run isn't saved locally) and for
    // deck-building generally.

    /// <summary>Build a custom Context: a synthesized RunState (display only) + the chosen act's elite pool as
    /// both the dashboard rows and the strength pool.</summary>
    public static Context BuildCustom(CharacterProfile profile, int actIndex, int asc, int hp, List<string> deck)
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

    /// <summary>Resolve "card [xN] | card N", add N copies (default 1), and return a status message. Returns
    /// ok=false (with the reason) when the card is unknown / unbuildable. Used by the TUI custom-deck editor.</summary>
    public static (bool ok, string msg) TryCustomAdd(List<string> deck, string arg)
    {
        if (string.IsNullOrWhiteSpace(arg)) return (false, "usage: add <card> [xN]");
        // Trailing count: "Strike x3" or "Strike 3".
        int count = 1;
        var toks = arg.Split(' ', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        if (toks.Length >= 2)
        {
            var last = toks[^1].TrimStart('x', 'X');
            if (int.TryParse(last, out int n) && n > 0) { count = Math.Min(n, 100); arg = string.Join(' ', toks[..^1]); }
        }
        var m = CardNameMatcher.Resolve(arg);
        if (m.Canonical == null) return (false, m.Suggestions.Count > 0 ? $"'{arg}'? did you mean: {string.Join(", ", m.Suggestions)}" : $"unknown card '{arg}'");
        try { Catalog.BuildCard(m.Canonical); } catch (ArgumentException) { return (false, $"'{m.Canonical}' isn't a buildable card"); }
        for (int i = 0; i < count; i++) deck.Add(m.Canonical);
        return (true, count > 1 ? $"added {count}× {m.Canonical}" : $"added {m.Canonical}");
    }

    /// <summary>Resolve a card and remove ONE matching copy (exact spec, else base-name so "rm bash" removes
    /// "Bash+1"), returning a status message. Used by the TUI custom-deck editor.</summary>
    public static (bool ok, string msg) TryCustomRemove(List<string> deck, string arg)
    {
        if (string.IsNullOrWhiteSpace(arg)) return (false, "usage: rm <card>");
        var m = CardNameMatcher.Resolve(arg);
        var target = m.Canonical;
        if (target == null) return (false, m.Suggestions.Count > 0 ? $"'{arg}'? did you mean: {string.Join(", ", m.Suggestions)}" : $"unknown card '{arg}'");
        int idx = deck.FindIndex(s => string.Equals(s, target, StringComparison.OrdinalIgnoreCase));
        if (idx < 0)
        {
            string baseName = target.Split('+')[0];
            idx = deck.FindIndex(s => string.Equals(s.Split('+')[0], baseName, StringComparison.OrdinalIgnoreCase));
        }
        if (idx < 0) return (false, $"'{target}' isn't in the deck");
        deck.RemoveAt(idx);
        return (true, $"removed {target}");
    }

    /// <summary>Resolve an act argument ("2" or a theme-name prefix) to a 0-based act index.</summary>
    public static bool TryParseAct(string arg, out int actIndex)
    {
        actIndex = 0;
        if (string.IsNullOrWhiteSpace(arg)) return false;
        if (int.TryParse(arg, out int n) && n >= 1 && n <= Catalog.ActThemes.Count) { actIndex = n - 1; return true; }
        for (int i = 0; i < Catalog.ActThemes.Count; i++)
            if (Catalog.ActThemes[i].StartsWith(arg, StringComparison.OrdinalIgnoreCase)) { actIndex = i; return true; }
        return false;
    }
}
