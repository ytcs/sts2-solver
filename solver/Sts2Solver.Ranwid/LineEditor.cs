namespace Sts2Solver.Ranwid;

/// <summary>
/// A minimal interactive line reader with Tab auto-complete on the last whitespace-delimited token. Append-
/// and-backspace editing only (no mid-line cursor movement) — enough for typing card names ergonomically.
/// Falls back to a plain <see cref="Console.ReadLine"/> when input is redirected (pipes/tests/headless), so
/// non-interactive use still works.
/// </summary>
public static class LineEditor
{
    /// <summary>Print <paramref name="prompt"/>, read a line. Tab completes the current token via
    /// <paramref name="complete"/> (unique → fills it in; multiple → extends to the common prefix and lists
    /// the candidates). Returns null on EOF (Ctrl-D).</summary>
    public static string? ReadLine(string prompt, Func<string, IReadOnlyList<string>> complete)
    {
        Console.Write(prompt);
        if (Console.IsInputRedirected) return Console.ReadLine();

        var buf = new System.Text.StringBuilder();
        while (true)
        {
            ConsoleKeyInfo k;
            try { k = Console.ReadKey(intercept: true); }
            catch (InvalidOperationException) { return Console.ReadLine(); }   // no console: degrade gracefully

            if (k.Key == ConsoleKey.Enter) { Console.WriteLine(); return buf.ToString(); }
            if (k.Key == ConsoleKey.Backspace)
            {
                if (buf.Length > 0) { buf.Length--; Console.Write("\b \b"); }
                continue;
            }
            if (k.Key == ConsoleKey.Tab) { HandleTab(buf, prompt, complete); continue; }
            // Ctrl-D / Ctrl-C → end the line / session.
            if (k.Key == ConsoleKey.D && (k.Modifiers & ConsoleModifiers.Control) != 0) return null;
            if (!char.IsControl(k.KeyChar)) { buf.Append(k.KeyChar); Console.Write(k.KeyChar); }
        }
    }

    private static void HandleTab(System.Text.StringBuilder buf, string prompt,
        Func<string, IReadOnlyList<string>> complete)
    {
        string line = buf.ToString();
        int start = line.LastIndexOf(' ') + 1;          // start of the last token
        string token = line[start..];
        if (token.Length == 0) return;

        var hits = complete(token);
        if (hits.Count == 0) return;

        if (hits.Count == 1)
        {
            Replace(buf, start, hits[0] + " ");
            Redraw(prompt, buf);
            return;
        }

        // Multiple: extend to the longest common prefix (if it grows the token), then list candidates.
        string lcp = CardNameMatcher.LongestCommonPrefix(hits);
        if (lcp.Length > token.Length) Replace(buf, start, lcp);
        Console.WriteLine();
        Console.WriteLine("  " + string.Join("   ", hits.Take(12)) + (hits.Count > 12 ? "  …" : ""));
        Redraw(prompt, buf);
    }

    private static void Replace(System.Text.StringBuilder buf, int start, string tail)
    {
        buf.Length = start;
        buf.Append(tail);
    }

    private static void Redraw(string prompt, System.Text.StringBuilder buf)
    {
        // Clear the current line first (CR to column 0 + erase-whole-line) so the rewritten prompt+input
        // replaces what's there instead of being appended to it (which produced "cards> Xcards> XY…" garble).
        Console.Write("\r\u001b[2K");
        Console.Write(prompt);
        Console.Write(buf.ToString());
    }
}
