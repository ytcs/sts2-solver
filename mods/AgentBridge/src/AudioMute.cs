using HarmonyLib;
using Godot;
using MegaCrit.Sts2.Core.Nodes.Audio;

namespace AgentBridge;

[HarmonyPatch(typeof(NAudioManager), nameof(NAudioManager.SetMasterVol))]
public static class AudioMute
{
    public static readonly bool Headless = IsHeadless();

    private static bool _logged;

    private static bool IsHeadless()
    {
        try
        {
            if (OS.GetCmdlineArgs().Contains("--headless") || System.Environment.GetCommandLineArgs().Contains("--headless"))
                return true;
            return DisplayServer.GetName().Equals("headless", System.StringComparison.OrdinalIgnoreCase);
        }
        catch { return false; }
    }

    static void Prefix(ref float volume)
    {
        if (!Headless) return;
        if (!_logged)
        {
            _logged = true;
            Main.Log.Info($"Audio muted (headless): FMOD master volume forced to 0 (requested {volume:0.##}; saved settings unchanged)", 0);
        }
        volume = 0f;
    }
}
