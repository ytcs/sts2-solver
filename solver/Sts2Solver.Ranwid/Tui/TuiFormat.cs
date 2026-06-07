using Terminal.Gui.Drawing;
using Attribute = Terminal.Gui.Drawing.Attribute;

namespace Sts2Solver.Ranwid.Tui;

/// <summary>
/// Shared palette + text/colour helpers for the Terminal.Gui dashboard. The colour thresholds mirror the
/// Spectre <see cref="Dashboard"/> exactly (strength: ≥75 green / ≥50 yellow / ≥25 orange / else red; survival
/// uses the same bands) so the two front-ends read identically. A single dark base scheme (white-on-black) keeps
/// per-label foreground colours legible regardless of the user's terminal theme.
/// </summary>
internal static class TuiFormat
{
    // 24-bit palette (RGBA). Matches the spirit of the Spectre colours (deepskyblue accent, green/yellow/orange/red).
    public static readonly Color Bg      = new(0x12, 0x12, 0x14, 0xFF);
    public static readonly Color Fg      = new(0xD0, 0xD0, 0xD0, 0xFF);
    public static readonly Color Grey    = new(0x80, 0x80, 0x80, 0xFF);
    public static readonly Color Accent  = new(0x3A, 0x9B, 0xDC, 0xFF);   // deepskyblue
    public static readonly Color Green   = new(0x2E, 0xCC, 0x71, 0xFF);
    public static readonly Color Yellow  = new(0xF1, 0xC4, 0x0F, 0xFF);
    public static readonly Color Orange  = new(0xE6, 0x7E, 0x22, 0xFF);
    public static readonly Color Red     = new(0xE7, 0x4C, 0x3C, 0xFF);

    /// <summary>The app-wide dark base scheme (foreground on <see cref="Bg"/>).</summary>
    public static Scheme Base => SchemeOf(Fg);

    /// <summary>A scheme that paints <paramref name="fg"/> on the dark base background.</summary>
    public static Scheme SchemeOf(Color fg) => new(new Attribute(fg, Bg));

    /// <summary>Tier colour for a 0–100 deck-strength / per-elite-strength value.</summary>
    public static Color StrengthColor(double s) =>
        double.IsNaN(s) ? Grey : s >= 75 ? Green : s >= 50 ? Yellow : s >= 25 ? Orange : Red;

    /// <summary>Tier colour for a 0–1 survival probability (same bands as <see cref="Dashboard"/>).</summary>
    public static Color SurviveColor(double p) =>
        p >= 0.80 ? Green : p >= 0.50 ? Yellow : p >= 0.25 ? Orange : Red;

    public static Color HpColor(int hp, int max)
    {
        double r = max > 0 ? (double)hp / max : 0;
        return r >= 0.66 ? Green : r >= 0.33 ? Yellow : Red;
    }

    /// <summary>"64/100  ███████──────" — the 20-cell strength bar from <see cref="Dashboard.RenderStrength"/>.</summary>
    public static string StrengthBar(double strength)
    {
        if (double.IsNaN(strength)) return "  …/100  " + new string('─', 20);
        int s = (int)System.Math.Round(strength);
        int filled = (int)System.Math.Round(s / 100.0 * 20);
        string bar = new string('█', filled) + new string('─', 20 - filled);
        return $"{s,3}/100  {bar}";
    }

    public static string Pct(double p) => $"{p * 100:F0}%";
}
