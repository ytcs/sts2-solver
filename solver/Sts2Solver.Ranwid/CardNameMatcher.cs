using Sts2Solver.Content;

namespace Sts2Solver.Ranwid;

/// <summary>
/// Ergonomic card-name resolution for the live advisor: turns whatever the user types ("iron wave", "bludgon",
/// "demon") into a canonical Catalog card name, and powers Tab auto-complete. Matching is separator/-case
/// insensitive and tolerant of typos:
///   1. exact (normalised) match,
///   2. unique prefix (auto-complete: "demon" → "DemonForm"),
///   3. nearest by edit distance within a length-scaled threshold (auto-correct: "bludgon" → "Bludgeon"),
/// otherwise it reports the closest candidates so the user can disambiguate. An optional <c>+N</c> upgrade
/// suffix is preserved ("bludgon+1" → "Bludgeon+1"). Pure/deterministic — unit-tested without a console.
/// </summary>
public static class CardNameMatcher
{
    // Canonical Catalog specs (Ironclad + Silent + Colorless) and their normalised forms, index-aligned.
    private static readonly string[] _names = Catalog.CardPool.OrderBy(x => x, StringComparer.Ordinal).ToArray();
    private static readonly string[] _norm = _names.Select(Normalize).ToArray();

    public static IReadOnlyList<string> AllNames => _names;

    /// <summary>The outcome of resolving a typed token. <see cref="Canonical"/> is null when the input was too
    /// ambiguous/unknown to pick one; <see cref="Suggestions"/> then lists the closest candidates. <see
    /// cref="Corrected"/> is true when we interpreted the input as something the user didn't type verbatim
    /// (a completion or a typo-fix), so the caller can surface "interpreting X as Y".</summary>
    public readonly record struct Match(string? Canonical, bool Corrected, IReadOnlyList<string> Suggestions);

    /// <summary>Lowercase and strip everything but letters/digits (so "Iron Wave", "iron_wave", "ironwave" all
    /// collapse to the same key).</summary>
    public static string Normalize(string s)
    {
        var sb = new System.Text.StringBuilder(s.Length);
        foreach (char c in s)
            if (char.IsLetterOrDigit(c)) sb.Append(char.ToLowerInvariant(c));
        return sb.ToString();
    }

    /// <summary>Resolve a typed token to a canonical card spec (auto-correct), preserving a <c>+N</c> suffix.</summary>
    public static Match Resolve(string input)
    {
        if (string.IsNullOrWhiteSpace(input)) return new Match(null, false, System.Array.Empty<string>());

        // Preserve an explicit upgrade suffix (+N) and resolve only the base name.
        string suffix = "";
        int plus = input.IndexOf('+');
        if (plus >= 0) { suffix = input[plus..].Trim(); input = input[..plus]; }

        string n = Normalize(input);
        if (n.Length == 0) return new Match(null, false, System.Array.Empty<string>());

        string Finish(int idx) => _names[idx] + suffix;

        // 1. exact normalised match.
        for (int i = 0; i < _norm.Length; i++)
            if (_norm[i] == n) return new Match(Finish(i), Corrected: false, System.Array.Empty<string>());

        // 2. prefix match — unique → auto-complete; multiple → let the user choose.
        var prefix = new List<int>();
        for (int i = 0; i < _norm.Length; i++) if (_norm[i].StartsWith(n, StringComparison.Ordinal)) prefix.Add(i);
        if (prefix.Count == 1) return new Match(Finish(prefix[0]), Corrected: true, System.Array.Empty<string>());
        if (prefix.Count > 1)
            return new Match(null, false, prefix.Take(6).Select(i => _names[i]).ToList());

        // 3. nearest by edit distance, within a length-scaled threshold → auto-correct.
        int best = -1, bestD = int.MaxValue, secondD = int.MaxValue;
        for (int i = 0; i < _norm.Length; i++)
        {
            int d = Levenshtein(n, _norm[i]);
            if (d < bestD) { secondD = bestD; bestD = d; best = i; }
            else if (d < secondD) secondD = d;
        }
        int threshold = Math.Max(1, n.Length / 3 + 1);
        if (best >= 0 && bestD <= threshold && bestD < secondD)
            return new Match(Finish(best), Corrected: true, System.Array.Empty<string>());

        // Too ambiguous/unknown — return the closest few as suggestions.
        var sugg = Enumerable.Range(0, _norm.Length)
            .OrderBy(i => Levenshtein(n, _norm[i])).Take(5).Select(i => _names[i]).ToList();
        return new Match(null, false, sugg);
    }

    /// <summary>Tab auto-complete: canonical names whose normalised form starts with the (normalised) prefix.</summary>
    public static IReadOnlyList<string> Complete(string prefix)
    {
        string n = Normalize(prefix);
        if (n.Length == 0) return System.Array.Empty<string>();
        var hits = new List<string>();
        for (int i = 0; i < _norm.Length; i++)
            if (_norm[i].StartsWith(n, StringComparison.Ordinal)) hits.Add(_names[i]);
        return hits;
    }

    /// <summary>The longest common prefix of the canonical names (used to extend the token on an ambiguous Tab).</summary>
    public static string LongestCommonPrefix(IReadOnlyList<string> names)
    {
        if (names.Count == 0) return "";
        string p = names[0];
        foreach (var s in names)
        {
            int k = 0; while (k < p.Length && k < s.Length && p[k] == s[k]) k++;
            p = p[..k];
        }
        return p;
    }

    private static int Levenshtein(string a, string b)
    {
        int n = a.Length, m = b.Length;
        if (n == 0) return m;
        if (m == 0) return n;
        var prev = new int[m + 1];
        var cur = new int[m + 1];
        for (int j = 0; j <= m; j++) prev[j] = j;
        for (int i = 1; i <= n; i++)
        {
            cur[0] = i;
            for (int j = 1; j <= m; j++)
            {
                int cost = a[i - 1] == b[j - 1] ? 0 : 1;
                cur[j] = Math.Min(Math.Min(prev[j] + 1, cur[j - 1] + 1), prev[j - 1] + cost);
            }
            (prev, cur) = (cur, prev);
        }
        return prev[m];
    }
}
