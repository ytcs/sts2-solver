using System.Reflection;
using HarmonyLib;
using MegaCrit.Sts2.Core.CardSelection;

namespace AgentBridge;

/// <summary>Records why the game is asking for cards (the prompt and the card / relic / power that asked) before a CardSelectCmd prompt reaches <see cref="AgentSelector"/>.</summary>
[HarmonyPatch]
public static class PromptPatch
{
    public static string? Next;
    /// <summary>The minimum pick a human has on the next screen when the game's call passes a looser one to a custom selector:
    /// `FromChooseACardScreen` always calls `Selector.GetSelectedCards(cards, 0, 1)`, but its screen allows skipping only with `canSkip`
    /// (Knowledge Demon's Curse of Knowledge and Toolbox pass false: a human must pick one).</summary>
    public static int? NextMin;

    static IEnumerable<MethodBase> TargetMethods() =>
        typeof(MegaCrit.Sts2.Core.Commands.CardSelectCmd).GetMethods(BindingFlags.Public | BindingFlags.Static)
            .Where(m => m.Name.StartsWith("From") && !m.IsGenericMethodDefinition);

    static void Prefix(MethodBase __originalMethod, object[] __args)
    {
        try
        {
            string prompt = __originalMethod.Name switch
            {
                "FromChooseACardScreen" => "choose a card",
                "FromHandForUpgrade" => "upgrade a card",
                "FromChooseABundleScreen" => "choose a bundle",
                _ => "",
            };
            string source = "";
            foreach (var a in __args)
            {
                if (a is CardSelectorPrefs p) prompt = Text.Loc(p.Prompt);
                else if (a is AbstractModel m) source = Title(m);
            }
            Next = source.Length > 0 ? $"{prompt} (from {source})" : prompt;
            NextMin = null;
            if (__originalMethod.Name == "FromChooseACardScreen")
            {
                bool canSkip = false;  // the parameter's default
                var ps = __originalMethod.GetParameters();
                for (int i = 0; i < ps.Length && i < __args.Length; i++)
                    if (ps[i].Name == "canSkip" && __args[i] is bool b) canSkip = b;
                if (!canSkip) NextMin = 1;
            }
        }
        catch { Next = null; NextMin = null; }
    }

    private static string Title(AbstractModel m) => m switch
    {
        CardModel c => c.Title,
        RelicModel r => Text.Loc(r.Title),
        PotionModel p => Text.Loc(p.Title),
        PowerModel w => Text.Loc(w.Title),
        _ => m.GetType().Name,
    };
}
