namespace Sts2Solver.Ranwid;

/// <summary>
/// Pure transforms between the run-save's <c>NAMESPACE.SCREAMING_SNAKE</c> ids and the solver's
/// PascalCase catalog keys. The game's card <c>className</c> (e.g. <c>StrikeIronclad</c>) IS the
/// <c>Catalog.BuildCard</c> key, and <c>CARD.STRIKE_IRONCLAD</c> → <c>StrikeIronclad</c> is a plain
/// snake→Pascal transform; the same holds for encounters and characters.
/// </summary>
public static class GameIds
{
    /// <summary>Strip a leading <c>PREFIX.</c> and PascalCase the SCREAMING_SNAKE remainder.</summary>
    public static string ClassName(string id)
    {
        int dot = id.IndexOf('.');
        var body = dot >= 0 ? id[(dot + 1)..] : id;
        var parts = body.Split('_', StringSplitOptions.RemoveEmptyEntries);
        var sb = new System.Text.StringBuilder(body.Length);
        foreach (var p in parts)
        {
            sb.Append(char.ToUpperInvariant(p[0]));
            if (p.Length > 1) sb.Append(p[1..].ToLowerInvariant());
        }
        return sb.ToString();
    }

    /// <summary>A <c>Catalog.BuildCard</c> spec: the card class name plus the solver's <c>+N</c> upgrade
    /// suffix when upgraded.</summary>
    public static string CardSpec(string cardId, int upgradeLevel)
    {
        var name = ClassName(cardId);
        return upgradeLevel > 0 ? $"{name}+{upgradeLevel}" : name;
    }

    /// <summary>The encounter class name for <c>Catalog.BuildEliteEncounter</c>
    /// (e.g. <c>ENCOUNTER.SKULKING_COLONY_ELITE</c> → <c>SkulkingColonyElite</c>).</summary>
    public static string EncounterClassName(string encounterId) => ClassName(encounterId);

    /// <summary>The five characters the solver fully models (88/88 cards each). The advisor supports a run of
    /// any of them; the starter relic that wires up each one's mechanics (Stars / Osty / orbs) is recognised by
    /// <see cref="ModelledRelicName"/>.</summary>
    public static bool IsSupportedCharacter(string characterId)
    {
        var c = characterId.ToLowerInvariant();
        return c.Contains("ironclad") || c.Contains("silent") || c.Contains("regent")
            || c.Contains("necrobinder") || c.Contains("defect");
    }

    public static string CharacterName(string characterId) => ClassName(characterId);

    /// <summary>The solver relic name if the relic is modelled (currently the five characters' combat-start
    /// starter relics — Burning Blood / Divine Right / Bound Phylactery / Cracked Core; the Silent's Ring of the
    /// Snake has no modelled effect and maps to null), else <c>null</c> (ignored). Generic: any relic the engine
    /// gains a model for is picked up automatically.</summary>
    public static string? ModelledRelicName(string relicId)
    {
        var name = ClassName(relicId);
        return Sts2Solver.Content.Catalog.IsModelledRelic(name) ? name : null;
    }
}
