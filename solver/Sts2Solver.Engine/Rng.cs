namespace Sts2Solver.Engine;

/// <summary>
/// Simple RNG for standalone concrete playthroughs and sampling. NOT byte-compatible with the game's
/// Rng — fidelity validation feeds explicit observed outcomes instead of relying on seed equality.
/// </summary>
public sealed class Rng
{
    private readonly Random _r;
    public Rng(int seed) => _r = new Random(seed);

    public int NextInt(int maxExclusive) => _r.Next(maxExclusive);
    public double NextDouble() => _r.NextDouble();
    public float NextFloat(float maxExclusive) => (float)(_r.NextDouble() * maxExclusive);

    public void Shuffle<T>(IList<T> list)
    {
        for (int i = list.Count - 1; i > 0; i--)
        {
            int j = _r.Next(i + 1);
            (list[i], list[j]) = (list[j], list[i]);
        }
    }

    /// <summary>Weighted pick over (item, weight) pairs.</summary>
    public T PickWeighted<T>(IReadOnlyList<(double weight, T item)> options)
    {
        double total = options.Sum(o => o.weight);
        double r = NextDouble() * total;
        foreach (var (weight, item) in options)
        {
            r -= weight;
            if (r <= 0) return item;
        }
        return options[^1].item;
    }
}
