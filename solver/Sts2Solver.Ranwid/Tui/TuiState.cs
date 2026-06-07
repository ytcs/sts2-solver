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

    // Current-act deck-strength index (0–100, HP-independent). NaN until first computed.
    public double Strength = double.NaN;
    public bool StrengthBusy;

    // Next-act deck strength (averaged over that act's elite pool + its possible bosses). HasNextAct is false on
    // the final act (no act after this one).
    public double NextStrength = double.NaN;
    public bool NextStrengthBusy;
    public bool HasNextAct = true;

    // The act boss (its own panel). Null when the run has no boss row at all.
    public EliteResult? Boss;
    public bool BossBusy;

    // The act elites, in table order.
    public List<RowState> Elites = new();

    // Advice overlay (removal / upgrade / reward): replaces the elites region while the deck/strength panels stay put.
    public enum Mode { Dashboard, Advice }
    public enum Kind { Removal, Upgrade, Reward }
    public Mode View = Mode.Dashboard;
    public Kind AdviceKind = Kind.Removal;
    public bool RewardEntry;            // reward mode is awaiting the offered-card text input
    public List<string> Offered = new();
    public int AdviceN = 1;             // how many cards to act on at once (1–3, +/- steps it)
    public bool AdviceExhaustive = true;
    public bool AdviceBusy;
    public Advisor.DualStrength AdviceBaseline;
    public List<Advisor.AdviceRow> AdviceRows = new();

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
