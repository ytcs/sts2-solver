using HarmonyLib;
using MegaCrit.Sts2.Core.Platform;

namespace AgentBridge;

[HarmonyPatch(typeof(PlatformUtil), nameof(PlatformUtil.SetRichPresenceValue))]
public static class RichPresenceCharacter
{
    public const string Label = "AI Self-Play";

    static void Prefix(string key, ref string? value)
    {
        if (key == "Character") value = Label;
    }
}

[HarmonyPatch(typeof(PlatformUtil), nameof(PlatformUtil.SetRichPresence))]
public static class RichPresenceNoJoin
{
    static void Prefix(ref string? playerGroup, ref int? groupSize)
    {
        playerGroup = null;
        groupSize = null;
    }
}
