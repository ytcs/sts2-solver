// Emits golden RNG vectors from the game's own Rng/StringHelper so the Rust port can be checked bit-for-bit.
using System.Text.Json;
using MegaCrit.Sts2.Core.Extensions;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Random;

var outPath = args.Length > 0 ? args[0] : "rng.json";
var names = new[] { "shuffle", "rewards", "combat_card_generation", "monster_ai", "", "a", "UNDERDOCKS_NORMAL_1", "ünïcode" };
var hashes = names.Select(n => new { name = n, hash = StringHelper.GetDeterministicHashCode(n).ToString() }).ToList();

var seeds = new ulong[] { 0, 1, 42, 123456789, ulong.MaxValue, 0xDEADBEEFCAFEBABE };
var streams = new List<object>();
foreach (var seed in seeds)
foreach (var name in new[] { "shuffle", "monster_ai" })
{
    var r = new Rng(seed, name);
    var ints = new List<int>(); var rng2 = new List<int>(); var dbl = new List<string>(); var flt = new List<string>(); var bools = new List<bool>(); var ulongs = new List<string>();
    for (int i = 0; i < 16; i++) ints.Add(r.NextInt(10));
    for (int i = 0; i < 16; i++) rng2.Add(r.NextInt(3, 40));
    for (int i = 0; i < 8; i++) dbl.Add(r.NextDouble().ToString("R"));
    for (int i = 0; i < 8; i++) flt.Add(BitConverter.SingleToInt32Bits(r.NextFloat()).ToString());
    for (int i = 0; i < 8; i++) bools.Add(r.NextBool());
    for (int i = 0; i < 4; i++) ulongs.Add(r.NextUnsignedLong().ToString());
    for (int i = 0; i < 4; i++) ulongs.Add(r.NextUnsignedLong(1000).ToString());
    var ser = r.ToSerializable();
    streams.Add(new { seed = seed.ToString(), name, ints, ranged = rng2, doubles = dbl, float_bits = flt, bools, ulongs, counter = ser.counter });
}

var shuffles = new List<object>();
foreach (var seed in seeds)
foreach (var n in new[] { 0, 1, 2, 5, 10, 17, 30 })
{
    var r = new Rng(seed, "shuffle");
    var list = Enumerable.Range(0, n).ToList();
    list.UnstableShuffle(r);
    var after = r.NextInt(1000);
    shuffles.Add(new { seed = seed.ToString(), n, order = list, next_after = after });
    var r2 = new Rng(seed, "shuffle"); var l2 = Enumerable.Range(0, n).ToList(); r2.Shuffle(l2);
    if (!l2.SequenceEqual(list)) throw new Exception("Shuffle != UnstableShuffle");
}

var weighted = new List<object>();
foreach (var seed in seeds)
{
    var r = new Rng(seed, "monster_ai");
    var w = new float[] { 0.3f, 0.5f, 0.2f, 1.0f };
    var picks = new List<int>();
    for (int i = 0; i < 12; i++) picks.Add(r.WeightedNextItem(Enumerable.Range(0, 4), k => w[k]));
    weighted.Add(new { seed = seed.ToString(), picks });
}

File.WriteAllText(outPath, JsonSerializer.Serialize(new { hashes, streams, shuffles, weighted }, new JsonSerializerOptions { WriteIndented = false }));
Console.WriteLine($"wrote {outPath}");
