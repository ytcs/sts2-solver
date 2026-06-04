using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;

namespace Sts2Solver.Ranwid;

/// <summary>
/// Deck advice: which single card-removal most improves how the deck fares against the Act's elites.
/// The unit of truth is <see cref="EncounterEvaluator.Evaluate"/> (auto exact-or-MCTS) per encounter; the
/// per-encounter <see cref="CombatStats"/> are aggregated <b>lexicographically</b> into a <see cref="DeckScore"/>:
/// primary = the WORST survival across the Act's elites (the bottleneck fight you must not lose), secondary =
/// total expected HP loss across them. A change is an improvement iff it raises the bottleneck survival, or —
/// at equal survival — lowers total expected loss. This mirrors the solver's own survival-first objective.
///
/// Deliberately engine-agnostic about deck size: it leans entirely on <see cref="EncounterEvaluator"/>, which
/// uses exact only for small/tractable fights and MCTS otherwise — so this scales to real (large) run decks
/// where exact is infeasible, exactly the regime the advisor runs in.
/// </summary>
public static class Advisor
{
    /// <summary>An Act encounter to benchmark against: a label plus a FRESH-monsters builder (monsters are
    /// mutated during combat, so each evaluation must rebuild them).</summary>
    public readonly record struct Encounter(string Name, Func<List<Monster>> Build);

    /// <summary>How much bottleneck-survival difference counts as "real" rather than sampling noise. While the
    /// MCTS survival estimate is still noisy (it under/over-shoots razor-thin fights), two decks whose worst
    /// survival is within this band are treated as TIED on survival and decided by expected HP loss — i.e. we
    /// lean on the better-calibrated HP-loss signal. Lower this toward 0 once survival is well-calibrated to
    /// recover strict survival-first ordering. Temporary tuning knob (see project notes).</summary>
    public const double SurvivalBand = 0.05;

    /// <summary>MCTS trial budget for removal/pick RANKING (vs the full budget the dashboard uses for the
    /// displayed survival/HP-loss). Ranking is relative, so the small extra convergence bias at this budget
    /// cancels — measured: the top cut + bottleneck are identical at 500/800/2000 trials, while the advice runs
    /// ~3x faster. Displayed headline numbers keep the caller's full budget to stay as un-pessimistic as possible.</summary>
    public const int AdviceTrials = 800;

    /// <summary>Deck score across the Act's encounters: maximise <see cref="MinSurvival"/> (the bottleneck
    /// fight) — but only when it differs by more than <see cref="SurvivalBand"/> (survival is noisy) — then
    /// minimise <see cref="TotalMeanLoss"/>.</summary>
    public readonly record struct DeckScore(double MinSurvival, double TotalMeanLoss)
    {
        /// <summary>True if this score is better than <paramref name="other"/>: clearly-higher survival wins;
        /// within the survival noise band, lower expected HP loss wins.</summary>
        public bool BetterThan(DeckScore other)
        {
            if (MinSurvival > other.MinSurvival + SurvivalBand) return true;
            if (MinSurvival < other.MinSurvival - SurvivalBand) return false;
            return TotalMeanLoss < other.TotalMeanLoss - 1e-6;        // survival tied (noisy) → HP loss decides
        }
    }

    public readonly record struct AdviceItem(
        string Card, DeckScore After, double SurvivalDelta, double LossDelta)
    {
        /// <summary>An improvement raises bottleneck survival, or (at equal survival) lowers total loss.</summary>
        public bool IsImprovement =>
            SurvivalDelta > 1e-6 || (Math.Abs(SurvivalDelta) <= 1e-6 && LossDelta > 1e-6);
    }

    /// <summary>Build the player, fight each encounter, and aggregate the per-encounter stats lexicographically.
    /// Advice only reads survival + mean (both from MCTS), so the rollout-distribution pass is skipped
    /// (<c>Rollouts = 1</c>) — it's only for the single-deck dashboard. Sequential over encounters: callers
    /// (<see cref="RemovalAdvice"/>/<see cref="PickAdvice"/>) parallelise the OUTER candidate loop instead, which
    /// saturates cores without nested over-subscription.</summary>
    public static DeckScore ScoreDeck(
        IReadOnlyList<string> deckSpecs, IReadOnlyList<Encounter> encounters,
        int playerHp, int playerMaxHp, int maxEnergy, IReadOnlyList<string> relics, EvalOptions opts)
    {
        var fast = opts with { Rollouts = 1 };   // advice never reads the HP-loss distribution
        double minSurvival = 1.0, totalLoss = 0.0;
        foreach (var enc in encounters)
        {
            // Fresh deck instances + fresh monsters per evaluation (both are mutated by combat).
            var deck = deckSpecs.Select(Catalog.BuildCard).ToList();
            var player = Catalog.BuildPlayer(deck, playerHp, playerMaxHp, maxEnergy, relics);
            var setup = Catalog.SetupCombat(player, enc.Build());
            var stats = EncounterEvaluator.Evaluate(setup, fast);
            minSurvival = Math.Min(minSurvival, stats.Survival);
            totalLoss += stats.MeanLoss;
        }
        return new DeckScore(minSurvival, totalLoss);
    }

    /// <summary>Deck strength index 0–100, independent of current run HP. Evaluates each Act elite from a FIXED
    /// 100 HP, averages the expected HP loss (death = the full 100 HP lost, capped), and reports
    /// <c>100 − average</c>. So 100 = takes zero damage from every elite, 0 = certain death to one. Uses the
    /// caller's FULL trial budget (it's a displayed headline, kept un-pessimistic) but skips the rollout
    /// distribution (only the mean is read). Parallel over the (independent) elites. Returns NaN when there's no
    /// elite or no deck to score.</summary>
    public static double DeckStrength(
        IReadOnlyList<string> deckSpecs, IReadOnlyList<Encounter> encounters,
        int maxEnergy, IReadOnlyList<string> relics, EvalOptions opts)
    {
        if (encounters.Count == 0 || deckSpecs.Count == 0) return double.NaN;
        var fast = opts with { Rollouts = 1 };   // only the mean loss is read, not the distribution
        double avgLoss = encounters.AsParallel().Select(enc =>
        {
            var deck = deckSpecs.Select(Catalog.BuildCard).ToList();
            var player = Catalog.BuildPlayer(deck, 100, 100, maxEnergy, relics);
            var stats = EncounterEvaluator.Evaluate(Catalog.SetupCombat(player, enc.Build()), fast);
            return Math.Clamp(stats.MeanLoss, 0, 100);   // death = full 100 lost; cap any overkill
        }).Average();
        return Math.Clamp(100 - avgLoss, 0, 100);
    }

    /// <summary>Rank single-card removals by how much each improves the lexicographic <see cref="DeckScore"/>.
    /// Returns the baseline score and one <see cref="AdviceItem"/> per DISTINCT removable card (a deck with one
    /// copy of N distinct cards yields N items), sorted best-improvement first. A removal that drops the deck
    /// below one card is skipped.</summary>
    public static (DeckScore baseline, List<AdviceItem> items) RemovalAdvice(
        IReadOnlyList<string> deckSpecs, IReadOnlyList<Encounter> encounters,
        int playerHp, int playerMaxHp, int maxEnergy, IReadOnlyList<string> relics, EvalOptions opts)
    {
        var advOpts = opts with { MctsTrials = AdviceTrials };   // ranking-budget (relative; bias cancels)
        var baseline = ScoreDeck(deckSpecs, encounters, playerHp, playerMaxHp, maxEnergy, relics, advOpts);
        if (deckSpecs.Count <= 1) return (baseline, new List<AdviceItem>());

        // Each candidate removal is an independent solve → evaluate them across cores. Deterministic: every
        // eval is independently seeded and the candidate set is sorted afterwards, so scheduling can't reorder
        // the result.
        var items = deckSpecs.Distinct().AsParallel().Select(spec =>
        {
            var reduced = new List<string>(deckSpecs);
            reduced.Remove(spec);   // remove exactly one copy
            var after = ScoreDeck(reduced, encounters, playerHp, playerMaxHp, maxEnergy, relics, advOpts);
            return new AdviceItem(
                Card: spec,
                After: after,
                SurvivalDelta: after.MinSurvival - baseline.MinSurvival,
                LossDelta: baseline.TotalMeanLoss - after.TotalMeanLoss);   // positive = less loss after removal
        }).ToList();

        // Best first: by the lexicographic score of the resulting deck.
        items.Sort((a, b) => a.After.BetterThan(b.After) ? -1 : b.After.BetterThan(a.After) ? 1 : 0);
        return (baseline, items);
    }

    /// <summary>One card-reward option's verdict: the resulting deck's score (taking it), or the
    /// skip-and-keep-the-deck score when <see cref="IsSkip"/>.</summary>
    public readonly record struct PickItem(string Card, bool IsSkip, DeckScore Score);

    /// <summary>Card-reward advice: score the deck WITH each offered card added, and the deck AS-IS (= skip),
    /// then rank by the lexicographic <see cref="DeckScore"/>. Skipping is a first-class option — it wins when
    /// no offered card beats the current deck against the Act's elites. Returns the skip score and the ranked
    /// options (best first; the head is the recommended pick).</summary>
    public static (DeckScore skip, List<PickItem> ranked) PickAdvice(
        IReadOnlyList<string> deckSpecs, IReadOnlyList<string> candidates, IReadOnlyList<Encounter> encounters,
        int playerHp, int playerMaxHp, int maxEnergy, IReadOnlyList<string> relics, EvalOptions opts)
    {
        var advOpts = opts with { MctsTrials = AdviceTrials };   // ranking-budget (relative; bias cancels)
        var skip = ScoreDeck(deckSpecs, encounters, playerHp, playerMaxHp, maxEnergy, relics, advOpts);
        // Score each candidate (and the skip option) across cores — independent, seeded solves; sorted after.
        var ranked = candidates.AsParallel().Select(cand =>
        {
            var withCard = new List<string>(deckSpecs) { cand };
            return new PickItem(cand, false,
                ScoreDeck(withCard, encounters, playerHp, playerMaxHp, maxEnergy, relics, advOpts));
        }).ToList();
        ranked.Add(new PickItem("(skip)", true, skip));
        ranked.Sort((a, b) => a.Score.BetterThan(b.Score) ? -1 : b.Score.BetterThan(a.Score) ? 1 : 0);
        return (skip, ranked);
    }
}
