using System.Text.Json;
using System.Text.Json.Nodes;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Commands;
using OracleCombat;

return Cli.Main(args);

static class Cli
{
    static string Usage = @"usage:
  OracleCombat run <scenario.json> [--out trace.jsonl] [--random SEED] [--record scenario_with_script.json] [opts]
  OracleCombat fuzz --encounters ALL|ID,ID --character IRONCLAD --seeds A-B [--out-dir DIR] [--keep-all] [--extra-cards N] [--extra-relics N] [--extra-potions N] [--ascension N] [--starter-only] [opts]
opts: --max-steps N  --max-rounds N  --lenient (do not abort on game Log.Error)  --verbose";

    public static int Main(string[] args)
    {
        if (args.Length == 0) { Console.Error.WriteLine(Usage); return 2; }
        string cmd = args[0];
        var kv = new Dictionary<string, string>(); var flags = new HashSet<string>(); string positional = null;
        for (int i = 1; i < args.Length; i++)
        {
            if (!args[i].StartsWith("--")) { positional = args[i]; continue; }
            string k = args[i][2..];
            if (k is "lenient" or "verbose" or "keep-all" or "starter-only") flags.Add(k);
            else kv[k] = args[++i];
        }
        Fatal.Lenient = flags.Contains("lenient");
        var pump = Pump.Install();
        Boot.Init(flags.Contains("verbose"));
        int maxSteps = kv.TryGetValue("max-steps", out var ms) ? int.Parse(ms) : 400;
        int maxRounds = kv.TryGetValue("max-rounds", out var mr) ? int.Parse(mr) : 60;
        if (cmd == "run")
        {
            if (positional == null) { Console.Error.WriteLine(Usage); return 2; }
            var sc = Scenario.Load(positional);
            string outPath = kv.GetValueOrDefault("out");
            using var w = outPath != null ? new StreamWriter(outPath, false, new System.Text.UTF8Encoding(false)) { NewLine = "\n" } : new StreamWriter(Console.OpenStandardOutput()) { NewLine = "\n" };
            int? rs = kv.TryGetValue("random", out var r) ? int.Parse(r) : null;
            Driver.PolicyKind = kv.GetValueOrDefault("policy", "random");
            var res = RunOne(sc, w, pump, rs, maxSteps, maxRounds);
            if (kv.TryGetValue("record", out var recordPath))
            {
                var o = (JsonObject)sc.Raw.DeepClone();
                o["script"] = new JsonArray(res.Recorded.Select(Scenario.ActionToJson).ToArray());
                o["result"] = res.Result;
                File.WriteAllText(recordPath, o.ToJsonString() + "\n");
            }
            Console.Error.WriteLine($"result={res.Result} actions={res.Recorded.Count}" + (res.Error != null ? " ERROR: " + res.Error : ""));
            return res.Error == null ? 0 : 1;
        }
        if (cmd == "dump-rng")
        {
            var set = new MegaCrit.Sts2.Core.Runs.RunRngSet(positional);
            var o = new JsonObject { ["seed"] = positional, ["seed_u64"] = set.Seed, ["rng"] = new JsonObject() };
            foreach (var (name, type) in Setup.Streams) o["rng"][name] = Dump.RngState(set.GetRng(type));
            if (kv.TryGetValue("out", out var op)) File.WriteAllText(op, o.ToJsonString() + "\n"); else Console.WriteLine(o.ToJsonString());
            return 0;
        }
        if (cmd == "check-shuffle")
        {
            var sc = Scenario.Load(positional);
            var rec = JsonNode.Parse(File.ReadLines(kv["trace"]).First()).AsObject();
            var deck = sc.Deck.Select(c => c.Id).ToList();
            ulong seed = MegaCrit.Sts2.Core.Helpers.StringHelper.GetDeterministicHashCode(sc.Seed);
            var rng = new MegaCrit.Sts2.Core.Random.Rng(seed, "shuffle");
            MegaCrit.Sts2.Core.Extensions.ListExtensions.UnstableShuffle(deck, rng);
            var actual = rec["hand"].AsArray().Concat(rec["draw"].AsArray()).Select(c => (string)c["id"]).ToList();
            bool ok = deck.SequenceEqual(actual);
            int shuf = (int)rec["rng"]["shuffle"]["counter"];
            Console.WriteLine($"shuffle order match: {ok}; shuffle counter at record 0 = {shuf} (expected {sc.Deck.Count - 1} if nothing else drew); niche counter = {(int)rec["rng"]["niche"]["counter"]}; enemies = {rec["enemies"].AsArray().Count}");
            return ok ? 0 : 1;
        }
        if (cmd == "catalog")
        {
            File.WriteAllText(kv["out"], Catalog.Build().ToJsonString() + "\n");
            return 0;
        }
        if (cmd == "dump-pools")
        {
            File.WriteAllText(kv["out"], Pools.Dump().ToJsonString() + "\n");
            return 0;
        }
        if (cmd == "batch" && !kv.ContainsKey("list") && !File.Exists(positional ?? "")) return DoBatch(positional, kv, pump, maxSteps, maxRounds);
        if (cmd == "list-meta")
        {
            var txt = Fuzz.ListMeta().ToJsonString(new JsonSerializerOptions { WriteIndented = false });
            if (kv.TryGetValue("out", out var lp)) File.WriteAllText(lp, txt + "\n"); else Console.WriteLine(txt);
            return 0;
        }
        if (cmd == "batch")
        {
            if (kv.ContainsKey("list"))
            {
            var paths = File.ReadAllLines(kv["list"]).Where(l => l.Trim().Length > 0).ToList();
            int pb = int.Parse(kv.GetValueOrDefault("policy-base", "0"));
            PlayBiasArg = double.Parse(kv.GetValueOrDefault("play-bias", "0"), System.Globalization.CultureInfo.InvariantCulture);
            int nbad = 0;
            for (int i = 0; i < paths.Count; i++)
            {
                string bp = paths[i].Replace(".scenario.json", "");
                RunResult res;
                try
                {
                    var sc = Scenario.Load(paths[i]);
                    using var w = new StreamWriter(bp + ".jsonl", false, new System.Text.UTF8Encoding(false)) { NewLine = "\n" };
                    res = RunOne(sc, w, pump, pb + i, maxSteps, maxRounds);
                }
                catch (Exception e) { res = new RunResult("exception", new List<ActionSpec>(), e.ToString()); }
                if (res.Error != null) { nbad++; File.WriteAllText(bp + ".err", res.Error + "\n"); }
                Console.WriteLine($"{(res.Error == null ? "ok " : "ERR")} {bp} {res.Result}");
            }
            Console.WriteLine($"batch done: {paths.Count} runs, {nbad} errors");
            }
            else
            {
            foreach (var line in File.ReadLines(positional))
            {
                var path = line.Trim();
                if (path.Length == 0) continue;
                string bas = path.EndsWith(".scenario.json") ? path[..^".scenario.json".Length] : path;
                try
                {
                    var sc = Scenario.Load(path);
                    RunResult res;
                    using (var w = new StreamWriter(bas + ".jsonl", false, new System.Text.UTF8Encoding(false)) { NewLine = "\n" })
                        res = RunOne(sc, w, pump, null, maxSteps, maxRounds);
                    if (res.Error != null) File.WriteAllText(bas + ".error.txt", res.Error);
                    File.WriteAllText(bas + ".done", res.Result);
                    Console.WriteLine($"{(res.Error == null ? "ok " : "ERR")} {path} {res.Result} {res.Recorded.Count}");
                }
                catch (Exception e) { File.WriteAllText(bas + ".error.txt", e.ToString()); File.WriteAllText(bas + ".done", "error"); Console.WriteLine("ERR " + path); }
            }
            }
            return 0;
        }
        if (cmd == "fuzz") return DoFuzz(kv, flags, pump, maxSteps, maxRounds);
        Console.Error.WriteLine(Usage);
        return 2;
    }

    record RunResult(string Result, List<ActionSpec> Recorded, string Error);

    static double PlayBiasArg = 0;
    static RunResult RunOne(Scenario sc, TextWriter w, Pump pump, int? randomSeed, int maxSteps, int maxRounds)
    {
        Fatal.Message = null; Fatal.Warnings.Clear();
        var driver = new Driver(sc, w, pump) { MaxSteps = maxSteps, MaxRounds = maxRounds, PlayBias = PlayBiasArg };
        if (sc.Raw["policy"] is JsonObject po)
        {
            if (po["endw"] != null) driver.EndWeight = (double)po["endw"];
            if (po["atkw"] != null) driver.AttackWeight = (double)po["atkw"];
            if (po["potw"] != null) driver.PotionWeight = (double)po["potw"];
            if (po["max_steps"] != null) driver.MaxSteps = (int)po["max_steps"];
            if (po["max_rounds"] != null) driver.MaxRounds = (int)po["max_rounds"];
            if (po["seed"] != null && !randomSeed.HasValue) randomSeed = (int)po["seed"];
        }
        if (randomSeed.HasValue) driver.RandomDriver = new Random(randomSeed.Value);
        string err = null;
        try { driver.Run(); }
        catch (Exception e) { err = e is OracleException ? e.Message : e.ToString(); }
        w.Flush();
        try { driver.Dispose(); } catch (Exception e) { err ??= "cleanup: " + e.Message; }
        return new RunResult(driver.Result, driver.Recorded, err);
    }

    static int DoBatch(string dir, Dictionary<string, string> kv, Pump pump, int maxSteps, int maxRounds)
    {
        var files = Directory.GetFiles(dir, "*.scenario.json").OrderBy(f => f, StringComparer.Ordinal).ToList();
        int bad = 0;
        foreach (var f in files)
        {
            string stem = f[..^".scenario.json".Length];
            if (File.Exists(stem + ".res") || File.Exists(stem + ".err")) continue;
            string err = null; RunResult res = null;
            try
            {
                var sc = Scenario.Load(f);
                var pol = sc.Raw["policy"]?.AsObject();
                int? seed = pol == null ? null : (pol["seed"]?.GetValue<int>() ?? 1);
                Driver.PolicyKind = (string)pol?["kind"] ?? "random";
                int ms = pol?["max_steps"]?.GetValue<int>() ?? maxSteps;
                int mr = pol?["max_rounds"]?.GetValue<int>() ?? maxRounds;
                using var w = new StreamWriter(stem + ".jsonl", false, new System.Text.UTF8Encoding(false)) { NewLine = "\n" };
                res = RunOne(sc, w, pump, seed, ms, mr);
                err = res.Error;
            }
            catch (Exception e) { err = e.ToString(); }
            if (err != null) { bad++; File.WriteAllText(stem + ".err", err + "\n"); }
            else File.WriteAllText(stem + ".res", $"{res.Result} {res.Recorded.Count}\n");
        }
        Console.WriteLine($"batch done: {files.Count} scenarios, {bad} oracle errors");
        return 0;
    }

    static int DoFuzz(Dictionary<string, string> kv, HashSet<string> flags, Pump pump, int maxSteps, int maxRounds)
    {
        string character = kv.GetValueOrDefault("character", "IRONCLAD").ToUpperInvariant();
        string encArg = kv.GetValueOrDefault("encounters", "ALL");
        var encs = encArg == "ALL" ? Fuzz.AllEncounterIds().ToList() : encArg.Split(',').Select(s => Scenario.StripCat(s).ToUpperInvariant()).ToList();
        var sp = kv.GetValueOrDefault("seeds", "1-1").Split('-');
        int s0 = int.Parse(sp[0]), s1 = int.Parse(sp.Length > 1 ? sp[1] : sp[0]);
        int ec = int.Parse(kv.GetValueOrDefault("extra-cards", "8")), er = int.Parse(kv.GetValueOrDefault("extra-relics", "2")), ep = int.Parse(kv.GetValueOrDefault("extra-potions", "2"));
        int asc = int.Parse(kv.GetValueOrDefault("ascension", "0"));
        string dir = kv.GetValueOrDefault("out-dir", "fuzz_out");
        Directory.CreateDirectory(dir);
        int bad = 0, n = 0;
        foreach (var enc in encs)
            for (int seed = s0; seed <= s1; seed++)
            {
                var o = Fuzz.Make(character, enc, seed, ec, er, ep, asc, flags.Contains("starter-only"));
                var sc = Scenario.Parse(o);
                string name = (string)o["name"];
                string tracePath = Path.Combine(dir, name + ".jsonl");
                RunResult res;
                var sw = System.Diagnostics.Stopwatch.StartNew();
                using (var w = new StreamWriter(tracePath, false, new System.Text.UTF8Encoding(false)) { NewLine = "\n" })
                    res = RunOne(sc, w, pump, seed, maxSteps, maxRounds);
                n++;
                bool ok = res.Error == null;
                if (!ok) bad++;
                Console.WriteLine($"{(ok ? "ok " : "ERR")} {name} result={res.Result} actions={res.Recorded.Count} {sw.ElapsedMilliseconds}ms" + (ok ? "" : " :: " + res.Error.Split('\n')[0]));
                if (ok && !flags.Contains("keep-all")) { File.Delete(tracePath); }
                else
                {
                    o["script"] = new JsonArray(res.Recorded.Select(Scenario.ActionToJson).ToArray());
                    o["result"] = res.Result;
                    if (!ok) o["error"] = res.Error;
                    File.WriteAllText(Path.Combine(dir, name + ".scenario.json"), o.ToJsonString() + "\n");
                }
            }
        Console.WriteLine($"fuzz done: {n} runs, {bad} errors");
        return bad == 0 ? 0 : 1;
    }
}
