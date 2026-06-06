using Godot;
using MegaCrit.Sts2.Core.Modding;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Entities.Powers;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Reflection;

namespace DataDumper.DataDumperCode;

/// <summary>
/// DataDumper mod entry point.
/// Dumps all card, monster, encounter, power, and relic data to JSON when the game starts.
/// Uses reflection to access protected/internal members since Publicizer is unreliable here.
/// Output is written to ~/Projects/sts2-solver/data/game_data/
/// </summary>
[ModInitializer(nameof(Initialize))]
public partial class MainFile : Node
{
    public const string ModId = "DataDumper";
    public static readonly string OutputDir = Path.Combine(
        System.Environment.GetFolderPath(System.Environment.SpecialFolder.UserProfile),
        "Projects", "sts2-solver", "data", "game_data");

    public static MegaCrit.Sts2.Core.Logging.Logger Logger { get; } =
        new(ModId, MegaCrit.Sts2.Core.Logging.LogType.Generic);

    private const BindingFlags ALL = BindingFlags.Public | BindingFlags.NonPublic |
                                     BindingFlags.Instance | BindingFlags.Static;

    public static void Initialize()
    {
        Logger.Info($"DataDumper initializing. Output: {OutputDir}", 0);
        
        try
        {
            Directory.CreateDirectory(OutputDir);
            var asm = typeof(CardModel).Assembly;
            DumpAllData(asm);
            Logger.Info("DataDumper complete!", 0);

            // Install the passive combat-trace recorder (for solver-engine validation).
            CombatOracle.Install();

            // If launched in batch mode (STS2_BATCH env), autonomously run a headless combat + quit.
            HeadlessBatch.MaybeStart();
        }
        catch (Exception e)
        {
            Logger.Info($"DataDumper ERROR: {e}", 0);
        }
    }

    // ── Reflection helpers ────────────────────────────────────────────────────

    private static T? Prop<T>(object? obj, string name)
    {
        if (obj == null) return default;
        try
        {
            var pi = obj.GetType().GetProperty(name, ALL);
            if (pi == null) return default;
            return (T?)pi.GetValue(obj);
        }
        catch { return default; }
    }

    private static string? PropStr(object? obj, string name)
    {
        try { return Prop<object>(obj, name)?.ToString(); }
        catch { return null; }
    }

    private static T? Field<T>(object? obj, string name)
    {
        if (obj == null) return default;
        try
        {
            var fi = obj.GetType().GetField(name, ALL);
            if (fi == null) return default;
            return (T?)fi.GetValue(obj);
        }
        catch { return default; }
    }

    private static T? InvokeMethod<T>(object? obj, string name, params object?[] args)
    {
        if (obj == null) return default;
        try
        {
            var mi = obj.GetType().GetMethod(name, ALL);
            if (mi == null) return default;
            return (T?)mi.Invoke(obj, args);
        }
        catch { return default; }
    }

    // ── Main dump ─────────────────────────────────────────────────────────────

    private static void DumpAllData(Assembly asm)
    {
        var allTypes = asm.GetTypes();
        
        // ── Cards ──────────────────────────────────────────────────────────
        Logger.Info("Dumping cards...", 0);
        var cardBaseType = typeof(CardModel);
        var cardTypes = allTypes.Where(t => 
            t.IsClass && !t.IsAbstract &&
            cardBaseType.IsAssignableFrom(t) &&
            t.Namespace != null &&
            !t.Namespace.Contains(".Mocks") &&
            !t.Name.Contains("Deprecated")).ToList();
        
        Logger.Info($"  {cardTypes.Count} CardModel types", 0);
        var cards = cardTypes.Select(type => InstantiateDump(type, DumpCard)).ToList();
        WriteJson("cards.json", cards);

        // ── Monsters ───────────────────────────────────────────────────────
        Logger.Info("Dumping monsters...", 0);
        var monsterBaseType = typeof(MonsterModel);
        var monsterTypes = allTypes.Where(t =>
            t.IsClass && !t.IsAbstract &&
            monsterBaseType.IsAssignableFrom(t) &&
            t.Namespace != null &&
            !t.Namespace.Contains(".Mocks") &&
            !t.Name.Contains("Deprecated") && !t.Name.Contains("Fake")).ToList();
        
        Logger.Info($"  {monsterTypes.Count} MonsterModel types", 0);
        var monsters = monsterTypes.Select(type => InstantiateDump(type, DumpMonster)).ToList();
        WriteJson("monsters.json", monsters);

        // ── Encounters ─────────────────────────────────────────────────────
        Logger.Info("Dumping encounters...", 0);
        var encounterBaseType = typeof(EncounterModel);
        var encounterTypes = allTypes.Where(t =>
            t.IsClass && !t.IsAbstract &&
            encounterBaseType.IsAssignableFrom(t) &&
            t.Namespace != null &&
            !t.Namespace.Contains(".Mocks") &&
            !t.Name.Contains("Deprecated") && !t.Name.Contains("Fake")).ToList();
        
        Logger.Info($"  {encounterTypes.Count} EncounterModel types", 0);
        var encounters = encounterTypes.Select(type => InstantiateDump(type, DumpEncounter)).ToList();
        WriteJson("encounters.json", encounters);

        // ── Powers ─────────────────────────────────────────────────────────
        Logger.Info("Dumping powers...", 0);
        var powerBaseType = typeof(PowerModel);
        var powerTypes = allTypes.Where(t =>
            t.IsClass && !t.IsAbstract &&
            powerBaseType.IsAssignableFrom(t) &&
            t.Namespace != null &&
            !t.Namespace.Contains(".Mocks") &&
            !t.Name.Contains("Deprecated") && !t.Name.Contains("Fake")).ToList();
        
        Logger.Info($"  {powerTypes.Count} PowerModel types", 0);
        var powers = powerTypes.Select(type => InstantiateDump(type, DumpPower)).ToList();
        WriteJson("powers.json", powers);

        // ── Enchantments ───────────────────────────────────────────────────
        // The card-modifier (buff) system. The dump captures the type set + LocStrings + scalar magnitudes; the
        // OnEnchant()/CanEnchant() method BODIES are not reflectable, so effects still need cross-referencing,
        // but the magnitudes (the part the solver port needs) come through as scalars.
        Logger.Info("Dumping enchantments...", 0);
        var enchBaseType = allTypes.FirstOrDefault(t => t.Name == "EnchantmentModel");
        if (enchBaseType != null)
        {
            var enchTypes = allTypes.Where(t =>
                t.IsClass && !t.IsAbstract &&
                enchBaseType.IsAssignableFrom(t) &&
                t.Namespace != null &&
                !t.Namespace.Contains(".Mocks") &&
                !t.Name.Contains("Deprecated")).ToList();
            Logger.Info($"  {enchTypes.Count} EnchantmentModel types", 0);
            var enchantments = enchTypes.Select(type => InstantiateDump(type, DumpEnchantment)).ToList();
            WriteJson("enchantments.json", enchantments);
        }
        else
        {
            Logger.Info("  EnchantmentModel base type not found — skipping enchantments.json", 0);
        }

        // ── AbstractModel subtype hierarchy ────────────────────────────────
        Logger.Info("Dumping type hierarchy...", 0);
        var hierarchy = allTypes
            .Where(t => typeof(AbstractModel).IsAssignableFrom(t))
            .Select(t => new { typeName = t.FullName, baseName = t.BaseType?.FullName, ns = t.Namespace })
            .ToList();
        WriteJson("type_hierarchy.json", hierarchy);

        // ── Enums ──────────────────────────────────────────────────────────
        Logger.Info("Dumping enums...", 0);
        var enumData = allTypes
            .Where(t => t.IsEnum && t.Namespace?.StartsWith("MegaCrit.Sts2") == true)
            .ToDictionary(t => t.FullName ?? t.Name, t => {
                try { return (object)Enum.GetNames(t); }
                catch { return (object)Array.Empty<string>(); }
            });
        WriteJson("enums.json", enumData);

        Logger.Info("All data dumped.", 0);
    }

    private static Dictionary<string, object?> InstantiateDump(Type type, Func<object, Type, Dictionary<string, object?>> dumper)
    {
        try
        {
            var instance = Activator.CreateInstance(type)!;
            return dumper(instance, type);
        }
        catch (Exception e)
        {
            return new Dictionary<string, object?>
            {
                ["className"] = type.Name,
                ["fullName"]  = type.FullName,
                ["error"]     = e.Message,
            };
        }
    }

    // ── Per-type dump functions ───────────────────────────────────────────────

    private static Dictionary<string, object?> DumpCard(object obj, Type type)
    {
        return new Dictionary<string, object?>
        {
            ["className"]      = type.Name,
            ["fullName"]       = type.FullName,
            ["namespace"]      = type.Namespace,
            ["title"]          = PropStr(obj, "Title"),
            ["type"]           = PropStr(obj, "Type"),
            ["rarity"]         = PropStr(obj, "Rarity"),
            ["energyCost"]     = Prop<object>(obj, "CanonicalEnergyCost")?.ToString()
                                 ?? Prop<object>(obj, "EnergyCost")?.ToString(),
            ["hasEnergyCostX"] = Prop<object>(obj, "HasEnergyCostX")?.ToString(),
            ["starCost"]       = Prop<object>(obj, "CanonicalStarCost")?.ToString()
                                 ?? PropStr(obj, "BaseStarCost"),
            ["baseReplayCount"]= PropStr(obj, "BaseReplayCount"),
            ["targetType"]     = PropStr(obj, "TargetType"),
            ["keywords"]       = DumpEnumerable(Prop<System.Collections.IEnumerable>(obj, "CanonicalKeywords")),
            ["tags"]           = DumpEnumerable(Prop<System.Collections.IEnumerable>(obj, "CanonicalTags")),
            ["orbEvokeType"]   = PropStr(obj, "OrbEvokeType"),
            ["gainsBlock"]     = PropStr(obj, "GainsBlock"),
            ["isBasicStrikeOrDefend"] = PropStr(obj, "IsBasicStrikeOrDefend"),
            ["isRemovable"]    = PropStr(obj, "IsRemovable"),
            ["isTransformable"]= PropStr(obj, "IsTransformable"),
            ["maxUpgradeLevel"]= PropStr(obj, "MaxUpgradeLevel"),
            ["canBeGeneratedInCombat"] = PropStr(obj, "CanBeGeneratedInCombat"),
            ["pool"]           = Prop<object>(obj, "Pool")?.GetType().Name,
            ["description"]    = PropStr(obj, "Description"),
            ["dynamicVars"]    = DumpDynamicVars(obj),
            ["multiplayerConstraint"] = PropStr(obj, "MultiplayerConstraint"),
        };
    }

    private static Dictionary<string, object?> DumpMonster(object obj, Type type)
    {
        return new Dictionary<string, object?>
        {
            ["className"]  = type.Name,
            ["fullName"]   = type.FullName,
            ["namespace"]  = type.Namespace,
            ["title"]      = PropStr(obj, "Title"),
            ["minHp"]      = PropStr(obj, "MinInitialHp"),
            ["maxHp"]      = PropStr(obj, "MaxInitialHp"),
            ["shouldShowInCompendium"] = PropStr(obj, "ShouldShowInCompendium"),
            ["moves"]      = DumpMonsterMoveStates(obj),
            // Numeric source of truth for move amounts (the intent telegraphs null out damage/block/times/power).
            ["scalars"]    = DumpScalarMembers(obj, type),
        };
    }

    private static Dictionary<string, object?> DumpEnchantment(object obj, Type type)
    {
        return new Dictionary<string, object?>
        {
            ["className"]   = type.Name,
            ["fullName"]    = type.FullName,
            ["namespace"]   = type.Namespace,
            ["title"]       = PropStr(obj, "Title") ?? PropStr(obj, "Name"),
            ["description"] = PropStr(obj, "Description"),
            // OnEnchant magnitudes / defaults live as scalar members on the concrete enchantment subclass.
            ["scalars"]     = DumpScalarMembers(obj, type),
        };
    }

    private static Dictionary<string, object?> DumpEncounter(object obj, Type type)
    {
        return new Dictionary<string, object?>
        {
            ["className"]  = type.Name,
            ["fullName"]   = type.FullName,
            ["namespace"]  = type.Namespace,
            ["title"]      = PropStr(obj, "Title"),
            ["roomType"]   = PropStr(obj, "RoomType"),
            ["isWeak"]     = PropStr(obj, "IsWeak"),
            ["shouldGiveRewards"] = PropStr(obj, "ShouldGiveRewards"),
            ["minGoldReward"] = PropStr(obj, "MinGoldReward"),
            ["maxGoldReward"] = PropStr(obj, "MaxGoldReward"),
            ["tags"]       = DumpEnumerable(Prop<System.Collections.IEnumerable>(obj, "Tags")),
            ["monsters"]   = DumpEnumerable(Prop<System.Collections.IEnumerable>(obj, "AllPossibleMonsters"),
                                            m => m?.GetType().Name),
        };
    }

    private static Dictionary<string, object?> DumpPower(object obj, Type type)
    {
        return new Dictionary<string, object?>
        {
            ["className"]  = type.Name,
            ["fullName"]   = type.FullName,
            ["namespace"]  = type.Namespace,
            ["title"]      = PropStr(obj, "Title"),
            ["description"]= PropStr(obj, "Description"),
            ["stackType"]  = PropStr(obj, "StackType"),
            ["powerType"]  = PropStr(obj, "Type"),
            ["instanceType"] = PropStr(obj, "InstanceType"),
            ["allowNegative"] = PropStr(obj, "AllowNegative"),
            ["isVisible"]  = PropStr(obj, "IsVisible"),
            ["amount"]     = PropStr(obj, "Amount"),
        };
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    private static List<string?>? DumpEnumerable(System.Collections.IEnumerable? seq,
                                                   Func<object?, string?>? selector = null)
    {
        if (seq == null) return null;
        var result = new List<string?>();
        foreach (var item in seq)
            result.Add(selector != null ? selector(item) : item?.ToString());
        return result;
    }

    private static List<object?>? DumpDynamicVars(object card)
    {
        try
        {
            var vars = Prop<System.Collections.IEnumerable>(card, "CanonicalVars");
            if (vars == null) return null;
            var result = new List<object?>();
            foreach (var v in vars)
            {
                if (v == null) continue;
                result.Add(new Dictionary<string, object?>
                {
                    ["key"]   = PropStr(v, "Key"),
                    ["value"] = PropStr(v, "Value"),
                    ["baseValue"] = PropStr(v, "BaseValue"),
                });
            }
            return result;
        }
        catch { return null; }
    }

    // Type names that are framework/base layers we never want member noise from. Any member DECLARED on one of
    // these (or on a Godot type) is skipped; everything declared on a game-specific subclass is kept.
    private static readonly HashSet<string> MemberBaseCutoff = new()
    {
        "AbstractModel", "MonsterModel", "CardModel", "PowerModel", "EncounterModel", "EnchantmentModel",
        "AfflictionModel", "RelicModel", "Node", "GodotObject", "Object", "RefCounted", "Resource",
    };

    private static bool IsDumpableScalar(Type t)
    {
        t = Nullable.GetUnderlyingType(t) ?? t;
        return t.IsEnum || t == typeof(bool)
            || t == typeof(int) || t == typeof(uint) || t == typeof(long) || t == typeof(ulong)
            || t == typeof(short) || t == typeof(ushort) || t == typeof(byte) || t == typeof(sbyte)
            || t == typeof(decimal) || t == typeof(float) || t == typeof(double);
    }

    /// <summary>Reflect every numeric / bool / enum field and property DECLARED on the concrete model subclass
    /// (not on the framework base layers) and return them as a flat map. This is the source of truth for move
    /// AMOUNTS — damage / block / hit-count / power magnitudes — which the intent telegraphs leave null (the
    /// numbers live on the instance, e.g. Aeonglass.EbbDamage / IncreasingIntensityBlock, and on enchantments
    /// the OnEnchant magnitude fields). Each member is read defensively: getters that throw without a live combat
    /// context are skipped rather than aborting the whole dump.</summary>
    private static Dictionary<string, object?> DumpScalarMembers(object obj, Type type)
    {
        var result = new Dictionary<string, object?>();
        bool Keep(MemberInfo m) =>
            m.DeclaringType != null
            && !MemberBaseCutoff.Contains(m.DeclaringType.Name)
            && m.DeclaringType.Namespace?.StartsWith("Godot") != true
            && !m.Name.Contains('<');   // skip compiler-generated backing fields

        foreach (var f in type.GetFields(ALL))
        {
            if (!Keep(f) || !IsDumpableScalar(f.FieldType)) continue;
            try { result[f.Name] = f.GetValue(obj)?.ToString(); } catch { /* skip */ }
        }
        foreach (var p in type.GetProperties(ALL))
        {
            if (!Keep(p) || p.GetIndexParameters().Length != 0 || !p.CanRead || !IsDumpableScalar(p.PropertyType))
                continue;
            if (result.ContainsKey(p.Name)) continue;   // a field of the same name already captured it
            try { result[p.Name] = p.GetValue(obj)?.ToString(); } catch { /* getter needs combat context — skip */ }
        }
        return result;
    }

    private static object? DumpMonsterMoveStates(object monster)
    {
        try
        {
            // Generate the move state machine to understand moves
            var machine = Prop<object>(monster, "MoveStateMachine") 
                          ?? InvokeMethod<object>((MonsterModel)monster, "GenerateMoveStateMachine");
            if (machine == null) return null;
            
            var states = Prop<System.Collections.IDictionary>(machine, "States");
            if (states == null) return null;
            
            var result = new Dictionary<string, object?>();
            foreach (System.Collections.DictionaryEntry entry in states)
            {
                var stateId = entry.Key?.ToString();
                var state = entry.Value;
                if (stateId == null || state == null) continue;
                
                result[stateId] = new Dictionary<string, object?>
                {
                    ["id"]      = PropStr(state, "Id"),
                    ["isMove"]  = PropStr(state, "IsMove"),
                    ["canTransitionAway"] = PropStr(state, "CanTransitionAway"),
                    ["intents"] = DumpStateIntents(state),
                    // Actual attack/block values come from the move implementation
                    // We'll need to also look at move perform methods
                };
            }
            return result;
        }
        catch (Exception e)
        {
            return new Dictionary<string, object?> { ["error"] = e.Message };
        }
    }

    private static object? DumpStateIntents(object state)
    {
        try
        {
            // Try to get intents from the state
            var intents = Prop<System.Collections.IEnumerable>(state, "Intents");
            if (intents == null) return null;
            
            var result = new List<object?>();
            foreach (var intent in intents)
            {
                if (intent == null) continue;
                result.Add(new Dictionary<string, object?>
                {
                    ["type"]   = intent.GetType().Name,
                    ["damage"] = PropStr(intent, "Damage"),
                    ["times"]  = PropStr(intent, "Times"),
                    ["block"]  = PropStr(intent, "Block"),
                    ["power"]  = PropStr(intent, "Power"),
                });
            }
            return result;
        }
        catch { return null; }
    }

    private static void WriteJson(string filename, object data)
    {
        var path = Path.Combine(OutputDir, filename);
        var json = JsonSerializer.Serialize(data, new JsonSerializerOptions
        {
            WriteIndented = true,
            ReferenceHandler = ReferenceHandler.IgnoreCycles,
            // System.Text.Json includes nulls by default (matches prior NullValueHandling.Include).
        });
        File.WriteAllText(path, json);
        Logger.Info($"  -> {path} ({json.Length / 1024}KB)", 0);
    }
}
