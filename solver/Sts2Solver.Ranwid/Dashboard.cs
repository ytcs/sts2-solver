using Spectre.Console;
using Spectre.Console.Rendering;
using Sts2Solver.Search;

namespace Sts2Solver.Ranwid;

/// <summary>
/// The live companion's screen, drawn with Spectre.Console: run header, current deck, per-elite survival /
/// HP-loss, and a prominent panel of what's being IGNORED (relics, unsupported cards). Deliberately free of
/// any engine/algorithm detail — the player sees the numbers and what's missing, never how they're computed.
/// </summary>
public static class Dashboard
{
    /// <summary>Repaint the whole dashboard from the loaded run + the latest elite results. <paramref
    /// name="strength"/> is the 0–100 deck-strength index (NaN until computed / when there's nothing to score).</summary>
    public static void Render(Companion.Context c, IReadOnlyList<EliteResult> elites, double strength, bool evaluating)
    {
        AnsiConsole.Clear();
        AnsiConsole.Write(new Rule("[bold deepskyblue1]ranwid[/]").LeftJustified());

        RenderStrength(strength);
        RenderHeader(c);
        AnsiConsole.WriteLine();
        AnsiConsole.Write(DeckPanel(c));
        AnsiConsole.WriteLine();
        AnsiConsole.Write(ElitesPanel(elites, evaluating));

        var ignored = IgnoredPanel(c);
        if (ignored != null) { AnsiConsole.WriteLine(); AnsiConsole.Write(ignored); }

        AnsiConsole.WriteLine();
        AnsiConsole.Write(new Markup(
            "  [grey]([/][white]r[/][grey]) removals   ([/][white]c[/][grey]) check a reward card   "
            + "([/][white]d[/][grey]) refresh   ([/][white]q[/][grey]) quit"
            + "          auto-refreshes when your run changes[/]"));
        AnsiConsole.WriteLine();
    }

    /// <summary>The headline deck-strength index (0–100): how well this deck handles the Act's elites from full
    /// HP, independent of the run's current HP. 100 = takes no damage from any elite; 0 = certain death.</summary>
    private static void RenderStrength(double strength)
    {
        if (double.IsNaN(strength)) return;
        int s = (int)Math.Round(strength);
        string color = s >= 75 ? "green" : s >= 50 ? "yellow" : s >= 25 ? "darkorange" : "red";
        int filled = (int)Math.Round(s / 100.0 * 20);
        string bar = new string('█', filled) + new string('─', 20 - filled);
        AnsiConsole.Write(new Markup($"  [grey]Deck strength[/]  [{color}]{s,3}[/][grey]/100[/]  [{color}]{bar}[/]"));
        AnsiConsole.WriteLine();
        AnsiConsole.WriteLine();
    }

    private static void RenderHeader(Companion.Context c)
    {
        var run = c.Run;
        string relics = run.RelicIds.Count == 0 ? "none"
            : string.Join(", ", c.RelicNames.Count > 0 ? c.RelicNames : new List<string> { "none modelled" });
        var line = new Markup(
            $"  [bold]{Esc(GameIds.CharacterName(run.Character))}[/]  ·  A{run.Ascension}  ·  "
            + $"Act {run.ActIndex + 1}  ·  {HpMarkup(run.PlayerHp, run.PlayerMaxHp)}  ·  {run.MaxEnergy} energy");
        AnsiConsole.Write(line);
        AnsiConsole.WriteLine();
        AnsiConsole.Write(new Markup($"  [grey]relics:[/] {Esc(relics)}"));
        AnsiConsole.WriteLine();
        if (run.PlayerCount > 1)
        {
            AnsiConsole.Write(new Markup(
                $"  [yellow]multiplayer run ({run.PlayerCount} players) — shown as a single-player fight (approximate)[/]"));
            AnsiConsole.WriteLine();
        }
    }

    private static Panel DeckPanel(Companion.Context c)
    {
        string body = c.DeckSummary == "(empty)" ? "[grey](no cards)[/]" : Esc(PrettyDeck(c.DeckSummary));
        return new Panel(new Markup(body))
        {
            Header = new PanelHeader($" Deck — {c.Run.Deck.Count} cards "),
            Border = BoxBorder.Rounded,
            Expand = true,
        };
    }

    private static IRenderable ElitesPanel(IReadOnlyList<EliteResult> elites, bool evaluating)
    {
        var table = new Table { Border = TableBorder.SimpleHeavy, Expand = true };
        table.AddColumn("[grey]Elite[/]");
        table.AddColumn(new TableColumn("[grey]Survive[/]").Centered());
        table.AddColumn(new TableColumn("[grey]Avg HP loss[/]").Centered());
        table.AddColumn(new TableColumn("[grey]Worst[/]").Centered());

        if (elites.Count == 0)
            table.AddRow("[grey](no elites to evaluate for this Act)[/]", "", "", "");

        foreach (var r in elites)
        {
            // Show the composition only when it adds something (multi-monster / differently-named), not when
            // it just repeats the elite's own name.
            bool showComp = !string.IsNullOrEmpty(r.Composition)
                && !string.Equals(r.Composition, r.Elite, StringComparison.OrdinalIgnoreCase);
            string name = $"[bold]{Esc(r.Elite)}[/]" + (showComp ? $"\n[grey]{Esc(r.Composition)}[/]" : "");
            if (r.Stats is { } s)
            {
                string worst = s.HasDistribution ? s.MaxLoss.ToString() : "—";
                table.AddRow(new Markup(name), new Markup(SurviveMarkup(s.Survival)),
                    new Markup($"{s.MeanLoss:F0}"), new Markup(worst));
            }
            else
            {
                table.AddRow(new Markup(name), new Markup($"[grey]{Esc(r.Skipped ?? "—")}[/]"), new Markup("—"), new Markup("—"));
            }
        }

        var header = evaluating ? " Elites — [yellow]evaluating…[/] " : " Elites ";
        return new Panel(table) { Header = new PanelHeader(header), Border = BoxBorder.Rounded, Expand = true };
    }

    /// <summary>The "what's missing" panel — only shown when something is ignored, so the player knows the
    /// numbers can be worse than their real run (unmodelled relics, unported cards).</summary>
    private static Panel? IgnoredPanel(Companion.Context c)
    {
        if (c.Warnings.Count == 0) return null;
        var sb = new System.Text.StringBuilder();
        foreach (var w in c.Warnings.Distinct()) sb.AppendLine($"  • {Esc(w)}");
        sb.Append("[grey]  these aren't counted, so survival / HP-loss may look worse than your real run.[/]");
        return new Panel(new Markup(sb.ToString().TrimEnd('\n')))
        {
            Header = new PanelHeader(" [yellow]Ignored[/] "),
            Border = BoxBorder.Rounded,
            BorderStyle = new Style(Color.Yellow),
            Expand = true,
        };
    }

    // ── Removal / reward command screens (also free of engine detail) ──────────────────────────

    public static void RenderRemovals((double baseline, List<Advisor.AdviceItem> items) advice)
    {
        var (baseline, items) = advice;
        var table = new Table { Border = TableBorder.SimpleHeavy, Expand = true };
        table.AddColumn("[grey]Remove[/]");
        table.AddColumn(new TableColumn("[grey]Deck strength[/]").Centered());
        table.AddColumn(new TableColumn("[grey]Δ[/]").Centered());

        table.AddRow(new Markup("[bold]keep as-is[/]"), new Markup(StrengthMarkup(baseline)), new Markup("[grey]—[/]"));
        // Show EVERY removable card, best-first (items are pre-sorted by resulting strength) — not just the ones
        // that strictly improve the deck. Cutting a basic Strike/Defend rarely raises strength in a single-combat
        // metric, but you still want to see which cut hurts LEAST (e.g. at a removal site you must cut something).
        bool anyImproves = items.Count > 0 && items[0].IsImprovement;
        foreach (var i in items.Take(14))
        {
            string deltaMk = i.Delta > 0.5 ? $"[green]+{i.Delta:F0}[/]"
                : i.Delta < -0.5 ? $"[red]{i.Delta:F0}[/]" : "[grey]0[/]";
            string nameMk = i.IsImprovement ? $"[green]{Esc(i.Card)}[/]" : Esc(i.Card);
            table.AddRow(new Markup(nameMk), new Markup(StrengthMarkup(i.Strength)), new Markup(deltaMk));
        }

        AnsiConsole.Write(new Panel(table)
        {
            Header = new PanelHeader(anyImproves ? " Removals — best cuts first " : " Removals — least-harmful first "),
            Border = BoxBorder.Rounded, Expand = true,
        });
        if (!anyImproves)
            AnsiConsole.Write(new Markup("  [grey]no single removal raises the deck's strength — the rows above are the least-harmful cuts.[/]\n"));
    }

    public static void RenderPick((double skip, List<Advisor.PickItem> ranked) advice)
    {
        var (_, ranked) = advice;
        var table = new Table { Border = TableBorder.SimpleHeavy, Expand = true };
        table.AddColumn("[grey]Option[/]");
        table.AddColumn(new TableColumn("[grey]Deck strength[/]").Centered());
        for (int i = 0; i < ranked.Count; i++)
        {
            var p = ranked[i];
            string label = p.IsSkip ? "skip (keep deck)" : p.Card;
            string deco = i == 0 ? $"[bold green]{Esc(label)}  ◀ pick[/]" : Esc(label);
            table.AddRow(new Markup(deco), new Markup(StrengthMarkup(p.Strength)));
        }
        AnsiConsole.Write(new Panel(table) { Header = new PanelHeader(" Reward "), Border = BoxBorder.Rounded, Expand = true });
    }

    private static string StrengthMarkup(double s)
    {
        if (double.IsNaN(s)) return "[grey]—[/]";
        int v = (int)Math.Round(s);
        string color = v >= 75 ? "green" : v >= 50 ? "yellow" : v >= 25 ? "darkorange" : "red";
        return $"[{color}]{v}[/][grey]/100[/]";
    }

    // ── Helpers ────────────────────────────────────────────────────────────────────────────────

    private static string SurviveMarkup(double p)
    {
        string color = p >= 0.80 ? "green" : p >= 0.50 ? "yellow" : p >= 0.25 ? "darkorange" : "red";
        return $"[{color}]{p:P0}[/]";
    }

    private static string HpMarkup(int hp, int max)
    {
        double r = max > 0 ? (double)hp / max : 0;
        string color = r >= 0.66 ? "green" : r >= 0.33 ? "yellow" : "red";
        return $"[{color}]{hp}[/][grey]/{max} HP[/]";
    }

    /// <summary>Turn the "4x StrikeIronclad, 1x Bash+1" summary into friendlier text (drop class suffixes,
    /// show upgrades as a star).</summary>
    private static string PrettyDeck(string summary) => summary;

    private static string Esc(string s) => Markup.Escape(s);

    // ── Preview (no live run needed) ─────────────────────────────────────────────────────────────

    /// <summary>Render the dashboard once with synthetic data, for previewing the layout (`ranwid --preview`).</summary>
    public static void RenderPreview()
    {
        var deck = new List<CardEntry>();
        void Add(string id, int n, int up = 0) { for (int i = 0; i < n; i++) deck.Add(new CardEntry(id, up, false)); }
        Add("CARD.STRIKE_IRONCLAD", 4); Add("CARD.DEFEND_IRONCLAD", 3); Add("CARD.BASH", 1);
        Add("CARD.INFLAME", 1); Add("CARD.DEMON_FORM", 1, 1); Add("CARD.UPPERCUT", 1); Add("CARD.WHIRLWIND", 1);

        var run = new RunState(
            Ascension: 0, ActIndex: 0, ActId: "ACT.ONE", Character: "CHARACTER.IRONCLAD",
            PlayerHp: 30, PlayerMaxHp: 80, MaxEnergy: 3, Deck: deck,
            RelicIds: new[] { "RELIC.BURNING_BLOOD", "RELIC.VAJRA" },
            EliteEncounterIds: new[] { "ENCOUNTER.TERROR_EEL_ELITE", "ENCOUNTER.BYRDONIS_ELITE" },
            BossId: null, PlayerCount: 1, PlayerNetId: 0);

        var ctx = new Companion.Context(
            run, "(preview)",
            DeckSpecs: new List<string>(),
            RelicNames: new List<string> { "BurningBlood" },
            Encounters: new List<Advisor.Encounter>(),
            EliteInfo: new List<(string, string)>(),
            StrengthPool: new List<Advisor.Encounter>(),
            Warnings: new List<string> { "relic Vajra ignored (only Burning Blood is modelled)" },
            DeckSummary: "4x Strike, 3x Defend, Bash, Inflame, DemonForm★, Uppercut, Whirlwind");

        var elites = new List<EliteResult>
        {
            new("TerrorEel", "TerrorEel", new CombatStats(EvalEngine.Mcts, 0.21, 28, 27, true, 6, 45, 14, 27, 41, 2000, 0, 0), null),
            new("Gremlins", "2× Gremlin Nob + Mad Gremlin", new CombatStats(EvalEngine.Mcts, 0.88, 18, 13, true, 2, 33, 6, 16, 28, 2000, 0, 0), null),
        };
        Render(ctx, elites, strength: 64, evaluating: false);
    }
}
