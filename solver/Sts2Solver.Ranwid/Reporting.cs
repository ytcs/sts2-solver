using Sts2Solver.Search;

namespace Sts2Solver.Ranwid;

/// <summary>One row of the elites table: either computed <see cref="Stats"/> or a <see cref="Skipped"/>
/// reason.</summary>
public sealed record EliteResult(string Elite, string Composition, CombatStats? Stats, string? Skipped);

public static class Reporting
{
    public static void Print(RunState run, string savePath, IReadOnlyList<EliteResult> results,
        IReadOnlyList<string> warnings, string deckSummary, TextWriter? w = null)
    {
        w ??= Console.Out;
        string act = $"Act {run.ActIndex + 1} ({Strip(run.ActId, "ACT.")})";
        string relics = run.RelicIds.Count == 0 ? "none"
            : string.Join(", ", run.RelicIds.Select(r => Strip(r, "RELIC.")));

        w.WriteLine();
        w.WriteLine("════ Ranwid — the seer reads your run ════");
        w.WriteLine($"  save : {savePath}");
        w.WriteLine($"  run  : {GameIds.CharacterName(run.Character)}  A{run.Ascension}  {act}   ·   "
                    + $"HP {run.PlayerHp}/{run.PlayerMaxHp}   ·   {run.MaxEnergy} energy   ·   relics: {relics}");
        w.WriteLine($"  deck ({run.Deck.Count}): {deckSummary}");
        if (run.PlayerCount > 1)
            w.WriteLine($"  ⚠ multiplayer run ({run.PlayerCount} players) — evaluating player net_id {run.PlayerNetId} "
                        + "as a SINGLE-PLAYER fight (the game scales/shares MP elites; numbers are an approximation).");
        w.WriteLine();

        if (results.Count == 0)
        {
            w.WriteLine("  (no evaluable elites for this Act)");
        }
        else
        {
            w.WriteLine("  Act elites — optimal play (maximise survival, then minimise HP loss):");
            w.WriteLine($"  {"Elite",-26} {"Survive",8} {"mean",7} {"net",7} {"p50",6} {"max",6}   engine");
            w.WriteLine("  " + new string('─', 80));
            foreach (var r in results)
            {
                if (r.Stats is { } s)
                {
                    string eng = $"{(s.Engine == EvalEngine.Exact ? "exact" : "mcts ")} "
                               + $"({(s.Engine == EvalEngine.Exact ? $"{s.Work:N0} st" : $"{s.Work:N0} tr")}, {s.ElapsedMs / 1000.0:F1}s)";
                    string p50 = s.HasDistribution ? $"{s.P50Loss}" : "—";
                    string max = s.HasDistribution ? $"{s.MaxLoss}" : "—";
                    w.WriteLine($"  {Trunc(r.Elite, 26),-26} {s.Survival,8:P1} {s.MeanLoss,7:F1} {s.NetMeanLoss,7:F1} "
                                + $"{p50,6} {max,6}   {eng}");
                }
                else
                {
                    w.WriteLine($"  {Trunc(r.Elite, 26),-26}  — skipped: {r.Skipped}");
                }
            }
            w.WriteLine();
            w.WriteLine("  net = mean HP loss after on-victory heal.  p50/max = HP-loss distribution over");
            w.WriteLine($"  {ResultRollouts(results)} rollouts — exact rows use the optimal policy; mcts rows use the");
            w.WriteLine("  intent-aware heuristic policy (an estimate; headline survival/mean are from MCTS).");
        }

        if (warnings.Count > 0)
        {
            w.WriteLine();
            w.WriteLine("  notes:");
            foreach (var note in warnings.Distinct()) w.WriteLine($"    · {note}");
        }
        w.WriteLine();
    }

    private static int ResultRollouts(IReadOnlyList<EliteResult> results) =>
        results.Select(r => r.Stats?.Rollouts ?? 0).DefaultIfEmpty(0).Max();

    private static string Strip(string s, string prefix) =>
        s.StartsWith(prefix, StringComparison.OrdinalIgnoreCase) ? s[prefix.Length..] : s;

    private static string Trunc(string s, int n) => s.Length <= n ? s : s[..(n - 1)] + "…";
}
