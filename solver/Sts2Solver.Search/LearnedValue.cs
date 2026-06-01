using Sts2Solver.Engine;

namespace Sts2Solver.Search;

/// <summary>
/// Phase-C learned leaf value: a compact, self-contained regression that predicts the lexicographic value
/// (P_win, E[HP loss]) of a decision state, fit by supervised regression to exact-solver labels (see
/// <see cref="VfTrainer"/> + the CLI <c>--train-vf</c>). It is the MCTS tip evaluator when
/// <see cref="MctsOptions.UseLearnedLeaf"/> is set, and the intended successor to the hand-tuned static
/// <see cref="CombatHeuristic.Evaluate"/> race model — the baseline it must beat (per
/// <c>docs/mcts-solver-design.md</c> / STATUS Phase C), especially in the razor-thin survival regime a
/// faithful greedy rollout under-estimates.
///
/// The feature vector deliberately INCLUDES the static heuristic's own (survival, loss) estimates alongside
/// raw combat geometry, so the model learns a CORRECTION on top of the baseline rather than relearning it —
/// which makes "at least as good as the baseline" the floor, not the ceiling. Two linear heads over
/// standardised features: a logistic head for win probability, a linear head for HP-loss fraction.
/// Weights are trained offline and embedded below as literals, so runtime evaluation is deterministic and
/// allocation-light.
/// </summary>
public static class LearnedValue
{
    // ---------- feature extraction ----------

    /// <summary>Number of features (must match the trained weight arrays).</summary>
    public const int FeatureCount = 18;

    /// <summary>Extract the feature vector for a decision state. Mirrors <see cref="CombatHeuristic.Evaluate"/>'s
    /// one-turn simulation to get the player's per-turn damage/block, then derives race geometry. The static
    /// heuristic's own survival + loss estimates are features 0 and 1.</summary>
    public static double[] Features(CombatState s, int maxTurns)
    {
        double maxHp = Math.Max(1, s.Player.MaxHp);

        // Player per-turn output (one simulated heuristic turn on a clone) — same gauge the static leaf uses.
        var (dmgPerTurn, blockPerTurn) = CombatHeuristic.SimulateTurnOutput(s);

        int incoming = CombatHeuristic.IncomingDamage(s);
        int unblocked = Math.Max(0, incoming - s.Player.Block);
        int hpAfter = s.Player.CurrentHp - unblocked;
        int enemyHp = CombatHeuristic.EnemyHpTotal(s);
        int netPerTurn = Math.Max(0, incoming - blockPerTurn);

        double ttk = dmgPerTurn <= 0 ? maxTurns + 1 : Math.Ceiling((double)enemyHp / dmgPerTurn);
        double ttd = netPerTurn <= 0 ? maxTurns + 1 : (double)Math.Max(0, hpAfter) / netPerTurn + (hpAfter <= 0 ? 0 : 1);
        double raceMargin = ttd - ttk;
        int turnsLeft = Math.Max(0, maxTurns - s.TurnNumber + 1);

        var baseline = CombatHeuristic.Evaluate(s, maxTurns);

        double strength = s.Player.GetPowerAmount("Strength");
        double enemyVuln = s.Monsters.Where(m => m.IsAlive).Sum(m => m.GetPowerAmount("Vulnerable"));

        var f = new double[FeatureCount];
        f[0] = baseline.Win;                                  // static heuristic survival (correct on top of it)
        f[1] = Math.Min(1.0, baseline.Loss / maxHp);          // static heuristic loss fraction
        f[2] = Clamp01(s.Player.CurrentHp / maxHp);
        f[3] = Clamp01(s.Player.Block / maxHp);
        f[4] = Clamp01(incoming / maxHp);
        f[5] = Clamp01(unblocked / maxHp);
        f[6] = hpAfter / maxHp;                               // signed (negative ⇒ lethal this turn)
        f[7] = Math.Min(4.0, enemyHp / maxHp);
        f[8] = Math.Min(1.0, ttk / Math.Max(1, maxTurns));
        f[9] = Math.Min(1.0, ttd / Math.Max(1, maxTurns));
        f[10] = Math.Tanh(raceMargin / 3.0);                  // squashed race margin (the dominant signal)
        f[11] = Clamp01((double)turnsLeft / Math.Max(1, maxTurns));
        f[12] = Clamp01(dmgPerTurn / maxHp);
        f[13] = Clamp01(blockPerTurn / maxHp);
        f[14] = Clamp01(netPerTurn / maxHp);
        f[15] = Math.Tanh(strength / 10.0);
        f[16] = Math.Tanh(enemyVuln / 5.0);
        f[17] = hpAfter <= 0 ? 1.0 : 0.0;                     // dies to this turn's telegraphed hit
        return f;
    }

    private static double Clamp01(double x) => x < 0 ? 0 : x > 1 ? 1 : x;

    // ---------- evaluation ----------

    /// <summary>Predict the leaf value of a non-terminal decision state. Terminal states are handled by the
    /// caller (this is only invoked at a live tip).</summary>
    public static Value Evaluate(CombatState s, int maxTurns)
    {
        var f = Features(s, maxTurns);
        double win = Predict(f, Weights.WinWeights, Weights.WinBias, logistic: true);
        double lossFrac = Predict(f, Weights.LossWeights, Weights.LossBias, logistic: false);
        double loss = Math.Clamp(lossFrac, 0, 1) * s.Player.MaxHp;
        return new Value(Math.Clamp(win, 0, 1), loss);
    }

    /// <summary>Standardise then apply a linear head (optionally squashed by the logistic for probabilities).</summary>
    public static double Predict(double[] f, double[] weights, double bias, bool logistic)
    {
        double z = bias;
        for (int i = 0; i < FeatureCount; i++)
        {
            double std = Weights.FeatureStd[i];
            double xi = std > 1e-9 ? (f[i] - Weights.FeatureMean[i]) / std : 0.0;
            z += weights[i] * xi;
        }
        return logistic ? 1.0 / (1.0 + Math.Exp(-z)) : z;
    }

    /// <summary>Trained model parameters. Replaced wholesale by the CLI <c>--train-vf</c> emitter; the
    /// committed values below were fit against the exact oracle over the generated training-fixture suite.</summary>
    public static class Weights
    {
        // Fit by `sts2solve --train-vf` (144-fixture TrainingFixtures grid, 425k exact-labelled states; train
        // win MAE 0.134 vs the static-heuristic baseline's 0.242). Regenerate by re-running --train-vf and
        // pasting /tmp/vf-weights.txt. (Feature 15 = player Strength is 0/0 here — the training decks carry no
        // Strength source — so it standardises to 0 and is inert; harmless.)
        public static double[] FeatureMean = new[] { 0.1898741757744541d, 0.19163493941879498d, 0.23086050919077186d, 0.08052048852523541d, 0.4032272713487607d, 0.3285860058638503d, -0.09772976505158461d, 0.5585695241502154d, 0.5114433808447876d, 0.14355782332344927d, -0.5518530470623869d, 0.5344270250036037d, 0.12658107227843865d, 0.11081461484492647d, 0.29850052488679174d, 0d, 0.06346917006416859d, 0.6633796029665848d };
        public static double[] FeatureStd = new[] { 0.30446020384588013d, 0.17783271059622394d, 0.20265650149345552d, 0.10424251234333216d, 0.21564159308071024d, 0.20638373278208516d, 0.2822972308787484d, 0.41129097856528135d, 0.39433419569684675d, 0.304224391862631d, 0.5755779553227737d, 0.2033502887826261d, 0.14028839134449722d, 0.11504839860642482d, 0.19819256502224503d, 0d, 0.13499589171150772d, 0.472553812105279d };
        public static double[] WinWeights = new[] { -0.6164955266418793d, 0.852220357828116d, 0.18181008931336756d, -0.17728662332612716d, -0.8267792297217608d, 0.14441462708702316d, 0.024498527165008308d, -3.1673430115431254d, -0.29046296324624293d, -0.7081644958503461d, 2.273152646690659d, 0.599216217149812d, 0.7583887080528756d, 0.2956853810500491d, -0.09498745153082022d, 0d, 0.17848351191462375d, -0.7884015327731044d };
        public static double WinBias = -2.0938013805776095d;
        public static double[] LossWeights = new[] { -0.012958239778739429d, 0.10771520894838048d, 0.009178866956406176d, 0.0037558461832234748d, 0.012916886998132905d, 0.011708737914531153d, -0.002028840919014943d, 0.0264725738325302d, -0.004773340306980023d, 0.050415467130667205d, -0.055824941223608766d, 0.004366564373668081d, -0.001759858732616632d, -0.005296436885209038d, -0.02590747738663318d, 0d, -0.005144455649430258d, 0.005108903918878239d };
        public static double LossBias = 0.13196041900529062d;
    }
}
