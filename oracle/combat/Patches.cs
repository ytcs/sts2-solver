using HarmonyLib;
using MegaCrit.Sts2.Core.Logging;

namespace OracleCombat;

public static class Patches
{
    public static bool VerboseLog = false;
    public static bool ApplyAscensionEffects = false;

    public static void Apply()
    {
        var h = new Harmony("oracle.combat");
        h.PatchAll(typeof(Patches).Assembly);
    }

    [HarmonyPatch(typeof(Logger), "GetIsRunningFromGodotEditor")]
    static class P_LoggerEditor { static bool Prefix(ref bool __result) { __result = false; return false; } }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Nodes.NGame), nameof(MegaCrit.Sts2.Core.Nodes.NGame.GetGameVersion))]
    static class P_Version { static bool Prefix(ref string __result) { __result = "v0.111.0"; return false; } }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Multiplayer.PeerVersionInfo), nameof(MegaCrit.Sts2.Core.Multiplayer.PeerVersionInfo.LocalDefault))]
    static class P_PeerVer { static bool Prefix(ref MegaCrit.Sts2.Core.Multiplayer.PeerVersionInfo __result) { __result = new MegaCrit.Sts2.Core.Multiplayer.PeerVersionInfo { version = "v0.111.0" }; return false; } }

    [HarmonyPatch(typeof(Godot.OS), nameof(Godot.OS.GetCmdlineArgs))]
    static class P_Cmdline { static bool Prefix(ref string[] __result) { __result = Array.Empty<string>(); return false; } }

    [HarmonyPatch(typeof(Godot.OS), nameof(Godot.OS.HasFeature))]
    static class P_HasFeature { static bool Prefix(ref bool __result) { __result = false; return false; } }

    [HarmonyPatch(typeof(Godot.OS), nameof(Godot.OS.GetExecutablePath))]
    static class P_ExePath { static bool Prefix(ref string __result) { __result = GameExe; return false; } }

    public static string GameExe = "/home/ytc/.local/share/Steam/steamapps/common/Slay the Spire 2/SlayTheSpire2";

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Runs.RunManager), nameof(MegaCrit.Sts2.Core.Runs.RunManager.ApplyAscensionEffects))]
    static class P_Asc { static bool Prefix() => ApplyAscensionEffects; }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Assets.PreloadManager), nameof(MegaCrit.Sts2.Core.Assets.PreloadManager.LoadRoomCombatAssets))]
    static class P_Preload { static bool Prefix(ref Task __result) { __result = Task.CompletedTask; return false; } }

    [HarmonyPatch(typeof(Godot.Time), nameof(Godot.Time.GetTicksMsec))]
    static class P_Msec { static bool Prefix(ref ulong __result) { __result = (ulong)Environment.TickCount64; return false; } }

    [HarmonyPatch(typeof(Godot.Time), nameof(Godot.Time.GetTicksUsec))]
    static class P_Usec { static bool Prefix(ref ulong __result) { __result = (ulong)Environment.TickCount64 * 1000UL; return false; } }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Localization.LocString), nameof(MegaCrit.Sts2.Core.Localization.LocString.GetFormattedText))]
    static class P_LocFmt { static bool Prefix(MegaCrit.Sts2.Core.Localization.LocString __instance, ref string __result) { __result = __instance.LocTable + "." + __instance.LocEntryKey; return false; } }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Localization.LocString), nameof(MegaCrit.Sts2.Core.Localization.LocString.GetRawText))]
    static class P_LocRaw { static bool Prefix(MegaCrit.Sts2.Core.Localization.LocString __instance, ref string __result) { __result = __instance.LocTable + "." + __instance.LocEntryKey; return false; } }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Localization.LocString), nameof(MegaCrit.Sts2.Core.Localization.LocString.Exists), new[] { typeof(string), typeof(string) })]
    static class P_LocExists { static bool Prefix(ref bool __result) { __result = true; return false; } }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Runs.RunManager), nameof(MegaCrit.Sts2.Core.Runs.RunManager.WriteReplay))]
    static class P_Replay { static bool Prefix() => false; }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Saves.SaveManager), nameof(MegaCrit.Sts2.Core.Saves.SaveManager.SaveRun))]
    static class P_SaveRun { static bool Prefix(ref Task __result) { __result = Task.CompletedTask; return false; } }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Saves.SaveManager), nameof(MegaCrit.Sts2.Core.Saves.SaveManager.UpdateProgressAfterCombatWon))]
    static class P_Progress { static bool Prefix() => false; }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Saves.SaveManager), nameof(MegaCrit.Sts2.Core.Saves.SaveManager.SaveProgressFile))]
    static class P_ProgressFile { static bool Prefix() => false; }

    [HarmonyPatch]
    static class P_Singleton
    {
        static System.Reflection.MethodBase TargetMethod() =>
            typeof(Godot.GodotObject).Assembly.GetType("Godot.NativeInterop.InteropUtils").GetMethod("EngineGetSingleton", System.Reflection.BindingFlags.Static | System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.NonPublic);
        static bool Prefix(string name, ref Godot.GodotObject __result)
        {
            var t = typeof(Godot.GodotObject).Assembly.GetType("Godot." + name) ?? typeof(Godot.GodotObject);
            __result = (Godot.GodotObject)System.Runtime.CompilerServices.RuntimeHelpers.GetUninitializedObject(t);
            return false;
        }
    }

    [HarmonyPatch]
    static class P_ResLoad {
        static System.Reflection.MethodBase TargetMethod() => typeof(Godot.ResourceLoader).GetMethods().First(m => m.Name == "Load" && !m.IsGenericMethod && m.GetParameters().Length == 3); static bool Prefix(ref Godot.Resource __result) { __result = null; return false; } }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Platform.PlatformUtil), nameof(MegaCrit.Sts2.Core.Platform.PlatformUtil.GetPlayerName))]
    static class P_PlayerName { static bool Prefix(ref string __result) { __result = "Player"; return false; } }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Models.Monsters.FakeMerchantMonster), "GetLinesForMove")]
    static class P_FakeMerchantLines { static bool Prefix(ref IEnumerable<MegaCrit.Sts2.Core.Localization.LocString> __result) { __result = Array.Empty<MegaCrit.Sts2.Core.Localization.LocString>(); return false; } }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Models.Monsters.Crusher), nameof(MegaCrit.Sts2.Core.Models.Monsters.Crusher.BeforeDeath))]
    static class P_CrusherDeath { static bool Prefix(ref Task __result) { __result = Task.CompletedTask; return false; } }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Models.Monsters.Rocket), nameof(MegaCrit.Sts2.Core.Models.Monsters.Rocket.BeforeDeath))]
    static class P_RocketDeath { static bool Prefix(ref Task __result) { __result = Task.CompletedTask; return false; } }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Models.Monsters.KnowledgeDemon), nameof(MegaCrit.Sts2.Core.Models.Monsters.KnowledgeDemon.BeforeRemovedFromRoom))]
    static class P_KnowledgeDemonRemoved { static bool Prefix() => false; }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Models.Monsters.TheInsatiable), nameof(MegaCrit.Sts2.Core.Models.Monsters.TheInsatiable.AfterDeath))]
    static class P_InsatiableDeath { static bool Prefix(ref Task __result) { __result = Task.CompletedTask; return false; } }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Commands.CardSelectCmd), nameof(MegaCrit.Sts2.Core.Commands.CardSelectCmd.FromChooseACardScreen))]
    static class P_ChooseACardSkip
    {
        static void Prefix(bool canSkip) { ChoiceSelector.ChooseACardMustPick = !canSkip; }
        static void Postfix() { ChoiceSelector.ChooseACardMustPick = false; }
    }
    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Models.Monsters.SoulNexus), "AfterDeath")]
    static class P_SoulNexusDeath { static bool Prefix() => false; }

    [HarmonyPatch(typeof(ConsoleLogPrinter), nameof(ConsoleLogPrinter.Print))]
    static class P_Print
    {
        static bool Prefix(LogLevel logLevel, string text)
        {
            if (VerboseLog || logLevel >= LogLevel.Warn) Console.Error.WriteLine($"[game:{logLevel}] {text}");
            return false;
        }
    }
}
