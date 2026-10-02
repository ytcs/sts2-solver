using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.TestSupport;

namespace OracleCombat;

public static class Boot
{
    private static bool _done;

    public static void Init(bool verbose)
    {
        if (_done) return;
        _done = true;
        GodotStub.Install();
        Patches.Apply();
        Patches.VerboseLog = verbose;
        TestMode.TurnOnInternal();   // game's own unit-test mode: no Godot nodes / waits / SFX
        typeof(MegaCrit.Sts2.Core.Modding.ModManager).GetProperty("State")
            .SetValue(null, MegaCrit.Sts2.Core.Modding.ModManagerState.Skipped);
        MegaCrit.Sts2.Core.Modding.AssemblyInfo.Init();
        ModelDb.Init();
        MegaCrit.Sts2.Core.Multiplayer.Serialization.ModelIdSerializationCache.Init();
        ModelDb.InitIds();
        MegaCrit.Sts2.Core.Context.LocalContext.NetId = 1UL;
        var sm = MegaCrit.Sts2.Core.Saves.SaveManager.Instance;   // in TestMode this is backed by an in-memory MockGodotFileIo
        sm.InitSettingsDataForTest();
        sm.InitPrefsDataForTest();
        sm.PrefsSave.FastMode = MegaCrit.Sts2.Core.Settings.FastModeType.Instant;
        Log.LogCallback += (level, text, skip) =>
        {
            if (level >= LogLevel.Error && !text.StartsWith("Tried to pop model")) Fatal.Set("game Log.Error: " + text);   // "Tried to pop model": benign choice-context bookkeeping error the game itself logs (Mayhem auto-play)
            else if (level == LogLevel.Warn) Fatal.Warnings.Add(text);
        };
        MegaCrit.Sts2.Core.Helpers.TaskHelper.UnobservedFault += e => Fatal.Set("unobserved task fault: " + e);
    }
}
