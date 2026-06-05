using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>The five playable characters as a uniform profile — starter deck, starter relic, starting HP and
/// energy — so callers (e.g. the ranwid custom-deck mode) can spin up a fresh run for any character without
/// hardcoding card lists. Starting HP values are the game's (Ironclad 80, Silent 70, Regent 75, Necrobinder 66,
/// Defect 75); the starter relic is the one that wires up the character's combat mechanic (Stars / Osty / orbs)
/// — Burning Blood and the Silent's inert Ring of the Snake have no in-combat effect, so the Silent has none.</summary>
public sealed record CharacterProfile(
    string Key, string CharacterId, string? StarterRelic, int StartingHp, int MaxEnergy,
    Func<List<CardModel>> StarterDeck)
{
    /// <summary>The starter deck as <c>BuildCard</c> specs (the card class names; starters are never upgraded).</summary>
    public List<string> StarterDeckSpecs() => StarterDeck().Select(c => c.Name).ToList();
}

public static partial class Catalog
{
    /// <summary>All five characters, in canonical order. Each <see cref="CharacterProfile.StarterDeck"/> is the
    /// same factory the game/tests use, so the spec list stays in lockstep with the registered cards.</summary>
    public static IReadOnlyList<CharacterProfile> CharacterProfiles { get; } = new[]
    {
        new CharacterProfile("Ironclad",    "CHARACTER.IRONCLAD",    "BurningBlood",    80, 3, IroncladStarterDeck),
        new CharacterProfile("Silent",      "CHARACTER.SILENT",      null,             70, 3, SilentStarterDeck),
        new CharacterProfile("Regent",      "CHARACTER.REGENT",      "DivineRight",     75, 3, RegentStarterDeck),
        new CharacterProfile("Necrobinder", "CHARACTER.NECROBINDER", "BoundPhylactery", 66, 3, NecrobinderStarterDeck),
        new CharacterProfile("Defect",      "CHARACTER.DEFECT",      "CrackedCore",     75, 3, DefectStarterDeck),
    };

    /// <summary>Find a character by key (e.g. "ironclad") or character-id fragment, case-insensitively. Null
    /// when nothing matches.</summary>
    public static CharacterProfile? FindCharacter(string keyOrId)
    {
        if (string.IsNullOrWhiteSpace(keyOrId)) return null;
        return CharacterProfiles.FirstOrDefault(p =>
            string.Equals(p.Key, keyOrId, StringComparison.OrdinalIgnoreCase)
            || p.CharacterId.Contains(keyOrId, StringComparison.OrdinalIgnoreCase)
            || keyOrId.Contains(p.Key, StringComparison.OrdinalIgnoreCase));
    }
}
