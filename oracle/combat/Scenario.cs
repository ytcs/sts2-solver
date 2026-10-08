using System.Text.Json;
using System.Text.Json.Nodes;

namespace OracleCombat;

public sealed class CardSpec
{
    public string Id;
    public int Upgrade;
    public string Enchantment;
    public int EnchantAmount = 1;
    public JsonObject Props;
    public int? FloorAdded;
}

public sealed class RelicSpec { public string Id; public JsonObject Props; }
public sealed class PotionSpec { public string Id; public int Slot = -1; }

public sealed class RngSpec { public int Counter; public ulong S0, S1, S2, S3; }

public sealed class ActionSpec
{
    public string Kind;
    public int HandPos;
    public int? Target;
    public int? TargetAlly;
    public int Slot;
    public int[] Choose;
}

public sealed class Scenario
{
    public string Name = "scenario";
    public int Ascension;
    public string Encounter;
    public string Character = "IRONCLAD";
    public int? Hp, MaxHp, MaxEnergy, Gold, MaxPotionSlots, BaseOrbSlots;
    public List<CardSpec> Deck = new();
    public List<RelicSpec> Relics = new();
    public List<PotionSpec> Potions = new();
    public string Seed = "1";
    public Dictionary<string, RngSpec> Rng = new();
    public int TotalFloor = 1;
    public int Act = 0;
    public bool ApplyAscensionEffects = false;
    public bool UseCharacterStarter = false;
    public List<ActionSpec> Script = new();
    public JsonObject Raw;

    public static string StripCat(string id)
    {
        int i = id.IndexOf('.');
        return i >= 0 ? id[(i + 1)..] : id;
    }

    public static Scenario Load(string path)
    {
        var node = JsonNode.Parse(File.ReadAllText(path), documentOptions: new JsonDocumentOptions { CommentHandling = JsonCommentHandling.Skip, AllowTrailingCommas = true }).AsObject();
        return Parse(node);
    }

    public static Scenario Parse(JsonObject o)
    {
        var s = new Scenario { Raw = o };
        if (o["name"] != null) s.Name = (string)o["name"];
        if (o["ascension"] != null) s.Ascension = (int)o["ascension"];
        s.Encounter = (string)o["encounter"];
        if (o["character"] != null) s.Character = (string)o["character"];
        s.Hp = (int?)o["hp"]; s.MaxHp = (int?)o["max_hp"]; s.MaxEnergy = (int?)o["max_energy"];
        s.Gold = (int?)o["gold"]; s.MaxPotionSlots = (int?)o["max_potion_slots"]; s.BaseOrbSlots = (int?)o["base_orb_slots"];
        if (o["seed"] != null) s.Seed = o["seed"].ToString();
        if (o["total_floor"] != null) s.TotalFloor = (int)o["total_floor"];
        if (o["act"] != null) s.Act = (int)o["act"];
        if (o["apply_ascension_effects"] != null) s.ApplyAscensionEffects = (bool)o["apply_ascension_effects"];
        if (o["deck"] is JsonArray deck)
            foreach (var c in deck) s.Deck.Add(ParseCard(c));
        else s.UseCharacterStarter = true;
        if (o["relics"] is JsonArray rel)
            foreach (var r in rel)
            {
                if (r is JsonObject ro) s.Relics.Add(new RelicSpec { Id = StripCat((string)ro["id"]), Props = ro["props"] as JsonObject });
                else s.Relics.Add(new RelicSpec { Id = StripCat((string)r) });
            }
        else s.UseCharacterStarter = true;
        if (o["potions"] is JsonArray pots)
        {
            int slot = 0;
            foreach (var p in pots)
            {
                if (p is JsonObject po) s.Potions.Add(new PotionSpec { Id = StripCat((string)po["id"]), Slot = po["slot"] != null ? (int)po["slot"] : slot });
                else s.Potions.Add(new PotionSpec { Id = StripCat((string)p), Slot = slot });
                slot++;
            }
        }
        if (o["rng"] is JsonObject rng)
            foreach (var kv in rng)
            {
                var ro = kv.Value.AsObject();
                s.Rng[kv.Key] = new RngSpec
                {
                    Counter = ro["counter"] != null ? (int)ro["counter"] : 0,
                    S0 = U64(ro["s0"] ?? ro["state0"]), S1 = U64(ro["s1"] ?? ro["state1"]),
                    S2 = U64(ro["s2"] ?? ro["state2"]), S3 = U64(ro["s3"] ?? ro["state3"]),
                };
            }
        if (o["script"] is JsonArray script)
            foreach (var a in script) s.Script.Add(ParseAction(a.AsObject()));
        return s;
    }

    private static ulong U64(JsonNode n)
    {
        if (n == null) throw new OracleException("rng spec requires s0..s3");
        var je = n.GetValue<JsonElement>();
        return je.ValueKind == JsonValueKind.String ? ulong.Parse(je.GetString()) : je.GetUInt64();
    }

    private static CardSpec ParseCard(JsonNode c)
    {
        if (c is JsonValue)
        {
            string t = (string)c;
            int up = 0;
            if (t.EndsWith("+")) { up = 1; t = t[..^1]; }
            return new CardSpec { Id = StripCat(t), Upgrade = up };
        }
        var o = c.AsObject();
        var cs = new CardSpec { Id = StripCat((string)o["id"]) };
        if (o["upgrade"] != null) cs.Upgrade = (int)o["upgrade"];
        if (o["enchantment"] is JsonObject eo) { cs.Enchantment = (string)eo["id"]; if (eo["amount"] != null) cs.EnchantAmount = (int)eo["amount"]; }
        else if (o["enchantment"] != null) cs.Enchantment = (string)o["enchantment"];
        cs.Props = o["props"] as JsonObject;
        cs.FloorAdded = (int?)o["floor_added"];
        return cs;
    }

    public static ActionSpec ParseAction(JsonObject a)
    {
        if (a["play"] is JsonObject p)
            return new ActionSpec { Kind = "play", HandPos = (int)p["hand_pos"], Target = (int?)p["target"], TargetAlly = (int?)p["target_ally"] };
        if (a["end_turn"] != null) return new ActionSpec { Kind = "end_turn" };
        if (a["use_potion"] is JsonObject u)
            return new ActionSpec { Kind = "use_potion", Slot = (int)u["slot"], Target = (int?)u["target"], TargetAlly = (int?)u["target_ally"] };
        if (a["choose"] is JsonArray ch)
            return new ActionSpec { Kind = "choose", Choose = ch.Select(x => (int)x).ToArray() };
        throw new OracleException("unknown action: " + a.ToJsonString());
    }

    public static JsonObject ActionToJson(ActionSpec a)
    {
        switch (a.Kind)
        {
            case "play":
                return new JsonObject { ["play"] = new JsonObject { ["hand_pos"] = a.HandPos, ["target"] = a.Target, ["target_ally"] = a.TargetAlly } };
            case "end_turn": return new JsonObject { ["end_turn"] = true };
            case "use_potion":
                return new JsonObject { ["use_potion"] = new JsonObject { ["slot"] = a.Slot, ["target"] = a.Target, ["target_ally"] = a.TargetAlly } };
            case "choose":
                return new JsonObject { ["choose"] = new JsonArray(a.Choose.Select(i => (JsonNode)i).ToArray()) };
        }
        throw new OracleException("bad action kind");
    }
}
