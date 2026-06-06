using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;

namespace Sts2Solver.Ranwid;

/// <summary>
/// Deck advice, ranked by the single <b>deck-strength index</b> (0–100) — the same metric the dashboard
/// headlines — so removals and reward picks read consistently with it. Deck strength = evaluate each Act elite
/// from a FIXED 100 HP, average the expected HP loss (death = the full 100 lost, capped), report
/// <c>100 − average</c> (100 = takes no damage from any elite, 0 = certain death). Removal advice ranks each
/// single-card cut by the resulting deck's strength; reward advice ranks each offered card (and skipping) the
/// same way. The unit of truth is <see cref="EncounterEvaluator.Evaluate"/> (MCTS in the advice regime).
///
/// HP-independent by design: a deck's strength shouldn't swing with your current run HP, so advice stays stable
/// across a run (unlike the per-elite dashboard rows, which use your actual HP for "can I take this fight now").
/// </summary>
public static class Advisor
{
    /// <summary>An Act encounter to benchmark against: a label plus a FRESH-monsters builder (monsters are
    /// mutated during combat, so each evaluation must rebuild them).</summary>
    public readonly record struct Encounter(string Name, Func<List<Monster>> Build);

    /// <summary>MCTS trial budget for removal/pick RANKING (vs the full budget the dashboard headline uses).
    /// Ranking is relative, so the small extra convergence bias at this budget cancels — measured: the ranking
    /// is identical at 500/800/2000 trials, while the advice runs ~3× faster.</summary>
    public const int AdviceTrials = 800;

    // ── Deck strength (the metric) ───────────────────────────────────────────────

    /// <summary>The clamped expected HP loss (death = 100) a deck takes from ONE elite, fought from 100 HP.</summary>
    private static double EliteLoss(IReadOnlyList<string> deckSpecs, Encounter enc,
        int maxEnergy, IReadOnlyList<string> relics, EvalOptions fast)
    {
        var deck = deckSpecs.Select(Catalog.BuildCard).ToList();   // fresh instances (combat mutates them)
        var player = Catalog.BuildPlayer(deck, 100, 100, maxEnergy, relics);
        var stats = EncounterEvaluator.Evaluate(Catalog.SetupCombat(player, enc.Build()), fast);
        return Math.Clamp(stats.MeanLoss, 0, 100);
    }

    /// <summary>Deck-strength index 0–100 (NaN if there's nothing to score). Parallel over the independent
    /// elites — for the single dashboard-headline call (use <see cref="DeckStrengthSeq"/> inside the parallel
    /// candidate loops to avoid nested over-subscription).</summary>
    public static double DeckStrength(
        IReadOnlyList<string> deckSpecs, IReadOnlyList<Encounter> encounters,
        int maxEnergy, IReadOnlyList<string> relics, EvalOptions opts)
    {
        if (encounters.Count == 0 || deckSpecs.Count == 0) return double.NaN;
        double avgLoss = encounters.AsParallel().Select(e => EliteLoss(deckSpecs, e, maxEnergy, relics, opts)).Average();
        return Math.Clamp(100 - avgLoss, 0, 100);
    }

    /// <summary>Sequential-over-elites deck strength, for use INSIDE a parallel candidate loop (the loop
    /// supplies the parallelism; nesting AsParallel would over-subscribe).</summary>
    private static double DeckStrengthSeq(
        IReadOnlyList<string> deckSpecs, IReadOnlyList<Encounter> encounters,
        int maxEnergy, IReadOnlyList<string> relics, EvalOptions fast)
    {
        double sum = 0;
        foreach (var e in encounters) sum += EliteLoss(deckSpecs, e, maxEnergy, relics, fast);
        return Math.Clamp(100 - sum / encounters.Count, 0, 100);
    }

    // ── Removal advice ───────────────────────────────────────────────────────────

    /// <summary>One single-card removal's verdict: the resulting deck's <see cref="Strength"/> and the
    /// <see cref="Delta"/> vs keeping the deck as-is (positive = removing the card makes the deck stronger).</summary>
    public readonly record struct AdviceItem(string Card, double Strength, double Delta)
    {
        public bool IsImprovement => Delta > 1e-6;
    }

    /// <summary>Rank single-card removals by the resulting deck's strength. Returns the baseline (current-deck)
    /// strength and one item per DISTINCT removable card, best-first. A removal that empties the deck is skipped.</summary>
    public static (double baseline, List<AdviceItem> items) RemovalAdvice(
        IReadOnlyList<string> deckSpecs, IReadOnlyList<Encounter> encounters,
        int maxEnergy, IReadOnlyList<string> relics, EvalOptions opts)
    {
        var advOpts = opts with { MctsTrials = AdviceTrials };
        double baseline = DeckStrength(deckSpecs, encounters, maxEnergy, relics, advOpts);
        if (deckSpecs.Count <= 1 || double.IsNaN(baseline)) return (baseline, new List<AdviceItem>());

        // Each candidate removal is an independent solve → across cores. Deterministic (seeded; sorted after).
        var items = deckSpecs.Distinct().AsParallel().Select(spec =>
        {
            var reduced = new List<string>(deckSpecs);
            reduced.Remove(spec);   // remove exactly one copy
            double after = DeckStrengthSeq(reduced, encounters, maxEnergy, relics, advOpts);
            return new AdviceItem(spec, after, after - baseline);
        }).ToList();

        items.Sort((a, b) => b.Strength.CompareTo(a.Strength));   // strongest resulting deck first
        return (baseline, items);
    }

    // ── Upgrade advice ─────────────────────────────────────────────────────────────

    /// <summary>One single-card upgrade's verdict: upgrading ONE copy of <see cref="Card"/> to
    /// <see cref="Upgraded"/> yields a deck of <see cref="Strength"/>, a <see cref="Delta"/> vs the deck as-is
    /// (upgrades are nearly always a gain, but the framework reports the rare wash/negative too).</summary>
    public readonly record struct UpgradeItem(string Card, string Upgraded, double Strength, double Delta)
    {
        public bool IsImprovement => Delta > 1e-6;
    }

    /// <summary>An un-upgraded Attack/Skill/Power has an upgrade to advise; already-upgraded copies, Statuses and
    /// Curses don't (multi-level cards like Searing Blow are advised one level at a time — only the first here).</summary>
    private static bool IsUpgradeable(string spec)
    {
        if (spec.Contains('+')) return false;   // already upgraded — nothing more to advise
        try { return Catalog.BuildCard(spec).Type is not (CardType.Status or CardType.Curse); }
        catch (ArgumentException) { return false; }   // unbuildable spec — skip
    }

    /// <summary>Rank single-card upgrades by the resulting deck's strength. Returns the baseline (current-deck)
    /// strength and one item per DISTINCT upgradeable card (an un-upgraded Attack/Skill/Power), best-first.
    /// Already-upgraded copies, Statuses and Curses are skipped (nothing to upgrade).</summary>
    public static (double baseline, List<UpgradeItem> items) UpgradeAdvice(
        IReadOnlyList<string> deckSpecs, IReadOnlyList<Encounter> encounters,
        int maxEnergy, IReadOnlyList<string> relics, EvalOptions opts)
    {
        var advOpts = opts with { MctsTrials = AdviceTrials };
        double baseline = DeckStrength(deckSpecs, encounters, maxEnergy, relics, advOpts);
        if (deckSpecs.Count == 0 || double.IsNaN(baseline)) return (baseline, new List<UpgradeItem>());

        // Each candidate upgrade is an independent solve → across cores. Deterministic (seeded; sorted after).
        var items = deckSpecs.Distinct().Where(IsUpgradeable).AsParallel().Select(spec =>
        {
            var up = spec + "+1";
            var swapped = new List<string>(deckSpecs);
            swapped[swapped.IndexOf(spec)] = up;   // upgrade exactly one copy
            double after = DeckStrengthSeq(swapped, encounters, maxEnergy, relics, advOpts);
            return new UpgradeItem(spec, up, after, after - baseline);
        }).ToList();

        items.Sort((a, b) => b.Strength.CompareTo(a.Strength));   // strongest resulting deck first
        return (baseline, items);
    }

    // ── Reward-pick advice ───────────────────────────────────────────────────────

    /// <summary>One reward option's verdict: the resulting deck's <see cref="Strength"/> (taking the card), or
    /// the keep-the-deck strength when <see cref="IsSkip"/>.</summary>
    public readonly record struct PickItem(string Card, bool IsSkip, double Strength);

    /// <summary>Reward advice: rank each offered card (added to the deck) and skipping (deck as-is) by the
    /// resulting deck strength, best-first. Skipping is a first-class option (wins when no card beats the deck).
    /// Returns the skip strength and the ranked options (head = recommended pick).</summary>
    public static (double skip, List<PickItem> ranked) PickAdvice(
        IReadOnlyList<string> deckSpecs, IReadOnlyList<string> candidates, IReadOnlyList<Encounter> encounters,
        int maxEnergy, IReadOnlyList<string> relics, EvalOptions opts)
    {
        var advOpts = opts with { MctsTrials = AdviceTrials };
        double skip = DeckStrength(deckSpecs, encounters, maxEnergy, relics, advOpts);
        var ranked = candidates.AsParallel().Select(cand =>
        {
            var withCard = new List<string>(deckSpecs) { cand };
            return new PickItem(cand, false, DeckStrengthSeq(withCard, encounters, maxEnergy, relics, advOpts));
        }).ToList();
        ranked.Add(new PickItem("(skip)", true, skip));
        ranked.Sort((a, b) => b.Strength.CompareTo(a.Strength));
        return (skip, ranked);
    }
}
