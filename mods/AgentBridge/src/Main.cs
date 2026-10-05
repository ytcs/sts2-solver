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
        new HarmonyLib.Harmony("agentbridge").PatchAll(typeof(Main).Assembly);
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
        }
        if (CardSelectCmd.Selector != AgentSelector.Instance)
            CardSelectCmd.PushSelector(AgentSelector.Instance);
    }
}
