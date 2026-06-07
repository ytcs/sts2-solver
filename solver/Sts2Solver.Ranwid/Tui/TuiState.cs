namespace Sts2Solver.Ranwid.Tui;

/// <summary>
/// The dashboard's view-model: the latest loaded run plus the in-flight / completed evaluation results that the
/// views render. Mutated only on the UI thread (background evals marshal their results back via
/// <c>Application.Invoke</c>), so no locking is needed. A null <see cref="Ctx"/> means "no readable run yet" and
/// the views show <see cref="Status"/> instead.
/// </summary>
internal sealed class TuiState
{
    public Companion.Context? Ctx;
    public string? Status = "waiting for a readable run… (start or load one)";

    // Deck-strength index (0–100, HP-independent). NaN until first computed.
    public double Strength = double.NaN;
    public bool StrengthBusy;

    // The act boss (its own panel). Null when the run has no boss row at all.
    public EliteResult? Boss;
    public bool BossBusy;

    // The act elites, in table order.
    public List<RowState> Elites = new();

    public sealed class RowState
    {
        public RowState(string label, string comp) { Label = label; Comp = comp; }
        public string Label;
        public string Comp;
        public EliteResult? Result;   // null while Busy / not yet started
        public bool Busy;

        // Stage 2: include/exclude this elite from the current-act deck-strength average (default: included).
        public bool Included = true;
        public Advisor.Encounter? Enc;   // the encounter this row scores (null = unported boss row)
    }
}
