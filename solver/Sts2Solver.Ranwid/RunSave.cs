using System.Text.Json;
using System.Text.Json.Serialization;

namespace Sts2Solver.Ranwid;

// ---------- Save JSON DTOs (snake_case via the naming policy) ----------

internal sealed class SaveDto
{
    public int Ascension { get; set; }
    public int CurrentActIndex { get; set; }
    public List<ActDto>? Acts { get; set; }
    public List<PlayerDto>? Players { get; set; }
}

internal sealed class ActDto
{
    public string? Id { get; set; }
    public RoomsDto? Rooms { get; set; }
}

internal sealed class RoomsDto
{
    public string? BossId { get; set; }
    public List<string>? EliteEncounterIds { get; set; }
}

internal sealed class PlayerDto
{
    public int NetId { get; set; }
    public string CharacterId { get; set; } = "";
    public int CurrentHp { get; set; }
    public int MaxHp { get; set; }
    public int MaxEnergy { get; set; }
    public List<CardDto>? Deck { get; set; }
    public List<RelicDto>? Relics { get; set; }
}

internal sealed class CardDto
{
    public string Id { get; set; } = "";
    public int CurrentUpgradeLevel { get; set; }
    public JsonElement Enchantment { get; set; }   // present only when enchanted

    public bool HasEnchant => Enchantment.ValueKind == JsonValueKind.Object;
}

internal sealed class RelicDto { public string Id { get; set; } = ""; }

// ---------- Domain model the rest of the program consumes ----------

public sealed record CardEntry(string Id, int Upgrade, bool HasEnchant);

public sealed record RunState(
    int Ascension,
    int ActIndex,
    string ActId,
    string Character,
    int PlayerHp,
    int PlayerMaxHp,
    int MaxEnergy,
    IReadOnlyList<CardEntry> Deck,
    IReadOnlyList<string> RelicIds,
    IReadOnlyList<string> EliteEncounterIds,   // distinct, in first-seen order
    string? BossId,
    int PlayerCount,
    int PlayerNetId);

/// <summary>Parses a <c>current_run.save</c> into a <see cref="RunState"/>.</summary>
public static class RunSaveReader
{
    private static readonly JsonSerializerOptions Opts = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
        PropertyNameCaseInsensitive = true,
        NumberHandling = JsonNumberHandling.AllowReadingFromString,
    };

    /// <summary>Parse the save at <paramref name="path"/>. If multiple players (multiplayer), pick the
    /// player whose net_id is <paramref name="preferNetId"/>, else the first Ironclad, else the first.
    /// Resilient to a momentarily half-written file (retries, then falls back to the <c>.backup</c>).</summary>
    public static RunState Parse(string path, int? preferNetId = null)
    {
        var dto = ReadResilient(path)
            ?? throw new InvalidDataException($"Could not read/parse a valid run save at '{path}'.");

        var players = dto.Players ?? throw new InvalidDataException("Save has no players.");
        if (players.Count == 0) throw new InvalidDataException("Save has an empty players list.");

        PlayerDto player =
            (preferNetId is int nid ? players.FirstOrDefault(p => p.NetId == nid) : null)
            ?? players.FirstOrDefault(p => GameIds.IsSupportedCharacter(p.CharacterId))
            ?? players[0];

        int actIdx = Math.Clamp(dto.CurrentActIndex, 0, (dto.Acts?.Count ?? 1) - 1);
        var act = dto.Acts is { Count: > 0 } ? dto.Acts[actIdx] : null;
        var rooms = act?.Rooms;
        var elites = (rooms?.EliteEncounterIds ?? new List<string>())
            .Where(s => !string.IsNullOrEmpty(s)).Distinct().ToList();

        var deck = (player.Deck ?? new List<CardDto>())
            .Select(c => new CardEntry(c.Id, c.CurrentUpgradeLevel, c.HasEnchant))
            .ToList();
        var relics = (player.Relics ?? new List<RelicDto>()).Select(r => r.Id).ToList();

        return new RunState(
            Ascension: dto.Ascension,
            ActIndex: actIdx,
            ActId: act?.Id ?? "ACT.?",
            Character: player.CharacterId,
            PlayerHp: player.CurrentHp,
            PlayerMaxHp: player.MaxHp,
            MaxEnergy: player.MaxEnergy > 0 ? player.MaxEnergy : 3,
            Deck: deck,
            RelicIds: relics,
            EliteEncounterIds: elites,
            BossId: rooms?.BossId,
            PlayerCount: players.Count,
            PlayerNetId: player.NetId);
    }

    private static SaveDto? ReadResilient(string path)
    {
        foreach (var candidate in new[] { path, path + ".backup" })
        {
            if (!File.Exists(candidate)) continue;
            for (int attempt = 0; attempt < 3; attempt++)
            {
                try
                {
                    var dto = JsonSerializer.Deserialize<SaveDto>(File.ReadAllText(candidate), Opts);
                    if (dto?.Players is { Count: > 0 }) return dto;
                }
                catch (IOException) { Thread.Sleep(120); }       // mid-write; retry
                catch (JsonException) { Thread.Sleep(120); }     // half-flushed; retry then try .backup
            }
        }
        return null;
    }
}
