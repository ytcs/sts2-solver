using Sts2Solver.Engine;

namespace Sts2Solver.Search;

/// <summary>
/// Enumerates the exact distribution of hands produced by drawing N cards. Draw/discard piles are
/// treated as unordered multisets (the shuffle makes order uniform), so a draw is a multivariate
/// hypergeometric sample. Reshuffling the discard into an empty draw pile is handled explicitly.
/// </summary>
public static class DrawEnumerator
{
    /// <summary>Yield (probability, resulting state) for drawing up to n cards from <paramref name="baseState"/>.</summary>
    public static IEnumerable<(double prob, CombatState state)> EnumerateDraw(CombatState baseState, int n)
    {
        var p = baseState.Player;
        int handSpace = Player.MaxHandSize - p.Hand.Count;
        n = Math.Min(n, handSpace);

        int draw = p.DrawPile.Count;
        int disc = p.DiscardPile.Count;
        int handBefore = p.Hand.Count;   // for the Murder draw-counter (only when TracksCardsDrawn)

        if (n <= 0)
        {
            yield return (1.0, baseState.Clone());
            yield break;
        }

        if (n <= draw)
        {
            var counts = CountsByKey(p.DrawPile);
            foreach (var (prob, taken) in Hypergeometric(counts, n))
            {
                var c = baseState.Clone();
                MovePileToHand(c.Player.DrawPile, c.Player.Hand, taken);
                CountDrawn(c, handBefore);
                yield return (prob, c);
            }
            yield break;
        }

        // n > draw: draw the whole draw pile, then reshuffle the discard and draw the remainder.
        int r = n - draw;
        if (disc == 0 || r >= disc)
        {
            var c = baseState.Clone();
            MoveAll(c.Player.DrawPile, c.Player.Hand);
            if (r >= disc && disc > 0) MoveAll(c.Player.DiscardPile, c.Player.Hand);
            CountDrawn(c, handBefore);
            yield return (1.0, c);
            yield break;
        }

        var discCounts = CountsByKey(p.DiscardPile);
        foreach (var (prob, taken) in Hypergeometric(discCounts, r))
        {
            var c = baseState.Clone();
            MoveAll(c.Player.DrawPile, c.Player.Hand);          // whole draw pile drawn
            MoveAll(c.Player.DiscardPile, c.Player.DrawPile);   // reshuffle discard -> draw
            MovePileToHand(c.Player.DrawPile, c.Player.Hand, taken);
            CountDrawn(c, handBefore);
            yield return (prob, c);
        }
    }

    /// <summary>Accumulate the cards-drawn-this-combat counter for the (Murder-bearing) state, by the number
    /// of cards this draw moved into hand. No-op unless the combat tracks it.</summary>
    private static void CountDrawn(CombatState c, int handBefore)
    {
        if (c.TracksCardsDrawn) c.CardsDrawnThisCombat += c.Player.Hand.Count - handBefore;
    }

    /// <summary>
    /// Cheap combinatorial count of the number of *distinct* hands (draw-multisets) that drawing n cards
    /// can produce, without materialising any state. Used to decide exact-enumeration vs DPW at a chance
    /// node. Mirrors <see cref="EnumerateDraw"/>'s reshuffle branches.
    /// </summary>
    public static long DistinctDrawCount(CombatState baseState, int n)
    {
        var p = baseState.Player;
        int handSpace = Player.MaxHandSize - p.Hand.Count;
        n = Math.Min(n, handSpace);
        if (n <= 0) return 1;

        int draw = p.DrawPile.Count;
        int disc = p.DiscardPile.Count;

        if (n <= draw)
            return SubmultisetCount(CountsByKey(p.DrawPile).Values, n);

        int r = n - draw;
        if (disc == 0 || r >= disc) return 1;            // draw the whole pile(s) deterministically
        return SubmultisetCount(CountsByKey(p.DiscardPile).Values, r);
    }

    /// <summary>Number of sub-multisets of size k drawable from a multiset with the given per-key counts.</summary>
    private static long SubmultisetCount(IEnumerable<int> counts, int k)
    {
        // DP over keys: ways[j] = number of ways to pick j cards using the keys seen so far.
        var ways = new long[k + 1];
        ways[0] = 1;
        foreach (int c in counts)
        {
            for (int j = k; j >= 1; j--)
            {
                long acc = 0;
                for (int t = 1; t <= Math.Min(c, j); t++) acc += ways[j - t];
                ways[j] += acc;
            }
        }
        return ways[k];
    }

    /// <summary>
    /// Draw n cards using <paramref name="rng"/>, returning the exact probability of the resulting hand
    /// multiset together with the new state and a canonical key for that outcome. The probability is the
    /// true multivariate-hypergeometric mass of the drawn multiset (closed form, reshuffle-aware), so DPW
    /// chance nodes can weight sampled children by their real probabilities (Partial Bellman backups).
    /// </summary>
    public static (double prob, CombatState state, string takenKey) SampleDraw(CombatState baseState, int n, Rng rng)
    {
        var p = baseState.Player;
        int handSpace = Player.MaxHandSize - p.Hand.Count;
        n = Math.Min(n, handSpace);

        int draw = p.DrawPile.Count;
        int disc = p.DiscardPile.Count;

        if (n <= 0) return (1.0, baseState.Clone(), "∅");

        if (n <= draw)
        {
            var counts = CountsByKey(p.DrawPile);
            var (prob, taken) = SampleSubmultiset(counts, n, rng);
            var c = baseState.Clone();
            MovePileToHand(c.Player.DrawPile, c.Player.Hand, taken);
            return (prob, c, "D:" + Canon(taken));
        }

        int r = n - draw;
        if (disc == 0 || r >= disc)
        {
            var c = baseState.Clone();
            MoveAll(c.Player.DrawPile, c.Player.Hand);
            if (r >= disc && disc > 0) MoveAll(c.Player.DiscardPile, c.Player.Hand);
            return (1.0, c, "ALL");
        }

        var discCounts = CountsByKey(p.DiscardPile);
        var (prob2, taken2) = SampleSubmultiset(discCounts, r, rng);
        var s = baseState.Clone();
        MoveAll(s.Player.DrawPile, s.Player.Hand);          // whole draw pile drawn
        MoveAll(s.Player.DiscardPile, s.Player.DrawPile);   // reshuffle discard -> draw
        MovePileToHand(s.Player.DrawPile, s.Player.Hand, taken2);
        return (prob2, s, "ALL+" + Canon(taken2));
    }

    /// <summary>Sample a size-k sub-multiset (draw without replacement) and return its exact probability.</summary>
    private static (double prob, Dictionary<string, int> taken) SampleSubmultiset(
        Dictionary<string, int> counts, int k, Rng rng)
    {
        // Build a flat bag and Fisher-Yates the first k positions.
        var bag = new List<string>();
        foreach (var (key, count) in counts)
            for (int i = 0; i < count; i++) bag.Add(key);

        int total = bag.Count;
        var taken = new Dictionary<string, int>();
        for (int i = 0; i < k; i++)
        {
            int j = i + rng.NextInt(total - i);
            (bag[i], bag[j]) = (bag[j], bag[i]);
            taken[bag[i]] = taken.GetValueOrDefault(bag[i]) + 1;
        }

        double numer = 1.0;
        foreach (var (key, t) in taken) numer *= Binomial(counts[key], t);
        double prob = numer / Binomial(total, k);
        return (prob, taken);
    }

    private static string Canon(Dictionary<string, int> taken) =>
        string.Join(",", taken.OrderBy(kv => kv.Key, StringComparer.Ordinal).Select(kv => $"{kv.Key}x{kv.Value}"));

    private static Dictionary<string, int> CountsByKey(List<CardModel> pile)
    {
        var d = new Dictionary<string, int>();
        foreach (var card in pile)
            d[card.StateKey()] = d.GetValueOrDefault(card.StateKey()) + 1;
        return d;
    }

    private static void MovePileToHand(List<CardModel> pile, List<CardModel> hand, Dictionary<string, int> taken)
    {
        foreach (var (key, count) in taken)
        {
            for (int i = 0; i < count; i++)
            {
                int idx = pile.FindIndex(c => c.StateKey() == key);
                if (idx < 0) throw new InvalidOperationException($"Pile missing card '{key}'.");
                hand.Add(pile[idx]);
                pile.RemoveAt(idx);
            }
        }
    }

    private static void MoveAll(List<CardModel> from, List<CardModel> to)
    {
        to.AddRange(from);
        from.Clear();
    }

    /// <summary>Enumerate sub-multisets of size k from a multiset, with their probabilities.</summary>
    private static IEnumerable<(double prob, Dictionary<string, int> taken)> Hypergeometric(
        Dictionary<string, int> counts, int k)
    {
        var keys = counts.Keys.ToList();
        int total = counts.Values.Sum();
        double denom = Binomial(total, k);

        foreach (var assignment in EnumerateAssignments(keys, counts, 0, k))
        {
            double numer = 1.0;
            foreach (var (key, taken) in assignment) numer *= Binomial(counts[key], taken);
            var dict = assignment.Where(a => a.taken > 0).ToDictionary(a => a.key, a => a.taken);
            yield return (numer / denom, dict);
        }
    }

    private static IEnumerable<List<(string key, int taken)>> EnumerateAssignments(
        List<string> keys, Dictionary<string, int> counts, int index, int remaining)
    {
        if (index == keys.Count)
        {
            if (remaining == 0) yield return new List<(string, int)>();
            yield break;
        }
        var key = keys[index];
        int avail = counts[key];
        // Reserve enough capacity for the remaining keys.
        int capacityAfter = keys.Skip(index + 1).Sum(k2 => counts[k2]);
        int min = Math.Max(0, remaining - capacityAfter);
        int max = Math.Min(avail, remaining);
        for (int take = min; take <= max; take++)
        {
            foreach (var rest in EnumerateAssignments(keys, counts, index + 1, remaining - take))
            {
                rest.Add((key, take));
                yield return rest;
            }
        }
    }

    private static double Binomial(int n, int k)
    {
        if (k < 0 || k > n) return 0;
        k = Math.Min(k, n - k);
        double result = 1;
        for (int i = 0; i < k; i++) result = result * (n - i) / (i + 1);
        return result;
    }
}
