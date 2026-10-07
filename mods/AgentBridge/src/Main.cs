using Godot;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Modding;

namespace AgentBridge;

/// <summary>Mod entry point: main-thread pump, card-choice interception, text server on 127.0.0.1:$STS2_BRIDGE_PORT (default 15555).</summary>
[ModInitializer(nameof(Init))]
public static class Main
{
    public static readonly MegaCrit.Sts2.Core.Logging.Logger Log = new("AgentBridge", MegaCrit.Sts2.Core.Logging.LogType.Generic);

    public static void Init()
    {
        var tree = (SceneTree)Engine.GetMainLoop();
        tree.ProcessFrame += MainThread.Pump;
        // the bridge waits in frames (screens settle after N stable frames): an uncapped frame rate makes every wait several times shorter
        try { Engine.MaxFps = 0; DisplayServer.WindowSetVsyncMode(DisplayServer.VSyncMode.Disabled); } catch { }
        new HarmonyLib.Harmony("agentbridge").PatchAll(typeof(Main).Assembly);
        // headless (dev.sh): FMOD ignores Godot's dummy audio driver, so AudioMute forces the master volume to 0 (the game sets it from the settings after mods load)
        if (AudioMute.Headless) Log.Info("Headless: game audio will be muted (AudioMute)", 0);
        int port = int.TryParse(System.Environment.GetEnvironmentVariable("STS2_BRIDGE_PORT"), out var p) ? p : 15555;
        Server.Start(port);
        Log.Info($"AgentBridge listening on 127.0.0.1:{port}", 0);
    }

    private static bool _setup;

    /// <summary>Card choices (hand / deck / grid prompts) go to <see cref="AgentSelector"/> instead of a UI screen. Installed lazily on the main thread.</summary>
    internal static void EnsureSelector()
    {
        if (!_setup)
        {
            _setup = true;
            // tutorial popups (FTUEs) block flows the agent cannot see; the game's own AutoSlay bot turns them off the same way
            try { MegaCrit.Sts2.Core.Saves.SaveManager.Instance.SetFtuesEnabled(enabled: false); } catch { }
            // no combat animations to wait for: the game's own Instant fast mode (7 s instead of 21 s for the same fight)
            try { MegaCrit.Sts2.Core.Saves.SaveManager.Instance.PrefsSave.FastMode = MegaCrit.Sts2.Core.Settings.FastModeType.Instant; } catch { }
        }
        if (CardSelectCmd.Selector != AgentSelector.Instance)
            CardSelectCmd.PushSelector(AgentSelector.Instance);
    }
}
