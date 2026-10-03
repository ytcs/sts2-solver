using System.Text.Json.Nodes;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Rooms;

namespace OracleCombat;

/// <summary>`dump-pools`: card / relic / potion pools and encounters (with act + room type) for tools/fuzz_gen.py.</summary>
public static class Pools
{
    public static JsonObject Dump()
    {
        var o = new JsonObject();
        var cards = new JsonObject();
        foreach (var pool in ModelDb.AllCardPools)
        {
            var arr = new JsonArray();
            foreach (var c in pool.AllCards.OrderBy(c => c.Id.Entry, StringComparer.Ordinal))
                arr.Add((JsonNode)new JsonObject { ["id"] = c.Id.Entry, ["rarity"] = c.Rarity.ToString(), ["type"] = c.Type.ToString(), ["max_upgrade"] = c.MaxUpgradeLevel });
            cards[pool.GetType().Name] = arr;
        }
        o["cards"] = cards;
        var relics = new JsonObject();
        foreach (var pool in ModelDb.AllRelicPools)
        {
            var arr = new JsonArray();
            foreach (var r in pool.AllRelics.OrderBy(r => r.Id.Entry, StringComparer.Ordinal))
                arr.Add((JsonNode)new JsonObject { ["id"] = r.Id.Entry, ["rarity"] = r.Rarity.ToString() });
            relics[pool.GetType().Name] = arr;
        }
        o["relics"] = relics;
        var potions = new JsonObject();
        foreach (var pool in ModelDb.AllPotionPools)
        {
            var arr = new JsonArray();
            foreach (var p in pool.AllPotions.OrderBy(p => p.Id.Entry, StringComparer.Ordinal))
                arr.Add((JsonNode)new JsonObject { ["id"] = p.Id.Entry, ["rarity"] = p.Rarity.ToString(), ["usage"] = p.Usage.ToString(), ["target"] = p.TargetType.ToString() });
            potions[pool.GetType().Name] = arr;
        }
        o["potions"] = potions;
        var encs = new JsonArray();
        var actOf = new Dictionary<string, string>();
        foreach (var act in ModelDb.Acts)
            foreach (var e in act.AllEncounters) actOf.TryAdd(e.Id.Entry, act.GetType().Name);
        foreach (var e in ModelDb.AllEncounters.OrderBy(e => e.Id.Entry, StringComparer.Ordinal))
            encs.Add((JsonNode)new JsonObject { ["id"] = e.Id.Entry, ["room"] = e.RoomType.ToString(), ["weak"] = e.IsWeak, ["act"] = actOf.GetValueOrDefault(e.Id.Entry, "Event") });
        o["encounters"] = encs;
        var ench = new JsonArray();
        var cardPool = new[] { "IroncladCardPool", "SilentCardPool", "ColorlessCardPool" }.SelectMany(n => ModelDb.AllCardPools.First(p => p.GetType().Name == n).AllCards).ToList();
        foreach (var en in ModelDb.DebugEnchantments.OrderBy(e => e.Id.Entry, StringComparer.Ordinal))
        {
            string tn = en.GetType().Name;
            if (en.GetType().Namespace.Contains("Mocks") || tn.StartsWith("Deprecated")) continue;
            var ok = new JsonArray();
            foreach (var c in cardPool) { bool can; try { can = en.CanEnchant(c); } catch { can = false; } if (can) ok.Add((JsonNode)c.Id.Entry); }
            ench.Add((JsonNode)new JsonObject { ["id"] = en.Id.Entry, ["show_amount"] = en.ShowAmount, ["stackable"] = en.IsStackable, ["cards"] = ok });
        }
        o["enchantments"] = ench;
        return o;
    }
}
