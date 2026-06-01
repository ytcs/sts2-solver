using Sts2Solver.Engine;

namespace Sts2Solver.Search;

/// <summary>
/// Offline trainer for the Phase-C <see cref="LearnedValue"/>: harvests (state → exact value) pairs from the
/// exact solver's memo across a suite of small, ground-truth-solvable fixtures, then fits two standardised
/// linear heads — a logistic head for win probability (cross-entropy against the exact soft label) and a
/// linear head for HP-loss fraction (MSE) — by batch gradient descent with L2. Emits the fitted parameters
/// as a C# literal block to paste into <see cref="LearnedValue.Weights"/>. Run via the CLI <c>--train-vf</c>.
///
/// Deterministic given the fixture set and seed (a fixed-seed RNG drives sub-sampling and init), so the
/// committed weights are reproducible.
/// </summary>
public static class VfTrainer
{
    public readonly record struct Example(double[] F, double Win, double LossFrac);

    /// <summary>Solve each fixture exactly (under a per-fixture wall-clock budget; a partial memo from a
    /// timeout still yields valid labels) and reservoir-free sub-sample its decision states into training
    /// examples. <paramref name="sampleRate"/> keeps features cheap (only sampled states are featurised).</summary>
    public static List<Example> Collect(
        IEnumerable<(CombatState setup, int maxTurns)> fixtures,
        double budgetSeconds = 12.0, double sampleRate = 0.12, int maxPerFixture = 4000, int seed = 12345)
    {
        var rng = new Random(seed);
        var examples = new List<Example>();
        foreach (var (setup, maxTurns) in fixtures)
        {
            var solver = new Solver { MaxTurns = maxTurns };
            using var cts = new CancellationTokenSource(TimeSpan.FromSeconds(budgetSeconds));
            solver.Ct = cts.Token;
            int kept = 0;
            solver.OnSolved = (st, v) =>
            {
                if (kept >= maxPerFixture || rng.NextDouble() > sampleRate) return;
                double maxHp = Math.Max(1, st.Player.MaxHp);
                examples.Add(new Example(LearnedValue.Features(st, maxTurns), v.Win, Math.Clamp(v.Loss / maxHp, 0, 1)));
                kept++;
            };
            try { solver.Solve(setup); }
            catch (OperationCanceledException) { /* partial memo already harvested */ }
        }
        return examples;
    }

    public sealed class Model
    {
        public double[] Mean = new double[LearnedValue.FeatureCount];
        public double[] Std = new double[LearnedValue.FeatureCount];
        public double[] WinW = new double[LearnedValue.FeatureCount];
        public double WinB;
        public double[] LossW = new double[LearnedValue.FeatureCount];
        public double LossB;
    }

    /// <summary>Fit both heads. <paramref name="epochs"/> full-batch passes with learning rate
    /// <paramref name="lr"/> and L2 <paramref name="l2"/>.</summary>
    public static Model Fit(List<Example> ex, int epochs = 4000, double lr = 0.3, double l2 = 1e-4, int seed = 1)
    {
        int n = ex.Count, d = LearnedValue.FeatureCount;
        if (n == 0) throw new InvalidOperationException("no training examples");

        var m = new Model();
        // Standardisation params from the raw features.
        for (int j = 0; j < d; j++)
        {
            double s = 0; foreach (var e in ex) s += e.F[j];
            double mean = s / n;
            double v = 0; foreach (var e in ex) { double t = e.F[j] - mean; v += t * t; }
            m.Mean[j] = mean;
            m.Std[j] = Math.Sqrt(v / n);
        }

        // Pre-standardise into a dense matrix for speed.
        var X = new double[n][];
        for (int i = 0; i < n; i++)
        {
            X[i] = new double[d];
            for (int j = 0; j < d; j++)
            {
                double std = m.Std[j];
                X[i][j] = std > 1e-9 ? (ex[i].F[j] - m.Mean[j]) / std : 0.0;
            }
        }

        FitLogistic(X, ex, m.WinW, ref m.WinB, epochs, lr, l2);
        FitLinear(X, ex, m.LossW, ref m.LossB, epochs, lr, l2);
        return m;
    }

    private static void FitLogistic(double[][] X, List<Example> ex, double[] w, ref double b,
        int epochs, double lr, double l2)
    {
        int n = X.Length, d = w.Length;
        var grad = new double[d];
        for (int epoch = 0; epoch < epochs; epoch++)
        {
            Array.Clear(grad);
            double gb = 0;
            for (int i = 0; i < n; i++)
            {
                double z = b;
                for (int j = 0; j < d; j++) z += w[j] * X[i][j];
                double q = 1.0 / (1.0 + Math.Exp(-z));
                double err = q - ex[i].Win;                 // d(cross-entropy)/dz for logistic
                for (int j = 0; j < d; j++) grad[j] += err * X[i][j];
                gb += err;
            }
            double inv = lr / n;
            for (int j = 0; j < d; j++) w[j] -= inv * grad[j] + lr * l2 * w[j];
            b -= inv * gb;
        }
    }

    private static void FitLinear(double[][] X, List<Example> ex, double[] w, ref double b,
        int epochs, double lr, double l2)
    {
        int n = X.Length, d = w.Length;
        var grad = new double[d];
        for (int epoch = 0; epoch < epochs; epoch++)
        {
            Array.Clear(grad);
            double gb = 0;
            for (int i = 0; i < n; i++)
            {
                double pred = b;
                for (int j = 0; j < d; j++) pred += w[j] * X[i][j];
                double err = pred - ex[i].LossFrac;
                for (int j = 0; j < d; j++) grad[j] += err * X[i][j];
                gb += err;
            }
            double inv = lr / n;
            for (int j = 0; j < d; j++) w[j] -= inv * grad[j] + lr * l2 * w[j];
            b -= inv * gb;
        }
    }

    /// <summary>Held-out style report (computed on the training set): mean abs error of each head vs labels,
    /// and — the headline — how the learned win head compares to the static heuristic baseline (feature 0).</summary>
    public static string Report(Model m, List<Example> ex)
    {
        int n = ex.Count, d = LearnedValue.FeatureCount;
        double winErr = 0, lossErr = 0, baseErr = 0;
        foreach (var e in ex)
        {
            // Standardise with THIS freshly-fit model's params (not the committed Weights) for the metric.
            double zw = m.WinB, zl = m.LossB;
            for (int j = 0; j < d; j++)
            {
                double std = m.Std[j];
                double xi = std > 1e-9 ? (e.F[j] - m.Mean[j]) / std : 0.0;
                zw += m.WinW[j] * xi; zl += m.LossW[j] * xi;
            }
            double qw = 1.0 / (1.0 + Math.Exp(-zw));
            winErr += Math.Abs(qw - e.Win);
            lossErr += Math.Abs(Math.Clamp(zl, 0, 1) - e.LossFrac);
            baseErr += Math.Abs(e.F[0] - e.Win);            // feature 0 = static heuristic survival
        }
        return $"examples={n}  win MAE: learned {winErr / n:F4} vs baseline {baseErr / n:F4}  |  loss-frac MAE {lossErr / n:F4}";
    }

    /// <summary>Emit the fitted model as a drop-in replacement for <see cref="LearnedValue.Weights"/>.</summary>
    public static string Emit(Model m)
    {
        string Arr(double[] a) => "{ " + string.Join(", ", a.Select(x => x.ToString("R") + "d")) + " }";
        var sb = new System.Text.StringBuilder();
        sb.AppendLine("    public static class Weights");
        sb.AppendLine("    {");
        sb.AppendLine($"        public static double[] FeatureMean = new[] {Arr(m.Mean)};");
        sb.AppendLine($"        public static double[] FeatureStd = new[] {Arr(m.Std)};");
        sb.AppendLine($"        public static double[] WinWeights = new[] {Arr(m.WinW)};");
        sb.AppendLine($"        public static double WinBias = {m.WinB.ToString("R")}d;");
        sb.AppendLine($"        public static double[] LossWeights = new[] {Arr(m.LossW)};");
        sb.AppendLine($"        public static double LossBias = {m.LossB.ToString("R")}d;");
        sb.AppendLine("    }");
        return sb.ToString();
    }
}
