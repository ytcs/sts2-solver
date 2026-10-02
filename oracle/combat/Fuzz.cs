using System.Text.Json.Nodes;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Models.CardPools;
using MegaCrit.Sts2.Core.Models.PotionPools;
using MegaCrit.Sts2.Core.Models.RelicPools;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Rooms;

namespace OracleCombat;

/// <summary>Random scenario generator for fuzzing: character starter + random extra cards/relics/potions.</summary>
public static class Fuzz
{
    public static IEnumerable<string> AllEncounterIds() =>
        ModelDb.AllEncounters.Select(e => e.Id.Entry).Distinct().OrderBy(x => x, StringComparer.Ordinal);

    public static JsonObject Make(string character, string encounter, int seed, int extraCards, int extraRelics, int extraPotions, int ascension, bool starterOnly)
    {
        var rng = new Random(seed * 7919 + StableHash(encounter) + StableHash(character));
        var ch = ModelDb.GetById<CharacterModel>(new ModelId("CHARACTER", character));
        var deck = new JsonArray();
        foreach (var c in ch.StartingDeck) deck.Add((JsonNode)new JsonObject { ["id"] = c.Id.Entry, ["upgrade"] = 0 });
        var relics = new JsonArray();
        foreach (var r in ch.StartingRelics) relics.Add((JsonNode)r.Id.Entry);
        var potions = new JsonArray();
        if (!starterOnly)
        {
            var cards = ch.CardPool.AllCards.Concat(ModelDb.CardPool<ColorlessCardPool>().AllCards)
                .Where(c => c.Rarity is CardRarity.Common or CardRarity.Uncommon or CardRarity.Rare)
                .OrderBy(c => c.Id.Entry, StringComparer.Ordinal).ToList();
            int nc = rng.Next(0, extraCards + 1);
            for (int i = 0; i < nc; i++)
            {
                var c = cards[rng.Next(cards.Count)];
                int up = c.MaxUpgradeLevel > 0 && rng.Next(3) == 0 ? 1 : 0;
                deck.Add((JsonNode)new JsonObject { ["id"] = c.Id.Entry, ["upgrade"] = up });
            }
            var rel = ch.RelicPool.AllRelics.Concat(ModelDb.RelicPool<SharedRelicPool>().AllRelics)
                .Where(r => !ch.StartingRelics.Any(s => s.Id == r.Id)).OrderBy(r => r.Id.Entry, StringComparer.Ordinal).ToList();
            int nr = rng.Next(0, extraRelics + 1);
            var seen = new HashSet<string>();
            for (int i = 0; i < nr; i++)
            {
                var r = rel[rng.Next(rel.Count)];
                if (seen.Add(r.Id.Entry)) relics.Add((JsonNode)r.Id.Entry);
            }
            var pots = ch.PotionPool.AllPotions.Concat(ModelDb.PotionPool<SharedPotionPool>().AllPotions)
                .OrderBy(p => p.Id.Entry, StringComparer.Ordinal).ToList();
            int np = rng.Next(0, extraPotions + 1);
            for (int i = 0; i < np && i < 3; i++) potions.Add((JsonNode)pots[rng.Next(pots.Count)].Id.Entry);
        }
        int maxHp = ch.StartingHp + rng.Next(0, 3) * 5;
        var o = new JsonObject
        {
            ["name"] = $"fuzz_{character}_{encounter}_{seed}".ToLowerInvariant(),
            ["ascension"] = ascension,
            ["encounter"] = encounter,
            ["character"] = character,
            ["hp"] = maxHp - (starterOnly ? 0 : rng.Next(0, 15)),
            ["max_hp"] = maxHp,
            ["seed"] = "fz" + seed,
            ["total_floor"] = 1 + rng.Next(0, 6),
            ["deck"] = deck,
            ["relics"] = relics,
            ["potions"] = potions,
        };
        return o;
    }

    public static int StableHash(string s) { int h = 17; foreach (var c in s) h = unchecked(h * 31 + c); return h & 0x7fffffff; }
}
