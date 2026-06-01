using System;
using System.Collections.Generic;
using System.Linq;
using System.Threading;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.AutoSlay.Helpers;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Models.Characters;
using MegaCrit.Sts2.Core.Models.Encounters;
using MegaCrit.Sts2.Core.Nodes;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Settings;
using MegaCrit.Sts2.Core.Saves;
using MegaCrit.Sts2.Core.Unlocks;
using MegaCrit.Sts2.Core.Assets;

namespace DataDumper.DataDumperCode;

/// <summary>
/// Fully autonomous, headless batch trace collection. Triggered by the STS2_BATCH env var (set to an
/// encounter key, e.g. "CULTISTS_NORMAL"). Starts a controlled Ironclad run programmatically
/// (mirroring NSceneBootstrapper), drops straight into the chosen encounter via EnterRoomDebug,
/// auto-plays it (no buffs, real manual plays → valid trace), then quits the process.
///
/// Run headless with Steam running and steam_appid.txt present:
///   STS2_BATCH=CULTISTS_NORMAL STS2_BATCH_KILL=1 STS2_TRACE_DIR=/abs/path \
///     HOME=/tmp/sts2-sandbox ./SlayTheSpire2 --headless
/// </summary>
public static class HeadlessBatch
{
    public static void MaybeStart()
    {
        var enc = Environment.GetEnvironmentVariable("STS2_BATCH");
        if (string.IsNullOrEmpty(enc)) return;
        MainFile.Logger.Info($"HeadlessBatch: requested encounter '{enc}'", 0);
        TaskHelper.RunSafely(RunAsync(enc!));
    }

    private static async Task RunAsync(string encounterKey)
    {
        using var cts = new CancellationTokenSource(TimeSpan.FromMinutes(5));
        var ct = cts.Token;
        try
        {
            // Wait until the MAIN MENU is fully loaded (startup complete) and the model DB is populated.
            // Starting the run earlier races NGame's startup continuation and disposes the menu mid-boot
            // (ObjectDisposedException), which destabilises the subsequent combat.
            await WaitHelper.Until(() => NGame.Instance?.MainMenu != null && ModelsReady(), ct,
                TimeSpan.FromSeconds(60), "main menu + models ready");
            await Task.Delay(500, ct); // let the menu settle

            try { SaveManager.Instance.PrefsSave.FastMode = FastModeType.Instant; } catch { }

            // ---- Start a controlled Ironclad run (mirrors NSceneBootstrapper.StartNewRun) ----
            var unlock = SaveManager.Instance.GenerateUnlockStateFromProgress();
            var ironclad = ModelDb.Character<Ironclad>();
            // Ensure a per-character stats entry exists (a fresh save profile may lack one, which the
            // Player constructor would otherwise KeyNotFound on).
            SaveManager.Instance.Progress.GetOrCreateCharacterStats(ironclad.Id);
            var player = Player.CreateForNewRun(ironclad, unlock, 1uL);
            MaybeInjectCustomDeck(player);
            var acts = ActModel.GetDefaultList().Select(a => a.ToMutable()).ToList();
            var seed = Environment.GetEnvironmentVariable("STS2_SEED") is { Length: > 0 } s ? s : SeedHelper.GetRandomSeed();
            // Ascension level for the run. Defaults to 10 (max) — the solver targets A10 play; override with
            // STS2_ASCENSION=0 to record a baseline trace. By A10 ToughEnemies (+HP) and DeadlyEnemies
            // (+damage) are active, which the recorded trace then carries for differential validation.
            int ascension = int.TryParse(Environment.GetEnvironmentVariable("STS2_ASCENSION"), out var asc) ? asc : 10;
            MainFile.Logger.Info($"HeadlessBatch: ascension level = {ascension}", 0);
            var runState = RunState.CreateForNewRun(
                new List<Player> { player }, acts, new List<ModifierModel>(), GameMode.Standard, ascension, seed);

            RunManager.Instance.SetUpNewSinglePlayer(runState, shouldSave: false);
            await PreloadManager.LoadRunAssets(new List<CharacterModel> { player.Character });
            RunManager.Instance.Launch();
            NGame.Instance!.RootSceneContainer.SetCurrentScene(NRun.Create(runState));
            await RunManager.Instance.SetActInternal(0);
            RunManager.Instance.RunLocationTargetedBuffer.OnLocationChanged(runState.RunLocation);
            RunManager.Instance.MapSelectionSynchronizer.OnLocationChanged(runState.MapLocation);
            MainFile.Logger.Info($"HeadlessBatch: run started (seed={seed}), entering {encounterKey}", 0);

            // ---- Enter the chosen combat encounter (honor its room type: Monster/Elite/Boss) ----
            var encounter = ResolveEncounter(encounterKey);
            var roomType = encounter.RoomType;
            var pointType = roomType switch
            {
                RoomType.Elite => MapPointType.Elite,
                RoomType.Boss => MapPointType.Boss,
                _ => MapPointType.Monster,
            };
            await RunManager.Instance.EnterRoomDebug(roomType, pointType, encounter.ToMutable());

            await WaitHelper.Until(() => CombatManager.Instance.IsInProgress, ct, TimeSpan.FromSeconds(20), "combat start");
            await KillRequestedEnemies();

            // ---- Auto-play (records the trace) ----
            await AutoPilot.DriveCombatAsync(ct);

            // Wait until the recorder has written the victory/end snapshot and closed the file.
            try { await WaitHelper.Until(() => CombatOracle.CombatClosed, ct, TimeSpan.FromSeconds(10), "trace flush"); }
            catch { /* fall through to quit even if the end event never fired */ }
            await Task.Delay(500, ct);
            MainFile.Logger.Info("HeadlessBatch: done, quitting.", 0);
        }
        catch (Exception e)
        {
            MainFile.Logger.Info($"HeadlessBatch error: {e}", 0);
        }
        finally
        {
            try { NGame.Instance?.GetTree().Quit(); } catch { }
        }
    }

    private static bool ModelsReady()
    {
        try { return ModelDb.Character<Ironclad>() != null; }
        catch { return false; }
    }

    private static async Task KillRequestedEnemies()
    {
        var spec = Environment.GetEnvironmentVariable("STS2_BATCH_KILL");
        if (string.IsNullOrEmpty(spec)) return;
        var state = CombatManager.Instance.DebugOnlyGetState();
        if (state == null) return;
        var enemies = state.Enemies.ToList();
        foreach (var part in spec.Split(',', StringSplitOptions.RemoveEmptyEntries))
        {
            if (int.TryParse(part.Trim(), out int idx) && idx >= 0 && idx < enemies.Count)
            {
                MainFile.Logger.Info($"HeadlessBatch: killing enemy {idx} ({enemies[idx].Name})", 0);
                await CreatureCmd.Kill(enemies[idx]);
            }
        }
    }

    /// <summary>
    /// Replace the freshly-created run deck with a custom one specified by the STS2_DECK env var, so
    /// specific cards can be exercised by the autopilot and the resulting trace validated against the
    /// solver. Format: comma-separated entries, each "ClassName" or "ClassName:Count" (count defaults
    /// to 1), e.g. "StrikeIronclad:4,IronWave:2,Bludgeon". Class names are the card model C# type names
    /// (exactly what the recorder writes via card.GetType().Name), so they round-trip to the solver's
    /// Catalog factory keys. Relics are left untouched (the standard Ironclad start, which the validator
    /// already handles). No-op when STS2_DECK is unset.
    /// </summary>
    private static void MaybeInjectCustomDeck(Player player)
    {
        var spec = Environment.GetEnvironmentVariable("STS2_DECK");
        if (string.IsNullOrWhiteSpace(spec)) return;

        var byName = CardTypesByName();
        var cards = new List<CardModel>();
        foreach (var raw in spec.Split(',', StringSplitOptions.RemoveEmptyEntries))
        {
            var entry = raw.Trim();
            if (entry.Length == 0) continue;
            int count = 1;
            var name = entry;
            int colon = entry.IndexOf(':');
            if (colon >= 0)
            {
                name = entry[..colon].Trim();
                int.TryParse(entry[(colon + 1)..].Trim(), out count);
                if (count < 1) count = 1;
            }
            if (!byName.TryGetValue(name, out var type))
            {
                MainFile.Logger.Info($"HeadlessBatch: STS2_DECK unknown card '{name}' — skipping.", 0);
                continue;
            }
            for (int i = 0; i < count; i++)
            {
                var card = ModelDb.GetById<CardModel>(ModelDb.GetId(type)).ToMutable();
                card.FloorAddedToDeck = 1;
                cards.Add(card);
            }
        }

        if (cards.Count == 0)
        {
            MainFile.Logger.Info("HeadlessBatch: STS2_DECK produced no cards — keeping starting deck.", 0);
            return;
        }

        player.Deck.Clear(silent: true);
        foreach (var card in cards) player.Deck.AddInternal(card, -1, silent: true);
        MainFile.Logger.Info($"HeadlessBatch: custom deck of {cards.Count} cards: " +
            string.Join(", ", cards.Select(c => c.GetType().Name)), 0);
    }

    private static Dictionary<string, Type>? _cardTypeCache;
    private static Dictionary<string, Type> CardTypesByName()
    {
        if (_cardTypeCache != null) return _cardTypeCache;
        var dict = new Dictionary<string, Type>(StringComparer.OrdinalIgnoreCase);
        foreach (var t in typeof(CardModel).Assembly.GetTypes())
        {
            if (t.IsAbstract || !typeof(CardModel).IsAssignableFrom(t)) continue;
            dict[t.Name] = t; // last one wins; card type names are unique within the assembly
        }
        return _cardTypeCache = dict;
    }

    /// <summary>Map an encounter key (class name or SCREAMING_SNAKE) to its model.</summary>
    private static EncounterModel ResolveEncounter(string key)
    {
        var k = key.Replace("_", "").ToLowerInvariant();
        return k switch
        {
            "cultistsnormal" => ModelDb.Encounter<CultistsNormal>(),
            "seapunknormal" => ModelDb.Encounter<SeapunkNormal>(),
            "corpseslugsweak" => ModelDb.Encounter<CorpseSlugsWeak>(),
            "corpseslugsnormal" => ModelDb.Encounter<CorpseSlugsNormal>(),
            "byrdoniselite" => ModelDb.Encounter<ByrdonisElite>(),
            "bygoneeffigyelite" => ModelDb.Encounter<BygoneEffigyElite>(),
            "phrogparasiteelite" => ModelDb.Encounter<PhrogParasiteElite>(),
            "terroreelelite" => ModelDb.Encounter<TerrorEelElite>(),
            "soulnexuselite" => ModelDb.Encounter<SoulNexusElite>(),
            "mechaknightelite" => ModelDb.Encounter<MechaKnightElite>(),
            "entomancerelite" => ModelDb.Encounter<EntomancerElite>(),
            "skulkingcolonyelite" => ModelDb.Encounter<SkulkingColonyElite>(),
            "infestedprismselite" => ModelDb.Encounter<InfestedPrismsElite>(),
            "phantasmalgardenerselite" => ModelDb.Encounter<PhantasmalGardenersElite>(),
            "knightselite" => ModelDb.Encounter<KnightsElite>(),
            "decimillipedeelite" => ModelDb.Encounter<DecimillipedeElite>(),
            _ => ModelDb.Encounter<CultistsNormal>(),
        };
    }
}
