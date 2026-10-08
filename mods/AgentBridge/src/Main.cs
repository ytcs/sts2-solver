using Godot;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Modding;

namespace AgentBridge;

[ModInitializer(nameof(Init))]
public static class Main
{
    public static readonly MegaCrit.Sts2.Core.Logging.Logger Log = new("AgentBridge", MegaCrit.Sts2.Core.Logging.LogType.Generic);

    public static void Init()
    {
        var tree = (SceneTree)Engine.GetMainLoop();
        tree.ProcessFrame += MainThread.Pump;
        try { Engine.MaxFps = 0; DisplayServer.WindowSetVsyncMode(DisplayServer.VSyncMode.Disabled); } catch { }
        new HarmonyLib.Harmony("agentbridge").PatchAll(typeof(Main).Assembly);
        if (AudioMute.Headless) Log.Info("Headless: game audio will be muted (AudioMute)", 0);
        int port = int.TryParse(System.Environment.GetEnvironmentVariable("STS2_BRIDGE_PORT"), out var p) ? p : 15555;
        Server.Start(port);
        Log.Info($"AgentBridge listening on 127.0.0.1:{port}", 0);
    }

    private static bool _setup;

    internal static void EnsureSelector()
    {
        if (!_setup)
        {
            _setup = true;
            try { MegaCrit.Sts2.Core.Saves.SaveManager.Instance.SetFtuesEnabled(enabled: false); } catch { }
            try { MegaCrit.Sts2.Core.Saves.SaveManager.Instance.PrefsSave.FastMode = MegaCrit.Sts2.Core.Settings.FastModeType.Instant; } catch { }
        }
        if (CardSelectCmd.Selector != AgentSelector.Instance)
            CardSelectCmd.PushSelector(AgentSelector.Instance);
    }
}
