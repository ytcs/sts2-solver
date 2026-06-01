namespace Sts2Solver.Ranwid;

/// <summary>Finds the live, **unmodded** run-save the player is currently in. The game writes the run to
/// plain-JSON <c>current_run.save</c> (and a sibling <c>current*run*.save</c> for multiplayer) itself —
/// no mod needed. Modded runs live under a <c>modded/</c> subtree and are excluded.</summary>
public static class SaveLocator
{
    public static string DefaultRoot =>
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
                     ".local", "share", "SlayTheSpire2");

    /// <summary>All unmodded live-run save files (single-player + multiplayer), newest first.</summary>
    public static IReadOnlyList<string> FindAll(string? root = null)
    {
        root ??= DefaultRoot;
        if (!Directory.Exists(root)) return Array.Empty<string>();
        var moddedSegment = $"{Path.DirectorySeparatorChar}modded{Path.DirectorySeparatorChar}";
        return Directory.EnumerateFiles(root, "current*run*.save", SearchOption.AllDirectories)
            .Where(p => !p.Contains(moddedSegment))
            .Where(p => !p.EndsWith(".backup", StringComparison.OrdinalIgnoreCase))
            .OrderByDescending(File.GetLastWriteTimeUtc)
            .ToList();
    }

    /// <summary>The most-recently-modified unmodded live-run save, or <c>null</c> if none is in progress.</summary>
    public static string? FindNewest(string? root = null) => FindAll(root).FirstOrDefault();
}
