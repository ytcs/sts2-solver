using System.Text.Json;
using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>
/// Differential validator: replays a recorded game combat trace (from the DataDumper CombatOracle)
/// through our standalone engine, feeding it the SAME observed random outcomes (the hand drawn each
/// turn and the monster move telegraphed each turn), and asserts the engine reproduces the game's
/// per-turn HP / block / power numbers. Any mismatch is an engine-fidelity bug.
/// </summary>
public static class TraceValidator
{
    public sealed class Report
    {
        public bool Ok = true;
        public readonly List<string> Lines = new();
        public int Checks;
        public int Failures;

        public void Pass(string msg) { Checks++; Lines.Add("  [ok]   " + msg); }
        public void Fail(string msg) { Checks++; Failures++; Ok = false; Lines.Add("  [FAIL] " + msg); }
        public void Info(string msg) => Lines.Add(msg);
    }

    public static Report Validate(string tracePath)
    {
        var r = new Report();
        try { return ValidateCore(tracePath, r); }
        catch (ArgumentException ex)
        {
            // Trace uses content we haven't ported yet — skip it rather than fail the run.
            r.Info($"Trace: {Path.GetFileName(tracePath)}");
            r.Info($"  SKIPPED (unmapped content): {ex.Message}");
            return r;
        }
    }

    private static Report ValidateCore(string tracePath, Report r)
    {
        var events = File.ReadAllLines(tracePath)
            .Where(l => l.Trim().Length > 0)
            .Select(l => JsonDocument.Parse(l).RootElement)
            .ToList();

        var setup = events.First(e => Str(e, "event") == "combat_setup");
        var won = events.FirstOrDefault(e => Str(e, "event") == "combat_won");

        // Player turn-starts are the actionable snapshots: energy reset to full, a freshly drawn hand.
        var playerStarts = events
            .Where(e => Str(e, "event") == "turn_start" && Int(e.GetProperty("player"), "energy") > 0
                        && e.GetProperty("player").GetProperty("hand").GetArrayLength() > 0)
            .ToList();

        r.Info($"Trace: {Path.GetFileName(tracePath)}  ({events.Count} events, {playerStarts.Count} player turns)");

        // ----- Reconstruct the initial combat -----
        var pSetup = setup.GetProperty("player");
        int playerHp = Int(pSetup, "hp"), playerMaxHp = Int(pSetup, "maxHp");
        var deckNames = pSetup.GetProperty("drawPile").EnumerateArray().Select(c => c.GetString()!).ToList();
        var deck = deckNames.Select(Catalog.BuildCard).ToList();
        // Pick the starter relic from the character implied by the deck: Necrobinder runs start with Bound
        // Phylactery (which summons Osty on combat start — essential for the Osty checks), everything else
        // with Burning Blood (the only relic the older Ironclad/Silent traces were recorded under).
        var starterRelic = IsNecrobinderDeck(deckNames) ? "BoundPhylactery" : "BurningBlood";
        var player = Catalog.BuildPlayer(deck, playerHp, playerMaxHp, Int(pSetup, "maxEnergy") is var me && me > 0 ? me : 3,
            relics: new[] { starterRelic });

        // Ascension level the trace was recorded at (absent on pre-A10 traces → 0). Drives monster
        // HP/damage scaling so the engine reproduces the same numbers the game dealt.
        int ascension = setup.TryGetProperty("ascension", out var ascEl) ? ascEl.GetInt32() : 0;
        var monsters = new List<Monster>();
        foreach (var mj in setup.GetProperty("monsters").EnumerateArray())
            monsters.Add(Catalog.BuildMonster(NoSpaces(Str(mj, "name")!), ascension));

        // Monster HP is rolled at combat start, so take the actual values from the first player
        // turn-start snapshot (positionally), not the pre-roll setup numbers.
        if (playerStarts.Count > 0)
        {
            var firstMonsters = playerStarts[0].GetProperty("monsters").EnumerateArray().ToList();
            for (int i = 0; i < monsters.Count && i < firstMonsters.Count; i++)
            {
                monsters[i].MaxHp = Int(firstMonsters[i], "maxHp");
                monsters[i].CurrentHp = Int(firstMonsters[i], "hp");
            }
        }
        var combat = Catalog.SetupCombat(player, monsters);

        // ----- Replay turn by turn -----
        // The recorder snapshots each player turn AFTER its start-of-turn hooks fire (so a card like
        // Crimson Mantle that loses HP / gains block at turn start is already reflected). So we begin the
        // next player turn BEFORE comparing against its snapshot — both the "after enemy Ti" and the
        // "T(i+1) start" checks then see the engine post-start-of-turn. Hence BeginPlayerTurn runs once
        // before the loop (turn 1) and again at the end of each body (the next turn), never at the top.
        CombatManager.BeginPlayerTurn(combat);
        for (int i = 0; i < playerStarts.Count; i++)
        {
            var pts = playerStarts[i];
            int turn = Int(pts, "turn");

            OverrideHand(combat, pts.GetProperty("player").GetProperty("hand"));
            SyncMonsters(combat, pts.GetProperty("monsters"));

            // Verify the state we're entering matches the trace (catches drift from a prior turn).
            CompareState(r, combat, pts, $"T{turn} start");

            // The snapshot the enemy turn will be validated against: the next player-start, else victory.
            JsonElement? anchor = i + 1 < playerStarts.Count ? playerStarts[i + 1]
                                : (won.ValueKind != JsonValueKind.Undefined ? won : null);

            // Apply the player's recorded plays for this turn.
            foreach (var play in PlaysForTurn(events, pts, anchor))
                ApplyPlay(combat, play);

            if (combat.AllMonstersDead)
            {
                CombatManager.OnVictory(combat);
                if (won.ValueKind != JsonValueKind.Undefined) CompareState(r, combat, won, "combat_won");
                break;
            }

            // No following snapshot (trace truncated / combat abandoned) — nothing to validate beyond here.
            if (anchor == null) { r.Info($"  (trace ends at T{turn}; no further ground truth to check)"); break; }

            CombatManager.EndPlayerTurn(combat);
            CombatManager.RunEnemyTurn(combat);
            CombatManager.BeginPlayerTurn(combat);   // next turn's start-of-turn effects, before comparing

            // A start-of-turn effect (Inferno's retaliate, a poison tick, …) can kill the last enemy here,
            // ending the combat at turn start — the recorded victory snapshot then includes the on-victory
            // relic heal (Burning Blood). Resolve victory before comparing, against the won snapshot.
            if (combat.AllMonstersDead)
            {
                CombatManager.OnVictory(combat);
                if (won.ValueKind != JsonValueKind.Undefined) CompareState(r, combat, won, "combat_won");
                break;
            }
            CompareState(r, combat, anchor.Value, $"after enemy T{turn}");
        }

        r.Info($"Result: {(r.Ok ? "PASS" : "FAIL")} — {r.Checks - r.Failures}/{r.Checks} checks passed.");
        return r;
    }

    // ----- Replay helpers -----

    private static void OverrideHand(CombatState combat, JsonElement hand)
    {
        // Feed the observed draw: set the hand to exactly the recorded cards.
        var p = combat.Player;
        foreach (var c in p.Hand) p.DrawPile.Add(c); // return current (unused) hand to a pile to keep card accounting sane
        p.Hand.Clear();
        foreach (var name in hand.EnumerateArray().Select(c => c.GetString()!))
        {
            // Skip cards not yet ported (e.g. a random card generated by Stoke / Infernal Blade from the
            // full pool). They were not built into the hand, so any later play of them is also skipped.
            try { p.Hand.Add(Catalog.BuildCard(name)); }
            catch (ArgumentException) { }
        }
    }

    /// <summary>
    /// Reconcile the engine's monsters with the trace snapshot: set each present monster's committed
    /// (telegraphed) move, and kill any engine monster the game no longer reports — e.g. removed by a
    /// `kill` console command or an Escape, which aren't replayable player actions.
    /// </summary>
    private static void SyncMonsters(CombatState combat, JsonElement monstersJson)
    {
        foreach (var (m, snap) in AlignMonsters(combat, monstersJson))
        {
            var moveId = Str(snap, "nextMoveId");
            if (!string.IsNullOrEmpty(moveId) && moveId != "UNSET_MOVE")
                m.Ai.CurrentMoveId = moveId!;
        }
    }

    /// <summary>
    /// Pair each snapshot monster with the engine's corresponding living monster by walking both lists in
    /// encounter order and matching on name. Any engine monster the snapshot skips over — i.e. the game
    /// removed it (a console <c>kill</c>, an Escape, or a death the engine hasn't applied) — is set to 0 HP
    /// so the engine's living set realigns with the game's. Matching by name (rather than blindly dropping
    /// the LAST engine monster) is what makes a NON-trailing removal correct, e.g. the middle Knight of the
    /// Knights elite being killed while the engine still has it alive — otherwise every later monster's HP
    /// compares against the wrong slot and the whole fight desyncs.
    /// </summary>
    private static List<(Monster monster, JsonElement snap)> AlignMonsters(CombatState combat, JsonElement monstersJson)
    {
        var snaps = monstersJson.EnumerateArray().ToList();
        var living = combat.Monsters.Where(m => m.IsAlive).ToList();
        var pairs = new List<(Monster, JsonElement)>();
        int j = 0;
        foreach (var snap in snaps)
        {
            var name = NoSpaces(Str(snap, "name") ?? "");
            int k = -1;
            for (int t = j; t < living.Count; t++)
                if (NoSpaces(living[t].Name) == name) { k = t; break; }
            if (k < 0) continue;                                   // snapshot lists a monster the engine lacks — skip
            for (int t = j; t < k; t++) living[t].CurrentHp = 0;   // engine monsters the game already removed
            pairs.Add((living[k], snap));
            j = k + 1;
        }
        for (int t = j; t < living.Count; t++) living[t].CurrentHp = 0;   // trailing removals
        return pairs;
    }

    private static void ApplyPlay(CombatState combat, JsonElement play)
    {
        if (play.TryGetProperty("isAutoPlay", out var auto) && auto.GetBoolean()) return; // skip power-triggered plays
        var cardName = Str(play, "card")!;
        var card = combat.Player.Hand.FirstOrDefault(c => c.GetType().Name == cardName);
        if (card == null)
        {
            // Not in the turn-start hand → it was drawn mid-turn (e.g. by Shrug It Off / Pommel Strike).
            // Mid-turn draws are no-ops in replay mode, so construct the played card and add it to hand.
            // (The draw card's own block/damage already resolved; only the played drawn card matters here.)
            try { card = Catalog.BuildCard(cardName); }
            catch { return; }   // unknown/unported card the game played — skip rather than crash
            combat.Player.Hand.Add(card);
        }
        Creature? target = null;
        // Prefer the recorded creature id (disambiguates same-named monsters, e.g. multiple Wrigglers);
        // fall back to name, then to any living monster.
        if (play.TryGetProperty("targetId", out var tid) && tid.ValueKind == JsonValueKind.Number)
            target = combat.Monsters.FirstOrDefault(m => m.Id == tid.GetInt32() && m.IsAlive);
        var tName = Str(play, "target");
        if (target == null && tName != null)
            target = combat.Monsters.FirstOrDefault(m => m.Name.Replace(" ", "") == NoSpaces(tName) && m.IsAlive) ?? combat.LivingMonsters.FirstOrDefault();
        CombatManager.PlayCard(combat, card, target);
    }

    // ----- Comparison -----

    private static void CompareState(Report r, CombatState combat, JsonElement snap, string label)
    {
        var pj = snap.GetProperty("player");
        Check(r, $"{label}: player HP", Int(pj, "hp"), combat.Player.CurrentHp);
        CompareOsty(r, combat, pj, label);

        // Align the engine's living monsters with the snapshot's by name-sequence (robust to a non-trailing
        // monster the game removed but the engine still has alive — see AlignMonsters).
        var aligned = AlignMonsters(combat, snap.GetProperty("monsters"));
        for (int i = 0; i < aligned.Count; i++)
        {
            var (m, mj) = aligned[i];
            // A monster summoned mid-combat (e.g. a Wriggler) has a randomly rolled HP we couldn't know;
            // adopt the observed roll on its first compared snapshot (it spawned stunned, so no combat has
            // touched it yet), then validate its HP normally from here on.
            if (m.NeedsSpawnHpSync) { m.MaxHp = Int(mj, "maxHp"); m.CurrentHp = Int(mj, "hp"); m.NeedsSpawnHpSync = false; }
            Check(r, $"{label}: {m.Name}[{i}] HP", Int(mj, "hp"), m.CurrentHp);
            Check(r, $"{label}: {m.Name}[{i}] block", Int(mj, "block"), m.Block);
            ComparePowers(r, $"{label}: {m.Name}[{i}]", mj.GetProperty("powers"), m);
        }
    }

    /// <summary>Compare Necrobinder's Osty pet. The recorder writes an "osty" field on the player snapshot:
    /// an object while a pet is alive, null/absent when it's missing (dead or never summoned). The engine
    /// keeps the pet on <c>Player.Osty</c> with HP ≤ 0 when missing — so a missing Osty on both sides agrees,
    /// and a one-sided presence is a fidelity bug.</summary>
    private static void CompareOsty(Report r, CombatState combat, JsonElement playerJson, string label)
    {
        bool gameHasOsty = playerJson.TryGetProperty("osty", out var oj) && oj.ValueKind == JsonValueKind.Object;
        var engineOsty = combat.Player.IsOstyAlive ? combat.Player.Osty : null;

        if (!gameHasOsty)
        {
            if (engineOsty != null)
                r.Fail($"{label}: Osty — engine has it alive (HP {engineOsty.CurrentHp}), game reports none");
            return;
        }
        if (engineOsty == null)
        {
            r.Fail($"{label}: Osty — game reports it alive (HP {Int(oj, "hp")}), engine has none");
            return;
        }
        Check(r, $"{label}: Osty HP", Int(oj, "hp"), engineOsty.CurrentHp);
        Check(r, $"{label}: Osty maxHp", Int(oj, "maxHp"), engineOsty.MaxHp);
        Check(r, $"{label}: Osty block", Int(oj, "block"), engineOsty.Block);
        ComparePowers(r, $"{label}: Osty", oj.GetProperty("powers"), engineOsty);
    }

    /// <summary>True if the recorded starting deck is a Necrobinder deck (any card registered in the
    /// Necrobinder card table) — used to pick the Bound Phylactery starter relic so Osty is summoned.</summary>
    private static bool IsNecrobinderDeck(IEnumerable<string> deckNames) =>
        deckNames.Any(n => Catalog.NecrobinderCardFactories.ContainsKey(StripUpgrade(n)));

    private static string StripUpgrade(string spec)
    {
        int plus = spec.IndexOf('+');
        return plus >= 0 ? spec[..plus] : spec;
    }

    private static void ComparePowers(Report r, string label, JsonElement powersJson, Creature creature)
    {
        foreach (var prop in powersJson.EnumerateObject())
        {
            var id = NormalizePowerId(prop.Name);
            Check(r, $"{label} {id}", prop.Value.GetInt32(), creature.GetPowerAmount(id));
        }
        // Flag powers the engine has that the game did not report.
        foreach (var p in creature.Powers)
            if (!powersJson.EnumerateObject().Any(pr => NormalizePowerId(pr.Name) == p.Id) && p.Amount != 0)
                r.Fail($"{label} {p.Id}: engine has {p.Amount}, game reported none");
    }

    private static void Check(Report r, string what, int expected, int actual)
    {
        if (expected == actual) r.Pass($"{what} = {actual}");
        else r.Fail($"{what}: game={expected} engine={actual}");
    }

    // ----- Trace navigation + parsing -----

    private static IEnumerable<JsonElement> PlaysForTurn(List<JsonElement> events, JsonElement turnStart, JsonElement? next)
    {
        int startIdx = events.FindIndex(e => Eq(e, turnStart));
        int endIdx = next.HasValue ? events.FindIndex(e => Eq(e, next.Value)) : events.Count;
        if (endIdx < 0) endIdx = events.Count;
        for (int i = startIdx + 1; i < endIdx; i++)
            if (Str(events[i], "event") == "card_played") yield return events[i];
    }

    private static bool Eq(JsonElement a, JsonElement b) => a.GetRawText() == b.GetRawText();
    private static string? Str(JsonElement e, string name) => e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.String ? v.GetString() : null;
    private static int Int(JsonElement e, string name) => e.TryGetProperty(name, out var v) && v.ValueKind == JsonValueKind.Number ? v.GetInt32() : 0;
    private static string NoSpaces(string s) => s.Replace(" ", "");
    private static string NormalizePowerId(string raw) => raw.EndsWith("Power") ? raw[..^"Power".Length] : raw;
}
