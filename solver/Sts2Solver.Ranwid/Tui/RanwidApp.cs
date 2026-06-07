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
/// bar, run header, deck, the act boss (its own panel), and the per-elite survival / HP-loss list. It is
/// <b>non-blocking</b> — each elite (and the strength index) is evaluated on a background
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

    // Custom-deck sandbox (ranwid --custom): no save file — the deck is built/edited by hand and the Context is
    // rebuilt on every edit. _custom selects this mode; the fields below are the editable inputs.
    private readonly bool _custom;
    private readonly string? _initialChar;
    private CharacterProfile _cProfile = null!;
    private int _cAct, _cAsc, _cHp;
    private List<string> _cDeck = new();

    // Views (created in BuildViews).
    private Window _win = null!;
    private Label _strengthLabel = null!, _nextStrengthLabel = null!, _headerLabel = null!, _relicsLabel = null!;
    private FrameView _deckFrame = null!;
    private Label[] _deckLines = null!;        // one per card type (Attack/Skill/Power/Status/Curse)
    private FrameView _bossFrame = null!;
    private Label _bossLabel = null!;
    private FrameView _elitesFrame = null!;
    private ListView _elitesList = null!;
    private TextField _pathField = null!;     // manual save-folder entry (save-not-found panel)
    private TextField _editField = null!;     // custom-deck command entry (+card / act N / …)
    private FrameView _adviceFrame = null!;   // overlays the elites region while showing removal/upgrade/reward advice
    private ListView _adviceList = null!;
    private TextField _rewardField = null!;   // reward-card entry (visible only in reward-entry mode)
    private Label _footer = null!;
    private int _adviceGen;

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
    private string? _overrideDir;   // a save folder typed at the save-not-found panel (takes precedence)

    public RanwidApp(string? fixedPath, string? saveDir, int? netId, EvalOptions opts)
    {
        _fixedPath = fixedPath; _saveDir = saveDir; _netId = netId; _opts = opts;
    }

    /// <summary>Custom-deck sandbox (<c>ranwid --custom [character]</c>): no save file. Starts from a character's
    /// starter deck and edits it by hand (press <c>e</c>). For multiplayer guests (whose run isn't saved locally)
    /// and for deck-building. Reuses the whole dashboard/advice machinery; only the source of the deck differs.</summary>
    public RanwidApp(EvalOptions opts, string? initialChar)
    {
        _opts = opts; _custom = true; _initialChar = initialChar;
    }

    public int Run()
    {
        // The dashboard fans many single-threaded MCTS solves (each elite, the boss, and the current- and next-act
        // strength projections) onto the thread pool at once. The pool injects new threads only ~1–2/sec, which
        // would dribble the elite rows in over tens of seconds (looking "stuck"); raise the floor so the first wave
        // all starts immediately. Single-threaded MCTS just time-slices when oversubscribed — no deadlock risk.
        ThreadPool.GetMinThreads(out int wmin, out int iomin);
        ThreadPool.SetMinThreads(Math.Max(wmin, Environment.ProcessorCount * 2 + 4), iomin);

        Application.Init();
        try
        {
            BuildViews();
            if (_custom)
            {
                InitCustom();
                RebuildCustom();
            }
            else
            {
                StartWatcher();
                LoadAndEval(initial: true);
            }
            Application.AddTimeout(TimeSpan.FromMilliseconds(300), () => { Tick(); return true; });
            Application.Run(_win);
        }
        finally
        {
            Application.KeyDown -= OnGlobalKey;
            if (_watcher != null) { _watcher.EnableRaisingEvents = false; _watcher.Dispose(); }
            _win?.Dispose();
            Application.Shutdown();
        }
        return 0;
    }

    // ── Layout ────────────────────────────────────────────────────────────────────

    private void BuildViews()
    {
        _win = new Window { Title = _custom ? " ranwid — custom deck sandbox " : " ranwid — live Slay the Spire 2 advisor " };
        _win.SetScheme(TuiFormat.Base);

        // The Run/Deck/Boss panels are display-only — make them non-focusable so Tab and the initial auto-focus
        // skip straight to the elites list (the only thing the arrow keys act on). Their child Labels are already
        // non-focusable, so this just stops the FrameView itself from becoming a tab stop.
        var top = new FrameView { Title = " Run ", X = 0, Y = 0, Width = Dim.Fill(), Height = 6, CanFocus = false };
        _strengthLabel     = new Label { X = 1, Y = 0, Width = Dim.Fill(1), Height = 1 };
        _nextStrengthLabel = new Label { X = 1, Y = 1, Width = Dim.Fill(1), Height = 1 };
        _headerLabel       = new Label { X = 1, Y = 2, Width = Dim.Fill(1), Height = 1 };
        _relicsLabel       = new Label { X = 1, Y = 3, Width = Dim.Fill(1), Height = 1 };
        top.Add(_strengthLabel, _nextStrengthLabel, _headerLabel, _relicsLabel);

        _deckFrame = new FrameView { Title = " Deck ", X = 0, Y = Pos.Bottom(top), Width = Dim.Fill(), Height = 7, CanFocus = false };
        _deckLines = new Label[5];
        for (int i = 0; i < _deckLines.Length; i++)
        {
            _deckLines[i] = new Label { X = 1, Y = i, Width = Dim.Fill(1), Height = 1 };
            _deckFrame.Add(_deckLines[i]);
        }

        _bossFrame = new FrameView { Title = " Boss ", X = 0, Y = Pos.Bottom(_deckFrame), Width = Dim.Fill(), Height = 3, CanFocus = false };
        _bossLabel = new Label { X = 1, Y = 0, Width = Dim.Fill(1), Height = 1 };
        _bossFrame.Add(_bossLabel);

        _elitesFrame = new FrameView { Title = " Elites ", X = 0, Y = Pos.Bottom(_bossFrame), Width = Dim.Fill(), Height = Dim.Fill(1) };
        _pathField = new TextField { X = 1, Y = 0, Width = Dim.Fill(2), Height = 1, Visible = false };
        _pathField.KeyDown += OnPathKey;
        _editField = new TextField { X = 1, Y = 0, Width = Dim.Fill(2), Height = 1, Visible = false };
        _editField.KeyDown += OnEditKey;
        _elitesList = new ListView { X = 0, Y = 0, Width = Dim.Fill(), Height = Dim.Fill() };
        _elitesList.SetScheme(TuiFormat.Base);
        _elitesFrame.Add(_pathField, _editField, _elitesList);

        // Advice overlay — same geometry as the elites frame, shown in its place (deck/strength panels persist).
        _adviceFrame = new FrameView { Title = " Advice ", X = 0, Y = Pos.Bottom(_bossFrame), Width = Dim.Fill(), Height = Dim.Fill(1), Visible = false };
        _rewardField = new TextField { X = 1, Y = 0, Width = Dim.Fill(2), Height = 1, Visible = false };
        _rewardField.KeyDown += OnRewardKey;
        _adviceList = new ListView { X = 0, Y = 0, Width = Dim.Fill(), Height = Dim.Fill() };
        _adviceList.SetScheme(TuiFormat.Base);
        _adviceFrame.Add(_rewardField, _adviceList);

        _footer = new Label { X = 0, Y = Pos.AnchorEnd(1), Width = Dim.Fill(), Height = 1 };

        _win.Add(top, _deckFrame, _bossFrame, _elitesFrame, _adviceFrame, _footer);
        // Route hotkeys through the GLOBAL key event, not _win/_elitesList KeyDown. A focused ListView's
        // KeystrokeNavigator (type-ahead "jump to item") consumes letter keys (r/u/c/d/q) before they can bubble
        // up to the window, so per-view KeyDown never sees them. Application.KeyDown fires first, so we can claim
        // the keys we act on (and mark them Handled to stop the navigator from also eating them).
        Application.KeyDown += OnGlobalKey;

        _elitesList.SetFocus();   // land on the elites list so ↑↓ work immediately (the display panels can't focus)
    }

    /// <summary>App-level key handler (subscribed to <see cref="Application.KeyDown"/>, which fires before the
    /// focused view). Only the keys it acts on are marked Handled; everything else (↑↓ etc.) falls through so the
    /// lists still scroll. While a text field is up (path / reward entry) it bows out entirely so the field owns
    /// every keystroke. Letters are matched on the rune (case-insensitive), so it's robust to key-casing.</summary>
    private void OnGlobalKey(object? sender, Key key)
    {
        if (_state.PathEntry || _state.RewardEntry || _state.DeckEntry) return;   // a focused TextField owns keys while typing

        int rune = key.AsRune.Value;
        char c = rune is > 0 and < 128 ? char.ToLowerInvariant((char)rune) : '\0';

        if (_state.View == TuiState.Mode.Advice)
        {
            if (key == Key.Esc) { CloseAdvice(); key.Handled = true; }
            else if (c is '+' or '=') { _state.AdviceN++; RunAdvice(); key.Handled = true; }
            else if (c is '-' or '_') { _state.AdviceN--; RunAdvice(); key.Handled = true; }
            else if (c == 'n') { _state.ShowNextAct = !_state.ShowNextAct; RenderFooter(); RunAdvice(); key.Handled = true; }
            else if (c == 'q') { Application.RequestStop(); key.Handled = true; }
            return;   // ↑↓ etc. fall through to the advice list
        }

        if (c == 'q' || key == Key.Esc) { Application.RequestStop(); key.Handled = true; return; }

        // Save-not-found screen: retry scan / type a folder manually (advice keys are inert with no run loaded).
        if (_state.Ctx is not { } ctx)
        {
            if (c is 'r' or 'd') { _dirty = false; LoadAndEval(force: true); key.Handled = true; }
            else if (c == 'm') { StartPathEntry(); key.Handled = true; }
            return;
        }

        switch (c)
        {
            case 'd':   // refresh: re-evaluate (re-read the save, or rebuild the custom deck)
                if (_custom) RebuildCustom(); else { _dirty = false; LoadAndEval(force: true); }
                key.Handled = true; break;
            case 'e':   // custom-deck sandbox only: open the deck editor
                if (_custom) { StartDeckEntry(); key.Handled = true; } break;
            case 'r': ShowAdvice(TuiState.Kind.Removal); key.Handled = true; break;
            case 'u': ShowAdvice(TuiState.Kind.Upgrade); key.Handled = true; break;
            case 'c': ShowAdvice(TuiState.Kind.Reward); key.Handled = true; break;
            case 'n':   // toggle the next-act projection (off by default — it's the costly half of every eval)
                _state.ShowNextAct = !_state.ShowNextAct;
                if (_state.ShowNextAct) StartNextStrength(ctx);
                else { _nextGen++; _state.NextStrengthBusy = false; _state.NextStrength = double.NaN; }
                RenderTop(); RenderFooter(); _win.SetNeedsDraw(); key.Handled = true;
                break;
            case ' ':   // toggle the selected elite in/out of the current-act strength, then re-evaluate it
                if (_elitesList.SelectedItem is int i && i >= 0 && i < _state.Elites.Count)
                {
                    _state.Elites[i].Included = !_state.Elites[i].Included;
                    RenderElites(); RecomputeStrength(ctx); _win.SetNeedsDraw(); key.Handled = true;
                }
                break;
        }
    }

    /// <summary>Reward-entry field: Enter resolves the typed cards and runs the take-vs-skip advice; Esc cancels.</summary>
    private void OnRewardKey(object? sender, Key key)
    {
        if (key == Key.Enter) { SubmitReward(); key.Handled = true; }
        else if (key == Key.Esc) { CloseAdvice(); key.Handled = true; }
    }

    private void SubmitReward()
    {
        var offered = new List<string>();
        foreach (var tok in (_rewardField.Text ?? "").Split(' ', StringSplitOptions.RemoveEmptyEntries))
        {
            var m = CardNameMatcher.Resolve(tok);
            if (m.Canonical != null) offered.Add(m.Canonical);
        }
        _state.Offered = offered;
        _state.RewardEntry = false;
        _rewardField.Visible = false; _adviceList.Visible = true;
        _adviceList.SetFocus();
        if (offered.Count == 0)
        {
            _adviceFrame.Title = " Reward — no known cards recognized (esc to go back) ";
            _adviceList.SetSource(new ObservableCollection<string> { "  Nothing recognized. Press esc, then 'c' to try again." });
            _win.SetNeedsDraw();
            return;
        }
        _state.AdviceN = Math.Min(_state.AdviceN, offered.Count);
        RunAdvice();
    }

    // ── Load + evaluate ─────────────────────────────────────────────────────────────

    /// <summary>Resolve the newest save, load it, and kick off a non-blocking re-evaluation. No-ops when the save
    /// hasn't changed (unless <paramref name="force"/>/<paramref name="initial"/>). When the run ends / the save is
    /// momentarily unreadable, keeps the last good dashboard rather than blanking.</summary>
    private void LoadAndEval(bool initial = false, bool force = false)
    {
        var path = _fixedPath ?? SaveSource.FindNewest(_overrideDir ?? _saveDir);
        if (path == null)
        {
            _state.Ctx = null;
            if (_overrideDir == null)
                _state.Status = "no ongoing run found";
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
                try
                {
                    var r = Companion.EvaluateRow(ctx, spec, _opts);
                    Post(gen, () => { _state.Elites[idx].Result = r; _state.Elites[idx].Busy = false; RenderElites(); });
                }
                catch  // a faulted fire-and-forget task would otherwise leave the row stuck on "…" forever
                {
                    Post(gen, () => { _state.Elites[idx].Result = new EliteResult(spec.Label, spec.Comp, null, "eval failed"); _state.Elites[idx].Busy = false; RenderElites(); });
                }
            });
        }

        var bossSpec = Companion.BossRowSpec(ctx);
        _state.Boss = null; _state.BossBusy = bossSpec != null;
        if (bossSpec is { } bs)
            Task.Run(() =>
            {
                try
                {
                    var r = Companion.EvaluateRow(ctx, bs, _opts);
                    Post(gen, () => { _state.Boss = r; _state.BossBusy = false; RenderBoss(); });
                }
                catch
                {
                    Post(gen, () => { _state.Boss = new EliteResult(bs.Label, bs.Comp, null, "eval failed"); _state.BossBusy = false; RenderBoss(); });
                }
            });

        // Current-act strength = average over the included elites + the known boss (#5/#7). Recomputed live on
        // toggle. This curates against "the fights I'll actually face", rather than a fixed representative pool.
        RecomputeStrength(ctx);
        if (_state.ShowNextAct) StartNextStrength(ctx);   // off by default — the next-act projection is the costly half
        else { _nextGen++; _state.NextStrength = double.NaN; _state.NextStrengthBusy = false; }
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
        var (elites, bosses) = NextActPools(ctx);
        var deckSpecs = ctx.DeckSpecs; int energy = ctx.Run.MaxEnergy; var relics = ctx.RelicNames;
        Task.Run(() =>
        {
            try
            {
                var s = Advisor.DeckStrengthNextAct(deckSpecs, elites, bosses, energy, relics, _opts);
                PostNext(ngen, () => { _state.NextStrength = s; _state.NextStrengthBusy = false; RenderTop(); });
            }
            catch
            {
                PostNext(ngen, () => { _state.NextStrength = double.NaN; _state.NextStrengthBusy = false; RenderTop(); });
            }
        });
    }

    private void PostNext(int ngen, Action a) => Application.Invoke(() =>
    {
        if (ngen != _nextGen) return;
        a();
        _win.SetNeedsDraw();
    });

    /// <summary>The next act's elite pool and boss pool as benchmark encounters; both empty on the final act.</summary>
    private (List<Advisor.Encounter> Elites, List<Advisor.Encounter> Bosses) NextActPools(Companion.Context ctx)
    {
        int nextAct = ctx.Run.ActIndex + 1, asc = ctx.Run.Ascension;
        if (nextAct >= Catalog.ActThemes.Count) return (new(), new());
        return (Companion.EncountersForClasses(Catalog.ActElitePool(nextAct), asc),
                Companion.EncountersForClasses(Catalog.ActBossPool(nextAct), asc));
    }

    // ── Custom-deck sandbox (no save file) ───────────────────────────────────────────

    /// <summary>Seed the custom session: the chosen character's starter deck, HP and energy, scoring against Act 1.</summary>
    private void InitCustom()
    {
        _cProfile = (_initialChar != null ? Catalog.FindCharacter(_initialChar) : null) ?? Catalog.CharacterProfiles[0];
        _cAct = 0; _cAsc = 0; _cHp = _cProfile.StartingHp;
        _cDeck = _cProfile.StarterDeckSpecs();
    }

    /// <summary>Rebuild the Context from the current custom inputs (profile/act/asc/hp/deck) and re-evaluate. Called
    /// once at startup and after every edit. There is no save file, so no watcher / dirty-key bookkeeping.</summary>
    private void RebuildCustom()
    {
        var ctx = Companion.BuildCustom(_cProfile, _cAct, _cAsc, _cHp, _cDeck);
        _state.Ctx = ctx; _state.Status = null;
        StartEval(ctx);
    }

    /// <summary>Open the deck-editor command line. Stays open across commands (each Enter applies one and clears the
    /// field) so you can build a deck in a flow; Esc returns to the dashboard. The deck/strength panels update live.</summary>
    private void StartDeckEntry()
    {
        _state.DeckEntry = true;
        _editField.Text = ""; _editField.Visible = true; _elitesList.Visible = false;
        _elitesFrame.Title = " Edit deck — +card  -card  act N  char X  hp N  asc N  reset ";
        _editField.SetFocus();
        RenderFooter();
        _win.SetNeedsDraw();
    }

    private void OnEditKey(object? sender, Key key)
    {
        if (key == Key.Enter) { ApplyEditCommand(); key.Handled = true; }
        else if (key == Key.Esc) { EndDeckEntry(); key.Handled = true; }
    }

    /// <summary>Parse and apply one deck-editor command (same grammar as the old prompt: <c>+card</c>/<c>-card</c>,
    /// <c>act N</c>, <c>char X</c>, <c>hp N</c>, <c>asc N</c>, <c>reset</c>; a bare card name adds). Rebuilds + re-evals
    /// on a real change, then clears the field and shows the result in the frame title, ready for the next command.</summary>
    private void ApplyEditCommand()
    {
        var line = (_editField.Text ?? "").Trim();
        _editField.Text = "";
        if (line.Length == 0) { _win.SetNeedsDraw(); return; }

        // "+card" / "-card" shorthand (no space); otherwise the first word is the verb.
        string verb, arg;
        if (line[0] is '+' or '-') { verb = line[0] == '+' ? "add" : "rm"; arg = line[1..].Trim(); }
        else { var sp = line.IndexOf(' '); verb = (sp < 0 ? line : line[..sp]).ToLowerInvariant(); arg = sp < 0 ? "" : line[(sp + 1)..].Trim(); }

        string msg; bool changed = false;
        switch (verb)
        {
            case "add": (changed, msg) = Companion.TryCustomAdd(_cDeck, arg); break;
            case "rm" or "remove" or "del" or "delete": (changed, msg) = Companion.TryCustomRemove(_cDeck, arg); break;
            case "act":
                if (Companion.TryParseAct(arg, out int ai)) { _cAct = ai; changed = true; msg = $"act {ai + 1} ({Catalog.ActThemes[ai]})"; }
                else msg = "usage: act <1-4 | name>"; break;
            case "char" or "character":
                if (Catalog.FindCharacter(arg) is { } np)
                { _cProfile = np; _cDeck = np.StarterDeckSpecs(); _cHp = np.StartingHp; changed = true; msg = $"character {np.Key} (starter deck)"; }
                else msg = $"unknown character '{arg}' (try: {string.Join(", ", Catalog.CharacterProfiles.Select(p => p.Key))})"; break;
            case "hp":
                if (int.TryParse(arg, out int h) && h > 0) { _cHp = h; changed = true; msg = $"hp {h}"; }
                else msg = "usage: hp <n>"; break;
            case "asc" or "ascension":
                if (int.TryParse(arg, out int a) && a >= 0) { _cAsc = a; changed = true; msg = $"ascension {a}"; }
                else msg = "usage: asc <n>"; break;
            case "reset":
                _cDeck = _cProfile.StarterDeckSpecs(); _cHp = _cProfile.StartingHp; changed = true; msg = "reset to starter deck"; break;
            case "q" or "quit" or "exit": Application.RequestStop(); return;
            default: (changed, msg) = Companion.TryCustomAdd(_cDeck, line); break;   // bare name → add
        }

        if (changed) RebuildCustom();
        // RebuildCustom → RenderElites repaints the (hidden) list but leaves the title alone in DeckEntry mode, so
        // set the command feedback here, after the rebuild, where it survives.
        _elitesFrame.Title = $" Edit deck · {msg} ";
        _win.SetNeedsDraw();
    }

    private void EndDeckEntry()
    {
        _state.DeckEntry = false;
        _editField.Visible = false; _elitesList.Visible = true;
        _elitesFrame.Title = " Elites ";
        RenderElites(); RenderFooter();
        _elitesList.SetFocus();
        _win.SetNeedsDraw();
    }

    // ── Advice overlay (removal / upgrade) ───────────────────────────────────────────

    /// <summary>Open the advice overlay for the given kind. Removal/upgrade compute immediately; reward first
    /// prompts for the offered card(s). The deck/strength panels stay put; +/- steps how many cards (1–3).</summary>
    private void ShowAdvice(TuiState.Kind kind)
    {
        if (_state.Ctx is not { } c) return;

        _state.View = TuiState.Mode.Advice;
        _state.AdviceKind = kind;
        _state.AdviceN = 1;
        _elitesFrame.Visible = false; _adviceFrame.Visible = true;

        if (kind == TuiState.Kind.Reward)
        {
            _state.RewardEntry = true; _state.Offered = new();
            _rewardField.Text = ""; _rewardField.Visible = true; _adviceList.Visible = false;
            _adviceFrame.Title = " Reward — type the offered card(s), space-separated, then Enter ";
            _rewardField.SetFocus();
            RenderFooter();
            _win.SetNeedsDraw();
            return;
        }

        if (Moves(c).Count == 0) { CloseAdvice(); return; }   // nothing to remove/upgrade
        _adviceList.SetFocus();
        RunAdvice();
    }

    /// <summary>The move-set for the current advice kind.</summary>
    private List<Advisor.CardMove> Moves(Companion.Context c) => _state.AdviceKind switch
    {
        TuiState.Kind.Removal => Advisor.RemovalMoves(c.DeckSpecs),
        TuiState.Kind.Upgrade => Advisor.UpgradeMoves(c.DeckSpecs),
        TuiState.Kind.Reward => _state.Offered
            .Select(card => new Advisor.CardMove(card, card, d => { var r = d.ToList(); r.Add(card); return r; })).ToList(),
        _ => new(),
    };

    /// <summary>(Re)compute the advice for the current kind + N on a background task. Caps N to what the deck and
    /// move-set allow (removals must leave ≥1 card).</summary>
    private void RunAdvice()
    {
        if (_state.Ctx is not { } c) return;
        var moves = Moves(c);
        if (moves.Count == 0) return;
        int maxN = _state.AdviceKind == TuiState.Kind.Removal
            ? Math.Min(Math.Min(3, moves.Count), Math.Max(1, c.DeckSpecs.Count - 1))
            : Math.Min(3, moves.Count);
        _state.AdviceN = Math.Clamp(_state.AdviceN, 1, Math.Max(1, maxN));
        int n = _state.AdviceN;

        _state.AdviceBusy = true; _state.AdviceRows = new();
        RenderAdvice(); RenderFooter();

        int agen = ++_adviceGen;
        var deck = c.DeckSpecs;
        var curPool = CuratedPool(c);
        // Next-act columns only when toggled on: empty pools make DeckStrengthNextActSeq short-circuit to NaN, so
        // no next-act solve runs per candidate (the costly half of advice ranking).
        var (nextElites, nextBosses) = _state.ShowNextAct
            ? NextActPools(c)
            : (new List<Advisor.Encounter>(), new List<Advisor.Encounter>());
        int energy = c.Run.MaxEnergy; var relics = c.RelicNames;
        Task.Run(() =>
        {
            try
            {
                var (baseline, rows, exhaustive) =
                    Advisor.RankMoveSets(deck, moves, n, curPool, nextElites, nextBosses, energy, relics, _opts);
                PostAdvice(agen, () =>
                {
                    _state.AdviceBaseline = baseline; _state.AdviceRows = rows;
                    _state.AdviceExhaustive = exhaustive; _state.AdviceBusy = false;
                    RenderAdvice();
                });
            }
            catch
            {
                PostAdvice(agen, () =>
                {
                    _state.AdviceRows = new(); _state.AdviceBusy = false;
                    _adviceFrame.Title = " Advice — evaluation failed (esc to go back) ";
                    RenderAdvice();
                });
            }
        });
    }

    private void CloseAdvice()
    {
        _adviceGen++;   // discard any in-flight advice result
        _state.View = TuiState.Mode.Dashboard;
        _state.RewardEntry = false;
        _rewardField.Visible = false; _adviceList.Visible = true;
        _adviceFrame.Visible = false; _elitesFrame.Visible = true;
        _elitesList.SetFocus();
        RenderFooter();
        _win.SetNeedsDraw();
    }

    private void PostAdvice(int agen, Action a) => Application.Invoke(() =>
    {
        if (agen != _adviceGen || _state.View != TuiState.Mode.Advice) return;
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
            try
            {
                var s = Advisor.DeckStrength(deckSpecs, pool, energy, relics, _opts);
                PostStrength(sgen, () => { _state.Strength = s; _state.StrengthBusy = false; RenderTop(); });
            }
            catch
            {
                PostStrength(sgen, () => { _state.Strength = double.NaN; _state.StrengthBusy = false; RenderTop(); });
            }
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

    // ── Save-not-found: manual folder entry ──────────────────────────────────────────

    private void StartPathEntry()
    {
        _state.PathEntry = true;
        _pathField.Text = ""; _pathField.Visible = true; _elitesList.Visible = false;
        _elitesFrame.Title = " Save folder — paste the SlayTheSpire2 folder (or a current_run.save), then Enter ";
        _pathField.SetFocus();
        RenderFooter();
        _win.SetNeedsDraw();
    }

    private void OnPathKey(object? sender, Key key)
    {
        if (key == Key.Enter) { SubmitPath(); key.Handled = true; }
        else if (key == Key.Esc) { CancelPathEntry(); key.Handled = true; }
    }

    private void CancelPathEntry()
    {
        _state.PathEntry = false;
        _pathField.Visible = false; _elitesList.Visible = true;
        _elitesFrame.Title = " Elites ";
        RenderElites(); RenderFooter();
        _elitesList.SetFocus();
        _win.SetNeedsDraw();
    }

    private void SubmitPath()
    {
        var typed = (_pathField.Text ?? "").Trim().Trim('"');
        _state.PathEntry = false;
        _pathField.Visible = false; _elitesList.Visible = true;
        _elitesFrame.Title = " Elites ";
        _elitesList.SetFocus();

        if (typed.Length == 0) { RenderElites(); _win.SetNeedsDraw(); return; }
        // A file → search its folder; a folder → search it; otherwise hand it to FindNewest as-is.
        _overrideDir = Directory.Exists(typed) ? typed
            : File.Exists(typed) ? Path.GetDirectoryName(typed)
            : typed;
        LoadAndEval(force: true);
        if (_state.Ctx == null)
            _state.Status = $"no current_run.save found under: {typed}";
        RenderAll();
        _win.SetNeedsDraw();
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

        if (!_state.ShowNextAct)
        {
            _nextStrengthLabel.Text = "Strength · next act   off — press (n) to project (slower)";
            _nextStrengthLabel.SetScheme(TuiFormat.SchemeOf(TuiFormat.Grey));
        }
        else
        {
            if (!_state.HasNextAct)
                _nextStrengthLabel.Text = "Strength · next act   —  (final act)";
            else if (!_state.NextStrengthBusy && double.IsNaN(_state.NextStrength))
                _nextStrengthLabel.Text = "Strength · next act   n/a";
            else
                _nextStrengthLabel.Text = "Strength · next act   " + TuiFormat.StrengthBar(_state.NextStrength)
                    + (_state.NextStrengthBusy ? "  (updating…)" : "");
            _nextStrengthLabel.SetScheme(TuiFormat.SchemeOf(
                _state.HasNextAct ? TuiFormat.StrengthColor(_state.NextStrength) : TuiFormat.Grey));
        }

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
        if (_state.Ctx == null)
        {
            _elitesFrame.Title = " No run found ";
            items.Add("");
            items.Add("  No ongoing Slay the Spire 2 run was found.");
            if (_state.Status != null && _state.Status.StartsWith("no current_run.save"))
                items.Add($"  {_state.Status}");
            items.Add("");
            items.Add("  (r) retry — scan again for a save");
            items.Add("  (m) enter the save folder path manually");
            items.Add("  (q) quit");
        }
        else
        {
            if (_state.Elites.Count == 0) items.Add("(no elites to evaluate for this Act)");
            else foreach (var row in _state.Elites) items.Add(EliteListLine(row));
            // Reflect ONLY the elite rows here. RenderElites is re-invoked when an elite finishes, but not when the
            // strength index or boss finishes (those re-render their own panels) — so folding their busy-state into
            // this title would leave it stuck on "evaluating…" whenever one of them completed last. The strength
            // bar shows its own "(updating…)"; the boss panel shows its own "evaluating…". While the deck editor is
            // open the frame title is the command prompt/feedback, so don't overwrite it here.
            if (!_state.DeckEntry)
                _elitesFrame.Title = _state.Elites.Any(e => e.Busy) ? " Elites — evaluating… " : " Elites ";
        }

        int sel = _elitesList.SelectedItem ?? 0;
        _elitesList.SetSource(items);
        if (items.Count > 0) _elitesList.SelectedItem = Math.Clamp(sel, 0, items.Count - 1);
    }

    private void RenderAdvice()
    {
        string kind = _state.AdviceKind switch
        {
            TuiState.Kind.Removal => "Removals",
            TuiState.Kind.Upgrade => "Upgrades",
            TuiState.Kind.Reward => "Reward — take",
            _ => "Advice",
        };
        string one = _state.AdviceKind switch
        {
            TuiState.Kind.Removal => "best cut",
            TuiState.Kind.Upgrade => "best upgrade",
            TuiState.Kind.Reward => "best card",
            _ => "best",
        };
        string what = _state.AdviceN == 1 ? one : $"best {_state.AdviceN} cards";
        string heur = !_state.AdviceBusy && !_state.AdviceExhaustive ? " · heuristic shortlist" : "";
        string busy = _state.AdviceBusy ? " · computing…" : "";
        _adviceFrame.Title = $" {kind} — {what}{heur}{busy} ";

        bool showNext = _state.ShowNextAct;
        var items = new ObservableCollection<string>();
        if (_state.AdviceBusy)
            items.Add(showNext ? "  evaluating each candidate against this act and the next…"
                               : "  evaluating each candidate against this act…");
        else
        {
            foreach (var s in AdviceLines("keep as-is", _state.AdviceBaseline, default, isBaseline: true, showNext)) items.Add(s);
            foreach (var r in _state.AdviceRows)
                foreach (var s in AdviceLines(r.Label, r.Result, r.Delta, isBaseline: false, showNext)) items.Add(s);
        }
        _adviceList.SetSource(items);
        if (items.Count > 0) _adviceList.SelectedItem = 0;
    }

    /// <summary>Render one advice candidate as one OR MORE display lines: the first card carries the strength
    /// columns, and each additional card of a multi-card move goes on its own "+ card" line so long names aren't
    /// truncated together. The next-act column is dropped entirely when the projection is toggled off.</summary>
    private static IEnumerable<string> AdviceLines(
        string label, Advisor.DualStrength res, Advisor.DualStrength delta, bool isBaseline, bool showNext)
    {
        static string S(double v) => double.IsNaN(v) ? "—" : ((int)Math.Round(v)).ToString();
        string D(double d) => isBaseline || double.IsNaN(d) ? "" : d > 0.5 ? $" (+{d:F0})" : d < -0.5 ? $" ({d:F0})" : " (0)";
        var cards = label.Split(" + ");
        string stats = showNext
            ? $"this {S(res.Cur),3}{D(delta.Cur),-6}  next {S(res.Next),3}{D(delta.Next)}"
            : $"this {S(res.Cur),3}{D(delta.Cur)}";
        yield return $"  {Clip(cards[0], 26),-26}  {stats}";
        for (int i = 1; i < cards.Length; i++)
            yield return $"      + {cards[i]}";
    }

    private void RenderFooter()
    {
        string next = _state.ShowNextAct ? "on" : "off";
        // The custom sandbox has no save to refresh; it exposes the deck editor on (e) instead of (d) refresh.
        string refreshOrEdit = _custom ? "(e) edit deck" : "(d) refresh";
        _footer.Text =
            _state.PathEntry ? "  type a folder/file path   (enter) use it   (esc) cancel"
            : _state.DeckEntry ? "  +card  -card  act N  char X  hp N  asc N  reset   (enter) apply   (esc) done"
            : _state.Ctx == null ? "  (r) retry   (m) enter save folder   (q) quit"
            : _state.View == TuiState.Mode.Advice ? $"  ↑↓ scroll   (+/–) cards (1–3)   (n) next-act: {next}   (esc) back   (q) quit"
            : $"  ↑↓ select   (space) in/excl elite   (r)emovals  (u)pgrades  (c) reward   (n) next-act: {next}   {refreshOrEdit}   (q) quit";
        _footer.SetScheme(TuiFormat.SchemeOf(TuiFormat.Grey));
    }


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
                try { var p = _fixedPath ?? SaveSource.FindNewest(_overrideDir ?? _saveDir); if (p != null && SaveKey(p) != _loadedKey) _dirty = true; }
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
