using Sts2Solver.Content;
using Sts2Solver.Engine;

namespace Sts2Solver.Ranwid.Tui;

/// <summary>
/// Turns the flat deck-spec list into a skimmable, type-grouped view for the deck panel: one line per card type
/// (Attacks / Skills / Powers / Status / Curses), each with a count and the cards within it grouped by name
/// (most-copies first). Far easier to read at a glance than one long comma-separated blob.
/// </summary>
internal static class DeckView
{
    private static readonly CardType[] Order =
        { CardType.Attack, CardType.Skill, CardType.Power, CardType.Status, CardType.Curse };

    /// <summary>One presentable line per non-empty card type, in <see cref="Order"/>.</summary>
    public static List<(CardType Type, string Text)> Lines(IReadOnlyList<string> deckSpecs)
    {
        var byType = new Dictionary<CardType, List<CardModel>>();
        foreach (var spec in deckSpecs)
        {
            CardModel card;
            try { card = Catalog.BuildCard(spec); } catch { continue; }   // specs are pre-validated; skip defensively
            if (!byType.TryGetValue(card.Type, out var list)) byType[card.Type] = list = new();
            list.Add(card);
        }

        var lines = new List<(CardType, string)>();
        foreach (var t in Order)
        {
            if (!byType.TryGetValue(t, out var cards)) continue;
            var groups = cards
                .GroupBy(DisplayName)
                .Select(g => (Name: g.Key, Count: g.Count()))
                .OrderByDescending(g => g.Count).ThenBy(g => g.Name, StringComparer.Ordinal);
            string joined = string.Join(", ", groups.Select(g => g.Count > 1 ? $"{g.Count}× {g.Name}" : g.Name));
            lines.Add((t, $"{TypeLabel(t),-8}{cards.Count,3}   {joined}"));
        }
        return lines;
    }

    /// <summary>Card name with a ★ suffix when upgraded (one or more times).</summary>
    private static string DisplayName(CardModel c) => c.Name + (c.Upgrades > 0 ? "★" : "");

    private static string TypeLabel(CardType t) => t switch
    {
        CardType.Attack => "Attacks",
        CardType.Skill  => "Skills",
        CardType.Power  => "Powers",
        CardType.Status => "Status",
        CardType.Curse  => "Curses",
        _               => t.ToString(),
    };
}
