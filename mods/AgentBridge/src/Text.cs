using System.Text.RegularExpressions;
using Godot;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Powers;
using MegaCrit.Sts2.Core.Localization;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.MonsterMoves.Intents;

namespace AgentBridge;

public static partial class Text
{
    [GeneratedRegex(@"(\d ?)?((?:\[img\][^\[]*energy_icon[^\[]*\[/img\])+)")] private static partial Regex EnergyIcon();
    [GeneratedRegex(@"\[img\][^\[]*star_icon[^\[]*\[/img\]")] private static partial Regex StarIcon();
    [GeneratedRegex(@"\[img\][^\[]*\[/img\]")] private static partial Regex AnyImg();
    [GeneratedRegex(@"\[[^\]]*\]")] private static partial Regex Tag();
    [GeneratedRegex(@"\s+")] private static partial Regex Ws();

    public static string Clean(string? s)
    {
        if (string.IsNullOrEmpty(s)) return "";
        s = EnergyIcon().Replace(s, m => m.Groups[1].Value.Length > 0 ? m.Groups[1].Value.Trim() + "E" : Regex.Matches(m.Groups[2].Value, @"\[img\]").Count + "E");
        s = StarIcon().Replace(s, "*");
        s = AnyImg().Replace(s, "");
        s = Tag().Replace(s, "");
        return Ws().Replace(s, " ").Trim();
    }

    public static string Loc(LocString? l)
    {
        if (l == null) return "";
        try { return Clean(l.GetFormattedText()); } catch { return ""; }
    }

    public static string Cost(CardModel c)
    {
        string e;
        try { e = c.EnergyCost.CostsX ? "X" : c.EnergyCost.GetWithModifiers(CostModifiers.All).ToString(); } catch { e = "?"; }
        try { if (c.CurrentStarCost >= 0 || c.HasStarCostX) e += "/" + (c.HasStarCostX ? "X" : c.GetStarCostWithModifiers().ToString()) + "*"; } catch { }
        return e;
    }

    public static string CardDesc(CardModel c, Creature? target = null, bool inHand = false)
    {
        try
        {
            if (inHand)
            {
                c.DynamicVars.ClearPreview();
                c.UpdateDynamicVarPreview(CardPreviewMode.Normal, target, c.DynamicVars);
            }
            return Clean(c.GetDescriptionForPile(inHand ? PileType.Hand : PileType.None, target));
        }
        catch { return ""; }
    }

    public static string Card(CardModel c, bool desc = true, bool inHand = false)
        => desc ? $"{c.Title}({Cost(c)}) {CardDesc(c, null, inHand)}" : $"{c.Title}({Cost(c)})";

    public static string Powers(Creature cr)
    {
        var ps = cr.Powers.Select(p => $"{Loc(p.Title)} {p.DisplayAmount}".Trim()).ToList();
        return ps.Count == 0 ? "" : " [" + string.Join(", ", ps) + "]";
    }

    public static string Intent(Creature m, IEnumerable<Creature> players)
    {
        var move = m.Monster?.NextMove;
        if (move == null) return "";
        var parts = new List<string>();
        foreach (var it in move.Intents)
        {
            if (it is AttackIntent a)
            {
                int d; try { d = a.GetSingleDamage(players, m); } catch { d = -1; }
                parts.Add(a.Repeats > 1 ? $"atk {d}x{a.Repeats}" : $"atk {d}");
            }
            else parts.Add(it.IntentType.ToString().ToLowerInvariant());
        }
        return string.Join("+", parts);
    }

    public static string NodeLabel(Node n)
    {
        foreach (var c in Walk(n))
        {
            string t = c switch { Label l => l.Text, RichTextLabel r => r.Text, _ => "" };
            t = Clean(t);
            if (t.Length > 0) return t;
        }
        return n.Name;
    }

    public static IEnumerable<Node> Walk(Node n)
    {
        if (!GodotObject.IsInstanceValid(n)) yield break;
        yield return n;
        foreach (Node c in n.GetChildren())
            foreach (var d in Walk(c)) yield return d;
    }
}
