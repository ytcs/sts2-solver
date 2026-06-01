using System.Reflection;
using System.Text.Json;
using HarmonyLib;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Runs;

namespace DataDumper.DataDumperCode;

/// <summary>
/// Passive combat-trace recorder for differential validation of the standalone solver's engine.
///
/// Subscribes to CombatManager lifecycle events and dumps a full state snapshot at every turn
/// boundary, plus a log of each card played (the player's actions) and the monster's telegraphed
/// move. The standalone engine is later fed the SAME actions + observed random outcomes (drawn hand,
/// monster move) and its per-turn snapshots are diffed against this ground-truth trace.
///
/// Output: ~/Projects/sts2-solver/data/combat_traces/combat-&lt;timestamp&gt;.jsonl (one JSON event per line).
/// </summary>
public static class CombatOracle
{
    private const BindingFlags ALL = BindingFlags.Public | BindingFlags.NonPublic |
                                     BindingFlags.Instance | BindingFlags.Static;

    // Overridable via STS2_TRACE_DIR so headless batch runs (which may use an isolated HOME to protect
    // real saves) still write traces to the project's data folder.
    public static readonly string TraceDir =
        Environment.GetEnvironmentVariable("STS2_TRACE_DIR") is { Length: > 0 } dir
            ? dir
            : Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
                "Projects", "sts2-solver", "data", "combat_traces");

    private static StreamWriter? _writer;

    /// <summary>True once the current combat's trace has been fully written and the file closed.
    /// The headless batch waits on this before quitting so the victory snapshot is never lost.</summary>
    public static volatile bool CombatClosed;
    private static readonly JsonSerializerOptions JsonOpts = new() { WriteIndented = false };

    public static void Install()
    {
        Directory.CreateDirectory(TraceDir);

        var cm = CombatManager.Instance;
        cm.CombatSetUp += OnCombatSetUp;
        cm.TurnStarted += s => WriteSnapshot("turn_start", s);
        cm.TurnEnded   += s => WriteSnapshot("turn_end", s);
        cm.CombatWon   += _ => OnCombatEnd("combat_won");
        cm.CombatEnded += _ => OnCombatEnd("combat_ended");

        // One small Harmony patch to capture the player's actions (card + target) as they happen.
        var harmony = new Harmony("DataDumper.CombatOracle");
        var onPlay = typeof(CardModel).GetMethod("OnPlayWrapper", ALL);
        if (onPlay != null)
            harmony.Patch(onPlay, prefix: new HarmonyMethod(typeof(CombatOracle).GetMethod(nameof(OnPlayPrefix), ALL)));

        MainFile.Logger.Info($"CombatOracle installed. Traces -> {TraceDir}", 0);
    }

    // ── Event handlers ────────────────────────────────────────────────────────

    private static void OnCombatSetUp(CombatState state)
    {
        try
        {
            _writer?.Flush();
            _writer?.Dispose();
            CombatClosed = false;
            var name = $"combat-{DateTime.Now:yyyyMMdd-HHmmss-fff}.jsonl";
            _writer = new StreamWriter(Path.Combine(TraceDir, name), append: false) { AutoFlush = true };
            WriteSnapshot("combat_setup", state);
        }
        catch (Exception e) { MainFile.Logger.Info($"CombatOracle setup error: {e}", 0); }
    }

    private static void OnCombatEnd(string evt)
    {
        try
        {
            var state = CombatManager.Instance.DebugOnlyGetState();
            if (state != null) WriteSnapshot(evt, state);
            _writer?.Flush();
            _writer?.Dispose();
            _writer = null;
            CombatClosed = true;
        }
        catch (Exception e) { MainFile.Logger.Info($"CombatOracle end error: {e}", 0); }
    }

    // Harmony prefix on CardModel.OnPlayWrapper — fires as each card is played.
    private static void OnPlayPrefix(CardModel __instance, Creature? target, bool isAutoPlay)
    {
        try
        {
            var state = CombatManager.Instance.DebugOnlyGetState();
            var rec = new Dictionary<string, object?>
            {
                ["event"] = "card_played",
                ["turn"] = TurnNumber(state),
                ["card"] = CardName(__instance),
                ["target"] = target != null ? Member<string>(target, "Name") : null,
                ["targetId"] = target != null ? (object?)Member<uint?>(target, "CombatId") : null,
                ["isAutoPlay"] = isAutoPlay,
            };
            WriteLine(rec);
        }
        catch (Exception e) { MainFile.Logger.Info($"CombatOracle play error: {e}", 0); }
    }

    // ── Snapshot ──────────────────────────────────────────────────────────────

    private static void WriteSnapshot(string evt, CombatState state)
    {
        try
        {
            var rec = new Dictionary<string, object?>
            {
                ["event"] = evt,
                ["side"] = state.CurrentSide.ToString(),
                ["round"] = Member<int>(state, "RoundNumber"),
                ["turn"] = TurnNumber(state),
                ["ascension"] = AscensionLevel(),
                ["player"] = SnapshotPlayer(state),
                ["monsters"] = SnapshotMonsters(state),
            };
            WriteLine(rec);
        }
        catch (Exception e) { MainFile.Logger.Info($"CombatOracle snapshot error: {e}", 0); }
    }

    private static Dictionary<string, object?>? SnapshotPlayer(CombatState state)
    {
        var creatures = Member<System.Collections.IEnumerable>(state, "Allies");
        if (creatures == null) return null;
        foreach (var c in creatures)
        {
            if (c == null) continue;
            var player = Member<object>(c, "Player");
            if (player == null) continue;
            var pcs = Member<object>(player, "PlayerCombatState");
            return new Dictionary<string, object?>
            {
                ["name"] = Member<string>(c, "Name"),
                ["hp"] = Member<int>(c, "CurrentHp"),
                ["maxHp"] = Member<int>(c, "MaxHp"),
                ["block"] = Member<int>(c, "Block"),
                ["energy"] = pcs != null ? (object?)Member<int>(pcs, "Energy") : null,
                ["maxEnergy"] = pcs != null ? (object?)Member<int>(pcs, "MaxEnergy") : null,
                ["powers"] = Powers(c),
                ["hand"] = PileCards(pcs, "Hand"),
                ["drawPile"] = PileCards(pcs, "DrawPile"),
                ["discardPile"] = PileCards(pcs, "DiscardPile"),
                ["exhaustPile"] = PileCards(pcs, "ExhaustPile"),
            };
        }
        return null;
    }

    private static List<object?> SnapshotMonsters(CombatState state)
    {
        var list = new List<object?>();
        var enemies = Member<System.Collections.IEnumerable>(state, "Enemies");
        if (enemies == null) return list;
        foreach (var c in enemies)
        {
            if (c == null) continue;
            var monster = Member<object>(c, "Monster");
            var nextMove = monster != null ? Member<object>(monster, "NextMove") : null;
            list.Add(new Dictionary<string, object?>
            {
                ["name"] = Member<string>(c, "Name"),
                ["hp"] = Member<int>(c, "CurrentHp"),
                ["maxHp"] = Member<int>(c, "MaxHp"),
                ["block"] = Member<int>(c, "Block"),
                ["alive"] = Member<bool>(c, "IsAlive"),
                ["powers"] = Powers(c),
                ["nextMoveId"] = nextMove != null ? Member<string>(nextMove, "Id") : null,
            });
        }
        return list;
    }

    private static Dictionary<string, object?> Powers(object creature)
    {
        var result = new Dictionary<string, object?>();
        var powers = Member<System.Collections.IEnumerable>(creature, "Powers");
        if (powers == null) return result;
        foreach (var p in powers)
        {
            if (p == null) continue;
            result[p.GetType().Name] = Member<int>(p, "Amount");
        }
        return result;
    }

    private static List<object?> PileCards(object? pcs, string pileName)
    {
        var result = new List<object?>();
        if (pcs == null) return result;
        var pile = Member<object>(pcs, pileName);
        var cards = pile != null ? Member<System.Collections.IEnumerable>(pile, "Cards") : null;
        if (cards == null) return result;
        foreach (var card in cards)
            if (card != null) result.Add(CardName(card));
        return result;
    }

    /// <summary>Card class name with an upgrade suffix (e.g. "StrikeIronclad+1") so the solver's
    /// <c>BuildCard</c> (which parses "+N") replays upgrades — Armaments / Dismantle upgrade cards in hand,
    /// changing their later block/damage; without this the solver would replay the base values and drift.</summary>
    private static string CardName(object card)
    {
        var name = card.GetType().Name;
        int up = Member<int>(card, "CurrentUpgradeLevel");
        return up > 0 ? $"{name}+{up}" : name;
    }

    /// <summary>The run's ascension level (0–10), so the solver can build monsters with the right
    /// ToughEnemies/DeadlyEnemies scaling for this trace. 0 if unavailable.</summary>
    private static int AscensionLevel()
    {
        try { return RunManager.Instance.State.AscensionLevel; } catch { return 0; }
    }

    private static int TurnNumber(CombatState? state)
    {
        if (state == null) return -1;
        var players = Member<System.Collections.IEnumerable>(state, "Players");
        if (players == null) return -1;
        foreach (var pl in players)
        {
            var pcs = pl != null ? Member<object>(pl, "PlayerCombatState") : null;
            if (pcs != null) return Member<int>(pcs, "TurnNumber");
        }
        return -1;
    }

    // ── IO + reflection ─────────────────────────────────────────────────────

    private static void WriteLine(Dictionary<string, object?> rec)
    {
        if (_writer == null) return;
        _writer.WriteLine(JsonSerializer.Serialize(rec, JsonOpts));
    }

    /// <summary>Read a property or field by name, tolerant of access modifiers.</summary>
    private static T? Member<T>(object? obj, string name)
    {
        if (obj == null) return default;
        try
        {
            var t = obj.GetType();
            var pi = t.GetProperty(name, ALL);
            if (pi != null) return (T?)pi.GetValue(obj);
            var fi = t.GetField(name, ALL);
            if (fi != null) return (T?)fi.GetValue(obj);
        }
        catch { }
        return default;
    }
}
