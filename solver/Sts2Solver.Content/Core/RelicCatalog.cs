using Sts2Solver.Engine;

namespace Sts2Solver.Content;

public static partial class Catalog
{
    /// <summary>Every relic the engine models, keyed by the game's relic class name — which is also the
    /// <c>Catalog.BuildRelic</c> key, the decompile file name, and the PascalCased run-save relic id (so
    /// ranwid's generic <c>GameIds.ModelledRelicName</c> picks any entry up automatically). The four character
    /// starter relics are defined in their character files; the combat pool lives in <c>Relics/</c>.
    /// <para>Scope is the to-do's "combat-affecting relics" (in-combat HP/block/damage/energy hooks), NOT the
    /// full cosmetic/economy/map/reward catalog. Every entry is deterministic ⇒ exact for the objective.</para></summary>
    private static readonly Dictionary<string, Func<RelicModel>> RelicFactories = new(StringComparer.OrdinalIgnoreCase)
    {
        // Character starter relics (combat-start mechanics: heal / Stars / Osty / orbs).
        ["BurningBlood"]     = () => new BurningBlood(),       // Ironclad   (Ironclad/IroncladRelics.cs)
        ["DivineRight"]      = () => new DivineRight(),        // Regent     (Regent/RegentCatalog.cs)
        ["BoundPhylactery"]  = () => new BoundPhylactery(),    // Necrobinder(Necrobinder/NecrobinderCatalog.cs)
        ["CrackedCore"]      = () => new CrackedCore(),        // Defect     (Defect/DefectCatalog.cs)

        // --- Combat pool, batch 1 (Relics/CombatRelics.cs) ---
        // Combat-start stat/block/power grants:
        ["Anchor"]           = () => new Anchor(),
        ["BronzeScales"]     = () => new BronzeScales(),
        ["Vajra"]            = () => new Vajra(),
        ["OddlySmoothStone"] = () => new OddlySmoothStone(),
        ["DataDisk"]         = () => new DataDisk(),
        ["Gorget"]           = () => new Gorget(),
        ["Akabeko"]          = () => new Akabeko(),
        ["BagOfMarbles"]     = () => new BagOfMarbles(),
        ["RedMask"]          = () => new RedMask(),
        ["BloodVial"]        = () => new BloodVial(),
        // Recurring / turn-numbered turn-start effects:
        ["Sai"]              = () => new Sai(),
        ["Brimstone"]        = () => new Brimstone(),
        ["Lantern"]          = () => new Lantern(),
        ["VeryHotCocoa"]     = () => new VeryHotCocoa(),
        ["Candelabra"]       = () => new Candelabra(),
        ["Chandelier"]       = () => new Chandelier(),
        ["FestivePopper"]    = () => new FestivePopper(),
        // Passive modifiers (install a hidden relic power):
        ["Ectoplasm"]        = () => new Ectoplasm(),
        ["StrikeDummy"]      = () => new StrikeDummy(),
        ["FakeStrikeDummy"]  = () => new FakeStrikeDummy(),
        ["MiniatureCannon"]  = () => new MiniatureCannon(),

        // --- Combat pool, batch 2 (HP-loss reducers + passive modifiers) ---
        ["TungstenRod"]      = () => new TungstenRod(),
        ["TheBoot"]          = () => new TheBoot(),
        ["SpikedGauntlets"]  = () => new SpikedGauntlets(),
        ["PaelsBlood"]       = () => new PaelsBlood(),
        ["BlessedAntler"]    = () => new BlessedAntler(),
    };
}
