using System.Text;
using AgentBridge;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.CardRewardAlternatives;
using MegaCrit.Sts2.Core.Entities.Merchant;
using MegaCrit.Sts2.Core.Entities.Potions;
using MegaCrit.Sts2.Core.Entities.TreasureRelicPicking;
using MegaCrit.Sts2.Core.Events;
using MegaCrit.Sts2.Core.Events.Custom.CrystalSphereEvent;
using MegaCrit.Sts2.Core.Events.Custom.CrystalSphereEvent.CrystalSphereItems;
using MegaCrit.Sts2.Core.GameActions;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Multiplayer.Game;
using MegaCrit.Sts2.Core.Random;
using MegaCrit.Sts2.Core.Rewards;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Unlocks;

namespace OracleCombat;

public sealed record Opt(string Label, Func<string[], string> Run);

public sealed class Decision
{
    public string Kind = "?";
    public readonly StringBuilder Info = new();
    public readonly List<Opt> Opts = new();
    public bool Busy;

    public void Add(string label, Func<string[], string> run) => Opts.Add(new Opt(label, run));

    public string Render()
    {
        var sb = new StringBuilder();
        sb.Append(Kind);
        if (Busy) sb.Append(" (busy)");
        sb.Append('\n').Append(Info);
        for (int i = 0; i < Opts.Count; i++) sb.Append(i).Append(' ').Append(Opts[i].Label).Append('\n');
        return sb.ToString();
    }
}

// Model-side port of mods/AgentBridge/src/Decisions.cs + Commands.cs: the same screens and option labels, derived from the
// game model where the mod reads Godot nodes; the UI state the nodes hold (map open, reward screens, ...) lives here.
public static class Ui
{
    public sealed class PendingRewards
    {
        public RewardsSet Set;
        public bool Terminal;
        public readonly HashSet<Reward> Claimed = new();
        public readonly TaskCompletionSource Tcs = new(TaskCreationOptions.RunContinuationsAsynchronously);
    }

    public sealed class PendingCardReward
    {
        public IReadOnlyList<CardCreationResult> Options;
        public IReadOnlyList<CardRewardAlternative> Alternatives;
        public int? Answer;
    }

    sealed class PendingChoice<T>
    {
        public IReadOnlyList<T> Items;
        public readonly TaskCompletionSource<int> Tcs = new(TaskCreationOptions.RunContinuationsAsynchronously);
    }

    public static PendingCardReward CardRew;
    public static CrystalSphereMinigame Sphere;
    static bool _sphereDone;
    static PendingChoice<RelicModel> _relic;
    static PendingChoice<IReadOnlyList<CardModel>> _bundle;
    static readonly List<PendingRewards> _rewards = new();
    static bool _mapOpen, _chestOpened, _relicPicked, _restChosen, _gameOverShown, _abandonModal;
    static AbstractRoom _room;
    static RunState _suspended;

    public static bool Prompted() => AgentSelector.Current != null || CardRew != null || _relic != null || _bundle != null;

    public static void DrainFatal()
    {
        if (Fatal.Message != null) { Console.Error.WriteLine("serve: " + Fatal.Message); Fatal.Message = null; }
    }

    static RunState Rs => RunManager.Instance?.DebugOnlyGetState();
    static Player Me(RunState rs) => rs != null ? LocalContext.GetMe(rs) : null;

    static void TrackRoom(RunState rs)
    {
        var room = rs?.CurrentRoom;
        if (room == _room) return;
        _room = room;
        _mapOpen = _chestOpened = _relicPicked = _restChosen = false;
    }

    // ---------------------------------------------------------------- build
    public static Decision Build()
    {
        var d = new Decision();
        var rs = _suspended != null ? null : Rs;
        var me = Me(rs);
        TrackRoom(rs);
        if (me != null) Header(d, rs, me);
        if (AgentSelector.Current is { } sel)
        {
            d.Kind = sel.Min == sel.Max ? $"SELECT {sel.Min}" : $"SELECT {sel.Min}-{sel.Max}";
            if (!string.IsNullOrEmpty(sel.Prompt)) d.Info.Append(sel.Prompt).Append('\n');
            d.Info.Append(sel.Min == 0 ? "answer: a <i> [<j> ...]  (a - for none)\n" : "answer: a <i> [<j> ...]\n");
            foreach (var c in sel.Options) d.Add(Text.Card(c), _ => "use: a <i> [<j> ...]");
            return d;
        }
        if (_abandonModal)
        {
            d.Kind = "MODAL NAbandonRunConfirmPopup";
            d.Info.Append("Are you sure?\n");
            d.Add("[PopupYesNoButton] No", _ => { _abandonModal = false; return null; });
            d.Add("[PopupYesNoButton] Yes", _ => { _abandonModal = false; var s = _suspended; _suspended = null; EndRun(); return null; });
            return d;
        }
        if (rs != null && me != null && !CombatManager.Instance.IsInProgress && (me.Creature.IsDead || rs.IsGameOver))
        {
            d.Kind = "GAME_OVER";
            d.Info.Append($"Floors Climbed: {rs.TotalFloor}\n");
            d.Add("main menu", _ => { EndRun(); return null; });
            return d;
        }
        if (CardRew is { } cr)
        {
            d.Kind = "CARD_REWARD";
            for (int i = 0; i < cr.Options.Count; i++) { int k = i; d.Add(Text.Card(cr.Options[i].Card), _ => { cr.Answer = k; return null; }); }
            for (int i = 0; i < cr.Alternatives.Count; i++)
            {
                int k = cr.Options.Count + i;
                string name; try { name = cr.Alternatives[i].Title.GetFormattedText(); } catch { name = cr.Alternatives[i].OptionId; }
                d.Add(name, _ => { cr.Answer = k; return null; });
            }
            return d;
        }
        if (_relic is { } rp)
        {
            d.Kind = "CHOOSE_RELIC";
            for (int i = 0; i < rp.Items.Count; i++) { int k = i; d.Add(Decisions.Relic(rp.Items[i]), _ => { rp.Tcs.TrySetResult(k); return null; }); }
            d.Add("skip", _ => { rp.Tcs.TrySetResult(-1); return null; });
            return d;
        }
        if (_bundle is { } bp)
        {
            d.Kind = "CHOOSE_BUNDLE";
            for (int i = 0; i < bp.Items.Count; i++) { int k = i; d.Add(string.Join(" + ", bp.Items[i].Select(c => Text.Card(c))), _ => { bp.Tcs.TrySetResult(k); return null; }); }
            return d;
        }
        if (_rewards.Count > 0 && !_mapOpen)
        {
            Rewards(d, _rewards[^1], me);
            return d;
        }
        if (Sphere != null)
        {
            CrystalSphere(d, Sphere);
            return d;
        }
        if (rs == null || me == null)
        {
            MainMenu(d);
            return d;
        }
        if (CombatManager.Instance.IsInProgress)
        {
            Combat(d, me);
            return d;
        }
        if ((_mapOpen || rs.CurrentRoom is MapRoom) && Map(d, rs, me)) return d;
        Room(d, rs, me);
        return d;
    }

    static void Header(Decision d, RunState rs, Player me)
    {
        var cr = me.Creature;
        d.Info.Append($"A{rs.CurrentActIndex + 1} F{rs.TotalFloor} {me.Character.Id.Entry} A{rs.AscensionLevel} HP {cr.CurrentHp}/{cr.MaxHp} G{me.Gold}");
        var pots = new List<string>();
        for (int i = 0; i < me.PotionSlots.Count; i++) pots.Add(me.PotionSlots[i] is { } p ? Text.Loc(p.Title) : "-");
        d.Info.Append(" pots[").Append(string.Join(", ", pots)).Append("]\n");
    }

    static bool Ready(Player me)
    {
        var cm = CombatManager.Instance;
        var pcs = me.PlayerCombatState;
        return pcs != null && pcs.Phase == PlayerTurnPhase.Play && !cm.PlayerActionsDisabled && !cm.IsOverOrEnding
               && !RunManager.Instance.ActionExecutor.IsRunning && RunManager.Instance.ActionQueueSet.IsEmpty;
    }

    static void Combat(Decision d, Player me)
    {
        var cm = CombatManager.Instance;
        var cs = cm.DebugOnlyGetState();
        var pcs = me.PlayerCombatState;
        d.Kind = "COMBAT";
        if (pcs == null) { d.Busy = true; return; }
        var players = cs.PlayerCreatures.ToList();
        d.Info.Append($"T{pcs.TurnNumber} E{pcs.Energy}/{pcs.MaxEnergy}");
        if (pcs.Stars > 0 || me.Character.Id.Entry.Contains("REGENT")) d.Info.Append($" *{pcs.Stars}");
        d.Info.Append($" draw{pcs.DrawPile.Cards.Count} disc{pcs.DiscardPile.Cards.Count} exh{pcs.ExhaustPile.Cards.Count}\n");
        d.Info.Append($"you b{me.Creature.Block}{Text.Powers(me.Creature)}\n");
        if (me.Osty is { IsAlive: true } osty) d.Info.Append($"osty {osty.CurrentHp}/{osty.MaxHp} b{osty.Block}{Text.Powers(osty)}\n");
        foreach (var pet in pcs.Pets.Where(p => p.IsAlive && p != me.Osty)) d.Info.Append($"pet {pet.Name} {pet.CurrentHp}/{pet.MaxHp}{Text.Powers(pet)}\n");
        if (pcs.OrbQueue.Capacity > 0)
            d.Info.Append($"orbs {pcs.OrbQueue.Orbs.Count}/{pcs.OrbQueue.Capacity}: ").Append(string.Join(", ", pcs.OrbQueue.Orbs.Select(o => $"{Text.Loc(o.Title)}({o.PassiveVal:0}/{o.EvokeVal:0})"))).Append('\n');
        var enemies = cs.Enemies.ToList();
        for (int i = 0; i < enemies.Count; i++)
        {
            var e = enemies[i];
            bool hittable = cs.HittableEnemies.Contains(e);
            if (!e.IsAlive && e.Monster?.NextMove == null) continue;
            d.Info.Append($"e{i} {e.Name} {e.CurrentHp}/{e.MaxHp} b{e.Block}{(hittable ? "" : " [untargetable]")} -> {Text.Intent(e, players)}{Text.Powers(e)}\n");
        }
        if (!Ready(me)) { d.Busy = true; return; }
        d.Info.Append("play: a <i> [e<target>]\n");
        foreach (var c in pcs.Hand.Cards.ToList())
        {
            bool ok = c.CanPlay() && (c.TargetType != TargetType.AnyEnemy || cs.HittableEnemies.Any());
            string tgt = c.TargetType == TargetType.AnyEnemy ? " ->e" : "";
            d.Add((ok ? "" : "(x) ") + Text.Card(c, true, true) + tgt, a =>
            {
                Creature t = null;
                if (c.TargetType == TargetType.AnyEnemy)
                {
                    t = PickEnemy(cs, a, out var err);
                    if (t == null) return err;
                }
                else if (c.TargetType is TargetType.AnyAlly or TargetType.AnyPlayer) t = me.Creature;
                else if (c.TargetType == TargetType.Osty) t = me.Osty;
                int pos = Snap.HandPos(c);
                var tg = Snap.TargetOf(t);
                if (!c.TryManualPlay(t)) return "cannot play " + c.Title;
                Snap.LogPlay(pos, tg);
                return null;
            });
        }
        for (int i = 0; i < me.PotionSlots.Count; i++)
        {
            if (me.PotionSlots[i] is not { } p) continue;
            if (p.Usage is not (PotionUsage.CombatOnly or PotionUsage.AnyTime)) continue;
            bool pok = p.TargetType != TargetType.AnyEnemy || cs.HittableEnemies.Any();
            d.Add((pok ? "" : "(x) ") + $"potion {Text.Loc(p.Title)}: {Text.Loc(p.DynamicDescription)}" + (p.TargetType == TargetType.AnyEnemy ? " ->e" : ""), a => UsePotion(p, me, cs, a));
        }
        d.Add("end turn", _ =>
        {
            RunManager.Instance.ActionQueueSynchronizer.RequestEnqueue(new EndPlayerTurnAction(me, pcs.TurnNumber));
            Snap.LogEndTurn();
            return null;
        });
    }

    public static string DoJson(string json)
    {
        var doc = System.Text.Json.Nodes.JsonNode.Parse(json)!.AsObject();
        if (doc.ContainsKey("choose"))
            return AgentSelector.Answer(doc["choose"]!.AsArray().Select(x => (int)x!).ToList());
        var rs = Rs;
        var me = Me(rs);
        var cm = CombatManager.Instance;
        if (me == null || !cm.IsInProgress) return "not in combat";
        var cs = cm.DebugOnlyGetState()!;
        var pcs = me.PlayerCombatState!;
        if (!Ready(me)) return "game is busy";
        if (doc.ContainsKey("end_turn"))
        {
            RunManager.Instance.ActionQueueSynchronizer.RequestEnqueue(new EndPlayerTurnAction(me, pcs.TurnNumber));
            Snap.LogEndTurn();
            return null;
        }
        Creature Target(System.Text.Json.Nodes.JsonObject o)
        {
            if (o["target"] != null) { var l = cs.Enemies.ToList(); int i = (int)o["target"]!; return i >= 0 && i < l.Count ? l[i] : null; }
            return null;
        }
        if (doc["play"] is System.Text.Json.Nodes.JsonObject pl)
        {
            var hand = pcs.Hand.Cards.ToList();
            int pos = (int)pl["hand_pos"]!;
            if (pos < 0 || pos >= hand.Count) return "bad hand_pos";
            var c = hand[pos];
            Creature t = Target(pl);
            if (c.TargetType == TargetType.AnyEnemy && t == null) return "needs a target";
            if (c.TargetType is TargetType.AnyAlly or TargetType.AnyPlayer) t = me.Creature;
            else if (c.TargetType == TargetType.Osty) t = me.Osty;
            var tg = Snap.TargetOf(t);
            if (!c.TryManualPlay(t)) return "cannot play " + c.Title;
            Snap.LogPlay(pos, tg);
            return null;
        }
        if (doc["use_potion"] is System.Text.Json.Nodes.JsonObject up)
        {
            int slot = (int)up["slot"]!;
            if (slot < 0 || slot >= me.PotionSlots.Count || me.PotionSlots[slot] is not { } p) return "no potion in that slot";
            var t = Target(up);
            return UsePotion(p, me, cs, t != null ? new[] { "e" + cs.Enemies.ToList().IndexOf(t) } : Array.Empty<string>());
        }
        return "unsupported action";
    }

    static Creature PickEnemy(CombatState cs, string[] a, out string err)
    {
        err = null;
        var enemies = cs.Enemies.ToList();
        var hittable = cs.HittableEnemies.ToList();
        if (a.Length == 0)
        {
            if (hittable.Count == 1) return hittable[0];
            err = "needs a target: e<i>";
            return null;
        }
        if (int.TryParse(a[0].TrimStart('e', 'E'), out int i) && i >= 0 && i < enemies.Count && hittable.Contains(enemies[i])) return enemies[i];
        err = "bad target " + a[0];
        return null;
    }

    static string UsePotion(PotionModel p, Player me, CombatState cs, string[] a)
    {
        if (!me.CanUseOrRemovePotions) return "cannot use potions now";
        Creature t = null;
        if (p.TargetType == TargetType.AnyEnemy)
        {
            if (cs == null) return "needs combat";
            t = PickEnemy(cs, a, out var err);
            if (t == null) return err;
        }
        else if (p.TargetType is TargetType.AnyAlly or TargetType.AnyPlayer or TargetType.Self) t = me.Creature;
        int slot = me.PotionSlots.ToList().IndexOf(p);
        var tg = Snap.TargetOf(t);
        p.EnqueueManualUse(t);
        if (CombatManager.Instance.IsInProgress) Snap.LogPotion(slot, tg);
        return null;
    }

    public static string DiscardPotion(string slotArg)
    {
        var me = Me(Rs);
        if (me == null) return "no run";
        if (!int.TryParse(slotArg, out int slot) || slot < 0 || slot >= me.PotionSlots.Count || me.PotionSlots[slot] == null) return "usage: a dp <slot> (0-based, non-empty)";
        if (!me.CanUseOrRemovePotions) return "cannot remove potions now";
        RunManager.Instance.ActionQueueSynchronizer.RequestEnqueue(new DiscardPotionGameAction(me, (uint)slot, CombatManager.Instance.IsInProgress));
        return null;
    }

    public static string Act(string[] args)
    {
        if (args.Length == 0) return "usage: a <i> [args]";
        if (AgentSelector.Current != null)
        {
            if (args[0] == "-") return AgentSelector.Answer(Array.Empty<int>());
            var idx = new List<int>();
            foreach (var a in args)
            {
                if (!int.TryParse(a, out var i)) return "bad index " + a;
                idx.Add(i);
            }
            return AgentSelector.Answer(idx);
        }
        if (args[0] == "dp") return DiscardPotion(args.Skip(1).FirstOrDefault());
        var d = Build();
        if (d.Busy && d.Opts.Count == 0) return "game is busy";
        if (!int.TryParse(args[0], out int n) || n < 0 || n >= d.Opts.Count) return $"bad option {args[0]} (0..{d.Opts.Count - 1})";
        return d.Opts[n].Run(args.Skip(1).ToArray());
    }

    // ---------------------------------------------------------------- rewards (NRewardsScreen, NCardRewardSelectionScreen)
    public static async Task OnRewards(RewardsSet set)
    {
        set.ThrowInTestIfRewardsNotTaken = false;
        var p = new PendingRewards { Set = set, Terminal = set.Room is CombatRoom };
        _rewards.Add(p);
        await p.Tcs.Task;
    }

    static void CloseRewards(PendingRewards p)
    {
        _rewards.Remove(p);
        p.Tcs.TrySetResult();
    }

    static IEnumerable<Reward> Remaining(PendingRewards p) => p.Set.Rewards.Where(r => !p.Claimed.Contains(r));

    static void Rewards(Decision d, PendingRewards p, Player me)
    {
        d.Kind = "REWARDS";
        foreach (var r in Remaining(p).ToList())
        {
            string label = RewardLabel(r);
            if (r is PotionReward && me is { HasOpenPotionSlots: false }) label += " (potion slots full: a dp <slot> first)";
            d.Add(label, _ => { Serve.Fire(TakeReward(p, r), "reward"); return null; });
        }
        if (!(p.Set.DisallowSkipping && Remaining(p).Any()))
            d.Add("proceed (skip the rest)", _ => { ProceedRewards(p); return null; });
    }

    static async Task TakeReward(PendingRewards p, Reward r)
    {
        if (await RunManager.Instance.RewardsSetSynchronizer.SelectLocalReward(r))
        {
            p.Claimed.Add(r);
            if (!Remaining(p).Any() && !p.Terminal) CloseRewards(p);
        }
    }

    static void ProceedRewards(PendingRewards p)
    {
        if (!p.Terminal)
        {
            RunManager.Instance.RewardsSetSynchronizer.SkipLocalRewardsSet();
            CloseRewards(p);
            return;
        }
        var rs = Rs;
        if (rs.CurrentRoom.RoomType == RoomType.Boss || rs.CurrentRoom.IsVictoryRoom)
        {
            CloseRewards(p);
            if (rs.Map.SecondBossMapPoint != null && rs.CurrentMapCoord == rs.Map.BossMapPoint.coord) { ProceedTerminal(); return; }
            RunManager.Instance.ActChangeSynchronizer.SetLocalPlayerReady();
            return;
        }
        CloseRewards(p);
        ProceedTerminal();
    }

    // RunManager.ProceedFromTerminalRewardsScreen, whose NMapScreen.Open is the map flag here
    static void ProceedTerminal()
    {
        var rs = Rs;
        bool resume = rs.CurrentRoomCount > 1 && rs.CurrentRoom is CombatRoom { ShouldResumeParentEventAfterCombat: not false };
        Serve.Fire(RunManager.Instance.ProceedFromTerminalRewardsScreen(), "proceed");
        if (!resume) _mapOpen = true;
    }

    public static async Task AfterCombatWon(CombatRoom room)
    {
        if (room.Encounter.ShouldGiveRewards) await room.OfferRoomEndRewards();
        else ProceedTerminal();
    }

    static string RewardLabel(Reward r) => r switch
    {
        null => "?",
        CardReward cr => "card: " + string.Join(" | ", cr.Cards.Select(c => c.Title)),
        RelicReward rr => "relic " + Decisions.Relic(rr.Relic),
        PotionReward pr => "potion " + (pr.Potion is { } p ? $"{Text.Loc(p.Title)}: {Text.Loc(p.DynamicDescription)}" : "?"),
        _ => Text.Loc(r.Description),
    };

    public static async Task<RelicModel> ChooseRelic(Player player, IReadOnlyList<RelicModel> relics)
    {
        uint id = RunManager.Instance.PlayerChoiceSynchronizer.ReserveChoiceId(player);
        var p = new PendingChoice<RelicModel> { Items = relics };
        _relic = p;
        int k;
        try { k = await p.Tcs.Task; } finally { _relic = null; }
        var r = k < 0 ? null : relics[k];
        RunManager.Instance.PlayerChoiceSynchronizer.SyncLocalChoice(player, id, PlayerChoiceResult.FromIndex(k));
        return r;
    }

    public static async Task<IEnumerable<CardModel>> ChooseBundle(Player player, IReadOnlyList<IReadOnlyList<CardModel>> bundles)
    {
        if (CombatManager.Instance.IsEnding || bundles.Count == 0) return Array.Empty<CardModel>();
        uint id = RunManager.Instance.PlayerChoiceSynchronizer.ReserveChoiceId(player);
        var p = new PendingChoice<IReadOnlyList<CardModel>> { Items = bundles };
        _bundle = p;
        int k;
        try { k = await p.Tcs.Task; } finally { _bundle = null; }
        RunManager.Instance.PlayerChoiceSynchronizer.SyncLocalChoice(player, id, PlayerChoiceResult.FromIndex(k));
        return bundles[k];
    }

    // ---------------------------------------------------------------- crystal sphere (NCrystalSphereScreen)
    static void CrystalSphere(Decision d, CrystalSphereMinigame g)
    {
        d.Kind = "CRYSTAL_SPHERE";
        d.Info.Append($"{g.DivinationCount} divinations left, tool {g.CrystalSphereTool} (big clears the 3x3 block around the cell, small one cell); an item pays out once every cell it covers is clear.\n");
        d.Info.Append("grid (# hidden, . clear, R relic, P potion, C card, X curse, g gold shown through clear cells); rows are y, columns x:\n    ");
        var size = g.GridSize;
        for (int x = 0; x < size.X; x++) d.Info.Append((x % 10).ToString());
        d.Info.Append('\n');
        for (int y = 0; y < size.Y; y++)
        {
            d.Info.Append(y.ToString().PadLeft(2)).Append("  ");
            for (int x = 0; x < size.X; x++)
            {
                var c = g.cells[x, y];
                d.Info.Append(c.IsHidden ? '#' : c.Item switch
                {
                    CrystalSphereRelic => 'R', CrystalSpherePotion => 'P', CrystalSphereCardReward => 'C', CrystalSphereCurse => 'X', CrystalSphereGold => 'g', null => '.', _ => '?',
                });
            }
            d.Info.Append('\n');
        }
        if (!g.IsFinished)
        {
            d.Add("divine <x> <y>: spend a divination on that cell", args =>
            {
                if (args.Length < 2 || !int.TryParse(args[0], out int x) || !int.TryParse(args[1], out int y)) return "usage: a 0 <x> <y>";
                if (x < 0 || y < 0 || x >= size.X || y >= size.Y) return $"cell {x},{y} is off the {size.X}x{size.Y} grid";
                var cell = g.cells[x, y];
                if (!cell.IsHidden && g.CrystalSphereTool != CrystalSphereMinigame.CrystalSphereToolType.Big) return $"cell {x},{y} is already clear";
                if (g.DivinationCount <= 0) return null;
                g.SetHoveredCell(cell);
                Serve.Fire(g.CellClicked(cell), "divine");
                return null;
            });
            d.Add("tool big", _ => { g.SetTool(CrystalSphereMinigame.CrystalSphereToolType.Big); return null; });
            d.Add("tool small", _ => { g.SetTool(CrystalSphereMinigame.CrystalSphereToolType.Small); return null; });
        }
        if (_sphereDone) d.Add("proceed", _ => { Sphere = null; _sphereDone = false; ProceedTerminal(); return null; });
        else if (g.IsFinished) d.Busy = true;
    }

    public static void SphereFinished() => _sphereDone = true;

    // ---------------------------------------------------------------- map (NMapScreen.RecalculateTravelability)
    static List<MapPoint> Travelable(RunState rs)
    {
        var map = rs.Map;
        var visited = rs.VisitedMapCoords;
        if (visited.Count == 0) return new List<MapPoint> { map.StartingMapPoint };
        var last = visited[visited.Count - 1];
        if (map.SecondBossMapPoint != null && last == map.BossMapPoint.coord) return new List<MapPoint> { map.SecondBossMapPoint };
        if (last.row == map.GetRowCount() - 1) return new List<MapPoint> { map.BossMapPoint };
        var cur = map.GetPoint(last);
        return cur == null ? new List<MapPoint> { map.StartingMapPoint } : MapTravel.GetTravelablePointsFrom(rs, cur).ToList();
    }

    static bool Map(Decision d, RunState rs, Player me)
    {
        d.Kind = "MAP";
        var pts = Travelable(rs).Where(p => p != null && !(rs.VisitedMapCoords.Contains(p.coord))).ToList();
        if (pts.Count == 0) return false;
        d.Info.Append("full map: m\n");
        foreach (var p in pts.OrderBy(p => p.coord.col))
            d.Add($"{p.PointType} r{p.coord.row}c{p.coord.col} -> " + string.Join(",", p.Children.OrderBy(c => c.coord.col).Select(c => $"{Decisions.Code(c.PointType)}c{c.coord.col}")), _ =>
            {
                var source = new MapLocation(rs.CurrentMapCoord, rs.CurrentActIndex);
                var vote = new MapVote { coord = p.coord, mapGenerationCount = RunManager.Instance.MapSelectionSynchronizer.MapGenerationCount };
                RunManager.Instance.ActionQueueSynchronizer.RequestEnqueue(new VoteForMapCoordAction(me, source, vote));
                _mapOpen = false;
                return null;
            });
        return true;
    }

    public static string FullMap()
    {
        var rs = Rs;
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
                sb.Append(' ').Append(visited.Contains(p.coord) ? "*" : "").Append(Decisions.Code(p.PointType)).Append('c').Append(p.coord.col)
                  .Append('>').Append(string.Join(",", p.Children.OrderBy(c => c.coord.col).Select(c => c.coord.col)));
            sb.Append('\n');
        }
        sb.Append($"boss: {rs.Map.BossMapPoint?.coord.row} {rs.Act.BossEncounter?.Id.Entry}");
        if (rs.Act.HasSecondBoss) sb.Append($" + {rs.Act.SecondBossEncounter?.Id.Entry}");
        sb.Append('\n');
        return sb.ToString();
    }

    // ---------------------------------------------------------------- rooms
    static void Room(Decision d, RunState rs, Player me)
    {
        var room = rs.CurrentRoom;
        d.Kind = room?.RoomType.ToString().ToUpperInvariant() ?? "ROOM";
        if (room == null || room.RoomType is RoomType.Unassigned or RoomType.Map) { d.Busy = true; return; }
        switch (room.RoomType)
        {
            case RoomType.Event: Event(d, rs, me); break;
            case RoomType.RestSite: Rest(d); break;
            case RoomType.Shop: Shop(d, me, ((MerchantRoom)room).GetLocalInventory(), "leave shop"); break;
            case RoomType.Treasure: Treasure(d, (TreasureRoom)room); break;
            case RoomType.Monster or RoomType.Elite or RoomType.Boss:
                if (_rewards.Count == 0) d.Add("proceed", _ => { ProceedTerminal(); return null; });
                break;
        }
        for (int i = 0; i < me.PotionSlots.Count; i++)
            if (me.PotionSlots[i] is { Usage: PotionUsage.AnyTime } p && d.Opts.Count > 0)
                d.Add($"potion {Text.Loc(p.Title)}: {Text.Loc(p.DynamicDescription)}", a => UsePotion(p, me, null, a));
        if (d.Opts.Count == 0) d.Busy = true;
    }

    static void Event(Decision d, RunState rs, Player me)
    {
        var ev = RunManager.Instance.EventSynchronizer.GetLocalEvent();
        if (ev == null) return;
        d.Info.Append(Text.Loc(ev.Title)).Append(": ");
        try
        {
            var desc = ev.Description;
            if (desc != null && desc.Exists())
            {
                ev.Owner.Character.AddDetailsTo(desc);
                desc.Add("IsMultiplayer", false);
                ev.DynamicVars.AddTo(desc);
                d.Info.Append(Text.Loc(desc));
            }
        }
        catch { }
        d.Info.Append('\n');
        if (ev.IsFinished)
        {
            var proceed = new EventOption(ev, null, "PROCEED", false, true);
            d.Add(OptionLabel(ev, proceed), _ => { _mapOpen = true; return null; });
            return;
        }
        if (ev is MegaCrit.Sts2.Core.Models.Events.FakeMerchant fm && ev.CurrentOptions.Count == 0 && fm.Inventory != null)
        {
            Shop(d, me, fm.Inventory, "leave the merchant");
            return;
        }
        var opts = ev.CurrentOptions;
        for (int i = 0; i < opts.Count; i++)
        {
            if (opts[i].IsLocked) continue;
            int k = i;
            d.Add(OptionLabel(ev, opts[i]), _ => { RunManager.Instance.EventSynchronizer.ChooseLocalOption(k); return null; });
        }
    }

    static string OptionLabel(EventModel ev, EventOption o)
    {
        string t = "", ds = "";
        try { ev.DynamicVars.AddTo(o.Title); t = Text.Loc(o.Title); } catch { }
        try { ev.DynamicVars.AddTo(o.Description); ds = Text.Loc(o.Description); } catch { }
        return ds.Length > 0 ? $"{t}: {ds}" : t;
    }

    static void Rest(Decision d)
    {
        var sync = RunManager.Instance.RestSiteSynchronizer;
        var opts = sync.GetLocalOptions();
        for (int i = 0; i < opts.Count; i++)
        {
            if (!opts[i].IsEnabled) continue;
            int k = i;
            d.Add($"{Text.Loc(opts[i].Title)}: {Text.Loc(opts[i].Description)}", _ => { Serve.Fire(Choose(k), "rest option"); return null; });
        }
        if (_restChosen || opts.Count == 0) d.Add("proceed", _ => { _mapOpen = true; return null; });

        static async Task Choose(int k)
        {
            if (await RunManager.Instance.RestSiteSynchronizer.ChooseLocalOption(k)) _restChosen = true;
        }
    }

    static void Shop(Decision d, Player me, MerchantInventory inv, string leave)
    {
        // NMerchantCardRemoval.FillSlot / OnCardRemovalUsed: the UI marks the removal slot used
        if (inv.CardRemovalEntry is { Used: false } rm && !MegaCrit.Sts2.Core.Hooks.Hook.ShouldAllowMerchantCardRemoval(me.RunState, me)) rm.SetUsed();
        foreach (var e in inv.AllEntries.Where(e => e.IsStocked).ToList())
        {
            string what = e switch
            {
                MerchantCardEntry c => "card " + (c.CreationResult?.Card is { } cm ? Text.Card(cm) : "?") + (c.IsOnSale ? " SALE" : ""),
                MerchantRelicEntry r => "relic " + Decisions.Relic(r.Model),
                MerchantPotionEntry p => "potion " + (p.Model is { } pm ? $"{Text.Loc(pm.Title)}: {Text.Loc(pm.DynamicDescription)}" : "?"),
                MerchantCardRemovalEntry => "remove a card",
                _ => e.GetType().Name,
            };
            d.Add($"{e.Cost}g {what}" + (e.EnoughGold ? "" : " (can't afford)"), _ =>
            {
                if (!e.EnoughGold) return "not enough gold";
                if (e is MerchantPotionEntry && !me.HasOpenPotionSlots) return "potion slots full (a dp <slot> first)";
                Serve.Fire(Purchase(e, inv), "purchase");
                return null;
            });
        }
        d.Add(leave, _ => { _mapOpen = true; return null; });
    }

    static async Task Purchase(MerchantEntry e, MerchantInventory inv)
    {
        if (await e.OnTryPurchaseWrapper(inv) && e is MerchantCardRemovalEntry rm) rm.SetUsed();
    }

    static void Treasure(Decision d, TreasureRoom room)
    {
        var sync = RunManager.Instance.TreasureRoomRelicSynchronizer;
        if (!_chestOpened)
        {
            d.Add("open chest", _ => { _chestOpened = true; Serve.Fire(OpenChest(room), "open chest"); return null; });
            return;
        }
        var relics = sync.CurrentRelics;
        if (!_relicPicked && relics != null)
            for (int i = 0; i < relics.Count; i++)
            {
                int k = i;
                d.Add("take " + Decisions.Relic(relics[i]), _ => { _relicPicked = true; sync.PickRelicLocally(k); return null; });
            }
        d.Add("proceed", _ =>
        {
            if (!_relicPicked) { _relicPicked = true; sync.SkipRelicLocally(); }
            ProceedTerminal();
            return null;
        });
    }

    static async Task OpenChest(TreasureRoom room)
    {
        await room.DoNormalRewards();
        await room.DoExtraRewardsIfNeeded();
    }

    // NTreasureRoomRelicCollection.AnimateRelicAwards obtains the relic in the shipped game
    static void OnRelicsAwarded(List<RelicPickingResult> results)
    {
        foreach (var r in results.OrderBy(r => r.type))
            if (r.type != RelicPickingResultType.Skipped)
                Serve.Fire(RelicCmd.Obtain(r.relic.ToMutable(), r.player), "obtain relic");
    }

    // ---------------------------------------------------------------- menu and run lifecycle
    static void MainMenu(Decision d)
    {
        d.Kind = "MENU";
        if (_suspended != null)
        {
            d.Add("continue run", _ => { _suspended = null; return null; });
            d.Add("abandon run", _ => { _abandonModal = true; return null; });
            return;
        }
        d.Add("new run  (a <i> <character> [ascension] [seed]; characters: ironclad silent defect regent necrobinder)", a =>
        {
            Serve.Fire(StartRun(a.Length > 0 ? a[0] : "ironclad", a.Length > 1 && int.TryParse(a[1], out var asc) ? asc : Serve.DefaultAscension, a.Length > 2 ? a[2] : null, null), "new run");
            return null;
        });
        d.Add("custom run  (a <i> <character> <ascension> <seed> [modifier ids, comma separated; see `mods`])", a =>
        {
            Serve.Fire(StartRun(a.Length > 0 ? a[0] : "ironclad", a.Length > 1 && int.TryParse(a[1], out var asc) ? asc : Serve.DefaultAscension, a.Length > 2 ? a[2] : null,
                a.Length > 3 ? a[3].Split(',', StringSplitOptions.RemoveEmptyEntries) : Array.Empty<string>()), "custom run");
            return null;
        });
    }

    // `menu`: save-and-quit to the main menu (the run is kept in memory; `continue run` resumes it)
    public static string Suspend()
    {
        var rs = Rs;
        if (rs == null) return "no run\n";
        _suspended = rs;
        return "run suspended; the menu offers continue / abandon\n";
    }

    static void EndRun()
    {
        try { RunManager.Instance.CleanUp(graceful: true); } catch (Exception e) { Console.Error.WriteLine("serve: cleanup: " + e.GetBaseException().Message); }
        LocalContext.NetId = 1UL;
        _rewards.Clear();
        CardRew = null; Sphere = null; _relic = null; _bundle = null;
        _room = null;
        _mapOpen = false;
    }

    public static async Task StartRun(string character, int ascension, string seed, string[] modifiers)
    {
        if (Rs != null) EndRun();
        var ch = ModelDb.AllCharacters.FirstOrDefault(c => c.Id.Entry.Equals(character, StringComparison.OrdinalIgnoreCase))
                 ?? ModelDb.AllCharacters.FirstOrDefault(c => c.Id.Entry.Contains(character, StringComparison.OrdinalIgnoreCase))
                 ?? throw new OracleException("unknown character " + character);
        seed ??= SeedHelper.GetRandomSeed();
        if (modifiers != null) seed = SeedHelper.CanonicalizeSeed(seed);
        var mods = modifiers == null ? new List<ModifierModel>() : ModelDb.GoodModifiers.Concat(ModelDb.BadModifiers)
            .Where(m => modifiers.Any(x => m.Id.Entry.Contains(x, StringComparison.OrdinalIgnoreCase))).Select(m => m.ToMutable()).ToList();
        var unlock = UnlockState.all;
        var rng = new Rng(StringHelper.GetDeterministicHashCode(seed), "act_selection");
        var acts = ActModel.GetRandomList(rng, unlock, isMultiplayer: false).ToList();
        var player = Player.CreateForNewRun(ch, unlock, 1UL);
        var run = RunState.CreateForNewRun(new List<Player> { player }, acts.Select(a => a.ToMutable()).ToList(), mods,
            modifiers == null ? GameMode.Standard : GameMode.Custom, ascension, seed);
        RunManager.Instance.SetUpNewSingleplayer(run, shouldSave: false);
        RunManager.Instance.TreasureRoomRelicSynchronizer.RelicsAwarded += OnRelicsAwarded;
        await RunManager.Instance.FinalizeStartingRelics();
        RunManager.Instance.Launch();
        await RunManager.Instance.EnterAct(0, doTransition: false);
    }

    // ---------------------------------------------------------------- read-only text (Commands.cs)
    public static string Deck()
    {
        var me = Me(Rs);
        if (me == null) return "no run\n";
        var sb = new StringBuilder();
        sb.Append($"deck ({me.Deck.Cards.Count}):\n");
        foreach (var g in me.Deck.Cards.GroupBy(c => Text.Card(c)).OrderBy(g => g.Key))
            sb.Append(g.Count() > 1 ? $"{g.Count()}x " : "").Append(g.Key).Append('\n');
        sb.Append("relics:\n");
        foreach (var r in me.Relics) sb.Append(Decisions.Relic(r)).Append('\n');
        sb.Append("potions:\n");
        foreach (var p in me.PotionSlots) if (p != null) sb.Append($"{Text.Loc(p.Title)}: {Text.Loc(p.DynamicDescription)}\n");
        return sb.ToString();
    }

    public static string Pile(string which)
    {
        var pcs = Me(Rs)?.PlayerCombatState;
        if (pcs == null) return "not in combat\n";
        var pile = which.StartsWith("disc") ? pcs.DiscardPile : which.StartsWith("ex") ? pcs.ExhaustPile : pcs.DrawPile;
        var sb = new StringBuilder($"{which} ({pile.Cards.Count}, unordered):\n");
        foreach (var g in pile.Cards.GroupBy(c => Text.Card(c, false)).OrderBy(g => g.Key))
            sb.Append(g.Count() > 1 ? $"{g.Count()}x " : "").Append(g.Key).Append('\n');
        return sb.ToString();
    }
}

// Helpers with the mod's exact text (Decisions.Relic / Decisions.Code).
public static class Decisions
{
    public static string Relic(RelicModel r) => r == null ? "?" : $"{Text.Loc(r.Title)}: {Text.Loc(r.DynamicDescription)}";

    public static string Code(MapPointType t) => t switch
    {
        MapPointType.Monster => "M", MapPointType.Elite => "E", MapPointType.RestSite => "R", MapPointType.Shop => "$",
        MapPointType.Treasure => "T", MapPointType.Unknown => "?", MapPointType.Boss => "B", MapPointType.Ancient => "A", _ => "."
    };
}
