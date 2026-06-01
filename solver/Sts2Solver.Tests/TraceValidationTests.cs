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
}
