using System.Text.Json.Nodes;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.MonsterMoves.Intents;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Random;
using MegaCrit.Sts2.Core.Saves.Runs;

namespace OracleCombat;

public static class Dump
{
    public static JsonObject Card(CardModel c)
    {
        var o = new JsonObject
        {
            ["id"] = c.Id.Entry,
            ["upgrade"] = c.CurrentUpgradeLevel,
        };
        try { var cost = c.EnergyCost; o["cost"] = cost.CostsX ? -1 : cost.GetResolved(); } catch { }
        try { if (c.HasStarCostX) o["star_cost"] = -1; else { var sc = c.GetStarCostWithModifiers(); if (sc >= 0) o["star_cost"] = sc; } } catch { }
        try { if (c.Id.Entry == "SOVEREIGN_BLADE" || c.Id.Entry == "KINGLY_PUNCH") o["base_damage"] = (int)c.DynamicVars.Damage.BaseValue; } catch { }
        try { o["keywords"] = new JsonArray(c.Keywords.Select(k => (JsonNode)k.ToString()).OrderBy(x => x.ToString(), StringComparer.Ordinal).ToArray()); } catch { }
        if (c.Enchantment != null)
            o["enchantment"] = new JsonObject { ["id"] = c.Enchantment.Id.Entry, ["amount"] = c.Enchantment.Amount };
        try { var p = SavedProperties.From(c); var pj = Props(p); if (pj != null) o["props"] = pj; } catch { }
        return o;
    }

    // Non-hand piles: id, upgrade, keywords (local + global sources: a stray Ethereal on a drawn card must be seen when it happens, not
    // many steps later when the card is drawn) and the enchantment.
    public static JsonObject CardBrief(CardModel c)
    {
        var o = new JsonObject { ["id"] = c.Id.Entry, ["upgrade"] = c.CurrentUpgradeLevel };
        try { o["keywords"] = new JsonArray(c.Keywords.Select(k => (JsonNode)k.ToString()).OrderBy(x => x.ToString(), StringComparer.Ordinal).ToArray()); } catch { }
        if (c.Enchantment != null)
            o["enchantment"] = new JsonObject { ["id"] = c.Enchantment.Id.Entry, ["amount"] = c.Enchantment.Amount };
        return o;
    }

    public static JsonObject Props(SavedProperties p)
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

    /// <summary>Debug aid: the combat-history entries added since the previous record (`log`), e.g. "play CATASTROPHE", "play* HIBERNATE" (auto-play), "draw X".
    /// Not compared by sts2diff; use tools/fuzz_show.py to see which nested plays / draws the real game did inside one action.</summary>
    public static JsonArray Log(ref int idx)
    {
        var a = new JsonArray();
        try
        {
            var entries = MegaCrit.Sts2.Core.Combat.CombatManager.Instance.History.Entries.ToList();
            if (idx > entries.Count) idx = 0;
            for (int i = idx; i < entries.Count; i++)
            {
                string t;
                switch (entries[i])
                {
                    case MegaCrit.Sts2.Core.Combat.History.Entries.CardPlayStartedEntry e: t = (e.CardPlay.IsAutoPlay ? "play* " : "play ") + e.CardPlay.Card.Id.Entry + " #" + (e.CardPlay.PlayIndex + 1) + "/" + e.CardPlay.PlayCount; break;
                    case MegaCrit.Sts2.Core.Combat.History.Entries.CardDrawnEntry e: t = "draw " + e.Card.Id.Entry; break;
                    case MegaCrit.Sts2.Core.Combat.History.Entries.CardDiscardedEntry e: t = "discard " + e.Card.Id.Entry; break;
                    case MegaCrit.Sts2.Core.Combat.History.Entries.CardExhaustedEntry e: t = "exhaust " + e.Card.Id.Entry; break;
                    case MegaCrit.Sts2.Core.Combat.History.Entries.CardGeneratedEntry e: t = "gen " + e.Card.Id.Entry; break;
                    case MegaCrit.Sts2.Core.Combat.History.Entries.PowerReceivedEntry e: t = "power " + e.Power.Id.Entry + " " + e.Amount; break;
                    case MegaCrit.Sts2.Core.Combat.History.Entries.EnergySpentEntry e: t = "energy " + e.Amount; break;
                    case MegaCrit.Sts2.Core.Combat.History.Entries.BlockGainedEntry e: t = "block " + e.Amount; break;
                    default: t = entries[i].GetType().Name.Replace("Entry", ""); break;
                }
                a.Add((JsonNode)t);
            }
            idx = entries.Count;
        }
        catch { }
        return a;
    }

    public static JsonArray Pile(CardPile pile, bool brief = false)
    {
        var a = new JsonArray();
        foreach (var c in pile.Cards) a.Add(brief ? CardBrief(c) : Card(c));
        return a;
    }

    public static JsonArray Powers(Creature cr)
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

    public static JsonObject Creature(Creature cr)
    {
        var o = new JsonObject
        {
            ["id"] = cr.IsPlayer ? "PLAYER" : cr.Monster?.Id.Entry,
            ["combat_id"] = cr.CombatId.HasValue ? cr.CombatId.Value : null,
            ["hp"] = cr.CurrentHp,
            ["max_hp"] = cr.MaxHp,
            ["block"] = cr.Block,
            ["alive"] = cr.IsAlive,
            ["powers"] = Powers(cr),
        };
        return o;
    }

    public static JsonObject Enemy(CombatState st, int idx, Creature e)
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

    public static JsonObject RngState(Rng r)
    {
        var s = r.ToSerializable();
        return new JsonObject { ["counter"] = s.counter, ["s0"] = s.state0, ["s1"] = s.state1, ["s2"] = s.state2, ["s3"] = s.state3 };
    }

    public static JsonObject Rngs(RunState run)
    {
        var o = new JsonObject();
        foreach (var (name, type) in Setup.Streams) o[name] = RngState(run.Rng.GetRng(type));
        return o;
    }

    public static JsonObject State(Player player, RunState run, CombatState st)
    {
        var pcs = player.PlayerCombatState;
        var cm = CombatManager.Instance;
        var o = new JsonObject();
        o["combat_in_progress"] = cm.IsInProgress;
        o["round"] = st?.RoundNumber;
        o["side"] = st?.CurrentSide.ToString().ToLowerInvariant();
        o["turn"] = pcs?.TurnNumber;
        o["phase"] = pcs?.Phase.ToString();
        o["energy"] = pcs?.Energy;
        o["max_energy"] = pcs?.MaxEnergy;
        o["stars"] = pcs?.Stars;
        o["gold"] = player.Gold;
        var pl = Creature(player.Creature);
        pl.Remove("id"); pl.Remove("combat_id");
        o["player"] = pl;
        var pets = new JsonArray();
        if (pcs != null) foreach (var p in pcs.Pets) pets.Add(Creature(p));
        o["pets"] = pets;
        var enemies = new JsonArray();
        if (st != null) { int i = 0; foreach (var e in st.Enemies) enemies.Add(Enemy(st, i++, e)); }
        o["enemies"] = enemies;
        o["hand"] = pcs != null ? Pile(pcs.Hand) : new JsonArray();
        o["draw"] = pcs != null ? Pile(pcs.DrawPile, brief: true) : new JsonArray();
        o["discard"] = pcs != null ? Pile(pcs.DiscardPile, brief: true) : new JsonArray();
        o["exhaust"] = pcs != null ? Pile(pcs.ExhaustPile, brief: true) : new JsonArray();
        o["play_pile"] = pcs != null ? Pile(pcs.PlayPile, brief: true) : new JsonArray();
        var orbs = new JsonArray();
        if (pcs != null) foreach (var ob in pcs.OrbQueue.Orbs) orbs.Add(new JsonObject { ["id"] = ob.Id.Entry, ["passive"] = (double)ob.PassiveVal, ["evoke"] = (double)ob.EvokeVal });
        o["orbs"] = orbs;
        o["orb_capacity"] = pcs?.OrbQueue.Capacity;
        var relics = new JsonArray();
        foreach (var r in player.Relics)
        {
            var ro = new JsonObject { ["id"] = r.Id.Entry };
            try { var pj = Props(SavedProperties.From(r)); if (pj != null) ro["props"] = pj; } catch { }
            try { if (r.ShowCounter) ro["counter"] = r.DisplayAmount; } catch { }
            relics.Add(ro);
        }
        o["relics"] = relics;
        var potions = new JsonArray();
        for (int i = 0; i < player.PotionSlots.Count; i++)
            if (player.PotionSlots[i] != null) potions.Add(new JsonObject { ["slot"] = i, ["id"] = player.PotionSlots[i].Id.Entry });
        o["potions"] = potions;
        o["potion_slots"] = player.PotionSlots.Count;
        o["rng"] = Rngs(run);
        return o;
    }
}
