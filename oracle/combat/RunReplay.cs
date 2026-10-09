using System.Text;
using System.Text.Json.Nodes;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Random;
using MegaCrit.Sts2.Core.Rewards;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Unlocks;

namespace OracleCombat;

// Whole-run replay from a seed via the game's own run-level code (NGame.StartNewSingleplayerRun path, TestMode UI-less).
public sealed class RunReplay
{
    private readonly Pump _pump;
    private readonly TextWriter _out;
    private readonly string _compactPath;
    private readonly JsonObject _head;
    private readonly List<JsonObject> _steps;
    private Player _player;
    private RunState _run;
    private readonly ChoiceSelector _sel = new();
    private readonly Dictionary<string, string> _alias = new();

    public RunReplay(string compactPath, TextWriter @out, Pump pump)
    {
        _pump = pump; _out = @out; _compactPath = compactPath;
        var lines = File.ReadLines(compactPath).Where(l => l.Trim().Length > 0).Select(l => JsonNode.Parse(l).AsObject()).ToList();
        _head = lines[0]; _steps = lines.Skip(1).ToList();
    }

    private void Emit(string ev, JsonObject o)
    {
        o["event"] = ev;
        _out.WriteLine(o.ToJsonString());
        _out.Flush();
    }

    private void Wait(Task t, string what)
    {
        _pump.RunUntil(() => t.IsCompleted || (Fatal.IsSet && !Fatal.Lenient), () => what + ": " + Describe());
        if (Fatal.IsSet && !Fatal.Lenient) throw new OracleException(what + ": " + Fatal.Message);
        if (t.IsFaulted) throw t.Exception.GetBaseException();
        _pump.Drain();
    }

    private string Describe() =>
        $"room={_run?.CurrentRoom?.GetType().Name} execRunning={RunManager.Instance.ActionExecutor?.IsRunning} execPaused={RunManager.Instance.ActionExecutor?.IsPaused} cur={RunManager.Instance.ActionExecutor?.CurrentlyRunningAction}";

    public void Run()
    {
        string seed = (string)_head["seed"];
        string charName = ((string)_head["character"]).ToUpperInvariant();
        int asc = (int)_head["ascension"];
        Patches.ApplyAscensionEffects = true;
        Patches.RealRunRng = true;
        var unlock = UnlockState.all;
        var rng = new Rng(StringHelper.GetDeterministicHashCode(seed), "act_selection");
        var acts = ActModel.GetRandomList(rng, unlock, isMultiplayer: false).ToList();
        var character = ModelDb.GetById<CharacterModel>(new ModelId("CHARACTER", charName));
        _player = Player.CreateForNewRun(character, unlock, 1UL);
        _run = RunState.CreateForNewRun(new List<Player> { _player }, acts.Select(a => a.ToMutable()).ToList(), Array.Empty<ModifierModel>(), GameMode.Standard, asc, seed);
        RunManager.Instance.SetUpNewSingleplayer(_run, shouldSave: false);
        Wait(Start(), "start run");
        Emit("start", new JsonObject
        {
            ["seed"] = seed, ["character"] = charName, ["ascension"] = asc,
            ["acts"] = new JsonArray(_run.Acts.Select(a => (JsonNode)a.Id.Entry).ToArray()),
            ["started_with_neow"] = _run.ExtraFields.StartedWithNeow,
            ["boss"] = _run.Act.BossEncounter?.Id.Entry,
            ["map"] = FullMap(),
            ["room"] = RoomInfo(),
            ["state"] = Dump.State(_player, _run, null),
        });
        foreach (var kv in _head["aliases"]?.AsObject() ?? new JsonObject()) _alias[kv.Key] = (string)kv.Value;
        using var selScope = CardSelectCmd.PushSelector(_sel);
        int i = 0;
        while (i < _steps.Count)
        {
            var st = _steps[i];
            string screen = (string)st["screen"];
            if (st["pick"] is JsonObject pu && pu["potion"] != null) { DrinkPotion(st, (int)pu["potion"]); i++; continue; }
            if (screen == "MAP" && st["pick"] is JsonObject pd && pd["discard_potion"] != null)
            {
                Wait(PotionCmd.Discard(_player.PotionSlots[(int)pd["discard_potion"]]), "discard potion");
                Emit("discard_potion", new JsonObject { ["floor"] = (int)st["floor"], ["slot"] = (int)pd["discard_potion"], ["state"] = State() });
                i++; continue;
            }
            switch (screen)
            {
                case "EVENT": i = QueueSelects(i); i = QueueEventCardRewards(i); DoEvent(st); break;
                case "RESTSITE": i = QueueSelects(i); DoRest(st); break;
                case "TREASURE": DoTreasure(st); i++; break;
                case "SHOP": i = QueueSelects(i); DoShop(st); break;
                case "MAP": DoMap(st); i++; break;
                case "COMBAT": DoCombat(st); i++; break;
                case "REWARDS": case "CARD_REWARD": i = DoRewards(i); break;
                default:
                    Emit("blocked", new JsonObject { ["step"] = i, ["screen"] = screen, ["record"] = st.DeepClone(), ["room"] = RoomInfo() });
                    return;
            }
        }
        Emit("end", new JsonObject { ["steps_done"] = i, ["room"] = RoomInfo(), ["state"] = Dump.State(_player, _run, null) });
    }

    public static string NameToId(string name)
    {
        var sb = new StringBuilder();
        foreach (char ch in name.Trim().TrimEnd('+'))
            if (char.IsLetterOrDigit(ch)) sb.Append(char.ToUpperInvariant(ch));
            else if (ch == ' ' || ch == '-' || ch == '_') sb.Append('_');
        return sb.ToString();
    }

    private string CardId(string aliasOrId)
    {
        string s = aliasOrId.Trim().TrimEnd('+');
        return _alias.TryGetValue(s, out var id) ? id : NameToId(s);
    }

    private string CardIdUp(string tok) => CardId(tok) + (tok.Trim().EndsWith("+") ? "+" : "");

    private JsonObject State() => Dump.State(_player, _run, CombatManager.Instance.DebugOnlyGetState());

    private void DoEvent(JsonObject st)
    {
        if (((int?)st["act"] ?? 0) > _run.CurrentActIndex)
        {
            // the record has no MAP step into this act's ancient
            Wait(RunManager.Instance.EnterNextAct(), "enter next act");
            var newMap = FullMap();
            Wait(RunManager.Instance.EnterMapCoord(_run.Map.StartingMapPoint.coord), "enter ancient");
            Emit("map", new JsonObject { ["floor"] = (int)st["floor"], ["pick"] = "ancient", ["total_floor"] = _run.TotalFloor, ["room"] = RoomInfo(), ["map"] = newMap, ["state"] = State() });
        }
        var ev = RunManager.Instance.EventSynchronizer.GetLocalEvent();
        var before = RoomInfo();
        string want = NameToId((string)st["pick"]);
        var opts = ev.CurrentOptions;
        int idx = -1;
        for (int k = 0; k < opts.Count; k++)
            if (opts[k].TextKey.Split('.').Last().ToUpperInvariant() == want) { idx = k; break; }
        if (idx < 0 && EventLabels().TryGetValue((ev.Id.Entry, ((string)st["pick"]).Trim()), out var key))
            for (int k = 0; k < opts.Count; k++)
                if (opts[k].TextKey.Split('.').Last() == key) { idx = k; want = key; break; }
        if (idx < 0 && ev.Id.Entry == "THE_FUTURE_OF_POTIONS" && want.StartsWith("LOSE_"))
        {
            // one POTION option per potion, in slot order; the record names the potion it gives up ("Lose Block Potion")
            var pots = _player.PotionSlots.Where(p => p != null).Take(opts.Count).ToList();
            int k = pots.FindIndex(p => p.Id.Entry == want[5..]);
            if (k >= 0 && opts[k].TextKey.EndsWith(".POTION")) idx = k;
        }
        if (idx < 0) throw new OracleException($"event pick {want} not among options: " + string.Join(", ", opts.Select(o => o.TextKey)));
        var picks = _eventPicks;
        if (picks != null)
            RewardsSet.testSelector = async set =>
            {
                set.ThrowInTestIfRewardsNotTaken = false;
                var taken = new HashSet<Reward>();
                foreach (var pk in picks)
                {
                    if (pk == "proceed") continue;
                    var r = set.Rewards.FirstOrDefault(x => !taken.Contains(x) && Matches(x, pk))
                        ?? throw new OracleException($"event reward pick {pk} not offered");
                    taken.Add(r);
                    await RunManager.Instance.RewardsSetSynchronizer.SelectLocalReward(r);
                }
                if (!RunManager.Instance.RewardsSetSynchronizer.IsRewardsSetCompleted(set)) RunManager.Instance.RewardsSetSynchronizer.SkipLocalRewardsSet();
            };
        try
        {
            RunManager.Instance.EventSynchronizer.ChooseLocalOption(idx);
            _pump.Drain();
            Wait(RunManager.Instance.EventSynchronizer.AwaitPendingOptionTasks(), "event option");
        }
        finally { if (picks != null) RewardsSet.testSelector = null; _eventPicks = null; }
        CheckSelectsUsed("event " + want);
        Emit("event", new JsonObject { ["floor"] = (int)st["floor"], ["pick"] = want, ["before"] = before, ["after"] = RoomInfo(), ["choices"] = _sel.Prompts.DeepClone(), ["state"] = State() });
        _sel.Prompts.Clear();
    }

    // SELECT steps right after step i answer the card prompts it opens; returns the next step index.
    private int QueueSelects(int i)
    {
        _sel.ScriptedIds.Clear();
        int j = i + 1;
        for (; j < _steps.Count && (string)_steps[j]["screen"] == "SELECT"; j++)
            _sel.ScriptedIds.Enqueue(_steps[j]["pick"].AsArray().Select(x => CardIdUp((string)x)).ToArray());
        return j;
    }

    // An event option that offers rewards (The Future of Potions: a card reward) opens them inside the option: the REWARDS /
    // CARD_REWARD steps right after the event script them; returns the next step index.
    private List<string> _eventPicks;
    private int QueueEventCardRewards(int j)
    {
        _eventPicks = null;
        for (; j < _steps.Count && (string)_steps[j]["screen"] is "REWARDS" or "CARD_REWARD"; j++)
        {
            _eventPicks ??= new List<string>();
            var pk = _steps[j]["pick"];
            if ((string)_steps[j]["screen"] == "CARD_REWARD") _sel.ScriptedCardReward.Enqueue(pk is JsonValue v ? CardId((string)v) : null);
            else if (pk is JsonArray arr) _eventPicks.AddRange(arr.Select(x => (string)x));
            else if (pk is JsonValue) _eventPicks.Add((string)pk);
        }
        return j;
    }

    private void CheckSelectsUsed(string what)
    {
        if (_sel.ScriptedIds.Count > 0) throw new OracleException($"{what}: {_sel.ScriptedIds.Count} unused SELECT picks");
    }

    private void DoRest(JsonObject st)
    {
        var opts = RunManager.Instance.RestSiteSynchronizer.GetLocalOptions();
        string want = (string)st["pick"] switch { "Rest" => "HEAL", var x => NameToId(x) };
        int idx = Enumerable.Range(0, opts.Count).FirstOrDefault(k => opts[k].OptionId == want, -1);
        var offered = new JsonArray(opts.Select(o => (JsonNode)o.OptionId).ToArray());
        if (idx < 0) throw new OracleException($"rest pick {want} not among " + offered.ToJsonString());
        Wait(RunManager.Instance.RestSiteSynchronizer.ChooseLocalOption(idx), "rest option");
        CheckSelectsUsed("rest " + want);
        Emit("rest", new JsonObject { ["floor"] = (int)st["floor"], ["offered"] = offered, ["pick"] = want, ["choices"] = _sel.Prompts.DeepClone(), ["state"] = State() });
        _sel.Prompts.Clear();
    }

    private void DrinkPotion(JsonObject st, int slot)
    {
        // an AnyTime potion used outside combat (the bridge lists `potion <name>: ...` on room screens)
        var p = _player.PotionSlots[slot] ?? throw new OracleException($"no potion in slot {slot}");
        p.EnqueueManualUse(null);
        _pump.RunUntil(() => _player.PotionSlots[slot] != p, () => "potion use: " + Describe());
        _pump.RunUntil(() => _player.Creature.CurrentHp > 0, () => "potion settle: " + Describe());
        Emit("potion", new JsonObject { ["floor"] = (int)st["floor"], ["slot"] = slot, ["potion"] = p.Id.Entry, ["state"] = State() });
    }

    private void DoTreasure(JsonObject st)
    {
        var room = _run.CurrentRoom as TreasureRoom ?? throw new OracleException("TREASURE outside a treasure room: " + _run.CurrentRoom?.GetType().Name);
        int goldBefore = _player.Gold;
        Wait(room.DoNormalRewards(), "treasure gold");
        Wait(room.DoExtraRewardsIfNeeded(), "treasure extra rewards");
        var sync = RunManager.Instance.TreasureRoomRelicSynchronizer;
        var relics = new JsonArray((sync.CurrentRelics ?? Array.Empty<RelicModel>()).Select(r => (JsonNode)r.Id.Entry).ToArray());
        string pick = (string)st["pick"];
        // NTreasureRoomRelicCollection.AnimateRelicAwards does the obtain in the real game.
        var obtained = new List<Task>();
        void Award(List<MegaCrit.Sts2.Core.Entities.TreasureRelicPicking.RelicPickingResult> results)
        {
            foreach (var r in results.OrderBy(r => r.type))
                if (r.type != MegaCrit.Sts2.Core.Entities.TreasureRelicPicking.RelicPickingResultType.Skipped)
                    obtained.Add(RelicCmd.Obtain(r.relic.ToMutable(), r.player));
        }
        sync.RelicsAwarded += Award;
        try
        {
            if (pick == "take") sync.PickRelicLocally(0); else sync.SkipRelicLocally();
            _pump.RunUntil(() => pick != "take" || obtained.Count > 0 && obtained.All(t => t.IsCompleted), () => "treasure relic: " + Describe());
            foreach (var t in obtained) if (t.IsFaulted) throw t.Exception.GetBaseException();
        }
        finally { sync.RelicsAwarded -= Award; }
        Emit("treasure", new JsonObject { ["floor"] = (int)st["floor"], ["gold"] = _player.Gold - goldBefore, ["relics"] = relics, ["pick"] = pick, ["state"] = State() });
    }

    // Same rule as agent/reenact.py map_paths: a map point is allowed if its type fits the room the record visited at that act floor.
    private static readonly Dictionary<string, string[]> RoomAllows = new()
    {
        ["Monster"] = new[] { "Monster", "Unknown" }, ["Elite"] = new[] { "Elite" }, ["Boss"] = new[] { "Boss" }, ["RestSite"] = new[] { "RestSite" },
        ["Shop"] = new[] { "Shop", "Unknown" }, ["Treasure"] = new[] { "Treasure", "Unknown" }, ["Event"] = new[] { "Unknown" }, ["Ancient"] = new[] { "Ancient" },
    };

    private Dictionary<int, string> RoomsByRow(int act)
    {
        var steps = _steps.Where(s => s["floor"] != null && ((int?)s["act"] ?? 0) == act).ToList();
        int first = steps.Min(s => (int)s["floor"]);
        var rooms = new Dictionary<int, string>();
        foreach (var s in steps)
        {
            string enc = (string)s["fight"]?["encounter"] ?? "";
            string room = (string)s["room"] ?? (enc.EndsWith("_BOSS") ? "Boss" : enc.EndsWith("_ELITE") ? "Elite" : enc.Length > 0 ? "Monster" : null)
                ?? (string)s["screen"] switch { "EVENT" => "Event", "RESTSITE" => "RestSite", "SHOP" => "Shop", "TREASURE" => "Treasure", _ => null };
            if (room != null) rooms[(int)s["floor"] - first] = room;
        }
        return rooms;
    }

    private static bool Consistent(MapPoint p, Dictionary<int, string> rooms, Dictionary<MapPoint, bool> memo)
    {
        if (memo.TryGetValue(p, out var v)) return v;
        bool ok = !rooms.TryGetValue(p.coord.row, out var want) || RoomAllows.GetValueOrDefault(want, new[] { want }).Contains(p.PointType.ToString());
        v = ok && (p.Children.Count == 0 || p.Children.Any(c => Consistent(c, rooms, memo)));
        memo[p] = v;
        return v;
    }

    private void DoShop(JsonObject st)
    {
        var room = _run.CurrentRoom as MerchantRoom ?? throw new OracleException("SHOP outside a merchant room: " + _run.CurrentRoom?.GetType().Name);
        var inv = room.GetLocalInventory();
        var stock = new JsonArray(inv.AllEntries.Where(e => e.IsStocked).Select(e => (JsonNode)(e switch
        {
            MegaCrit.Sts2.Core.Entities.Merchant.MerchantCardEntry c => new JsonObject { ["card"] = c.CreationResult.Card.Id.Entry, ["cost"] = c.Cost, ["sale"] = c.IsOnSale },
            MegaCrit.Sts2.Core.Entities.Merchant.MerchantRelicEntry r => new JsonObject { ["relic"] = r.Model.Id.Entry, ["cost"] = r.Cost },
            MegaCrit.Sts2.Core.Entities.Merchant.MerchantPotionEntry p => new JsonObject { ["potion"] = p.Model.Id.Entry, ["cost"] = p.Cost },
            MegaCrit.Sts2.Core.Entities.Merchant.MerchantCardRemovalEntry rm => new JsonObject { ["remove"] = rm.Cost },
            _ => new JsonObject { ["entry"] = e.GetType().Name },
        })).ToArray());
        var pk = st["pick"];
        string label = pk is JsonObject po ? "dp:" + (int)po["discard_potion"] : (string)pk;
        if (label.StartsWith("dp:")) Wait(PotionCmd.Discard(_player.PotionSlots[int.Parse(label[3..])]), "discard potion");
        else if (label == "remove a card") Wait(inv.CardRemovalEntry.OnTryPurchaseWrapper(inv), "card removal");
        else if (label != "leave shop")
        {
            string id = NameToId(label);
            var e = inv.AllEntries.FirstOrDefault(x => x.IsStocked && x switch
            {
                MegaCrit.Sts2.Core.Entities.Merchant.MerchantCardEntry c => c.CreationResult.Card.Id.Entry == id,
                MegaCrit.Sts2.Core.Entities.Merchant.MerchantRelicEntry r => r.Model.Id.Entry == id,
                MegaCrit.Sts2.Core.Entities.Merchant.MerchantPotionEntry p => p.Model.Id.Entry == id,
                _ => false,
            }) ?? throw new OracleException($"shop pick {id} not stocked: " + stock.ToJsonString());
            var t = e.OnTryPurchaseWrapper(inv);
            Wait(t, "purchase " + id);
            if (!t.Result) throw new OracleException($"purchase {id} failed (gold {_player.Gold}, cost {e.Cost})");
        }
        CheckSelectsUsed("shop " + label);
        Emit("shop", new JsonObject { ["floor"] = (int)st["floor"], ["stock"] = stock, ["pick"] = label, ["choices"] = _sel.Prompts.DeepClone(), ["state"] = State() });
        _sel.Prompts.Clear();
    }

    private Dictionary<(string, string), string> _labels;

    // English option titles -> option keys, from data/events.json (initial pages of non-ancient events).
    private Dictionary<(string, string), string> EventLabels()
    {
        if (_labels != null) return _labels;
        _labels = new();
        var dir = new DirectoryInfo(Path.GetDirectoryName(Path.GetFullPath(_compactPath)));
        while (dir != null && !File.Exists(Path.Combine(dir.FullName, "data", "events.json"))) dir = dir.Parent;
        if (dir == null) return _labels;
        foreach (var e in JsonNode.Parse(File.ReadAllText(Path.Combine(dir.FullName, "data", "events.json")))["events"].AsArray())
            foreach (var o in e["options"].AsArray())
                if (o["label"] != null) _labels.TryAdd(((string)e["key"], (string)o["label"]), (string)o["key"]);
        return _labels;
    }

    private void DoMap(JsonObject st)
    {
        var pick = st["pick"];
        int act = (int?)st["act"] ?? 0;
        string newMap = null;
        if (act > _run.CurrentActIndex)
        {
            Wait(RunManager.Instance.EnterNextAct(), "enter next act");
            newMap = FullMap();
        }
        MapCoord coord;
        if (pick is JsonValue)
        {
            var m = System.Text.RegularExpressions.Regex.Match((string)pick, @"^r(\d+)c(\d+)$");
            if (!m.Success) throw new OracleException("bad map pick " + pick);
            coord = new MapCoord(int.Parse(m.Groups[2].Value), int.Parse(m.Groups[1].Value));
        }
        else
        {
            var po = pick.AsObject();
            var type = Enum.Parse<MapPointType>((string)po["room"]);
            int? row = (int?)po["row"], col = (int?)po["col"];
            if (type == MapPointType.Boss)
                coord = _run.CurrentMapPoint == _run.Map.BossMapPoint ? _run.Map.SecondBossMapPoint.coord : _run.Map.BossMapPoint.coord;
            else if (type == MapPointType.Ancient) coord = _run.Map.StartingMapPoint.coord;
            else
            {
                var rooms = RoomsByRow(act);
                var memo = new Dictionary<MapPoint, bool>();
                var cands = (_run.CurrentMapPoint?.Children ?? _run.Map.startMapPoints)
                    .Where(p => p.PointType == type && (row == null || p.coord.row == row) && (col == null || p.coord.col == col) && Consistent(p, rooms, memo))
                    .OrderBy(p => p.coord.col).ToList();
                if (cands.Count == 0) throw new OracleException($"map pick {pick.ToJsonString()}: no candidate consistent with the record's later rooms");
                if (cands.Count > 1) Emit("guess", new JsonObject { ["floor"] = (int)st["floor"], ["options"] = new JsonArray(cands.Select(p => (JsonNode)$"r{p.coord.row}c{p.coord.col}").ToArray()) });
                coord = cands[0].coord;
            }
        }
        Wait(RunManager.Instance.EnterMapCoord(coord), "enter map coord " + pick.ToJsonString());
        var rec = new JsonObject { ["floor"] = (int)st["floor"], ["pick"] = $"r{coord.row}c{coord.col}", ["total_floor"] = _run.TotalFloor, ["room"] = RoomInfo(), ["state"] = State() };
        if (newMap != null) rec["map"] = newMap;
        Emit("map", rec);
    }

    private void DoCombat(JsonObject st)
    {
        var fight = st["fight"].AsObject();
        var enc = (_run.CurrentRoom as CombatRoom)?.Encounter?.Id.Entry;
        if (enc != (string)fight["encounter"]) throw new OracleException($"combat: room encounter {enc} != record {(string)fight["encounter"]}");
        var d = new Driver(_out, _pump, _player, _run) { Sel = _sel, Tag = "combat" };
        d.Settle();
        d.Record(null);
        foreach (var turn in fight["turns"].AsArray())
        {
            var acts = turn["acts"].AsArray();
            for (int k = 0; k < acts.Count; k++)
            {
                var a = acts[k].AsArray();
                string kind = (string)a[0];
                _sel.ScriptedIds.Clear();
                while (k + 1 < acts.Count && (string)acts[k + 1][0] == "c")
                {
                    k++;
                    _sel.ScriptedIds.Enqueue(acts[k].AsArray().Skip(1).Select(x => CardIdUp((string)x)).ToArray());
                }
                ActionSpec spec;
                switch (kind)
                {
                    case "p":
                    {
                        // token = alias + '+' upgraded, '*' enchanted, '@k' hand index
                        string tok = (string)a[1];
                        int? at = null;
                        if (tok.Contains('@')) { at = int.Parse(tok[(tok.IndexOf('@') + 1)..]); tok = tok[..tok.IndexOf('@')]; }
                        bool ench = tok.EndsWith("*"); tok = tok.TrimEnd('*');
                        bool up = tok.EndsWith("+");
                        string id = CardId(tok);
                        var hand = _player.PlayerCombatState.Hand.Cards;
                        // agent/reenact.py _hand_index: exact (id, upgraded, enchanted); a plain token may take an enchanted copy
                        bool Ok(int j, bool e) => hand[j].Id.Entry == id && (hand[j].CurrentUpgradeLevel > 0) == up && (hand[j].Enchantment != null) == e;
                        int pos = at ?? Enumerable.Range(0, hand.Count).FirstOrDefault(j => Ok(j, ench), -1);
                        if (pos < 0 && at == null && !ench) pos = Enumerable.Range(0, hand.Count).FirstOrDefault(j => Ok(j, true), -1);
                        if (pos >= 0 && hand[pos].Id.Entry != id) throw new OracleException($"combat {fight["id"]}: hand[{pos}] is {hand[pos].Id.Entry}, record says {id}");
                        if (pos < 0) throw new OracleException($"combat {fight["id"]}: {id} not playable in hand: " + string.Join(", ", hand.Select(c => { c.CanPlay(out var why, out _); return c.Id.Entry + "(" + why + ")"; })) + " " + d.Stuck());
                        spec = new ActionSpec { Kind = "play", HandPos = pos, Target = a.Count > 2 && a[2] != null ? (int)a[2] : null };
                        break;
                    }
                    case "pot":
                        spec = new ActionSpec { Kind = "use_potion", Slot = (int)a[1], Target = a.Count > 2 && a[2] != null ? (int)a[2] : null };
                        break;
                    case "e":
                        spec = new ActionSpec { Kind = "end_turn" };
                        break;
                    default: throw new OracleException("unknown combat act " + kind);
                }
                d.Exec(spec);
                if (_sel.ScriptedIds.Count > 0) throw new OracleException($"combat {fight["id"]}: unused choices");
            }
        }
        if (CombatManager.Instance.IsInProgress) throw new OracleException($"combat {fight["id"]}: still in progress after the recorded actions");
    }

    private int DoRewards(int i)
    {
        var picks = new List<string>();
        var cardPicks = new Queue<string>();
        var drinks = new List<JsonObject>();  // AnyTime potions drunk on the rewards screen: applied after the selection
        int j = i;
        for (; j < _steps.Count; j++)
        {
            var st = _steps[j];
            string screen = (string)st["screen"];
            if (screen == "CARD_REWARD") { var p = st["pick"]; cardPicks.Enqueue(p == null ? null : CardId((string)p)); continue; }
            if (screen != "REWARDS") break;
            var pk = st["pick"];
            if (pk is JsonArray arr) picks.AddRange(arr.Select(x => (string)x));
            else if (pk is JsonValue) picks.Add((string)pk);
            else if (pk is JsonObject po && po["discard_potion"] != null) picks.Add("dp:" + (int)po["discard_potion"]);
            else if (pk is JsonObject pu && pu["potion"] != null) drinks.Add(st);
            else throw new OracleException("unsupported rewards pick " + pk.ToJsonString());
        }
        var room = _run.CurrentRoom as CombatRoom ?? throw new OracleException("rewards outside a combat room: " + _run.CurrentRoom?.GetType().Name);
        var done = new TaskCompletionSource();
        JsonObject offered = null;
        RewardsSet.testSelector = async set =>
        {
            set.ThrowInTestIfRewardsNotTaken = false;
            offered = new JsonObject { ["rewards"] = new JsonArray(set.Rewards.Select(r => (JsonNode)RewardJson(r)).ToArray()) };
            var taken = new HashSet<Reward>();
            foreach (var pk in picks)
            {
                if (pk == "proceed") continue;
                if (pk.StartsWith("dp:"))
                {
                    await MegaCrit.Sts2.Core.Commands.PotionCmd.Discard(_player.PotionSlots[int.Parse(pk[3..])]);
                    continue;
                }
                var r = set.Rewards.FirstOrDefault(x => !taken.Contains(x) && Matches(x, pk));
                if (r == null) { done.TrySetException(new OracleException($"reward pick {pk} not offered: " + offered.ToJsonString())); return; }
                taken.Add(r);
                if (r is CardReward) _sel.ScriptedCardReward.Enqueue(cardPicks.Count > 0 ? cardPicks.Dequeue() : null);
                await RunManager.Instance.RewardsSetSynchronizer.SelectLocalReward(r);
            }
            done.TrySetResult();
        };
        try
        {
            Wait(room.OfferRoomEndRewards(), "offer rewards");
            Wait(done.Task, "select rewards");
        }
        finally { RewardsSet.testSelector = null; }
        foreach (var d in drinks) DrinkPotion(d, (int)d["pick"]["potion"]);
        Emit("rewards", new JsonObject { ["floor"] = (int)_steps[i]["floor"], ["offered"] = offered, ["picks"] = new JsonArray(picks.Select(x => (JsonNode)x).ToArray()), ["choices"] = _sel.Prompts.DeepClone(), ["state"] = State() });
        _sel.Prompts.Clear();
        return j;
    }

    private static bool Matches(Reward r, string pick)
    {
        string p = pick.ToLowerInvariant();
        return r switch
        {
            GoldReward g => p == "gold" || p == g.Amount + " gold",
            PotionReward pr => p == "potion" || NameToId(pick) == pr.Potion?.Id.Entry,
            CardReward c => p == "card" || c.Cards.Any(x => x.Id.Entry == NameToId(pick)),
            RelicReward rr => p == "relic" || NameToId(pick) == rr.Relic?.Id.Entry,
            SpecialCardReward sc => p == "special card" || NameToId(pick) == sc.ToSerializable().SpecialCard?.Id?.Entry,
            _ => false,
        };
    }

    private static JsonObject RewardJson(Reward r) => r switch
    {
        GoldReward g => new JsonObject { ["type"] = "gold", ["amount"] = g.Amount },
        PotionReward p => new JsonObject { ["type"] = "potion", ["id"] = p.Potion?.Id.Entry },
        CardReward c => new JsonObject { ["type"] = "card", ["cards"] = new JsonArray(c.Cards.Select(x => (JsonNode)Dump.CardBrief(x)).ToArray()) },
        RelicReward rr => new JsonObject { ["type"] = "relic", ["id"] = rr.Relic?.Id.Entry },
        SpecialCardReward sc => new JsonObject { ["type"] = "special card", ["id"] = sc.ToSerializable().SpecialCard?.Id?.Entry },
        _ => new JsonObject { ["type"] = r.GetType().Name },
    };

    private async Task Start()
    {
        await RunManager.Instance.FinalizeStartingRelics();
        RunManager.Instance.Launch();
        await RunManager.Instance.EnterAct(0, doTransition: false);
    }

    private JsonObject RoomInfo()
    {
        var room = _run.CurrentRoom;
        var o = new JsonObject { ["type"] = room?.GetType().Name, ["room_type"] = room?.RoomType.ToString() };
        if (room is EventRoom)
        {
            var ev = RunManager.Instance.EventSynchronizer.GetLocalEvent();
            o["event"] = ev.Id.Entry;
            o["finished"] = ev.IsFinished;
            o["options"] = new JsonArray(ev.CurrentOptions.Select(op => (JsonNode)new JsonObject
            {
                ["key"] = op.TextKey, ["title"] = SafeText(() => op.Title?.GetFormattedText()), ["locked"] = op.IsLocked, ["proceed"] = op.IsProceed,
            }).ToArray());
        }
        if (room is CombatRoom cr) o["encounter"] = cr.Encounter?.Id.Entry;
        return o;
    }

    private static string SafeText(Func<string> f) { try { return f(); } catch (Exception e) { return "<" + e.GetType().Name + ">"; } }

    public static string Code(MapPointType t) => t switch
    {
        MapPointType.Monster => "M", MapPointType.Elite => "E", MapPointType.RestSite => "R", MapPointType.Shop => "$",
        MapPointType.Treasure => "T", MapPointType.Unknown => "?", MapPointType.Boss => "B", MapPointType.Ancient => "A", _ => "."
    };

    // Same text as mods/AgentBridge Decisions.FullMap (the live replay log's `map` event).
    public string FullMap()
    {
        var rs = _run;
        if (rs?.Map == null) return "no map\n";
        var sb = new StringBuilder("rows bottom->top; point = <type>c<col>><child cols>; * = visited\n");
        var visited = rs.VisitedMapCoords.ToHashSet();
        int rows = rs.Map.GetRowCount();
        for (int r = 0; r < rows; r++)
        {
            var row = rs.Map.GetPointsInRow(r).Where(p => p != null).OrderBy(p => p.coord.col).ToList();
            if (row.Count == 0) continue;
            sb.Append('r').Append(r).Append(':');
            foreach (var p in row)
                sb.Append(' ').Append(visited.Contains(p.coord) ? "*" : "").Append(Code(p.PointType)).Append('c').Append(p.coord.col)
                  .Append('>').Append(string.Join(",", p.Children.OrderBy(c => c.coord.col).Select(c => c.coord.col)));
            sb.Append('\n');
        }
        sb.Append($"boss: {rs.Map.BossMapPoint?.coord.row} {rs.Act.BossEncounter?.Id.Entry}");
        if (rs.Act.HasSecondBoss) sb.Append($" + {rs.Act.SecondBossEncounter?.Id.Entry}");
        sb.Append('\n');
        return sb.ToString();
    }
}
