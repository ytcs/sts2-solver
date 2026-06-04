using System.Runtime.Versioning;
using System.Text.RegularExpressions;

namespace Sts2Solver.Ranwid;

/// <summary>Finds the live, **unmodded** run-save the player is currently in. The game writes the run to
/// plain-JSON <c>current_run.save</c> (and a sibling <c>current*run*.save</c> for multiplayer) itself —
/// no mod needed. Modded runs live under a <c>modded/</c> subtree and are excluded.
///
/// <para>The tool only ever needs the SAVE directory (the tree containing <c>current*run*.save</c>); it never
/// reads the game install. On Linux the save lives under
/// <c>~/.local/share/SlayTheSpire2/steam/&lt;id&gt;/…</c>; on Windows it lives at the mirror location
/// <c>%APPDATA%\SlayTheSpire2\steam\&lt;id&gt;\…</c> (confirmed) — we probe that first, then fall back to other
/// profile bases and, as a last resort, derive Steam's location from the registry to scan Steam's per-user
/// <c>userdata</c> tree as well.</para></summary>
public static class SaveLocator
{
    private const int Sts2AppId = 2868840;

    /// <summary>The canonical Linux/macOS save root (kept for back-compat; preserved as the first candidate
    /// on those platforms).</summary>
    public static string DefaultRoot =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
                     ".local", "share", "SlayTheSpire2");

    /// <summary>All platform-appropriate roots under which the STS2 save tree might live, most-likely first.
    /// Roots are not required to exist; callers filter.</summary>
    public static IReadOnlyList<string> CandidateRoots()
    {
        var roots = new List<string>();
        void Add(string? p) { if (!string.IsNullOrWhiteSpace(p)) roots.Add(p!); }

        if (OperatingSystem.IsWindows())
        {
            // STS2 (Godot) on Windows writes its run saves under %APPDATA%\SlayTheSpire2\steam\<steamid>\… —
            // confirmed (Steam community + save-location guides), and the exact mirror of the Linux layout
            // ~/.local/share/SlayTheSpire2/steam/<id>. Probe that canonical root FIRST, then fall back to every
            // plausible base × app-folder-name variant in case a future build/locale differs.
            var roaming = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);        // %APPDATA%
            var local = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);     // %LOCALAPPDATA%
            var profile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);            // %USERPROFILE%
            Add(Path.Combine(roaming, "SlayTheSpire2", "steam"));   // ← the confirmed canonical save root
            var bases = new[]
            {
                roaming, local, profile,
                Path.Combine(profile, "Documents"),
                Path.Combine(profile, "Saved Games"),
            };
            // Folder-name variants the game might use.
            var names = new[] { "SlayTheSpire2", "Slay the Spire 2", "SlaytheSpire2" };
            foreach (var b in bases)
                foreach (var n in names)
                    Add(Path.Combine(b, n));

            // Last resort: Steam's per-user cloud/save tree, e.g. <Steam>\userdata\<uid>\2868840\…
            foreach (var sp in SteamRoots())
            {
                Add(Path.Combine(sp, "userdata"));                 // scan all users for the app-id subtree
                Add(Path.Combine(sp, "steamapps", "common", "Slay the Spire 2"));
            }
        }
        else
        {
            // Linux/macOS — unchanged behaviour, plus a couple of harmless extra fallbacks.
            Add(DefaultRoot);
            var xdg = Environment.GetEnvironmentVariable("XDG_DATA_HOME");
            Add(string.IsNullOrWhiteSpace(xdg) ? null : Path.Combine(xdg, "SlayTheSpire2"));
            // macOS Application Support, in case anyone runs there.
            Add(Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
                             "Library", "Application Support", "SlayTheSpire2"));
        }
        // De-dup, preserve order.
        return roots.Distinct(StringComparer.OrdinalIgnoreCase).ToList();
    }

    /// <summary>All unmodded live-run save files (single-player + multiplayer), newest first.
    /// When <paramref name="root"/> is null, scans every platform candidate root.</summary>
    public static IReadOnlyList<string> FindAll(string? root = null)
    {
        var roots = root != null ? new[] { root } : CandidateRoots().ToArray();
        var moddedSegment = $"{Path.DirectorySeparatorChar}modded{Path.DirectorySeparatorChar}";
        var hits = new List<string>();
        foreach (var r in roots)
        {
            if (!Directory.Exists(r)) continue;
            try
            {
                hits.AddRange(Directory.EnumerateFiles(r, "current*run*.save", SearchOption.AllDirectories)
                    .Where(p => !p.Contains(moddedSegment))
                    .Where(p => !p.EndsWith(".backup", StringComparison.OrdinalIgnoreCase)));
            }
            catch (UnauthorizedAccessException) { /* skip unreadable trees */ }
            catch (IOException) { /* transient/locked: skip */ }
        }
        return hits
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .OrderByDescending(File.GetLastWriteTimeUtc)
            .ToList();
    }

    /// <summary>The most-recently-modified unmodded live-run save, or <c>null</c> if none is in progress.</summary>
    public static string? FindNewest(string? root = null) => FindAll(root).FirstOrDefault();

    // ── Steam install discovery (Windows-only; used purely as a hint for the save tree) ─────────────────────

    /// <summary>Steam install roots discovered on Windows (registry + libraryfolders.vdf). Empty off Windows.</summary>
    public static IReadOnlyList<string> SteamRoots()
    {
        if (!OperatingSystem.IsWindows()) return Array.Empty<string>();
        return SteamRootsWindows();
    }

    [SupportedOSPlatform("windows")]
    private static IReadOnlyList<string> SteamRootsWindows()
    {
        var roots = new List<string>();
        void Add(string? p) { if (!string.IsNullOrWhiteSpace(p)) roots.Add(p!); }

        string? baseSteam = ReadSteamPathFromRegistry();
        Add(baseSteam);
        Add(@"C:\Program Files (x86)\Steam");
        Add(@"C:\Program Files\Steam");

        // Parse libraryfolders.vdf from whatever base we found, to pick up extra library drives.
        foreach (var s in roots.ToArray())
        {
            var vdf = Path.Combine(s, "steamapps", "libraryfolders.vdf");
            if (!File.Exists(vdf)) continue;
            try
            {
                foreach (var lib in ParseLibraryFolders(File.ReadAllText(vdf)))
                    Add(lib);
            }
            catch (IOException) { }
            catch (UnauthorizedAccessException) { }
        }
        return roots.Distinct(StringComparer.OrdinalIgnoreCase).ToList();
    }

    [SupportedOSPlatform("windows")]
    private static string? ReadSteamPathFromRegistry()
    {
        // HKCU first (per-user, set on every install), then HKLM 32/64-bit views.
        try
        {
            using var hkcu = Microsoft.Win32.Registry.CurrentUser.OpenSubKey(@"Software\Valve\Steam");
            if (hkcu?.GetValue("SteamPath") is string p && !string.IsNullOrWhiteSpace(p))
                return p.Replace('/', '\\');
        }
        catch { }
        foreach (var (view, key, value) in new[]
        {
            (Microsoft.Win32.RegistryView.Registry64, @"SOFTWARE\WOW6432Node\Valve\Steam", "InstallPath"),
            (Microsoft.Win32.RegistryView.Registry32, @"SOFTWARE\Valve\Steam", "InstallPath"),
        })
        {
            try
            {
                using var hklm = Microsoft.Win32.RegistryKey.OpenBaseKey(Microsoft.Win32.RegistryHive.LocalMachine, view);
                using var sub = hklm.OpenSubKey(key);
                if (sub?.GetValue(value) is string p && !string.IsNullOrWhiteSpace(p))
                    return p.Replace('/', '\\');
            }
            catch { }
        }
        return null;
    }

    /// <summary>Extract library <c>"path"</c> entries from a libraryfolders.vdf body. Simple line-scan — no VDF
    /// dependency. Public + platform-neutral so it is unit-testable on Linux.</summary>
    public static IEnumerable<string> ParseLibraryFolders(string vdf)
    {
        // Lines look like:  "path"   "D:\\SteamLibrary"
        foreach (Match m in Regex.Matches(vdf, "\"path\"\\s*\"([^\"]+)\"", RegexOptions.IgnoreCase))
        {
            var raw = m.Groups[1].Value;
            // VDF escapes backslashes; collapse \\ → \.
            yield return raw.Replace(@"\\", @"\");
        }
    }
}
