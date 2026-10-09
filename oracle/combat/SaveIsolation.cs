using HarmonyLib;
using MegaCrit.Sts2.Core.Saves;

namespace OracleCombat;

// `serve` never touches the game's save folders: every GodotFileIo operation is confined to the per-instance --state-dir
// (writes dropped when there is none). Settings/prefs are the in-memory test saves; the unlock state is UnlockState.all.
public static class SaveIsolation
{
    public static string Dir;
    public static int Writes;

    static string Map(string path)
    {
        if (Dir == null || path == null) return null;
        var name = string.Concat(path.Select(c => char.IsLetterOrDigit(c) || c == '.' || c == '_' || c == '-' ? c : '_'));
        return Path.Combine(Dir, name.TrimStart('_'));
    }

    static void Note(string what, string path)
    {
        if (Writes++ < 20) Console.Error.WriteLine($"serve: save {what} kept in the instance ({path} -> {Map(path) ?? "dropped"})");
    }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.ReadFile))]
    static class P_Read { static bool Prefix(string path, ref string __result) { var p = Map(path); __result = p != null && File.Exists(p) ? File.ReadAllText(p) : null; return false; } }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.ReadFileAsync))]
    static class P_ReadAsync { static bool Prefix(string path, ref Task<string> __result) { var p = Map(path); __result = Task.FromResult(p != null && File.Exists(p) ? File.ReadAllText(p) : null); return false; } }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.WriteFile), new[] { typeof(string), typeof(string) })]
    static class P_WriteS { static bool Prefix(string path, string content) { Note("write", path); var p = Map(path); if (p != null) File.WriteAllText(p, content); return false; } }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.WriteFile), new[] { typeof(string), typeof(byte[]) })]
    static class P_WriteB { static bool Prefix(string path, byte[] bytes) { Note("write", path); var p = Map(path); if (p != null) File.WriteAllBytes(p, bytes); return false; } }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.WriteFileAsync), new[] { typeof(string), typeof(string) })]
    static class P_WriteAS { static bool Prefix(string path, string content, ref Task __result) { Note("write", path); var p = Map(path); if (p != null) File.WriteAllText(p, content); __result = Task.CompletedTask; return false; } }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.WriteFileAsync), new[] { typeof(string), typeof(byte[]) })]
    static class P_WriteAB { static bool Prefix(string path, byte[] bytes, ref Task __result) { Note("write", path); var p = Map(path); if (p != null) File.WriteAllBytes(p, bytes); __result = Task.CompletedTask; return false; } }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.FileExists))]
    static class P_Exists { static bool Prefix(string path, ref bool __result) { var p = Map(path); __result = p != null && File.Exists(p); return false; } }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.DirectoryExists))]
    static class P_DirExists { static bool Prefix(ref bool __result) { __result = Dir != null; return false; } }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.DeleteFile))]
    static class P_Delete { static bool Prefix(string path) { var p = Map(path); if (p != null && File.Exists(p)) File.Delete(p); return false; } }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.RenameFile))]
    static class P_Rename { static bool Prefix(string sourcePath, string destinationPath) { var a = Map(sourcePath); var b = Map(destinationPath); if (a != null && File.Exists(a)) File.Move(a, b, true); return false; } }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.GetFilesInDirectory))]
    static class P_Files { static bool Prefix(ref string[] __result) { __result = Array.Empty<string>(); return false; } }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.GetDirectoriesInDirectory))]
    static class P_Dirs { static bool Prefix(ref string[] __result) { __result = Array.Empty<string>(); return false; } }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.CreateDirectory))]
    static class P_MkDir { static bool Prefix() => false; }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.DeleteDirectory))]
    static class P_RmDir { static bool Prefix() => false; }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.DeleteTemporaryFiles))]
    static class P_RmTmp { static bool Prefix() => false; }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.SetLastModifiedTime))]
    static class P_Touch { static bool Prefix() => false; }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.GetLastModifiedTime))]
    static class P_MTime { static bool Prefix(ref DateTimeOffset __result) { __result = DateTimeOffset.UnixEpoch; return false; } }

    [HarmonyPatch(typeof(GodotFileIo), nameof(GodotFileIo.GetFileSize))]
    static class P_Size { static bool Prefix(string path, ref int __result) { var p = Map(path); __result = p != null && File.Exists(p) ? (int)new FileInfo(p).Length : 0; return false; } }
}
