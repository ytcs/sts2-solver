using System.Text.Json.Nodes;
using HarmonyLib;
using MegaCrit.Sts2.Core.Saves.Runs;

namespace AgentBridge;

public static class Snap
{
    public static string? Scenario;
    public static int FightId = (int)((DateTimeOffset.UtcNow.ToUnixTimeSeconds() - 1767225600) / 60) * 1000;
    public static readonly List<string> Log = new();
    public static readonly Dictionary<int, string> Obs = new();

    [HarmonyPatch(typeof(CombatManager), nameof(CombatManager.SetUpCombat))]
    public static class SetUpPatch
    {
        static void Postfix(CombatState state)
        {
            try
            {
                Log.Clear();
                Obs.Clear();
                FightId++;
                Scenario = BuildScenario(state);
            }
            catch (Exception e) { Scenario = null; Main.Log.Info("snap scenario failed: " + e, 0); }
        }
    }

    private static Player? Me()
    {
        var rs = RunManager.Instance.DebugOnlyGetState();
        return rs != null ? LocalContext.GetMe(rs) : null;
    }

    private static JsonObject? Props(SavedProperties? p)
    {
        if (p == null) return null;
        var o = new JsonObject();
        if (p.ints != null) foreach (var x in p.ints) o[x.name] = x.value;
        if (p.bools != null) foreach (var x in p.bools) o[x.name] = x.value;
        if (p.strings != null) foreach (var x in p.strings) o[x.name] = x.value;
        if (p.intArrays != null) foreach (var x in p.intArrays) o[x.name] = new JsonArray(x.value.Select(i => (JsonNode)i).ToArray());
        if (p.modelIds != null) foreach (var x in p.modelIds) o[x.name] = x.value.ToString();
        return o.Count == 0 ? null : o;
    }

    private static string BuildScenario(CombatState state)
    {
        var rs = RunManager.Instance.DebugOnlyGetState()!;
        var me = LocalContext.GetMe(rs)!;
        var sp = me.ToSerializable();
        var o = new JsonObject
        {
            ["name"] = "live",
            ["ascension"] = rs.AscensionLevel,
            ["encounter"] = state.Encounter?.Id.Entry,
            ["character"] = me.Character.Id.Entry,
            ["hp"] = sp.CurrentHp,
            ["max_hp"] = sp.MaxHp,
            ["max_energy"] = sp.MaxEnergy,
            ["gold"] = sp.Gold,
            ["max_potion_slots"] = sp.MaxPotionSlotCount,
            ["base_orb_slots"] = sp.BaseOrbSlotCount,
            // the real run seed is never exported: it determines the hidden shuffles
            ["seed"] = "placeholder",
            ["total_floor"] = rs.TotalFloor,
            ["act"] = rs.CurrentActIndex,
        };
        var deck = new JsonArray();
        foreach (var c in sp.Deck)
        {
            var co = new JsonObject { ["id"] = c.Id.Entry, ["upgrade"] = c.CurrentUpgradeLevel };
            if (c.Enchantment != null) co["enchantment"] = new JsonObject { ["id"] = c.Enchantment.Id.Entry, ["amount"] = c.Enchantment.Amount };
            var pj = Props(c.Props);
            if (pj != null) co["props"] = pj;
            deck.Add(co);
        }
        o["deck"] = deck;
        o["relics"] = Relics(me);
        var potions = new JsonArray();
        foreach (var p in sp.Potions) potions.Add(new JsonObject { ["id"] = p.Id.Entry, ["slot"] = p.SlotIndex });
        o["potions"] = potions;
        return o.ToJsonString();
    }

    private static JsonArray Relics(Player me)
    {
        var relics = new JsonArray();
        foreach (var r in me.Relics)
        {
            var ro = new JsonObject { ["id"] = r.Id.Entry };
            try { var pj = Props(SavedProperties.From(r)); if (pj != null) ro["props"] = pj; } catch { }
            try { if (r.ShowCounter) ro["counter"] = r.DisplayAmount; } catch { }
            relics.Add(ro);
        }
        return relics;
    }

    public static int HandPos(CardModel card) => Me()?.PlayerCombatState?.Hand.Cards.ToList().IndexOf(card) ?? -1;

    public static (int e, int a) TargetOf(Creature? t)
    {
        var me = Me(); var cs = CombatManager.Instance.DebugOnlyGetState();
        if (t == null || me == null || cs == null) return (-1, -1);
        int e = cs.Enemies.ToList().IndexOf(t);
        if (e >= 0) return (e, -1);
        var allies = new List<Creature> { me.Creature };
        if (me.PlayerCombatState != null) allies.AddRange(me.PlayerCombatState.Pets);
        return (-1, allies.IndexOf(t));
    }

    private static void SetTarget(JsonObject p, (int e, int a) tg)
    {
        if (tg.e >= 0) p["target"] = tg.e;
        else if (tg.a >= 0) p["target_ally"] = tg.a;
    }

    public static void LogPlay(int pos, (int e, int a) target)
    {
        var p = new JsonObject { ["hand_pos"] = pos };
        SetTarget(p, target);
        Log.Add(new JsonObject { ["play"] = p }.ToJsonString());
    }

    public static void LogPotion(int slot, (int e, int a) target)
    {
        var p = new JsonObject { ["slot"] = slot };
        SetTarget(p, target);
        Log.Add(new JsonObject { ["use_potion"] = p }.ToJsonString());
    }

    public static void LogEndTurn() => Log.Add("{\"end_turn\":true}");

    public static void LogChoose(IReadOnlyList<int> idx)
    {
        if (!CombatManager.Instance.IsInProgress) return;
        Log.Add(new JsonObject { ["choose"] = new JsonArray(idx.Select(i => (JsonNode)i).ToArray()) }.ToJsonString());
    }

    private static JsonObject Card(CardModel c, bool brief)
    {
        var o = new JsonObject { ["id"] = c.Id.Entry, ["upgrade"] = c.CurrentUpgradeLevel };
        if (!brief)
        {
            try { var cost = c.EnergyCost; o["cost"] = cost.CostsX ? -1 : cost.GetResolved(); } catch { }
            try { if (c.HasStarCostX) o["star_cost"] = -1; else { var sc = c.GetStarCostWithModifiers(); if (sc >= 0) o["star_cost"] = sc; } } catch { }
            try { if (c.Id.Entry == "SOVEREIGN_BLADE" || c.Id.Entry == "KINGLY_PUNCH") o["base_damage"] = (int)c.DynamicVars.Damage.BaseValue; } catch { }
        }
        try { o["keywords"] = new JsonArray(c.Keywords.Select(k => (JsonNode)k.ToString()).OrderBy(x => x.ToString(), StringComparer.Ordinal).ToArray()); } catch { }
        if (c.Enchantment != null) o["enchantment"] = new JsonObject { ["id"] = c.Enchantment.Id.Entry, ["amount"] = c.Enchantment.Amount };
        if (!brief) { try { var pj = Props(SavedProperties.From(c)); if (pj != null) o["props"] = pj; } catch { } }
        return o;
    }

    private static JsonArray Pile(IEnumerable<CardModel> cards, bool sorted)
    {
        var l = cards.ToList();
        if (sorted) l = l.OrderBy(c => c.Id.Entry, StringComparer.Ordinal).ThenBy(c => c.CurrentUpgradeLevel).ToList();
        var a = new JsonArray();
        foreach (var c in l) a.Add(Card(c, brief: true));
        return a;
    }

    private static JsonArray Powers(Creature cr)
    {
        var a = new JsonArray();
        foreach (var p in cr.Powers)
        {
            var o = new JsonObject { ["id"] = p.Id.Entry, ["amount"] = p.Amount };
            if (p.AmountOnTurnStart != p.Amount) o["amount_on_turn_start"] = p.AmountOnTurnStart;
            a.Add(o);
        }
        return a;
    }

    private static JsonObject Creature(Creature cr) => new()
    {
        ["id"] = cr.IsPlayer ? "PLAYER" : cr.Monster?.Id.Entry,
        ["combat_id"] = cr.CombatId.HasValue ? cr.CombatId.Value : null,
        ["hp"] = cr.CurrentHp,
        ["max_hp"] = cr.MaxHp,
        ["block"] = cr.Block,
        ["alive"] = cr.IsAlive,
        ["powers"] = Powers(cr),
    };

    private static JsonObject Enemy(CombatState st, int idx, Creature e)
    {
        var o = Creature(e);
        o["index"] = idx;
        if (e.SlotName != null) o["slot"] = e.SlotName;
        var m = e.Monster;
        if (m != null)
        {
            var mv = m.NextMove;
            o["next_move"] = mv?.Id;
            var intents = new JsonArray();
            if (mv != null)
                foreach (var it in mv.Intents)
                {
                    var io = new JsonObject { ["type"] = it.IntentType.ToString() };
                    if (it is AttackIntent ai)
                    {
                        try
                        {
                            var targets = st.PlayerCreatures;
                            io["damage"] = ai.GetSingleDamage(targets, e);
                            io["hits"] = Math.Max(1, ai.Repeats);
                            io["total_damage"] = ai.GetTotalDamage(targets, e);
                        }
                        catch (Exception ex) { io["damage_error"] = ex.GetType().Name; }
                    }
                    intents.Add(io);
                }
            o["intents"] = intents;
        }
        return o;
    }

    public static string? State()
    {
        var me = Me();
        var cm = CombatManager.Instance;
        var st = cm.DebugOnlyGetState();
        var pcs = me?.PlayerCombatState;
        if (me == null || st == null || pcs == null) return null;
        var o = new JsonObject
        {
            ["combat_in_progress"] = cm.IsInProgress,
            ["round"] = st.RoundNumber,
            ["side"] = st.CurrentSide.ToString().ToLowerInvariant(),
            ["turn"] = pcs.TurnNumber,
            ["phase"] = pcs.Phase.ToString(),
            ["energy"] = pcs.Energy,
            ["max_energy"] = pcs.MaxEnergy,
            ["stars"] = pcs.Stars,
            ["gold"] = me.Gold,
        };
        var pl = Creature(me.Creature);
        pl.Remove("id"); pl.Remove("combat_id");
        o["player"] = pl;
        var pets = new JsonArray();
        foreach (var p in pcs.Pets) pets.Add(Creature(p));
        o["pets"] = pets;
        var enemies = new JsonArray();
        int i = 0;
        foreach (var e in st.Enemies) enemies.Add(Enemy(st, i++, e));
        o["enemies"] = enemies;
        var hand = new JsonArray();
        foreach (var c in pcs.Hand.Cards) hand.Add(Card(c, brief: false));
        o["hand"] = hand;
        o["draw"] = Pile(pcs.DrawPile.Cards, sorted: true);
        o["discard"] = Pile(pcs.DiscardPile.Cards, sorted: false);
        o["exhaust"] = Pile(pcs.ExhaustPile.Cards, sorted: false);
        o["play_pile"] = Pile(pcs.PlayPile.Cards, sorted: false);
        var orbs = new JsonArray();
        foreach (var ob in pcs.OrbQueue.Orbs) orbs.Add(new JsonObject { ["id"] = ob.Id.Entry, ["passive"] = (double)ob.PassiveVal, ["evoke"] = (double)ob.EvokeVal });
        o["orbs"] = orbs;
        o["orb_capacity"] = pcs.OrbQueue.Capacity;
        o["relics"] = Relics(me);
        var potions = new JsonArray();
        for (int k = 0; k < me.PotionSlots.Count; k++)
            if (me.PotionSlots[k] != null) potions.Add(new JsonObject { ["slot"] = k, ["id"] = me.PotionSlots[k].Id.Entry });
        o["potions"] = potions;
        o["potion_slots"] = me.PotionSlots.Count;
        return o.ToJsonString();
    }

    public static void Observe()
    {
        if (!CombatManager.Instance.IsInProgress) return;
        var s = State();
        if (s != null) Obs[Log.Count] = s;
    }

    public static string? Fight()
    {
        var state = State();
        if (state == null || Scenario == null) return null;
        Obs[Log.Count] = state;
        var states = Enumerable.Range(0, Log.Count + 1).Select(i => Obs.TryGetValue(i, out var s) ? s : "null");
        return "{\"id\":" + FightId + ",\"scenario\":" + Scenario + ",\"log\":[" + string.Join(",", Log) + "],\"states\":[" + string.Join(",", states) + "],\"state\":" + state + "}";
    }

    public static string? DeckJson()
    {
        var rs = RunManager.Instance.DebugOnlyGetState();
        var me = rs != null ? LocalContext.GetMe(rs) : null;
        if (me == null) return null;
        var sp = me.ToSerializable();
        var o = new JsonObject
        {
            ["ascension"] = rs!.AscensionLevel,
            ["character"] = me.Character.Id.Entry,
            ["hp"] = sp.CurrentHp, ["max_hp"] = sp.MaxHp, ["max_energy"] = sp.MaxEnergy, ["gold"] = sp.Gold,
            ["max_potion_slots"] = sp.MaxPotionSlotCount, ["base_orb_slots"] = sp.BaseOrbSlotCount,
            ["seed"] = "placeholder", ["total_floor"] = rs.TotalFloor, ["act"] = rs.CurrentActIndex,
        };
        var deck = new JsonArray();
        foreach (var c in sp.Deck)
        {
            var co = new JsonObject { ["id"] = c.Id.Entry, ["upgrade"] = c.CurrentUpgradeLevel };
            if (c.Enchantment != null) co["enchantment"] = new JsonObject { ["id"] = c.Enchantment.Id.Entry, ["amount"] = c.Enchantment.Amount };
            var pj = Props(c.Props);
            if (pj != null) co["props"] = pj;
            deck.Add(co);
        }
        o["deck"] = deck;
        var relics = new JsonArray();
        foreach (var r in sp.Relics)
        {
            var ro = new JsonObject { ["id"] = r.Id.Entry };
            var pj = Props(r.Props);
            if (pj != null) ro["props"] = pj;
            relics.Add(ro);
        }
        o["relics"] = relics;
        var potions = new JsonArray();
        foreach (var p in sp.Potions) potions.Add(new JsonObject { ["id"] = p.Id.Entry, ["slot"] = p.SlotIndex });
        o["potions"] = potions;
        return o.ToJsonString();
    }
}
