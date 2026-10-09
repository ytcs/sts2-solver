using System.Text;
using Godot;
using MegaCrit.Sts2.Core.Entities.Merchant;
using MegaCrit.Sts2.Core.Events.Custom.CrystalSphereEvent;
using MegaCrit.Sts2.Core.Events.Custom.CrystalSphereEvent.CrystalSphereItems;
using MegaCrit.Sts2.Core.Nodes.Events.Custom.CrystalSphere;
using MegaCrit.Sts2.Core.Nodes.Events.Custom;
using MegaCrit.Sts2.Core.Entities.Potions;
using MegaCrit.Sts2.Core.GameActions;
using MegaCrit.Sts2.Core.Nodes.Cards.Holders;
using MegaCrit.Sts2.Core.Nodes.Screens.CharacterSelect;
using MegaCrit.Sts2.Core.Nodes.Screens.GameOverScreen;

namespace AgentBridge;

public sealed record Opt(string Label, Func<string[], string?> Run);

public sealed class Decision
{
    public string Kind = "?";
    public readonly StringBuilder Info = new();
    public readonly List<Opt> Opts = new();
    public bool Busy;

    public void Add(string label, Func<string[], string?> run) => Opts.Add(new Opt(label, run));
    public void Click(string label, NClickableControl b) => Add(label, _ => { b.ForceClick(); return null; });

    public string Render()
    {
        var sb = new StringBuilder();
        sb.Append(Kind);
        if (Busy) sb.Append(" (busy)");
        sb.Append('\n').Append(Info);
        for (int i = 0; i < Opts.Count; i++) sb.Append(i).Append(' ').Append(Opts[i].Label).Append('\n');
        return sb.ToString();
    }

    public string Sig() => Kind + Busy + Info + string.Join("|", Opts.Select(o => o.Label));
}

public static class Decisions
{
    private static Node Root => ((SceneTree)Engine.GetMainLoop()).Root;

    private static void Fire(Task t) => t.ContinueWith(x => Main.Log.Info($"AgentBridge task error: {x.Exception}", 0), TaskContinuationOptions.OnlyOnFaulted);

    public static Decision Build()
    {
        var d = new Decision();
        var rs = RunManager.Instance?.DebugOnlyGetState();
        var me = rs != null ? LocalContext.GetMe(rs) : null;
        if (me != null) Header(d, rs!, me);

        if (AgentSelector.Current is { } sel)
        {
            d.Kind = sel.Min == sel.Max ? $"SELECT {sel.Min}" : $"SELECT {sel.Min}-{sel.Max}";
            if (!string.IsNullOrEmpty(sel.Prompt)) d.Info.Append(sel.Prompt).Append('\n');
            d.Info.Append(sel.Min == 0 ? "answer: a <i> [<j> ...]  (a - for none)\n" : "answer: a <i> [<j> ...]\n");
            foreach (var c in sel.Options) d.Add(Text.Card(c), _ => "use: a <i> [<j> ...]");
            return d;
        }
        if (NModalContainer.Instance?.OpenModal is Control modal && GodotObject.IsInstanceValid(modal) && modal.IsVisibleInTree())
        {
            d.Kind = "MODAL " + modal.GetType().Name;
            d.Info.Append(Text.NodeLabel(modal)).Append('\n');
            Buttons(d, modal);
            return d;
        }
        if (rs != null && NOverlayStack.Instance?.Peek() is NRewardsScreen && NMapScreen.Instance is { IsOpen: true } m1 && Map(d, m1, rs)) return d;
        if (NOverlayStack.Instance?.Peek() is Node top && GodotObject.IsInstanceValid(top))
        {
            Overlay(d, top, me);
            if (d.Opts.Count > 0 || d.Busy) return d;
            if (rs != null && NMapScreen.Instance is { IsOpen: true } m0 && Map(d, m0, rs)) return d;
            d.Busy = true;
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
        if (NMapScreen.Instance is { IsOpen: true } map && Map(d, map, rs)) return d;
        Room(d, rs, me);
        return d;
    }

    private static void Header(Decision d, RunState rs, Player me)
    {
        var cr = me.Creature;
        d.Info.Append($"A{rs.CurrentActIndex + 1} F{rs.TotalFloor} {me.Character.Id.Entry} A{rs.AscensionLevel} HP {cr.CurrentHp}/{cr.MaxHp} G{me.Gold}");
        var pots = new List<string>();
        for (int i = 0; i < me.PotionSlots.Count; i++)
            pots.Add(me.PotionSlots[i] is { } p ? Text.Loc(p.Title) : "-");
        d.Info.Append(" pots[").Append(string.Join(", ", pots)).Append("]\n");
    }

    private static void Combat(Decision d, Player me)
    {
        var cm = CombatManager.Instance;
        var cs = cm.DebugOnlyGetState()!;
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
        bool ready = pcs.Phase == PlayerTurnPhase.Play && !cm.PlayerActionsDisabled && !cm.IsOverOrEnding
                     && !RunManager.Instance.ActionExecutor.IsRunning && RunManager.Instance.ActionQueueSet.IsEmpty;
        if (!ready) { d.Busy = true; return; }

        d.Info.Append("play: a <i> [e<target>]\n");
        foreach (var c in pcs.Hand.Cards.ToList())
        {
            bool ok = c.CanPlay() && (c.TargetType != TargetType.AnyEnemy || cs.HittableEnemies.Any());
            string tgt = c.TargetType == TargetType.AnyEnemy ? " ->e" : "";
            d.Add((ok ? "" : "(x) ") + Text.Card(c, true, true) + tgt, a =>
            {
                Creature? t = null;
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
            bool usable = p.Usage is PotionUsage.CombatOnly or PotionUsage.AnyTime;
            if (!usable) continue;
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

    public static string? DoJson(string json)
    {
        Main.EnsureSelector();
        var doc = System.Text.Json.Nodes.JsonNode.Parse(json)!.AsObject();
        if (doc.ContainsKey("choose"))
            return AgentSelector.Answer(doc["choose"]!.AsArray().Select(x => (int)x!).ToList());
        var rs = RunManager.Instance.DebugOnlyGetState();
        var me = rs != null ? LocalContext.GetMe(rs) : null;
        var cm = CombatManager.Instance;
        if (me == null || !cm.IsInProgress) return "not in combat";
        var cs = cm.DebugOnlyGetState()!;
        var pcs = me.PlayerCombatState!;
        bool ready = pcs.Phase == PlayerTurnPhase.Play && !cm.PlayerActionsDisabled && !cm.IsOverOrEnding
                     && !RunManager.Instance.ActionExecutor.IsRunning && RunManager.Instance.ActionQueueSet.IsEmpty;
        if (!ready) return "game is busy";
        if (doc.ContainsKey("end_turn"))
        {
            RunManager.Instance.ActionQueueSynchronizer.RequestEnqueue(new EndPlayerTurnAction(me, pcs.TurnNumber));
            Snap.LogEndTurn();
            return null;
        }
        Creature? Target(System.Text.Json.Nodes.JsonObject o)
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
            Creature? t = Target(pl);
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

    private static Creature? PickEnemy(CombatState cs, string[] a, out string? err)
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

    private static string? UsePotion(PotionModel p, Player me, CombatState? cs, string[] a)
    {
        if (!me.CanUseOrRemovePotions) return "cannot use potions now";
        Creature? t = null;
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

    private static void Overlay(Decision d, Node top, Player? me)
    {
        d.Kind = top.GetType().Name.TrimStart('N').ToUpperInvariant();
        switch (top)
        {
            case NRewardsScreen s:
                d.Kind = "REWARDS";
                if (s._disableProceedForever) { d.Busy = true; return; }
                foreach (var b in UiHelper.FindAll<NRewardButton>(s).Where(b => b.IsEnabled && b.IsVisibleInTree()))
                {
                    string label = RewardLabel(b.Reward);
                    if (b.Reward is PotionReward && me is { HasOpenPotionSlots: false }) label += " (potion slots full: a dp <slot> first)";
                    d.Click(label, b);
                }
                if (me != null)
                    foreach (var p in me.PotionSlots)
                        if (p is { Usage: PotionUsage.AnyTime })
                            d.Add($"potion {Text.Loc(p.Title)}: {Text.Loc(p.DynamicDescription)}", a => UsePotion(p, me, null, a));
                ProceedOpt(d, s, "proceed (skip the rest)");
                return;
            case NCardRewardSelectionScreen s:
                d.Kind = "CARD_REWARD";
                Holders(d, s);
                foreach (var b in UiHelper.FindAll<NCardRewardAlternativeButton>(s).Where(b => b.IsVisibleInTree()))
                    d.Click(b._optionName ?? Text.NodeLabel(b), b);
                return;
            case NChooseACardSelectionScreen s:
                d.Kind = "CHOOSE_CARD";
                Holders(d, s);
                Skip(d, s);
                return;
            case NChooseARelicSelection s:
                d.Kind = "CHOOSE_RELIC";
                foreach (var h in UiHelper.FindAll<NRelicBasicHolder>(s).Where(h => h.IsVisibleInTree()))
                    d.Click(Relic(h.Relic?.Model), h);
                Skip(d, s);
                return;
            case NChooseABundleSelectionScreen s:
                d.Kind = "CHOOSE_BUNDLE";
                foreach (var bundle in UiHelper.FindAll<NCardBundle>(s))
                    d.Add(string.Join(" + ", bundle.Bundle.Select(c => Text.Card(c))), _ =>
                    {
                        bundle.Hitbox.ForceClick();
                        Fire(ConfirmLater(s));
                        return null;
                    });
                return;
            case NCrystalSphereScreen s:
                CrystalSphere(d, s);
                return;
            case NGameOverScreen s:
                d.Kind = "GAME_OVER";
                d.Info.Append(Text.NodeLabel(s)).Append('\n');
                if (UiHelper.FindFirst<NGameOverContinueButton>(s) is { IsEnabled: true } cont && cont.IsVisibleInTree()) d.Click("continue", cont);
                if (UiHelper.FindFirst<NReturnToMainMenuButton>(s) is { IsEnabled: true } mm && mm.IsVisibleInTree()) d.Click("main menu", mm);
                if (d.Opts.Count == 0) d.Busy = true;
                return;
            default:
                Buttons(d, top);
                return;
        }
    }

    private static void CrystalSphere(Decision d, NCrystalSphereScreen s)
    {
        d.Kind = "CRYSTAL_SPHERE";
        var g = s._entity;
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
                    CrystalSphereRelic => 'R',
                    CrystalSpherePotion => 'P',
                    CrystalSphereCardReward => 'C',
                    CrystalSphereCurse => 'X',
                    CrystalSphereGold => 'g',
                    null => '.',
                    _ => '?',
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
                var cell = s._cellContainer.GetChildren().OfType<NCrystalSphereCell>().FirstOrDefault(c => c.Entity.X == x && c.Entity.Y == y);
                if (cell == null) return $"no cell node at {x},{y}";
                if (!cell.Entity.IsHidden && g.CrystalSphereTool != CrystalSphereMinigame.CrystalSphereToolType.Big) return $"cell {x},{y} is already clear";
                cell.ForceClick();
                return null;
            });
            d.Add("tool big", _ => { s._bigDivinationButton.ForceClick(); return null; });
            d.Add("tool small", _ => { s._smallDivinationButton.ForceClick(); return null; });
        }
        if (s._proceedButton is { IsEnabled: true } p && p.IsVisibleInTree()) d.Click("proceed", p);
    }

    private static async Task ConfirmLater(Node screen)
    {
        await MainThread.Until(() => UiHelper.FindFirst<NConfirmButton>(screen) is { IsEnabled: true } c && c.IsVisibleInTree(), 3000);
        if (UiHelper.FindFirst<NConfirmButton>(screen) is { IsEnabled: true } c2) c2.ForceClick();
    }

    private static void Holders(Decision d, Node s)
    {
        foreach (var h in UiHelper.FindAll<NCardHolder>(s).Where(h => h.CardModel != null && h.IsVisibleInTree()))
            d.Add(Text.Card(h.CardModel!), _ => { h.EmitSignal(NCardHolder.SignalName.Pressed, h); return null; });
    }

    private static void Skip(Decision d, Node s)
    {
        if (UiHelper.FindFirst<NChoiceSelectionSkipButton>(s) is { } b && b.IsVisibleInTree() && b.IsEnabled) d.Click("skip", b);
    }

    private static void ProceedOpt(Decision d, Node n, string label = "proceed")
    {
        if (UiHelper.FindFirst<NProceedButton>(n) is { IsEnabled: true } b && b.IsVisibleInTree()) d.Click(label, b);
    }

    private static string RewardLabel(Reward? r) => r switch
    {
        null => "?",
        CardReward cr => "card: " + string.Join(" | ", cr.Cards.Select(c => c.Title)),
        RelicReward rr => "relic " + Relic(rr.Relic),
        PotionReward pr => "potion " + (pr.Potion is { } p ? $"{Text.Loc(p.Title)}: {Text.Loc(p.DynamicDescription)}" : "?"),
        _ => Text.Loc(r.Description),
    };

    public static string Relic(RelicModel? r) => r == null ? "?" : $"{Text.Loc(r.Title)}: {Text.Loc(r.DynamicDescription)}";

    private static void Buttons(Decision d, Node n)
    {
        foreach (var b in UiHelper.FindAll<NClickableControl>(n).Where(b => b.IsEnabled && b.IsVisibleInTree()).Take(40))
            d.Click($"[{b.GetType().Name.TrimStart('N')}] {Text.NodeLabel(b)}", b);
        foreach (var b in UiHelper.FindAll<BaseButton>(n).Where(b => !b.Disabled && b.IsVisibleInTree()).Take(20))
            d.Add($"[{b.GetType().Name}] {Text.NodeLabel(b)}", _ => { b.EmitSignal(BaseButton.SignalName.Pressed); return null; });
    }

    public static string? DiscardPotion(string? slotArg)
    {
        var rs = RunManager.Instance.DebugOnlyGetState();
        var me = rs != null ? LocalContext.GetMe(rs) : null;
        if (me == null) return "no run";
        if (!int.TryParse(slotArg, out int slot) || slot < 0 || slot >= me.PotionSlots.Count || me.PotionSlots[slot] == null) return "usage: a dp <slot> (0-based, non-empty)";
        if (!me.CanUseOrRemovePotions) return "cannot remove potions now";
        RunManager.Instance.ActionQueueSynchronizer.RequestEnqueue(new DiscardPotionGameAction(me, (uint)slot, CombatManager.Instance.IsInProgress));
        return null;
    }

    private static bool Map(Decision d, NMapScreen map, RunState rs)
    {
        d.Kind = "MAP";
        if (map.IsTraveling) { d.Busy = true; return true; }
        var pts = UiHelper.FindAll<NMapPoint>(map).Where(p => p.IsEnabled && p.State == MapPointState.Travelable).ToList();
        if (pts.Count == 0 || !map.IsTravelEnabled) return false;
        d.Info.Append("full map: m\n");
        foreach (var p in pts.OrderBy(p => p.Point.coord.col))
            d.Click($"{p.Point.PointType} r{p.Point.coord.row}c{p.Point.coord.col} -> " + string.Join(",", p.Point.Children.OrderBy(c => c.coord.col).Select(c => $"{Code(c.PointType)}c{c.coord.col}")), p);
        return true;
    }

    public static string Code(MapPointType t) => t switch
    {
        MapPointType.Monster => "M", MapPointType.Elite => "E", MapPointType.RestSite => "R", MapPointType.Shop => "$",
        MapPointType.Treasure => "T", MapPointType.Unknown => "?", MapPointType.Boss => "B", MapPointType.Ancient => "A", _ => "."
    };

    public static string FullMap()
    {
        var rs = RunManager.Instance.DebugOnlyGetState();
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

    private static void Room(Decision d, RunState rs, Player me)
    {
        var room = rs.CurrentRoom;
        d.Kind = room?.RoomType.ToString().ToUpperInvariant() ?? "ROOM";
        if (room == null || room.RoomType is RoomType.Unassigned or RoomType.Map) { d.Busy = true; return; }
        switch (room?.RoomType)
        {
            case RoomType.Event: Event(d, me); break;
            case RoomType.RestSite: Rest(d); break;
            case RoomType.Shop: Shop(d, me); break;
            case RoomType.Treasure: Treasure(d); break;
            case RoomType.Monster or RoomType.Elite or RoomType.Boss:
                if (NCombatRoom.Instance is { } cr) ProceedOpt(d, cr);
                if (d.Opts.Count == 0) { d.Busy = true; return; }
                break;
        }
        for (int i = 0; i < me.PotionSlots.Count; i++)
            if (me.PotionSlots[i] is { Usage: PotionUsage.AnyTime } p && d.Opts.Count > 0)
                d.Add($"potion {Text.Loc(p.Title)}: {Text.Loc(p.DynamicDescription)}", a => UsePotion(p, me, null, a));
        if (d.Opts.Count == 0)
        {
            var run = Root.GetNodeOrNull("/root/Game/RootSceneContainer/Run/RoomContainer");
            if (run != null) Buttons(d, run);
            if (d.Opts.Count == 0) d.Busy = true;
        }
    }

    private static void Event(Decision d, Player me)
    {
        var room = Root.GetNodeOrNull("/root/Game/RootSceneContainer/Run/RoomContainer/EventRoom");
        if (room == null) return;
        var buttons = UiHelper.FindAll<NEventOptionButton>(room).Where(b => !b.Option.IsLocked && b.IsEnabled && b.IsVisibleInTree()).ToList();
        var ev = buttons.FirstOrDefault()?.Event ?? (RunManager.Instance.DebugOnlyGetState()?.CurrentRoom as EventRoom)?.LocalMutableEvent;
        if (ev != null)
        {
            d.Info.Append(Text.Loc(ev.Title)).Append(": ");
            try
            {
                var desc = ev.Description;
                if (desc != null) { ev.DynamicVars.AddTo(desc); d.Info.Append(Text.Loc(desc)); }
            }
            catch { }
            d.Info.Append('\n');
        }
        foreach (var b in buttons)
        {
            string t = "", ds = "";
            try { b.Event.DynamicVars.AddTo(b.Option.Title); t = Text.Loc(b.Option.Title); } catch { }
            try { b.Event.DynamicVars.AddTo(b.Option.Description); ds = Text.Loc(b.Option.Description); } catch { }
            d.Click(ds.Length > 0 ? $"{t}: {ds}" : t, b);
        }
        if (buttons.Count == 0 && UiHelper.FindFirst<NFakeMerchant>(room) is { } fake && fake.Inventory is { } finv)
        {
            Inventory(d, me, finv, () => fake.Call(NFakeMerchant.MethodName.OpenInventory), fake, UiHelper.FindFirst<NProceedButton>(fake), "leave the merchant");
            return;
        }
        if (buttons.Count == 0 && UiHelper.FindFirst<NAncientEventLayout>(room) is { } anc
            && anc.GetNodeOrNull<NButton>("%DialogueHitbox") is { } hit && hit.Visible && hit.IsEnabled)
            d.Add("continue dialogue", _ => { hit.EmitSignal(NClickableControl.SignalName.Released, hit); return null; });
        if (d.Opts.Count == 0) ProceedOpt(d, room);
    }

    private static void Rest(Decision d)
    {
        if (NRestSiteRoom.Instance is not { } room) return;
        foreach (var b in UiHelper.FindAll<NRestSiteButton>(room).Where(b => b.Option.IsEnabled && b.IsEnabled && b.IsVisibleInTree()))
            d.Click($"{Text.Loc(b.Option.Title)}: {Text.Loc(b.Option.Description)}", b);
        if (room.ProceedButton is { IsEnabled: true } p && p.IsVisibleInTree()) d.Click("proceed", p);
    }

    private static void Shop(Decision d, Player me)
    {
        if (NMerchantRoom.Instance is not { } room) return;
        Inventory(d, me, room.Inventory, room.OpenInventory, room, room.ProceedButton, "leave shop");
    }

    private static void Inventory(Decision d, Player me, NMerchantInventory inv, Action open, Node root, NProceedButton? proceed, string leave)
    {
        foreach (var slot in inv.GetAllSlots().Where(s => s.Entry.IsStocked))
        {
            var e = slot.Entry;
            string what = e switch
            {
                MerchantCardEntry c => "card " + (c.CreationResult?.Card is { } cm ? Text.Card(cm) : "?") + (c.IsOnSale ? " SALE" : ""),
                MerchantRelicEntry r => "relic " + Relic(r.Model),
                MerchantPotionEntry p => "potion " + (p.Model is { } pm ? $"{Text.Loc(pm.Title)}: {Text.Loc(pm.DynamicDescription)}" : "?"),
                MerchantCardRemovalEntry => "remove a card",
                _ => e.GetType().Name,
            };
            d.Add($"{e.Cost}g {what}" + (e.EnoughGold ? "" : " (can't afford)"), _ =>
            {
                if (!e.EnoughGold) return "not enough gold";
                if (e is MerchantPotionEntry && !me.HasOpenPotionSlots) return "potion slots full (a dp <slot> first)";
                if (!inv.IsOpen) open();
                Fire(e.OnTryPurchaseWrapper(inv.Inventory));
                return null;
            });
        }
        if (proceed == null) return;
        d.Add(leave, _ =>
        {
            if (inv.IsOpen && UiHelper.FindFirst<NBackButton>(root) is { } back) back.ForceClick();
            Fire(ProceedLater(proceed));
            return null;
        });
    }

    private static async Task ProceedLater(NProceedButton b)
    {
        await MainThread.Until(() => b.IsEnabled && b.IsVisibleInTree(), 3000);
        if (b.IsEnabled) b.ForceClick();
    }

    private static void Treasure(Decision d)
    {
        if (NRun.Instance?.TreasureRoom is not { } room) return;
        var relics = UiHelper.FindAll<NTreasureRoomRelicHolder>(room).Where(h => h.IsEnabled && h.IsVisibleInTree()).ToList();
        foreach (var h in relics) d.Click("take " + Relic(h.Relic?.Model), h);
        if (relics.Count == 0 && room.GetNodeOrNull<NClickableControl>("Chest") is { IsEnabled: true } chest && chest.IsVisibleInTree()) d.Click("open chest", chest);
        if (room.ProceedButton is { IsEnabled: true } p && p.IsVisibleInTree()) d.Click("proceed", p);
    }

    private static void MainMenu(Decision d)
    {
        d.Kind = "MENU";
        var menu = Root.GetNodeOrNull<Control>("/root/Game/RootSceneContainer/MainMenu");
        if (menu == null || !menu.IsVisibleInTree()) { d.Busy = true; return; }
        var cont = menu.GetNodeOrNull<NButton>("MainMenuTextButtons/ContinueButton");
        var ab = menu.GetNodeOrNull<NButton>("MainMenuTextButtons/AbandonRunButton");
        var sp = menu.GetNodeOrNull<NButton>("MainMenuTextButtons/SingleplayerButton");
        bool Shown(NButton? b) => b != null && b.Visible && b.IsEnabled;
        if (!Shown(cont) && !Shown(sp)) { d.Busy = true; return; }
        if (Shown(cont)) d.Click("continue run", cont!);
        if (Shown(ab)) d.Click("abandon run", ab!);
        if (Shown(sp))
        {
            d.Add("new run  (a <i> <character> [ascension] [seed]; characters: ironclad silent defect regent necrobinder)", a =>
            {
                Fire(NewRun(menu, a.Length > 0 ? a[0] : "ironclad", a.Length > 1 && int.TryParse(a[1], out var asc) ? asc : (int?)null, a.Length > 2 ? a[2] : null));
                return null;
            });
            d.Add("custom run  (a <i> <character> <ascension> <seed> [modifier ids, comma separated; see `mods`])", a =>
            {
                Fire(NewCustomRun(menu, a.Length > 0 ? a[0] : "ironclad", a.Length > 1 && int.TryParse(a[1], out var asc) ? asc : (int?)null, a.Length > 2 ? a[2] : null,
                    a.Length > 3 ? a[3].Split(',', StringSplitOptions.RemoveEmptyEntries) : Array.Empty<string>()));
                return null;
            });
        }
    }

    private static async Task NewCustomRun(Control menu, string character, int? ascension, string? seed, string[] modifiers)
    {
        Main.Log.Info($"AgentBridge: custom run {character} asc={ascension} seed={seed} mods={string.Join(",", modifiers)}", 0);
        menu.GetNode<NButton>("MainMenuTextButtons/SingleplayerButton").ForceClick();
        await MainThread.Until(() => menu.GetNodeOrNull<NButton>("Submenus/SingleplayerSubmenu/CustomRunButton") is { Visible: true }, 5000);
        menu.GetNodeOrNull<NButton>("Submenus/SingleplayerSubmenu/CustomRunButton")?.ForceClick();
        NCustomRunScreen? screen = null;
        await MainThread.Until(() => (screen = UiHelper.FindAll<NCustomRunScreen>(menu).FirstOrDefault(s => s.IsVisibleInTree())) != null, 5000);
        if (screen == null) { Main.Log.Info("AgentBridge: custom run screen not found", 0); return; }
        await MainThread.Until(() => false, 400);
        var btn = UiHelper.FindAll<NCharacterSelectButton>(screen)
            .FirstOrDefault(b => !b.IsLocked && !b.IsRandom && b.Character.Id.Entry.Contains(character, StringComparison.OrdinalIgnoreCase));
        if (btn == null) { Main.Log.Info($"AgentBridge: character {character} not found/locked", 0); return; }
        btn.Select();
        await MainThread.Until(() => false, 300);
        if (ascension is { } asc && screen.GetNodeOrNull<NAscensionPanel>("%AscensionPanel") is { } panel)
            panel.SetAscensionLevel(asc);
        if (seed != null)
        {
            if (screen.GetNodeOrNull<Godot.LineEdit>("%SeedInput") is { } edit) edit.Text = seed;
            screen.Lobby.SetSeed(seed);
        }
        if (modifiers.Length > 0 && screen.GetNodeOrNull<NCustomRunModifiersList>("%ModifiersList") is { } list)
        {
            var chosen = ModelDb.GoodModifiers.Concat(ModelDb.BadModifiers)
                .Where(m => modifiers.Any(x => m.Id.Entry.Contains(x, StringComparison.OrdinalIgnoreCase))).ToList();
            list.SetTickedModifiers(chosen);
        }
        await MainThread.Until(() => false, 300);
        UiHelper.FindAll<NConfirmButton>(screen).FirstOrDefault()?.ForceClick();
    }

    private static async Task NewRun(Control menu, string character, int? ascension, string? seed)
    {
        Main.Log.Info($"AgentBridge: new run {character} asc={ascension}", 0);
        menu.GetNode<NButton>("MainMenuTextButtons/SingleplayerButton").ForceClick();
        await MainThread.Until(() => menu.GetNodeOrNull<Control>("Submenus/CharacterSelectScreen") is { Visible: true }
                                     || menu.GetNodeOrNull<NButton>("Submenus/SingleplayerSubmenu/StandardButton") is { Visible: true }, 5000);
        if (menu.GetNodeOrNull<Control>("Submenus/CharacterSelectScreen") is not { Visible: true })
        {
            menu.GetNodeOrNull<NButton>("Submenus/SingleplayerSubmenu/StandardButton")?.ForceClick();
            await MainThread.Until(() => menu.GetNodeOrNull<Control>("Submenus/CharacterSelectScreen") is { Visible: true }, 5000);
        }
        var screen = menu.GetNode<NCharacterSelectScreen>("Submenus/CharacterSelectScreen");
        var btn = UiHelper.FindAll<NCharacterSelectButton>(screen.GetNode("CharSelectButtons/ButtonContainer"))
            .FirstOrDefault(b => !b.IsLocked && !b.IsRandom && b.Character.Id.Entry.Contains(character, StringComparison.OrdinalIgnoreCase));
        if (btn == null) { Main.Log.Info($"AgentBridge: character {character} not found/locked", 0); return; }
        btn.Select();
        await MainThread.Until(() => false, 200);
        if (ascension is { } asc && screen.GetNodeOrNull<NAscensionPanel>("%AscensionPanel") is { } panel)
            panel.SetAscensionLevel(Math.Min(asc, Math.Max(panel.Ascension, screen.Lobby.MaxAscension)));
        if (seed != null) NGame.Instance!.DebugSeedOverride = seed;
        await MainThread.Until(() => false, 100);
        screen.GetNode<NConfirmButton>("ConfirmButton").ForceClick();
    }
}
