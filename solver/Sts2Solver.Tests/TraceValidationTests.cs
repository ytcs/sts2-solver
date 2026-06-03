using Sts2Solver.Content;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Differential fidelity guard: replays every recorded game trace (data/combat_traces/*.jsonl)
/// through our engine and asserts the numbers match. Traces using content we haven't ported are
/// skipped. If no traces are present, the test no-ops (so CI stays green without the game).
/// </summary>
public class TraceValidationTests
{
    private readonly ITestOutputHelper _out;
    public TraceValidationTests(ITestOutputHelper o) => _out = o;

    private static string TraceDir => Path.Combine(
        Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
        "Projects", "sts2-solver", "data", "combat_traces");

    [Fact]
    public void Engine_Matches_All_Recorded_Game_Traces()
    {
        if (!Directory.Exists(TraceDir)) { _out.WriteLine($"No trace dir ({TraceDir}); skipping."); return; }
        var files = Directory.GetFiles(TraceDir, "*.jsonl").OrderBy(f => f).ToArray();
        if (files.Length == 0) { _out.WriteLine("No traces captured yet; skipping."); return; }

        bool allOk = true;
        foreach (var file in files)
        {
            var rep = TraceValidator.Validate(file);
            foreach (var line in rep.Lines) _out.WriteLine(line);
            _out.WriteLine("");
            allOk &= rep.Ok;
        }
        Assert.True(allOk, "One or more recorded game traces did not match the engine (see test output).");
    }

    /// <summary>Murder scales its damage on <c>1 + cumulative cards drawn this combat</c>, and the game counts the
    /// turn-start hand draw (<c>fromHandDraw</c>) in that total. Trace replay sets the hand directly
    /// (<c>OverrideHand</c> bypasses <c>DrawCards</c>), so before the validator's <c>SeedTurnStartDraw</c> fix the
    /// counter stayed 0 during replay and a Murder deck could not be live-validated. This synthetic one-turn trace
    /// pins the fix: a Silent opens a 5-card hand (Murder + 4 Strikes), so the seeded count is 5 and Murder deals
    /// 1+5 = 6 — exactly lethal to the 6-HP Cultist, ending the combat in a clean win. WITHOUT the seed Murder
    /// would deal 1, the Cultist would survive, and the replay would diverge from the recorded victory — so a
    /// passing report is a direct proof the turn-start draw is counted during replay.</summary>
    [Fact]
    public void Validator_Counts_Turn_Start_Draw_For_Murder()
    {
        const string trace = """
            {"event":"combat_setup","turn":1,"ascension":0,"player":{"name":"The Silent","hp":50,"maxHp":50,"block":0,"energy":0,"maxEnergy":3,"powers":{},"hand":[],"drawPile":["Murder","StrikeSilent","StrikeSilent","StrikeSilent","StrikeSilent"],"discardPile":[],"exhaustPile":[]},"monsters":[{"name":"Calcified Cultist","hp":6,"maxHp":6,"block":0,"alive":true,"powers":{},"nextMoveId":"UNSET_MOVE"}]}
            {"event":"turn_start","turn":1,"player":{"name":"The Silent","hp":50,"maxHp":50,"block":0,"energy":3,"maxEnergy":3,"powers":{},"hand":["Murder","StrikeSilent","StrikeSilent","StrikeSilent","StrikeSilent"],"drawPile":[],"discardPile":[],"exhaustPile":[]},"monsters":[{"name":"Calcified Cultist","hp":6,"maxHp":6,"block":0,"alive":true,"powers":{},"nextMoveId":"UNSET_MOVE"}]}
            {"event":"card_played","turn":1,"card":"Murder","target":"Calcified Cultist","targetId":1,"isAutoPlay":false}
            {"event":"combat_won","turn":1,"player":{"name":"The Silent","hp":50,"maxHp":50,"block":0,"energy":0,"maxEnergy":3,"powers":{},"hand":[],"drawPile":[],"discardPile":[],"exhaustPile":[]},"monsters":[{"name":"Calcified Cultist","hp":0,"maxHp":6,"block":0,"alive":false,"powers":{}}]}
            """;
        var path = Path.Combine(Path.GetTempPath(), $"sts2-murder-trace-{Guid.NewGuid():N}.jsonl");
        File.WriteAllText(path, trace);
        try
        {
            var rep = TraceValidator.Validate(path);
            foreach (var line in rep.Lines) _out.WriteLine(line);
            Assert.True(rep.Ok, "Murder did not reproduce the recorded victory — the turn-start draw was not counted during replay.");
            Assert.Equal(0, rep.Failures);
            Assert.True(rep.Checks >= 1, "expected at least the combat_won player-HP check to run");
            Assert.Contains(rep.Lines, l => l.Contains("combat_won"));
        }
        finally { File.Delete(path); }
    }
}
