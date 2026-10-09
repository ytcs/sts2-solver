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
        foreach (var t in AccessTools.GetTypesFromAssembly(typeof(Patches).Assembly).Where(t => t.Namespace == "OracleCombat"))
            h.CreateClassProcessor(t).Patch();
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
    static class P_LocFmt { static bool Prefix(MegaCrit.Sts2.Core.Localization.LocString __instance, ref string __result) { __result = Loc.Real ? Loc.Format(__instance) : __instance.LocTable + "." + __instance.LocEntryKey; return false; } }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Localization.LocString), nameof(MegaCrit.Sts2.Core.Localization.LocString.GetRawText))]
    static class P_LocRaw { static bool Prefix(MegaCrit.Sts2.Core.Localization.LocString __instance, ref string __result) { if (Loc.Real) return true; __result = __instance.LocTable + "." + __instance.LocEntryKey; return false; } }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Localization.LocString), nameof(MegaCrit.Sts2.Core.Localization.LocString.Exists), new[] { typeof(string), typeof(string) })]
    static class P_LocExists { static bool Prefix(ref bool __result) { if (Loc.Real) return true; __result = true; return false; } }

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

    // run-replay: undo TestMode-only behaviour that changes run-level RNG or rewards
    public static bool RealRunRng = false;

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Entities.Merchant.MerchantPotionEntry), nameof(MegaCrit.Sts2.Core.Entities.Merchant.MerchantPotionEntry.CalcCost))]
    static class P_PotionCost
    {
        static void Postfix(MegaCrit.Sts2.Core.Entities.Merchant.MerchantPotionEntry __instance)
        {
            if (!RealRunRng) return;
            var t = Traverse.Create(__instance);
            var player = t.Field("_player").GetValue<MegaCrit.Sts2.Core.Entities.Players.Player>();
            int cost = t.Field("_cost").GetValue<int>();
            t.Field("_cost").SetValue((int)Godot.Mathf.Round((float)cost * player.PlayerRng.Shops.NextFloat(0.95f, 1.05f)));
        }
    }

    // Calling Bell / Cauldron: the shipped game's random rewards, not TestMode's fixed relics and potions
    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Models.Relics.CallingBell), "GenerateRewards")]
    static class P_CallingBell
    {
        static bool Prefix(MegaCrit.Sts2.Core.Models.Relics.CallingBell __instance, ref List<MegaCrit.Sts2.Core.Rewards.Reward> __result)
        {
            if (!RealRunRng) return true;
            __result = new List<MegaCrit.Sts2.Core.Rewards.Reward>
            {
                new MegaCrit.Sts2.Core.Rewards.RelicReward(MegaCrit.Sts2.Core.Entities.Relics.RelicRarity.Common, __instance.Owner),
                new MegaCrit.Sts2.Core.Rewards.RelicReward(MegaCrit.Sts2.Core.Entities.Relics.RelicRarity.Uncommon, __instance.Owner),
                new MegaCrit.Sts2.Core.Rewards.RelicReward(MegaCrit.Sts2.Core.Entities.Relics.RelicRarity.Rare, __instance.Owner),
            };
            return false;
        }
    }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Models.Relics.Cauldron), "GenerateRewards")]
    static class P_Cauldron
    {
        static bool Prefix(MegaCrit.Sts2.Core.Models.Relics.Cauldron __instance, ref List<MegaCrit.Sts2.Core.Rewards.Reward> __result)
        {
            if (!RealRunRng) return true;
            __result = Enumerable.Range(0, __instance.DynamicVars["Potions"].IntValue).Select(_ => (MegaCrit.Sts2.Core.Rewards.Reward)new MegaCrit.Sts2.Core.Rewards.PotionReward(__instance.Owner)).ToList();
            return false;
        }
    }

    // event options that reach node singletons without a null check (audio, screen rumble): the same model steps without them
    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Models.Events.JungleMazeAdventure), "SafetyInNumbers")]
    static class P_JungleMazeJoin
    {
        static bool Prefix(MegaCrit.Sts2.Core.Models.Events.JungleMazeAdventure __instance, ref Task __result) { __result = Run(__instance); return false; }
        static async Task Run(MegaCrit.Sts2.Core.Models.Events.JungleMazeAdventure ev)
        {
            await MegaCrit.Sts2.Core.Commands.Cmd.CustomScaledWait(0f, 0.2f);
            await MegaCrit.Sts2.Core.Commands.PlayerCmd.GainGold(ev.DynamicVars["JoinForcesGold"].BaseValue, ev.Owner);
            ev.SetEventFinished(ev.L10NLookup("JUNGLE_MAZE_ADVENTURE.pages.JOIN_FORCES.description"));
        }
    }

    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Models.Events.DenseVegetation), "Rest")]
    static class P_DenseVegetationRest
    {
        static bool Prefix(MegaCrit.Sts2.Core.Models.Events.DenseVegetation __instance, ref Task __result) { __result = Run(__instance); return false; }
        static async Task Run(MegaCrit.Sts2.Core.Models.Events.DenseVegetation ev)
        {
            await MegaCrit.Sts2.Core.Commands.PlayerCmd.MimicRestSiteHeal(ev.Owner, playSfx: false);
            if (MegaCrit.Sts2.Core.Context.LocalContext.IsMe(ev.Owner)) await MegaCrit.Sts2.Core.Commands.Cmd.CustomScaledWait(0.7f, 1.5f);
            var fight = (Func<Task>)AccessTools.Method(typeof(MegaCrit.Sts2.Core.Models.Events.DenseVegetation), "Fight").CreateDelegate(typeof(Func<Task>), ev);
            ev.SetEventState(ev.L10NLookup("DENSE_VEGETATION.pages.REST.description"),
                new[] { new MegaCrit.Sts2.Core.Events.EventOption(ev, fight, "DENSE_VEGETATION.pages.REST.options.FIGHT") });
        }
    }

    // The Trial's Accept reaches NEventRoom.Instance (portrait, vfx) behind IsMe: the same RNG draw, options and page without the nodes
    [HarmonyPatch(typeof(MegaCrit.Sts2.Core.Models.Events.Trial), "Accept")]
    static class P_TrialAccept
    {
        static bool Prefix(MegaCrit.Sts2.Core.Models.Events.Trial __instance, ref Task __result)
        {
            var ev = __instance;
            Func<Task> M(string name) => (Func<Task>)AccessTools.Method(typeof(MegaCrit.Sts2.Core.Models.Events.Trial), name).CreateDelegate(typeof(Func<Task>), ev);
            MegaCrit.Sts2.Core.Events.EventOption O(string m, string key, IEnumerable<MegaCrit.Sts2.Core.HoverTips.IHoverTip> tips) => new(ev, M(m), key, tips);
            var none = Array.Empty<MegaCrit.Sts2.Core.HoverTips.IHoverTip>();
            var tip = MegaCrit.Sts2.Core.HoverTips.HoverTipFactory.FromCardWithCardHoverTips<MegaCrit.Sts2.Core.Models.Cards.Regret>;
            string entry;
            MegaCrit.Sts2.Core.Events.EventOption[] opts;
            switch (ev.Rng.NextInt(3))
            {
                case 0:
                    entry = "TRIAL.pages.MERCHANT.description";
                    opts = new[] { O("MerchantGuilty", "TRIAL.pages.MERCHANT.options.GUILTY", tip()),
                        O("MerchantInnocent", "TRIAL.pages.MERCHANT.options.INNOCENT", MegaCrit.Sts2.Core.HoverTips.HoverTipFactory.FromCardWithCardHoverTips<MegaCrit.Sts2.Core.Models.Cards.Shame>()) };
                    break;
                case 1:
                    entry = "TRIAL.pages.NOBLE.description";
                    opts = new[] { O("NobleGuilty", "TRIAL.pages.NOBLE.options.GUILTY", none), O("NobleInnocent", "TRIAL.pages.NOBLE.options.INNOCENT", tip()) };
                    break;
                default:
                    entry = "TRIAL.pages.NONDESCRIPT.description";
                    var doubt = MegaCrit.Sts2.Core.HoverTips.HoverTipFactory.FromCardWithCardHoverTips<MegaCrit.Sts2.Core.Models.Cards.Doubt>;
                    opts = new[] { O("NondescriptGuilty", "TRIAL.pages.NONDESCRIPT.options.GUILTY", doubt()),
                        O("NondescriptInnocent", "TRIAL.pages.NONDESCRIPT.options.INNOCENT",
                            doubt().Concat(new[] { MegaCrit.Sts2.Core.HoverTips.HoverTipFactory.Static(MegaCrit.Sts2.Core.HoverTips.StaticHoverTip.Transform) })) };
                    break;
            }
            var loc = ev.L10NLookup("TRIAL.trialFormat");
            loc.Add(new MegaCrit.Sts2.Core.Localization.DynamicVars.StringVar("TrialStory", ev.L10NLookup(entry).GetRawText()));
            ev.SetEventState(loc, opts);
            __result = Task.CompletedTask;
            return false;
        }
    }

    [HarmonyPatch(typeof(Godot.ResourceLoader), nameof(Godot.ResourceLoader.Exists))]
    static class P_ResExists { static bool Prefix(ref bool __result) { __result = false; return false; } }

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
