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

    /// <summary>Metadata for external scenario generators (tools/fuzz_gen_orb_pet.py): encounters, card/relic/potion pools.</summary>
    public static JsonObject ListMeta()
    {
        var o = new JsonObject();
        var encs = new JsonArray();
        foreach (var act in ModelDb.Acts)
            foreach (var e in act.AllEncounters.Where(e => e != null))
                encs.Add((JsonNode)new JsonObject { ["id"] = e.Id.Entry, ["act"] = act.Index, ["act_name"] = act.GetType().Name, ["room"] = e.RoomType.ToString(), ["weak"] = e.IsWeak });
        foreach (var e in ModelDb.EventEncounters)
            encs.Add((JsonNode)new JsonObject { ["id"] = e.Id.Entry, ["act"] = -1, ["act_name"] = "Event", ["room"] = e.RoomType.ToString(), ["weak"] = e.IsWeak });
        o["encounters"] = encs;
        JsonArray Cards(IEnumerable<CardModel> cs) { var a = new JsonArray(); foreach (var c in cs.OrderBy(c => c.Id.Entry, StringComparer.Ordinal)) a.Add((JsonNode)new JsonObject { ["id"] = c.Id.Entry, ["rarity"] = c.Rarity.ToString(), ["type"] = c.Type.ToString(), ["max_up"] = c.MaxUpgradeLevel, ["mp"] = c.MultiplayerConstraint.ToString() }); return a; }
        JsonArray Ids(IEnumerable<string> ids) { var a = new JsonArray(); foreach (var i in ids.OrderBy(x => x, StringComparer.Ordinal)) a.Add((JsonNode)i); return a; }
        var chars = new JsonObject();
        foreach (var ch in ModelDb.AllCharacters)
        {
            chars[ch.Id.Entry] = new JsonObject
            {
                ["hp"] = ch.StartingHp,
                ["starting_deck"] = Ids(ch.StartingDeck.Select(c => c.Id.Entry)),
                ["starting_relics"] = Ids(ch.StartingRelics.Select(r => r.Id.Entry)),
                ["cards"] = Cards(ch.CardPool.AllCards),
                ["relics"] = Ids(ch.RelicPool.AllRelics.Select(r => r.Id.Entry)),
                ["potions"] = Ids(ch.PotionPool.AllPotions.Select(p => p.Id.Entry)),
            };
        }
        o["characters"] = chars;
        o["colorless"] = Cards(ModelDb.CardPool<ColorlessCardPool>().AllCards);
        o["curse"] = Cards(ModelDb.CardPool<CurseCardPool>().AllCards);
        o["status"] = Cards(ModelDb.CardPool<StatusCardPool>().AllCards);
        o["shared_relics"] = new JsonArray(ModelDb.RelicPool<SharedRelicPool>().AllRelics.OrderBy(r => r.Id.Entry, StringComparer.Ordinal).Select(r => (JsonNode)new JsonObject { ["id"] = r.Id.Entry, ["rarity"] = r.Rarity.ToString() }).ToArray());
        var ens = new JsonArray();
        foreach (var e in ModelDb.DebugEnchantments.OrderBy(e => e.Id.Entry, StringComparer.Ordinal))
        {
            if (e.GetType().Namespace != null && e.GetType().Namespace.Contains("Mock")) continue;
            var ok = new JsonArray();
            foreach (var c in ModelDb.AllCards.OrderBy(c => c.Id.Entry, StringComparer.Ordinal))
            {
                bool can = false;
                try { can = e.CanEnchant(c); } catch { }
                if (can) ok.Add((JsonNode)c.Id.Entry);
            }
            ens.Add((JsonNode)new JsonObject { ["id"] = e.Id.Entry, ["show_amount"] = e.ShowAmount, ["stackable"] = e.IsStackable, ["cards"] = ok });
        }
        o["enchantments"] = ens;
        o["shared_potions"] = Ids(ModelDb.PotionPool<SharedPotionPool>().AllPotions.Select(p => p.Id.Entry));
        return o;
    }
}
