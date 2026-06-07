using System.Collections.ObjectModel;
using Sts2Solver.Content;
using Sts2Solver.Search;
using Terminal.Gui.App;
using Terminal.Gui.Input;
using Terminal.Gui.Views;
using Terminal.Gui.ViewBase;

namespace Sts2Solver.Ranwid.Tui;

// The legacy static Application.* lifecycle API is marked obsolete in Terminal.Gui v2.4 ("going away"), but it is
// the documented, stable surface today and the instance API (Application.Instance) does not yet expose a clean
// Shutdown. Suppress the deprecation here and migrate when the replacement lands.
#pragma warning disable CS0618

/// <summary>
/// The Terminal.Gui front-end for ranwid (<c>ranwid --tui</c>). A persistent, always-on dashboard: deck-strength
/// bar, run header, deck, the act boss (its own panel), and the per-elite survival / HP-loss list. Unlike the
/// Spectre screen it is <b>non-blocking</b> — each elite (and the strength index) is evaluated on a background
/// task and its number streams into the view as it finishes (<c>Application.Invoke</c> marshals the result back
/// to the UI thread), so you can navigate while the solver runs. A save-file watcher reloads + re-evaluates when
/// the run changes. Reuses the whole <see cref="Companion"/> / <see cref="Advisor"/> compute layer unchanged.
/// </summary>
public sealed class RanwidApp
{
    private readonly string? _fixedPath, _saveDir;
    private readonly int? _netId;
    private readonly EvalOptions _opts;
    private readonly TuiState _state = new();

    // Views (created in BuildViews).
    private Window _win = null!;
    private Label _strengthLabel = null!, _nextStrengthLabel = null!, _headerLabel = null!, _relicsLabel = null!;
    private FrameView _deckFrame = null!;
    private Label[] _deckLines = null!;        // one per card type (Attack/Skill/Power/Status/Curse)
    private FrameView _bossFrame = null!;
    private Label _bossLabel = null!;
    private FrameView _elitesFrame = null!;
    private ListView _elitesList = null!;
    private Label _footer = null!;

    // Each StartEval bumps the generation; background results whose generation is stale are discarded on arrival,
    // since the in-flight MCTS solves cannot themselves be cancelled mid-trial.
    private int _gen;

    // Strength is recomputed independently of a full load (toggling an elite's include/exclude re-runs only it),
    // so it has its own generation guard. A load bumps both (via StartEval → RecomputeStrength).
    private int _strengthGen;

    // Next-act strength depends only on deck + act (not on toggles), so it is computed once per load under its own
    // generation guard.
    private int _nextGen;

    // Save watcher → flips _dirty; the UI-thread timer picks it up.
    private volatile bool _dirty;
    private string? _loadedKey;
    private FileSystemWatcher? _watcher;

    public RanwidApp(string? fixedPath, string? saveDir, int? netId, EvalOptions opts)
    {
        _fixedPath = fixedPath; _saveDir = saveDir; _netId = netId; _opts = opts;
    }

    public int Run()
    {
        Application.Init();
        try
        {
            BuildViews();
            StartWatcher();
            LoadAndEval(initial: true);
            Application.AddTimeout(TimeSpan.FromMilliseconds(300), () => { Tick(); return true; });
            Application.Run(_win);
        }
        finally
        {
            if (_watcher != null) { _watcher.EnableRaisingEvents = false; _watcher.Dispose(); }
            _win?.Dispose();
            Application.Shutdown();
        }
        return 0;
    }

    // ── Layout ────────────────────────────────────────────────────────────────────

    private void BuildViews()
    {
        _win = new Window { Title = " ranwid — live Slay the Spire 2 advisor " };
        _win.SetScheme(TuiFormat.Base);

        var top = new FrameView { Title = " Run ", X = 0, Y = 0, Width = Dim.Fill(), Height = 6 };
        _strengthLabel     = new Label { X = 1, Y = 0, Width = Dim.Fill(1), Height = 1 };
        _nextStrengthLabel = new Label { X = 1, Y = 1, Width = Dim.Fill(1), Height = 1 };
        _headerLabel       = new Label { X = 1, Y = 2, Width = Dim.Fill(1), Height = 1 };
        _relicsLabel       = new Label { X = 1, Y = 3, Width = Dim.Fill(1), Height = 1 };
        top.Add(_strengthLabel, _nextStrengthLabel, _headerLabel, _relicsLabel);

        _deckFrame = new FrameView { Title = " Deck ", X = 0, Y = Pos.Bottom(top), Width = Dim.Fill(), Height = 7 };
        _deckLines = new Label[5];
        for (int i = 0; i < _deckLines.Length; i++)
        {
            _deckLines[i] = new Label { X = 1, Y = i, Width = Dim.Fill(1), Height = 1 };
            _deckFrame.Add(_deckLines[i]);
        }

        _bossFrame = new FrameView { Title = " Boss ", X = 0, Y = Pos.Bottom(_deckFrame), Width = Dim.Fill(), Height = 3 };
        _bossLabel = new Label { X = 1, Y = 0, Width = Dim.Fill(1), Height = 1 };
        _bossFrame.Add(_bossLabel);

        _elitesFrame = new FrameView { Title = " Elites ", X = 0, Y = Pos.Bottom(_bossFrame), Width = Dim.Fill(), Height = Dim.Fill(1) };
        _elitesList = new ListView { X = 0, Y = 0, Width = Dim.Fill(), Height = Dim.Fill() };
        _elitesList.SetScheme(TuiFormat.Base);
        _elitesFrame.Add(_elitesList);

        _footer = new Label { X = 0, Y = Pos.AnchorEnd(1), Width = Dim.Fill(), Height = 1 };

        _win.Add(top, _deckFrame, _bossFrame, _elitesFrame, _footer);
        _win.KeyDown += OnKey;
        _elitesList.KeyDown += OnElitesKey;
    }

    private void OnKey(object? sender, Key key)
    {
        if (key == Key.Q || key == Key.Esc) { Application.RequestStop(); return; }
        if (key == Key.D) { _dirty = false; LoadAndEval(force: true); }
    }

    /// <summary>Space toggles whether the selected elite counts toward the current-act deck strength, then
    /// re-evaluates only the strength index (non-blocking).</summary>
    private void OnElitesKey(object? sender, Key key)
    {
        if (key != Key.Space || _state.Ctx is not { } c) return;
        if (_elitesList.SelectedItem is int i && i >= 0 && i < _state.Elites.Count)
        {
            _state.Elites[i].Included = !_state.Elites[i].Included;
            RenderElites();
            RecomputeStrength(c);
            _win.SetNeedsDraw();
            key.Handled = true;
        }
    }

    // ── Load + evaluate ─────────────────────────────────────────────────────────────

    /// <summary>Resolve the newest save, load it, and kick off a non-blocking re-evaluation. No-ops when the save
    /// hasn't changed (unless <paramref name="force"/>/<paramref name="initial"/>). When the run ends / the save is
    /// momentarily unreadable, keeps the last good dashboard rather than blanking.</summary>
    private void LoadAndEval(bool initial = false, bool force = false)
    {
        var path = _fixedPath ?? SaveSource.FindNewest(_saveDir);
        if (path == null)
        {
            _state.Ctx = null;
            _state.Status = "waiting for a readable run… (start a run, or pass --save / --save-dir)";
            _loadedKey = null;
            RenderAll();
            return;
        }

        var key = SaveKey(path);
        if (!force && !initial && key == _loadedKey) return;

        var ctx = Companion.Load(path, _netId, quiet: true);
        if (ctx == null)
        {
            if (_state.Ctx == null) { _state.Status = "waiting for a readable run…"; RenderAll(); }
            return;   // keep the last good dashboard
        }

        _loadedKey = key;
        _state.Ctx = ctx;
        _state.Status = null;
        StartEval(ctx);
    }

    /// <summary>Fan the deck-strength index + boss + each elite out onto background tasks; each posts its own
    /// result back to the UI thread as it completes. Stale results (a newer load happened) are dropped by gen.</summary>
    private void StartEval(Companion.Context ctx)
    {
        int gen = ++_gen;

        // Elites first, so the strength pool can read each row's Included flag (all included on a fresh load).
        var specs = Companion.EliteRowSpecs(ctx);
        _state.Elites = specs.Select(s => new TuiState.RowState(s.Label, s.Comp) { Busy = true, Enc = s.Enc }).ToList();
        for (int i = 0; i < specs.Count; i++)
        {
            int idx = i; var spec = specs[i];
            Task.Run(() =>
            {
                var r = Companion.EvaluateRow(ctx, spec, _opts);
                Post(gen, () => { _state.Elites[idx].Result = r; _state.Elites[idx].Busy = false; RenderElites(); });
            });
        }

        var bossSpec = Companion.BossRowSpec(ctx);
        _state.Boss = null; _state.BossBusy = bossSpec != null;
        if (bossSpec is { } bs)
            Task.Run(() =>
            {
                var r = Companion.EvaluateRow(ctx, bs, _opts);
                Post(gen, () => { _state.Boss = r; _state.BossBusy = false; RenderBoss(); });
            });

        // Current-act strength = average over the included elites + the known boss (#5/#7). Recomputed live on
        // toggle. This curates against "the fights I'll actually face", unlike the Spectre path's representative pool.
        RecomputeStrength(ctx);
        StartNextStrength(ctx);
        RenderAll();
    }

    /// <summary>Project deck strength onto the NEXT act: averaged over that act's whole elite pool plus a single
    /// averaged-boss term (we don't yet know which boss). Independent of the elite toggles, so it runs once per
    /// load. NaN / hidden on the final act.</summary>
    private void StartNextStrength(Companion.Context ctx)
    {
        int ngen = ++_nextGen;
        int nextAct = ctx.Run.ActIndex + 1;
        if (nextAct >= Catalog.ActThemes.Count)
        {
            _state.HasNextAct = false; _state.NextStrength = double.NaN; _state.NextStrengthBusy = false;
            RenderTop();
            return;
        }

        _state.HasNextAct = true; _state.NextStrengthBusy = true;
        int asc = ctx.Run.Ascension;
        var elites = Companion.EncountersForClasses(Catalog.ActElitePool(nextAct), asc);
        var bosses = Companion.EncountersForClasses(Catalog.ActBossPool(nextAct), asc);
        var deckSpecs = ctx.DeckSpecs; int energy = ctx.Run.MaxEnergy; var relics = ctx.RelicNames;
        Task.Run(() =>
        {
            var s = Advisor.DeckStrengthNextAct(deckSpecs, elites, bosses, energy, relics, _opts);
            PostNext(ngen, () => { _state.NextStrength = s; _state.NextStrengthBusy = false; RenderTop(); });
        });
    }

    private void PostNext(int ngen, Action a) => Application.Invoke(() =>
    {
        if (ngen != _nextGen) return;
        a();
        _win.SetNeedsDraw();
    });

    /// <summary>Re-evaluate only the current-act deck-strength index over the curated pool (included elites + boss),
    /// on a background task. Its own generation guard means a newer load or toggle supersedes an in-flight run.</summary>
    private void RecomputeStrength(Companion.Context ctx)
    {
        int sgen = ++_strengthGen;
        var pool = CuratedPool(ctx);
        if (pool.Count == 0) { _state.Strength = double.NaN; _state.StrengthBusy = false; RenderTop(); return; }

        _state.StrengthBusy = true;
        var deckSpecs = ctx.DeckSpecs; int energy = ctx.Run.MaxEnergy; var relics = ctx.RelicNames;
        Task.Run(() =>
        {
            var s = Advisor.DeckStrength(deckSpecs, pool, energy, relics, _opts);
            PostStrength(sgen, () => { _state.Strength = s; _state.StrengthBusy = false; RenderTop(); });
        });
        RenderTop();
    }

    /// <summary>The encounters the current-act strength averages over: each included elite, plus the run's known
    /// boss (always counted, per #5). An unported boss (no <see cref="Companion.Context.Boss"/>) is excluded.</summary>
    private List<Advisor.Encounter> CuratedPool(Companion.Context ctx)
    {
        var pool = new List<Advisor.Encounter>();
        foreach (var row in _state.Elites)
            if (row.Included && row.Enc is { } e) pool.Add(e);
        if (ctx.Boss is { } boss) pool.Add(boss);
        return pool;
    }

    /// <summary>Run <paramref name="a"/> on the UI thread, but only if it belongs to the current load generation.</summary>
    private void Post(int gen, Action a) => Application.Invoke(() =>
    {
        if (gen != _gen) return;
        a();
        _win.SetNeedsDraw();
    });

    /// <summary>As <see cref="Post"/>, but guarded by the strength generation (toggles supersede each other).</summary>
    private void PostStrength(int sgen, Action a) => Application.Invoke(() =>
    {
        if (sgen != _strengthGen) return;
        a();
        _win.SetNeedsDraw();
    });

    private void Tick()
    {
        if (_dirty) { _dirty = false; LoadAndEval(); }
    }

    // ── Rendering (UI thread only) ───────────────────────────────────────────────────

    private void RenderAll()
    {
        RenderTop();
        RenderDeck();
        RenderBoss();
        RenderElites();
        RenderFooter();
        _win.SetNeedsDraw();
    }

    private void RenderTop()
    {
        if (_state.Ctx is not { } c)
        {
            _strengthLabel.Text = "  " + (_state.Status ?? "");
            _strengthLabel.SetScheme(TuiFormat.SchemeOf(TuiFormat.Grey));
            _headerLabel.Text = ""; _relicsLabel.Text = "";
            return;
        }

        var run = c.Run;
        if (!_state.StrengthBusy && double.IsNaN(_state.Strength))
            _strengthLabel.Text = "Strength · this act   n/a  (no elites selected — space a row to include one)";
        else
            _strengthLabel.Text = "Strength · this act   " + TuiFormat.StrengthBar(_state.Strength)
                + (_state.StrengthBusy ? "  (updating…)" : "");
        _strengthLabel.SetScheme(TuiFormat.SchemeOf(TuiFormat.StrengthColor(_state.Strength)));

        if (!_state.HasNextAct)
            _nextStrengthLabel.Text = "Strength · next act   —  (final act)";
        else if (!_state.NextStrengthBusy && double.IsNaN(_state.NextStrength))
            _nextStrengthLabel.Text = "Strength · next act   n/a";
        else
            _nextStrengthLabel.Text = "Strength · next act   " + TuiFormat.StrengthBar(_state.NextStrength)
                + (_state.NextStrengthBusy ? "  (updating…)" : "");
        _nextStrengthLabel.SetScheme(TuiFormat.SchemeOf(
            _state.HasNextAct ? TuiFormat.StrengthColor(_state.NextStrength) : TuiFormat.Grey));

        _headerLabel.Text = $"{GameIds.CharacterName(run.Character)}  ·  A{run.Ascension}  ·  Act {run.ActIndex + 1}"
            + $"  ·  {run.PlayerHp}/{run.PlayerMaxHp} HP  ·  {run.MaxEnergy} energy"
            + (run.PlayerCount > 1 ? $"   (multiplayer ×{run.PlayerCount}, shown as single-player)" : "");
        _headerLabel.SetScheme(TuiFormat.Base);

        string relics = run.RelicIds.Count == 0 ? "none"
            : c.RelicNames.Count > 0 ? string.Join(", ", c.RelicNames) : "none modelled";
        _relicsLabel.Text = "relics: " + relics;
        _relicsLabel.SetScheme(TuiFormat.SchemeOf(TuiFormat.Grey));
        _deckFrame.Title = $" Deck — {run.Deck.Count} cards ";
    }

    private void RenderDeck()
    {
        foreach (var l in _deckLines) l.Text = "";
        if (_state.Ctx is not { } c) return;
        if (c.DeckSpecs.Count == 0)
        {
            _deckLines[0].Text = "(no cards)";
            _deckLines[0].SetScheme(TuiFormat.SchemeOf(TuiFormat.Grey));
            return;
        }

        var lines = DeckView.Lines(c.DeckSpecs);
        int w = WrapWidth();
        for (int i = 0; i < _deckLines.Length && i < lines.Count; i++)
        {
            var (type, text) = lines[i];
            _deckLines[i].Text = Clip(text, w);
            _deckLines[i].SetScheme(TuiFormat.SchemeOf(TuiFormat.CardTypeColor(type)));
        }
    }

    private void RenderBoss()
    {
        if (_state.Ctx == null) { _bossLabel.Text = ""; return; }
        if (_state.BossBusy) { _bossLabel.Text = "  evaluating…"; _bossLabel.SetScheme(TuiFormat.SchemeOf(TuiFormat.Grey)); return; }
        if (_state.Boss is not { } r) { _bossLabel.Text = "  (no act boss for this run)"; _bossLabel.SetScheme(TuiFormat.SchemeOf(TuiFormat.Grey)); return; }

        if (r.Stats is { } s)
        {
            _bossLabel.Text = $"{r.Elite,-26}  survive {TuiFormat.Pct(s.Survival),4}   E[HP loss] {s.MeanLoss:F0}";
            _bossLabel.SetScheme(TuiFormat.SchemeOf(TuiFormat.SurviveColor(s.Survival)));
        }
        else
        {
            _bossLabel.Text = $"{r.Elite,-26}  {r.Skipped ?? "—"}";
            _bossLabel.SetScheme(TuiFormat.SchemeOf(TuiFormat.Grey));
        }
    }

    private void RenderElites()
    {
        var items = new ObservableCollection<string>();
        if (_state.Ctx == null) { /* leave empty */ }
        else if (_state.Elites.Count == 0) items.Add("(no elites to evaluate for this Act)");
        else foreach (var row in _state.Elites) items.Add(EliteListLine(row));

        int sel = _elitesList.SelectedItem ?? 0;
        _elitesList.SetSource(items);
        if (items.Count > 0) _elitesList.SelectedItem = Math.Clamp(sel, 0, items.Count - 1);
        _elitesFrame.Title = AnyBusy() ? " Elites — evaluating… " : " Elites ";
    }

    private void RenderFooter()
    {
        _footer.Text = "  ↑↓ select   (space) include/exclude elite   (d) refresh   (q) quit"
            + "        ·  auto-refreshes on run change        [r/u/c advice — coming]";
        _footer.SetScheme(TuiFormat.SchemeOf(TuiFormat.Grey));
    }

    private bool AnyBusy() => _state.StrengthBusy || _state.BossBusy || _state.Elites.Any(e => e.Busy);

    private static string EliteListLine(TuiState.RowState row)
    {
        string box = row.Included ? "[x] " : "[ ] ";
        string name = row.Comp.Length > 0 && !string.Equals(row.Comp, row.Label, StringComparison.OrdinalIgnoreCase)
            ? $"{row.Label} ({row.Comp})" : row.Label;
        if (name.Length > 40) name = name[..39] + "…";
        if (row.Busy || row.Result == null) return $"  {box}{name,-42}  …";
        var r = row.Result;
        string excl = row.Included ? "" : "  (excluded)";
        if (r.Stats is { } s) return $"  {box}{name,-42}  survive {TuiFormat.Pct(s.Survival),4}   E[HP loss] {s.MeanLoss,3:F0}{excl}";
        return $"  {box}{name,-42}  {r.Skipped ?? "—"}";
    }

    // ── Watcher ──────────────────────────────────────────────────────────────────────

    private void StartWatcher()
    {
        try
        {
            var dir = _saveDir ?? (_fixedPath != null ? Path.GetDirectoryName(_fixedPath) : null)
                      ?? Path.GetDirectoryName(SaveSource.FindNewest(_saveDir) ?? "");
            if (!string.IsNullOrEmpty(dir) && Directory.Exists(dir))
            {
                var fsw = new FileSystemWatcher(dir, "*.save")
                {
                    NotifyFilter = NotifyFilters.LastWrite | NotifyFilters.Size | NotifyFilters.FileName | NotifyFilters.CreationTime,
                    IncludeSubdirectories = _saveDir != null,
                    EnableRaisingEvents = true,
                };
                void Touch(object? _, FileSystemEventArgs __) => _dirty = true;
                fsw.Changed += Touch; fsw.Created += Touch; fsw.Renamed += (_, __) => _dirty = true;
                _watcher = fsw;
            }
        }
        catch { /* rely on the poll fallback */ }

        var t = new Thread(() =>
        {
            while (true)
            {
                Thread.Sleep(2000);
                try { var p = _fixedPath ?? SaveSource.FindNewest(_saveDir); if (p != null && SaveKey(p) != _loadedKey) _dirty = true; }
                catch { /* transient FS errors */ }
            }
        }) { IsBackground = true, Name = "ranwid-tui-watch" };
        t.Start();
    }

    private static string SaveKey(string path)
    {
        try { return $"{path}|{File.GetLastWriteTimeUtc(path).Ticks}"; } catch { return path; }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────────────

    private static int WrapWidth()
    {
        try { int w = Application.Screen.Width; return w > 12 ? w - 6 : 80; } catch { return 80; }
    }

    /// <summary>Greedy word-wrap to <paramref name="width"/>, capped at <paramref name="maxLines"/> (last line gets
    /// an ellipsis if more would follow).</summary>
    private static string WrapToLines(string text, int width, int maxLines)
    {
        if (width < 8) width = 8;
        var words = text.Split(' ', StringSplitOptions.RemoveEmptyEntries);
        var lines = new List<string>();
        var cur = new System.Text.StringBuilder();
        bool truncated = false;
        foreach (var w in words)
        {
            if (cur.Length > 0 && cur.Length + 1 + w.Length > width)
            {
                lines.Add(cur.ToString()); cur.Clear();
                if (lines.Count == maxLines) { truncated = true; break; }
            }
            if (cur.Length > 0) cur.Append(' ');
            cur.Append(w);
        }
        if (!truncated && cur.Length > 0) lines.Add(cur.ToString());
        if (truncated && lines.Count > 0) lines[^1] = Trim(lines[^1], width - 1) + "…";
        return string.Join("\n", lines);
    }

    private static string Trim(string s, int n) => s.Length <= n ? s : s[..Math.Max(0, n)];

    /// <summary>Clip a single line to <paramref name="width"/>, appending an ellipsis when truncated.</summary>
    private static string Clip(string s, int width) => s.Length <= width ? s : Trim(s, Math.Max(1, width - 1)) + "…";
}
