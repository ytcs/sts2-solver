using System.Text.Json.Nodes;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Models.CardPools;
using MegaCrit.Sts2.Core.Models.PotionPools;
using MegaCrit.Sts2.Core.Models.RelicPools;

namespace OracleCombat;

/// <summary>`catalog`: dumps pools (cards/relics/potions per pool) and encounters per act as JSON for the fuzz generator.</summary>
public static class Catalog
{
    public static JsonObject Build()
    {
        var o = new JsonObject();
        JsonArray Cards(CardPoolModel p) => new JsonArray(p.AllCards.Select(c => (JsonNode)new JsonObject
        {
            ["id"] = c.Id.Entry, ["rarity"] = c.Rarity.ToString(), ["type"] = c.Type.ToString(), ["max_upgrade"] = c.MaxUpgradeLevel,
            ["target"] = c.TargetType.ToString(), ["cost"] = c.EnergyCost.Canonical, ["x"] = c.EnergyCost.CostsX,
        }).ToArray());
        var cards = new JsonObject
        {
            ["IRONCLAD"] = Cards(ModelDb.CardPool<IroncladCardPool>()), ["SILENT"] = Cards(ModelDb.CardPool<SilentCardPool>()),
            ["DEFECT"] = Cards(ModelDb.CardPool<DefectCardPool>()), ["NECROBINDER"] = Cards(ModelDb.CardPool<NecrobinderCardPool>()),
            ["REGENT"] = Cards(ModelDb.CardPool<RegentCardPool>()), ["COLORLESS"] = Cards(ModelDb.CardPool<ColorlessCardPool>()),
            ["CURSE"] = Cards(ModelDb.CardPool<CurseCardPool>()), ["STATUS"] = Cards(ModelDb.CardPool<StatusCardPool>()),
            ["EVENT"] = Cards(ModelDb.CardPool<EventCardPool>()), ["TOKEN"] = Cards(ModelDb.CardPool<TokenCardPool>()),
            ["QUEST"] = Cards(ModelDb.CardPool<QuestCardPool>()),
        };
        o["cards"] = cards;
        JsonArray Relics(RelicPoolModel p) => new JsonArray(p.AllRelics.Select(r => (JsonNode)new JsonObject { ["id"] = r.Id.Entry, ["rarity"] = r.Rarity.ToString() }).ToArray());
        o["relics"] = new JsonObject
        {
            ["IRONCLAD"] = Relics(ModelDb.RelicPool<IroncladRelicPool>()), ["SILENT"] = Relics(ModelDb.RelicPool<SilentRelicPool>()),
            ["DEFECT"] = Relics(ModelDb.RelicPool<DefectRelicPool>()), ["NECROBINDER"] = Relics(ModelDb.RelicPool<NecrobinderRelicPool>()),
            ["REGENT"] = Relics(ModelDb.RelicPool<RegentRelicPool>()), ["SHARED"] = Relics(ModelDb.RelicPool<SharedRelicPool>()),
            ["EVENT"] = Relics(ModelDb.RelicPool<EventRelicPool>()),
        };
        JsonArray Potions(PotionPoolModel p) => new JsonArray(p.AllPotions.Select(x => (JsonNode)new JsonObject { ["id"] = x.Id.Entry, ["rarity"] = x.Rarity.ToString(), ["usage"] = x.Usage.ToString(), ["target"] = x.TargetType.ToString() }).ToArray());
        o["potions"] = new JsonObject
        {
            ["IRONCLAD"] = Potions(ModelDb.PotionPool<IroncladPotionPool>()), ["SILENT"] = Potions(ModelDb.PotionPool<SilentPotionPool>()),
            ["DEFECT"] = Potions(ModelDb.PotionPool<DefectPotionPool>()), ["NECROBINDER"] = Potions(ModelDb.PotionPool<NecrobinderPotionPool>()),
            ["REGENT"] = Potions(ModelDb.PotionPool<RegentPotionPool>()), ["SHARED"] = Potions(ModelDb.PotionPool<SharedPotionPool>()),
            ["EVENT"] = Potions(ModelDb.PotionPool<EventPotionPool>()),
        };
        var enc = new JsonArray();
        int ai = 0;
        foreach (var act in ModelDb.Acts)
        {
            foreach (var e in act.AllEncounters)
                enc.Add((JsonNode)new JsonObject { ["id"] = e.Id.Entry, ["act"] = ai, ["act_name"] = act.Id.Entry, ["room"] = e.RoomType.ToString(), ["weak"] = e.IsWeak, ["event"] = false });
            ai++;
        }
        foreach (var e in ModelDb.EventEncounters)
            enc.Add((JsonNode)new JsonObject { ["id"] = e.Id.Entry, ["act"] = -1, ["room"] = e.RoomType.ToString(), ["weak"] = e.IsWeak, ["event"] = true });
        o["encounters"] = enc;
        return o;
    }
}
