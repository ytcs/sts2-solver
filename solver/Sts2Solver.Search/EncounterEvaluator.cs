using System.Diagnostics;
using Sts2Solver.Engine;

namespace Sts2Solver.Search;

/// <summary>Which solver produced the headline value.</summary>
public enum EvalEngine { Exact, Mcts }

/// <summary>Tunables for <see cref="EncounterEvaluator.Evaluate"/>.</summary>
public sealed class EvalOptions
{
    /// <summary>Turn horizon (a fight not won by here counts as a loss).</summary>
    public int MaxTurns { get; init; } = 30;

    /// <summary>Wall-clock cap for the exact search before falling back to sampling/MCTS. Kept small:
    /// calibration shows the faithful-rollout MCTS tracks exact within a few % while being 10–50× faster,
    /// so exact is worth only a brief attempt (it solves trivial fights, elites fall through quickly).</summary>
    public double BudgetSeconds { get; init; } = 8.0;

    /// <summary>Faithful playouts used to estimate the HP-loss distribution (exact path only).</summary>
    public int Rollouts { get; init; } = 2000;

    /// <summary>Trial budget for the MCTS fallback. Calibrated to ~2k: the rollout leaf reaches exact survival
    /// by ~2k trials, and E[HP loss] settles to within ~3 HP (mildly pessimistic — acceptable for advice). This
    /// keeps a 30-card-vs-elite solve to a couple of seconds; raise it for tighter absolute E[loss].</summary>
    public int MctsTrials { get; init; } = 2_000;

    /// <summary>Base seed for both MCTS and the rollout sampler (reproducible).</summary>
    public int Seed { get; init; } = 1;

    /// <summary>Exact expectimax is only tractable for small decks / short races; a real (advice-regime) deck
    /// can't be solved within any sane budget, so ATTEMPTING exact on it just burns the whole
    /// <see cref="BudgetSeconds"/> before falling to MCTS — pure waste when the advisor runs hundreds of
    /// evaluations. So skip exact outright when the starting draw pile exceeds this (the result is the MCTS one
    /// we'd have reached anyway, minus the wasted budget). The default sits well above the exact-tractable
    /// calibration fixtures (≤ ~10 cards) and well below real run decks (25+). Set to <c>int.MaxValue</c> to
    /// always attempt exact (then <see cref="BudgetSeconds"/> alone bounds it).</summary>
    public int ExactMaxDrawPile { get; init; } = 14;
}

/// <summary>
/// The result of evaluating a deck against one encounter. <see cref="Survival"/> / <see cref="MeanLoss"/>
/// always come from the chosen engine's value. The HP-loss distribution
/// (<see cref="MinLoss"/>…<see cref="P90Loss"/>) is only populated when the *exact* optimal policy was
/// found (<see cref="HasDistribution"/>) — a heuristic policy can't faithfully reproduce optimal play, so
/// on the MCTS fallback we report survival + mean only rather than a misleading spread.
/// </summary>
public sealed record CombatStats(
    EvalEngine Engine,
    double Survival,
    double MeanLoss,
    double NetMeanLoss,
    bool HasDistribution,
    int MinLoss, int MaxLoss, int P10Loss, int P50Loss, int P90Loss,
    int Rollouts,
    long ElapsedMs,
    long Work);

/// <summary>
/// Single entry point for "how does this deck fare against this encounter?". Owns the auto-engine choice
/// (exact expectimax under a wall-clock budget, falling back to MCTS) and — on the exact path — the
/// policy-rollout sampling that turns the optimal policy into a full HP-loss distribution. Userland builds
/// the <see cref="CombatState"/> and calls this; all combat math lives here.
/// </summary>
public static class EncounterEvaluator
{
    /// <summary>The "winnable but very risky" survival floor used when MCTS backs up 0.0% yet the search
    /// observed a win (see <see cref="MctsSolver.ObservedWin"/>) — small enough to read as "don't count on
    /// it", nonzero so the fight isn't dismissed as impossible.</summary>
    private const double SurvivalFloor = 0.005;

    public static CombatStats Evaluate(CombatState setup, EvalOptions? options = null)
    {
        var opt = options ?? new EvalOptions();
        var sw = Stopwatch.StartNew();

        // Tighten the search horizon to a sound upper bound where one can be proven (ramping single-enemy
        // fights the deck can't out-block) — this only ever *reduces* MaxTurns below opt.MaxTurns, never
        // cutting a winning line, so it's safe for every fight (it bails to opt.MaxTurns otherwise). A
        // shorter horizon shrinks the exact tree (more fights solved within budget) and focuses MCTS.
        int maxTurns = HorizonBound.Compute(setup, opt.MaxTurns);

        // Admissible per-node loss certificate (provably-lost decision nodes resolve to their exact value
        // without expansion). Shared by the exact and MCTS paths; null when the fight doesn't qualify.
        var lossProof = LossCertificate.TryBuild(setup, maxTurns);

        // Tractability gate: only attempt exact when the deck is small enough that it can plausibly finish
        // within the budget. On a real advice-regime deck exact never finishes, so attempting it just burns
        // BudgetSeconds before falling to MCTS — skip straight to MCTS (same result, no wasted budget).
        bool tryExact = opt.BudgetSeconds > 0 && setup.Player.DrawPile.Count <= opt.ExactMaxDrawPile;
        if (tryExact)
        {
            var solver = new Solver { MaxTurns = maxTurns, LossProof = lossProof };
            using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(opt.BudgetSeconds));
            solver.Ct = cts.Token;
            try
            {
                var value = solver.Solve(setup);
                solver.Ct = CancellationToken.None;   // clear the budget so rollouts run uninterrupted
                // Exact optimal policy in hand → sample the true HP-loss distribution under it.
                var dist = PolicyRollout.Sample(setup, new ExactMemoPolicy(solver), opt.Rollouts, maxTurns, opt.Seed);
                sw.Stop();
                return new CombatStats(
                    Engine: EvalEngine.Exact,
                    Survival: value.Win,
                    MeanLoss: value.Loss,
                    NetMeanLoss: dist.NetMeanLoss,
                    HasDistribution: true,
                    MinLoss: dist.MinLoss, MaxLoss: dist.MaxLoss,
                    P10Loss: dist.P10Loss, P50Loss: dist.P50Loss, P90Loss: dist.P90Loss,
                    Rollouts: dist.Samples,
                    ElapsedMs: sw.ElapsedMilliseconds,
                    Work: solver.StatesEvaluated);
            }
            catch (OperationCanceledException) { /* exact blew the budget → fall through to MCTS */ }
        }

        // MCTS path: either the deck was gated out of exact, or exact timed out. We DO produce a rollout
        // distribution here (the shared intent-aware HeuristicPolicy characterises the loss spread well enough);
        // headline survival/mean still come from MCTS.
        {
            var mcts = new MctsSolver(new MctsOptions
            {
                Trials = opt.MctsTrials,
                MaxTurns = maxTurns,
                Seed = opt.Seed,
            }) { LossProof = lossProof };
            var value = mcts.Solve(setup);
            // Never report a misleading hard 0% on a fight the search proved winnable (a win was observed but
            // didn't accrue enough probability mass to register): floor it to a small "winnable-but-risky"
            // value. This is the one survival output that matters for play decisions — it stops a player
            // wrongly skipping a beatable elite. HP-loss (the deck-strength proxy) is reported as computed.
            double survival = value.Win;
            if (survival <= 0 && mcts.ObservedWin) survival = SurvivalFloor;
            var dist = PolicyRollout.Sample(setup, new HeuristicPolicy(), opt.Rollouts, maxTurns, opt.Seed);
            sw.Stop();
            return new CombatStats(
                Engine: EvalEngine.Mcts,
                Survival: survival,
                MeanLoss: value.Loss,
                NetMeanLoss: dist.NetMeanLoss,
                HasDistribution: true,
                MinLoss: dist.MinLoss, MaxLoss: dist.MaxLoss,
                P10Loss: dist.P10Loss, P50Loss: dist.P50Loss, P90Loss: dist.P90Loss,
                Rollouts: dist.Samples,
                ElapsedMs: sw.ElapsedMilliseconds,
                Work: mcts.TrialsRun);
        }
    }
}
