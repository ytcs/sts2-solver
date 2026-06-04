namespace Sts2Solver.Ranwid;

/// <summary>
/// Resolves WHERE to look for the live run-save, with a robust fallback chain so a tester double-clicking the
/// Windows exe always ends up pointed at the right folder:
/// <list type="number">
///   <item>an explicit <c>--save &lt;file&gt;</c> (a specific save file) — handled by callers, takes priority;</item>
///   <item>an explicit root override via <c>--save-dir &lt;folder&gt;</c> or the <c>RANWID_SAVE_DIR</c> env var;</item>
///   <item>a path the tester chose on a previous launch (persisted next to the exe);</item>
///   <item>automatic detection across the platform's candidate roots (<see cref="SaveLocator.CandidateRoots"/>);</item>
///   <item>an interactive console prompt asking the tester to paste the folder, validated before use and persisted.</item>
/// </list>
/// Everything here is cross-platform; the Windows-specific probing lives in <see cref="SaveLocator"/>.
/// </summary>
public static class SaveSource
{
    public const string EnvVar = "RANWID_SAVE_DIR";

    /// <summary>The root directory the tester explicitly chose (CLI <c>--save-dir</c>, env var, or persisted),
    /// or null if none. Validated to exist.</summary>
    public static string? ResolveRoot(string? cliSaveDir)
    {
        var candidate = !string.IsNullOrWhiteSpace(cliSaveDir) ? cliSaveDir
                      : Environment.GetEnvironmentVariable(EnvVar) is { Length: > 0 } e ? e
                      : LoadPersisted();
        candidate = candidate?.Trim().Trim('"');
        return !string.IsNullOrWhiteSpace(candidate) && Directory.Exists(candidate) ? candidate : null;
    }

    /// <summary>Find the newest unmodded save: honour an explicit root override first, else auto-scan all
    /// platform candidates. Returns null if nothing was found anywhere.</summary>
    public static string? FindNewest(string? cliSaveDir)
    {
        var root = ResolveRoot(cliSaveDir);
        return root != null ? SaveLocator.FindNewest(root) : SaveLocator.FindNewest();
    }

    /// <summary>The full resolve-or-prompt flow used at startup. Returns a concrete save FILE path, or null if
    /// the tester gave up (or input is non-interactive and nothing was found).</summary>
    public static string? ResolveSaveFileInteractive(string? cliSaveDir)
    {
        var found = FindNewest(cliSaveDir);
        if (found != null) return found;

        // Nothing auto-detected. Tell the tester what we tried, then prompt (if we have a console).
        Console.Error.WriteLine("ranwid: couldn't find an ongoing Slay the Spire 2 run automatically.");
        if (OperatingSystem.IsWindows())
            Console.Error.WriteLine("  (Make sure a run is in progress, then point ranwid at your save folder.)");
        Console.Error.WriteLine("  Probed locations:");
        foreach (var r in SaveLocator.CandidateRoots())
            Console.Error.WriteLine($"    {(Directory.Exists(r) ? "[exists] " : "[missing] ")}{r}");

        if (Console.IsInputRedirected)
        {
            Console.Error.WriteLine($"  Tip: re-run with --save-dir <folder>, or set {EnvVar}=<folder>.");
            return null;
        }
        return PromptForFolder();
    }

    /// <summary>Interactive console prompt: ask the tester to paste their STS2 save folder, validate it actually
    /// contains a run save, and persist it for next time. Returns a save FILE path, or null if they bail.</summary>
    private static string? PromptForFolder()
    {
        Console.WriteLine();
        Console.WriteLine("Please paste the full path to your Slay the Spire 2 save folder and press Enter.");
        Console.WriteLine("  (On Windows this is usually under your user profile, e.g.");
        Console.WriteLine(@"   C:\Users\<you>\AppData\Roaming\SlayTheSpire2 — paste the SlayTheSpire2 folder, or any");
        Console.WriteLine("   parent of it; ranwid searches it recursively. Leave blank and press Enter to quit.)");

        for (int attempt = 0; attempt < 5; attempt++)
        {
            Console.Write("\nSave folder> ");
            var line = Console.ReadLine();
            if (line == null) return null;                       // EOF
            line = line.Trim().Trim('"');
            if (line.Length == 0) return null;                   // blank = give up

            if (!Directory.Exists(line))
            {
                Console.Error.WriteLine($"  '{line}' is not a folder that exists. Try again.");
                continue;
            }

            var save = SaveLocator.FindNewest(line);
            if (save == null)
            {
                Console.Error.WriteLine("  No current_run.save was found under that folder (start/continue a run first?).");
                continue;
            }

            Save(line);                                          // remember for next launch
            Console.WriteLine($"  Found run save: {save}");
            return save;
        }
        Console.Error.WriteLine("ranwid: giving up after several attempts.");
        return null;
    }

    // ── Persistence (a tiny text file next to the exe / in the user profile) ─────────────────────────────

    private static string PersistPath
    {
        get
        {
            var dir = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
            if (string.IsNullOrWhiteSpace(dir))
                dir = AppContext.BaseDirectory;                  // fallback: beside the exe
            return Path.Combine(dir, "ranwid", "save-dir.txt");
        }
    }

    private static string? LoadPersisted()
    {
        try
        {
            var p = PersistPath;
            return File.Exists(p) ? File.ReadAllText(p).Trim() : null;
        }
        catch { return null; }
    }

    /// <summary>Persist the tester's chosen save root so the next launch finds it without prompting.</summary>
    public static void Save(string root)
    {
        try
        {
            var p = PersistPath;
            Directory.CreateDirectory(Path.GetDirectoryName(p)!);
            File.WriteAllText(p, root);
        }
        catch { /* best-effort: persistence is a nice-to-have, not required */ }
    }
}
