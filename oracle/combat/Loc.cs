using System.Globalization;
using System.Reflection;
using System.Text;
using System.Text.Json;
using HarmonyLib;
using MegaCrit.Sts2.Core.Localization;

namespace OracleCombat;

// Real English localization for `serve`: localization/eng/*.json read (never written) from the game's SlayTheSpire2.pck.
public static class Loc
{
    public static bool Real;

    public static string DefaultPck()
    {
        var gameDir = typeof(Loc).Assembly.GetCustomAttributes<AssemblyMetadataAttribute>().FirstOrDefault(a => a.Key == "GameDir")?.Value;
        return Environment.GetEnvironmentVariable("STS2_PCK") ?? Path.Combine(Path.GetDirectoryName(gameDir ?? ".") ?? ".", "SlayTheSpire2.pck");
    }

    public static void Install(string pckPath)
    {
        var tables = new Dictionary<string, LocTable>();
        foreach (var (name, bytes) in ReadPck(pckPath, "localization/eng/", ".json"))
        {
            var data = JsonSerializer.Deserialize<Dictionary<string, string>>(Encoding.UTF8.GetString(bytes));
            string table = Path.GetFileNameWithoutExtension(name);
            tables[table] = new LocTable(table, data);
        }
        if (tables.Count == 0) throw new OracleException("no localization/eng tables in " + pckPath);
        var lm = (LocManager)System.Runtime.CompilerServices.RuntimeHelpers.GetUninitializedObject(typeof(LocManager));
        var t = typeof(LocManager);
        AccessTools.Field(t, "_tables").SetValue(lm, tables);
        AccessTools.Field(t, "_localeChangeCallbacks").SetValue(lm, new List<LocManager.LocaleChangeCallback>());
        AccessTools.Field(t, "_languageKeyCount").SetValue(lm, new Dictionary<string, int>());
        var en = CultureInfo.GetCultureInfo("en");
        AccessTools.Property(t, "Language").SetValue(lm, "eng");
        AccessTools.Property(t, "CultureInfo").SetValue(lm, en);
        AccessTools.Property(t, "StringComparer").SetValue(lm, StringComparer.Create(en, CompareOptions.None));
        AccessTools.Property(t, "ValidationErrors").SetValue(lm, Array.Empty<LocValidationError>());
        AccessTools.Method(t, "LoadLocFormatters").Invoke(lm, null);
        AccessTools.Property(t, "Instance").SetValue(null, lm);
        Real = true;
    }

    // LocManager.SmartFormat as in the shipped game: a formatting error yields the raw text (TestMode rethrows; its `when` filter defeats Harmony)
    public static string Format(LocString l)
    {
        var lm = LocManager.Instance;
        string raw = l.GetRawText();
        var provider = lm.GetTable(l.LocTable).IsLocalKey(l.LocEntryKey) ? lm.CultureInfo : LocManager._englishCultureInfo;
        try { return LocManager._smartFormatter.Format(provider, raw, l._variables); }
        catch (Exception e) when (e is SmartFormat.Core.Formatting.FormattingException || e is SmartFormat.Core.Parsing.ParsingErrors) { return raw; }
    }

    // Godot 4 pack directory (formats 2 and 3), as tools/dashboard/pck.py.
    public static IEnumerable<(string name, byte[] bytes)> ReadPck(string path, string prefix, string suffix)
    {
        using var f = File.OpenRead(path);
        using var r = new BinaryReader(f);
        if (Encoding.ASCII.GetString(r.ReadBytes(4)) != "GDPC") throw new OracleException("not a Godot pack: " + path);
        uint fmt = r.ReadUInt32(); r.ReadUInt32(); r.ReadUInt32(); r.ReadUInt32();
        if (fmt != 2 && fmt != 3) throw new OracleException("unsupported pack format " + fmt);
        uint flags = r.ReadUInt32();
        ulong fileBase = r.ReadUInt64();
        if ((flags & 1) != 0) throw new OracleException("encrypted pack directory");
        if (fmt == 3) f.Seek((long)r.ReadUInt64(), SeekOrigin.Begin);
        else r.ReadBytes(16 * 4);
        ulong bas = (flags & 2) != 0 ? fileBase : 0;
        uint count = r.ReadUInt32();
        var want = new List<(string, ulong, ulong)>();
        for (uint i = 0; i < count; i++)
        {
            int plen = r.ReadInt32();
            string name = Encoding.UTF8.GetString(r.ReadBytes(plen)).TrimEnd('\0');
            ulong off = r.ReadUInt64(), size = r.ReadUInt64();
            r.ReadBytes(16);
            uint ff = r.ReadUInt32();
            if (name.StartsWith("res://")) name = name[6..];
            if ((ff & 3) == 0 && name.StartsWith(prefix) && name.EndsWith(suffix) && name.IndexOf('/', prefix.Length) < 0) want.Add((name, bas + off, size));
        }
        foreach (var (name, off, size) in want)
        {
            f.Seek((long)off, SeekOrigin.Begin);
            yield return (name, r.ReadBytes((int)size));
        }
    }
}
