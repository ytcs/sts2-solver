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

    /// <summary>Build the player, fight each encounter, and aggregate the per-encounter stats lexicographically.</summary>
    public static DeckScore ScoreDeck(
        IReadOnlyList<string> deckSpecs, IReadOnlyList<Encounter> encounters,
        int playerHp, int playerMaxHp, int maxEnergy, IReadOnlyList<string> relics, EvalOptions opts)
    {
        double minSurvival = 1.0, totalLoss = 0.0;
        foreach (var enc in encounters)
        {
            // Fresh deck instances + fresh monsters per evaluation (both are mutated by combat).
            var deck = deckSpecs.Select(Catalog.BuildCard).ToList();
            var player = Catalog.BuildPlayer(deck, playerHp, playerMaxHp, maxEnergy, relics);
            var setup = Catalog.SetupCombat(player, enc.Build());
            var stats = EncounterEvaluator.Evaluate(setup, opts);
            minSurvival = Math.Min(minSurvival, stats.Survival);
            totalLoss += stats.MeanLoss;
        }
        return new DeckScore(minSurvival, totalLoss);
    }

    /// <summary>Rank single-card removals by how much each improves the lexicographic <see cref="DeckScore"/>.
    /// Returns the baseline score and one <see cref="AdviceItem"/> per DISTINCT removable card (a deck with one
    /// copy of N distinct cards yields N items), sorted best-improvement first. A removal that drops the deck
    /// below one card is skipped.</summary>
    public static (DeckScore baseline, List<AdviceItem> items) RemovalAdvice(
        IReadOnlyList<string> deckSpecs, IReadOnlyList<Encounter> encounters,
        int playerHp, int playerMaxHp, int maxEnergy, IReadOnlyList<string> relics, EvalOptions opts)
    {
        var baseline = ScoreDeck(deckSpecs, encounters, playerHp, playerMaxHp, maxEnergy, relics, opts);
        var items = new List<AdviceItem>();
        if (deckSpecs.Count <= 1) return (baseline, items);

        foreach (var spec in deckSpecs.Distinct())
        {
            // Remove exactly one copy of this spec.
            var reduced = new List<string>(deckSpecs);
            reduced.Remove(spec);
            var after = ScoreDeck(reduced, encounters, playerHp, playerMaxHp, maxEnergy, relics, opts);
            items.Add(new AdviceItem(
                Card: spec,
                After: after,
                SurvivalDelta: after.MinSurvival - baseline.MinSurvival,
                LossDelta: baseline.TotalMeanLoss - after.TotalMeanLoss));   // positive = less loss after removal
        }

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
        var skip = ScoreDeck(deckSpecs, encounters, playerHp, playerMaxHp, maxEnergy, relics, opts);
        var ranked = new List<PickItem> { new("(skip)", true, skip) };
        foreach (var cand in candidates)
        {
            var withCard = new List<string>(deckSpecs) { cand };
            ranked.Add(new PickItem(cand, false,
                ScoreDeck(withCard, encounters, playerHp, playerMaxHp, maxEnergy, relics, opts)));
        }
        ranked.Sort((a, b) => a.Score.BetterThan(b.Score) ? -1 : b.Score.BetterThan(a.Score) ? 1 : 0);
        return (skip, ranked);
    }

    /// <summary>Human-readable card-pick block: the recommendation plus every option's deck score.</summary>
    public static string FormatPick((DeckScore skip, List<PickItem> ranked) advice)
    {
        var (_, ranked) = advice;
        var sb = new System.Text.StringBuilder();
        var best = ranked[0];
        sb.AppendLine(best.IsSkip
            ? "Card reward → SKIP (no offered card improves the deck against the Act's elites):"
            : $"Card reward → take {best.Card} (best deck vs the Act's elites):");
        foreach (var p in ranked)
        {
            var tag = p == ranked[0] ? " ◀ pick" : "";
            sb.AppendLine($"  {(p.IsSkip ? "skip" : p.Card),-22} bottleneck {p.Score.MinSurvival,6:P1}, "
                + $"E[HP loss] {p.Score.TotalMeanLoss,5:F1}{tag}");
        }
        return sb.ToString();
    }

    /// <summary>Human-readable advice block for the report. Shows the bottleneck-survival baseline and the
    /// top improving removals (or a note that no single removal helps).</summary>
    public static string Format((DeckScore baseline, List<AdviceItem> items) advice, int top = 3)
    {
        var (baseline, items) = advice;
        var sb = new System.Text.StringBuilder();
        sb.AppendLine($"Card-removal advice (bottleneck survival {baseline.MinSurvival:P1}, "
            + $"total E[HP loss] {baseline.TotalMeanLoss:F1} across the Act's elites):");
        var improving = items.Where(i => i.IsImprovement).Take(top).ToList();
        if (improving.Count == 0)
        {
            sb.AppendLine("  no single card removal improves the bottleneck — the deck is already lean for these fights.");
            return sb.ToString();
        }
        foreach (var i in improving)
            sb.AppendLine($"  remove {i.Card,-22} → bottleneck {i.After.MinSurvival:P1} "
                + $"({i.SurvivalDelta:+0.0%;-0.0%} surv, {i.LossDelta:+0.0;-0.0} HP loss)");
        return sb.ToString();
    }
}
