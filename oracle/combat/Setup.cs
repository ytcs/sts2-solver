using System.Text.Json.Nodes;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.Rngs;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Saves;
using MegaCrit.Sts2.Core.Saves.Runs;
using MegaCrit.Sts2.Core.Unlocks;

namespace OracleCombat;

public static class Setup
{
    public static readonly (string name, RunRngType type)[] Streams =
    {
        ("shuffle", RunRngType.Shuffle),
        ("combat_card_generation", RunRngType.CombatCardGeneration),
        ("combat_potion_generation", RunRngType.CombatPotionGeneration),
        ("combat_card_selection", RunRngType.CombatCardSelection),
        ("combat_energy_costs", RunRngType.CombatEnergyCosts),
        ("combat_targets", RunRngType.CombatTargets),
        ("monster_ai", RunRngType.MonsterAi),
        ("niche", RunRngType.Niche),
        ("combat_orbs", RunRngType.CombatOrbs),
    };

    private static SavedProperties MakeProps(JsonObject o)
    {
        if (o == null) return null;
        var sp = new SavedProperties();
        foreach (var kv in o)
        {
            switch (kv.Value.GetValue<System.Text.Json.JsonElement>().ValueKind)
            {
                case System.Text.Json.JsonValueKind.True:
                case System.Text.Json.JsonValueKind.False:
                    (sp.bools ??= new()).Add(new SavedProperties.SavedProperty<bool>(kv.Key, (bool)kv.Value)); break;
                case System.Text.Json.JsonValueKind.String:
                    (sp.strings ??= new()).Add(new SavedProperties.SavedProperty<string>(kv.Key, (string)kv.Value)); break;
                default:
                    (sp.ints ??= new()).Add(new SavedProperties.SavedProperty<int>(kv.Key, (int)kv.Value)); break;
            }
        }
        return sp;
    }

    public static ModelId Id(string cat, string entry) => new ModelId(cat, Scenario.StripCat(entry).ToUpperInvariant());

    public static (Player player, RunState run) BuildRun(Scenario sc)
    {
        var charId = Id("CHARACTER", sc.Character);
        var character = ModelDb.GetById<CharacterModel>(charId);
        var template = Player.CreateForNewRun(character, UnlockState.all, 1UL);
        var sp = template.ToSerializable();
        sp.NetId = 1UL;
        sp.CurrentHp = sc.Hp ?? sc.MaxHp ?? character.StartingHp;
        sp.MaxHp = sc.MaxHp ?? sc.Hp ?? character.StartingHp;
        if (sc.MaxEnergy.HasValue) sp.MaxEnergy = sc.MaxEnergy.Value;
        if (sc.Gold.HasValue) sp.Gold = sc.Gold.Value;
        if (sc.MaxPotionSlots.HasValue) sp.MaxPotionSlotCount = sc.MaxPotionSlots.Value;
        if (sc.BaseOrbSlots.HasValue) sp.BaseOrbSlotCount = sc.BaseOrbSlots.Value;
        if (!sc.UseCharacterStarter || sc.Deck.Count > 0)
        {
            sp.Deck = sc.Deck.Select(c => new SerializableCard
            {
                Id = Id("CARD", c.Id),
                CurrentUpgradeLevel = c.Upgrade,
                Enchantment = c.Enchantment == null ? null : new SerializableEnchantment { Id = Id("ENCHANTMENT", c.Enchantment), Amount = c.EnchantAmount },
                Props = MakeProps(c.Props),
                FloorAddedToDeck = c.FloorAdded ?? 1,
            }).ToList();
        }
        if (!sc.UseCharacterStarter || sc.Relics.Count > 0)
        {
            sp.Relics = sc.Relics.Select(r => new SerializableRelic { Id = Id("RELIC", r.Id), Props = MakeProps(r.Props), FloorAddedToDeck = 1 }).ToList();
        }
        if (sc.Potions.Count > 0 || !sc.UseCharacterStarter)
        {
            sp.Potions = sc.Potions.Select(p => new SerializablePotion { Id = Id("POTION", p.Id), SlotIndex = p.Slot }).ToList();
        }
        var player = Player.FromSerializable(sp);

        var acts = ActModel.GetDefaultList();
        var run = RunState.CreateForTest(new List<Player> { player }, acts, null, GameMode.Standard, sc.Ascension, sc.Seed);
        if (sc.Act != 0) run.CurrentActIndex = sc.Act;
        run.Map = new MockSinglePointActMap();

        foreach (var kv in sc.Rng)
        {
            var st = Streams.FirstOrDefault(s => s.name == kv.Key);
            if (st.name == null) throw new OracleException("unknown rng stream '" + kv.Key + "'");
            run.Rng.GetRng(st.type).LoadFromSerializable(new SerializableRng
            {
                counter = kv.Value.Counter, state0 = kv.Value.S0, state1 = kv.Value.S1, state2 = kv.Value.S2, state3 = kv.Value.S3
            });
        }
        for (int i = 1; i < sc.TotalFloor; i++)
            run.AppendToMapPointHistory(MapPointType.Monster, RoomType.Monster, null);
        return (player, run);
    }

    public static async Task EnterCombat(Scenario sc, Player player, RunState run)
    {
        Patches.ApplyAscensionEffects = sc.ApplyAscensionEffects;
        RunManager.Instance.SetUpTest(run, new NetSingleplayerGameService(), disableCombatStateSync: true, shouldSave: false);
        var enc = ModelDb.GetById<EncounterModel>(Id("ENCOUNTER", sc.Encounter));
        await RunManager.Instance.EnterRoomDebug(RoomType.Monster, MapPointType.Monster, enc.ToMutable(), showTransition: false);
    }
}
