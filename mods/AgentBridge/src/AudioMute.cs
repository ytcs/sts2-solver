using HarmonyLib;
using Godot;
using MegaCrit.Sts2.Core.Nodes.Audio;

namespace AgentBridge;

/// <summary>Silence for headless runs (`--headless`, as dev.sh launches the game). The game plays sound through FMOD (a GDScript
/// "Proxy" node under <see cref="NAudioManager"/>), which ignores Godot's dummy audio driver. Every master-volume change goes through
/// <see cref="NAudioManager.SetMasterVol"/> (startup from the saved settings, the settings slider, mute-in-background fade and unmute),
/// so a prefix forcing it to 0 keeps FMOD's master bus silent while the saved settings stay untouched.</summary>
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
