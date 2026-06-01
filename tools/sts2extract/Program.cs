using System.Reflection;
using System.Text;
using Newtonsoft.Json;

// ─── Configuration ──────────────────────────────────────────────────────────
var gameDir = "/home/ytc/.local/share/Steam/steamapps/common/Slay the Spire 2/data_sts2_linuxbsd_x86_64";
var outDir  = "/home/ytc/Projects/sts2-solver/data";
Directory.CreateDirectory(outDir);

var resolver = new PathAssemblyResolver(Directory.GetFiles(gameDir, "*.dll"));
using var ctx = new MetadataLoadContext(resolver);
var asm = ctx.LoadFromAssemblyPath(Path.Combine(gameDir, "sts2.dll"));
var allTypes = asm.GetTypes();

// ─── Helper utilities ────────────────────────────────────────────────────────

static string TypeStr(Type? t) =>
    t == null ? "null" : (t.IsGenericType
        ? $"{t.Name.Split('`')[0]}<{string.Join(",", t.GetGenericArguments().Select(TypeStr))}>"
        : t.Name);

static string FieldsSummary(Type t) =>
    string.Join("\n", t.GetFields(BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.DeclaredOnly)
        .Where(f => !f.Name.Contains("<") && !f.Name.Contains(">"))
        .Select(f => $"  {TypeStr(f.FieldType),-40} {f.Name}"));

static string PropsSummary(Type t) =>
    string.Join("\n", t.GetProperties(BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.DeclaredOnly)
        .Select(p => $"  {TypeStr(p.PropertyType),-40} {p.Name}"));

// Collect all public methods (name + signature) for a type
static string MethodsSummary(Type t) =>
    string.Join("\n", t.GetMethods(BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance | BindingFlags.DeclaredOnly)
        .Where(m => !m.IsSpecialName && !m.Name.Contains("<") && !m.Name.Contains(">"))
        .Select(m => $"  {TypeStr(m.ReturnType),-30} {m.Name}({string.Join(", ", m.GetParameters().Select(p => TypeStr(p.ParameterType) + " " + p.Name))})"));

// ─── Output builder ───────────────────────────────────────────────────────────

var sb = new StringBuilder();
void W(string s = "") { sb.AppendLine(s); }
void H1(string s) { W(); W($"{'='*80}"); W($"  {s}"); W($"{'='*80}"); }
void H2(string s) { W(); W($"{'─'*60}"); W($"  {s}"); W($"{'─'*60}"); }
void DumpType(Type t, bool withMethods = false)
{
    W($"\nType: {t.FullName}");
    if (t.BaseType != null) W($"Base: {TypeStr(t.BaseType)}");
    var ifaces = t.GetInterfaces();
    if (ifaces.Length > 0) W($"Impl: {string.Join(", ", ifaces.Select(i => TypeStr(i)))}");
    var fields = FieldsSummary(t);
    if (fields.Length > 0) { W("Fields:"); W(fields); }
    var props = PropsSummary(t);
    if (props.Length > 0) { W("Properties:"); W(props); }
    if (withMethods)
    {
        var methods = MethodsSummary(t);
        if (methods.Length > 0) { W("Methods:"); W(methods); }
    }
}

// ─── 1. All type names by namespace (index) ──────────────────────────────────

H1("ALL TYPES BY NAMESPACE");
foreach (var grp in allTypes.GroupBy(t => t.Namespace ?? "(none)").OrderBy(g => g.Key))
{
    W($"\n[{grp.Key}]");
    foreach (var t in grp.OrderBy(t => t.Name))
        W($"  {t.Name}");
}

// ─── 2. CardData / CardDefinition types ──────────────────────────────────────

H1("CARD DATA TYPES");
var cardTypes = allTypes.Where(t =>
    t.Name.Contains("Card") &&
    !t.Name.StartsWith("<") &&
    !t.Name.Contains("Node") &&
    !t.Name.Contains("Screen") &&
    !t.Name.Contains("Button")).ToList();

foreach (var t in cardTypes.OrderBy(t => t.FullName))
    DumpType(t, withMethods: true);

// ─── 3. CardModel specifically ────────────────────────────────────────────────

H1("CardModel");
var cardModel = allTypes.FirstOrDefault(t => t.Name == "CardModel");
if (cardModel != null) DumpType(cardModel, withMethods: true);

// ─── 4. Effect / Action types (what cards DO) ────────────────────────────────

H1("EFFECT / CARD ACTION TYPES");
var effectTypes = allTypes.Where(t =>
    (t.Name.Contains("Effect") || t.Name.Contains("Action") || t.Name.Contains("Command")) &&
    !t.Name.StartsWith("<") &&
    !t.Name.Contains("Node") &&
    !t.Name.Contains("Screen")).ToList();

foreach (var t in effectTypes.OrderBy(t => t.FullName))
    DumpType(t, withMethods: true);

// ─── 5. Creature / Enemy types ───────────────────────────────────────────────

H1("CREATURE / ENEMY TYPES");
var creatureTypes = allTypes.Where(t =>
    (t.Name.Contains("Creature") || t.Name.Contains("Enemy") || t.Name.Contains("Monster")) &&
    !t.Name.StartsWith("<") &&
    !t.Name.Contains("Node") &&
    !t.Name.Contains("Screen")).ToList();

foreach (var t in creatureTypes.OrderBy(t => t.FullName))
    DumpType(t, withMethods: true);

// ─── 6. Power / Status types ─────────────────────────────────────────────────

H1("POWER / STATUS TYPES");
var powerTypes = allTypes.Where(t =>
    (t.Name.Contains("Power") || t.Name.Contains("Status")) &&
    !t.Name.StartsWith("<") &&
    !t.Name.Contains("Node") &&
    !t.Name.Contains("Screen")).ToList();

foreach (var t in powerTypes.OrderBy(t => t.FullName))
    DumpType(t, withMethods: true);

// ─── 7. Combat state types ───────────────────────────────────────────────────

H1("COMBAT STATE TYPES");
var combatTypes = allTypes.Where(t =>
    (t.Name.Contains("Combat") || t.Name.Contains("ICombat")) &&
    !t.Name.StartsWith("<") &&
    !t.Name.Contains("Node") &&
    !t.Name.Contains("Screen")).ToList();

foreach (var t in combatTypes.OrderBy(t => t.FullName))
    DumpType(t, withMethods: true);

// ─── 8. Damage types ─────────────────────────────────────────────────────────

H1("DAMAGE TYPES");
var dmgTypes = allTypes.Where(t =>
    t.Name.Contains("Damage") && !t.Name.StartsWith("<")).ToList();

foreach (var t in dmgTypes.OrderBy(t => t.FullName))
    DumpType(t, withMethods: true);

// ─── 9. Intent / AI types (enemy behavior) ───────────────────────────────────

H1("INTENT / AI / ENCOUNTER TYPES");
var intentTypes = allTypes.Where(t =>
    (t.Name.Contains("Intent") || t.Name.Contains("AI") || t.Name.Contains("Encounter") ||
     t.Name.Contains("Move") || t.Name.Contains("Brain") || t.Name.Contains("Behavior") ||
     t.Name.Contains("Room") || t.Name.Contains("EnemySet")) &&
    !t.Name.StartsWith("<") &&
    !t.Name.Contains("Node") &&
    !t.Name.Contains("Screen")).ToList();

foreach (var t in intentTypes.OrderBy(t => t.FullName))
    DumpType(t, withMethods: true);

// ─── 10. Keyword / enum types ─────────────────────────────────────────────────

H1("ENUMS (KEYWORDS, CARD TYPE, ETC)");
var enumTypes = allTypes.Where(t => t.IsEnum && !t.Name.StartsWith("<")).ToList();
foreach (var t in enumTypes.OrderBy(t => t.FullName))
{
    W($"\nEnum: {t.FullName}");
    try
    {
        var names = t.GetEnumNames();
        foreach (var n in names) W($"  {n}");
    }
    catch { W("  (could not read values)"); }
}

// ─── 11. Hook type - all hooks ────────────────────────────────────────────────

H1("HOOK CLASS - ALL METHODS");
var hookType = allTypes.FirstOrDefault(t => t.FullName == "MegaCrit.Sts2.Core.Hooks.Hook");
if (hookType != null)
{
    DumpType(hookType, withMethods: true);
}

// ─── 12. Orb types (for defect-like classes) ─────────────────────────────────

H1("ORB TYPES");
var orbTypes = allTypes.Where(t => t.Name.Contains("Orb") && !t.Name.StartsWith("<")).ToList();
foreach (var t in orbTypes.OrderBy(t => t.FullName))
    DumpType(t, withMethods: true);

// ─── 13. Player types ─────────────────────────────────────────────────────────

H1("PLAYER TYPES");
var playerTypes = allTypes.Where(t =>
    (t.Name.Contains("Player") || t.Name.Contains("Character")) &&
    !t.Name.StartsWith("<") &&
    !t.Name.Contains("Node") &&
    !t.Name.Contains("Screen")).ToList();

foreach (var t in playerTypes.OrderBy(t => t.FullName))
    DumpType(t, withMethods: true);

// ─── 14. ValueProp / modifier types ──────────────────────────────────────────

H1("VALUE PROP / MODIFIER TYPES");
var valuePropTypes = allTypes.Where(t =>
    (t.Name.Contains("ValueProp") || t.Name.Contains("Modifier") ||
     t.Name.Contains("Prop") || t.Name.Contains("Scalar")) &&
    !t.Name.StartsWith("<")).ToList();

foreach (var t in valuePropTypes.OrderBy(t => t.FullName))
    DumpType(t, withMethods: true);

// ─── 15. Data definition types (content definitions) ─────────────────────────

H1("CONTENT DEFINITION TYPES");
var defTypes = allTypes.Where(t =>
    (t.Name.EndsWith("Data") || t.Name.EndsWith("Definition") || t.Name.EndsWith("Config") ||
     t.Name.EndsWith("Def")) &&
    !t.Name.StartsWith("<")).ToList();

foreach (var t in defTypes.OrderBy(t => t.FullName))
    DumpType(t, withMethods: true);

// ─── 16. Relic types ─────────────────────────────────────────────────────────

H1("RELIC TYPES");
var relicTypes = allTypes.Where(t =>
    t.Name.Contains("Relic") && !t.Name.StartsWith("<") &&
    !t.Name.Contains("Node") && !t.Name.Contains("Screen")).ToList();

foreach (var t in relicTypes.OrderBy(t => t.FullName))
    DumpType(t, withMethods: true);

// ─── Write output ─────────────────────────────────────────────────────────────

var outputPath = Path.Combine(outDir, "full_type_inspection.txt");
File.WriteAllText(outputPath, sb.ToString());
Console.WriteLine($"Written: {outputPath} ({sb.Length / 1024}KB)");

// ─── Also generate a focused JSON summary of key combat types ─────────────────

Console.WriteLine("\nGenerating focused summary...");
var summary = new Dictionary<string, object>
{
    ["gameVersion"]  = "v0.106.1",
    ["extracted"]    = DateTime.UtcNow.ToString("O"),
    ["allTypeCount"] = allTypes.Length,
    ["namespaces"]   = allTypes.Select(t => t.Namespace).Distinct().OrderBy(x => x).ToArray(),
    ["cardTypeNames"]      = cardTypes.Select(t => t.FullName).OrderBy(x => x).ToArray(),
    ["effectTypeNames"]    = effectTypes.Select(t => t.FullName).OrderBy(x => x).ToArray(),
    ["creatureTypeNames"]  = creatureTypes.Select(t => t.FullName).OrderBy(x => x).ToArray(),
    ["powerTypeNames"]     = powerTypes.Select(t => t.FullName).OrderBy(x => x).ToArray(),
    ["combatTypeNames"]    = combatTypes.Select(t => t.FullName).OrderBy(x => x).ToArray(),
    ["intentTypeNames"]    = intentTypes.Select(t => t.FullName).OrderBy(x => x).ToArray(),
    ["relicTypeNames"]     = relicTypes.Select(t => t.FullName).OrderBy(x => x).ToArray(),
    ["orbTypeNames"]       = orbTypes.Select(t => t.FullName).OrderBy(x => x).ToArray(),
    ["enumTypes"]          = enumTypes.ToDictionary(
        t => t.FullName ?? t.Name,
        t => {
            try { return (object)t.GetEnumNames(); } catch { return (object)Array.Empty<string>(); }
        }),
};

var summaryPath = Path.Combine(outDir, "type_summary.json");
File.WriteAllText(summaryPath, JsonConvert.SerializeObject(summary, Formatting.Indented));
Console.WriteLine($"Written: {summaryPath}");

Console.WriteLine("\nDone!");
// -- Extra: ModelDb and AbstractModelSubtypes --
H1("ModelDb AND AbstractModelSubtypes");
foreach (var name in new[] { "ModelDb", "AbstractModelSubtypes", "RunManager", "EpochModelSubtypes" })
{
    var t = allTypes.FirstOrDefault(x => x.Name == name);
    if (t != null) DumpType(t, withMethods: true);
    else W($"\nType {name} not found");
}
