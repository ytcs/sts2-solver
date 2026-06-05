using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// Combat-affecting relic pool (batch 1) — ported 1:1 from MegaCrit.Sts2.Core.Models.Relics. Scope: relics whose
// effect changes in-combat HP/block/damage/energy (the to-do "combat-affecting relics", NOT the cosmetic/economy/
// map/reward catalog). Each relic reuses an existing power or installs a hidden relic power, so no new engine
// pipeline is needed. ranwid picks any registered relic up automatically (GameIds.ModelledRelicName is generic).
//
// SOUNDNESS: every relic here is deterministic (no RNG card/orb generation, no choice), so it is exact for the
// objective — no pessimistic/optimistic gap. Enemy-side downsides (Brimstone's enemy Strength) ARE modelled so
// the relic cannot read as a free upside. The game grants several of these "at the start of the player's first
// turn (TurnNumber<=1)"; OnCombatStart fires before turn 1 with no intervening enemy turn, so it is equivalent.

// ---- Combat-start stat / block / power grants (player) ----

/// <summary>Anchor: gain 10 Block at the start of combat. (MegaCrit Anchor.)</summary>
public sealed class Anchor : RelicModel
{
    public override string Id => "Anchor";
    public override void OnCombatStart(CombatState combat) => Cmd.GainBlock(combat, combat.Player, 10, ValueProp.Unpowered, null);
}

/// <summary>Bronze Scales: start each combat with 3 Thorns. (MegaCrit BronzeScales.)</summary>
public sealed class BronzeScales : RelicModel
{
    public override string Id => "BronzeScales";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new ThornsPower(), 3, combat.Player);
}

/// <summary>Vajra: start each combat with 1 Strength. (MegaCrit Vajra.)</summary>
public sealed class Vajra : RelicModel
{
    public override string Id => "Vajra";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new StrengthPower(), 1, combat.Player);
}

/// <summary>Oddly Smooth Stone: start each combat with 1 Dexterity. (MegaCrit OddlySmoothStone.)</summary>
public sealed class OddlySmoothStone : RelicModel
{
    public override string Id => "OddlySmoothStone";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new DexterityPower(), 1, combat.Player);
}

/// <summary>Data Disk: start each combat with 1 Focus. (MegaCrit DataDisk.)</summary>
public sealed class DataDisk : RelicModel
{
    public override string Id => "DataDisk";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new FocusPower(), 1, combat.Player);
}

/// <summary>Gorget: start each combat with 4 Plating (Plated Armor). (MegaCrit Gorget.)</summary>
public sealed class Gorget : RelicModel
{
    public override string Id => "Gorget";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new PlatingPower(), 4, combat.Player);
}

/// <summary>Akabeko: your first attack each combat deals additional damage (8 Vigor). (MegaCrit Akabeko.)</summary>
public sealed class Akabeko : RelicModel
{
    public override string Id => "Akabeko";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new VigorPower(), 8, combat.Player);
}

/// <summary>Bag of Marbles: at combat start apply 1 Vulnerable to ALL enemies. (MegaCrit BagOfMarbles.)</summary>
public sealed class BagOfMarbles : RelicModel
{
    public override string Id => "BagOfMarbles";
    public override void OnCombatStart(CombatState combat)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.ApplyPower(combat, m, new VulnerablePower(), 1, combat.Player);
    }
}

/// <summary>Red Mask: at combat start apply 1 Weak to ALL enemies. (MegaCrit RedMask.)</summary>
public sealed class RedMask : RelicModel
{
    public override string Id => "RedMask";
    public override void OnCombatStart(CombatState combat)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.ApplyPower(combat, m, new WeakPower(), 1, combat.Player);
    }
}

/// <summary>Blood Vial: at the start of combat, heal 2 HP. (MegaCrit BloodVial.)</summary>
public sealed class BloodVial : RelicModel
{
    public override string Id => "BloodVial";
    public override void OnCombatStart(CombatState combat) => combat.Player.Heal(2);
}

// ---- Recurring / turn-numbered turn-start effects (player) ----

/// <summary>Sai: at the start of EACH turn, gain 7 Block. (MegaCrit Sai.)</summary>
public sealed class Sai : RelicModel
{
    public override string Id => "Sai";
    public override void OnPlayerTurnStart(CombatState combat) => Cmd.GainBlock(combat, combat.Player, 7, ValueProp.Unpowered, null);
}

/// <summary>Brimstone: at the start of EACH turn, gain 2 Strength and ALL enemies gain 1 Strength.
/// The enemy Strength (a downside) is modelled so the relic cannot read as a free upside. (MegaCrit Brimstone.)</summary>
public sealed class Brimstone : RelicModel
{
    public override string Id => "Brimstone";
    public override void OnPlayerTurnStart(CombatState combat)
    {
        Cmd.ApplyPower(combat, combat.Player, new StrengthPower(), 2, combat.Player);
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.ApplyPower(combat, m, new StrengthPower(), 1, combat.Player);
    }
}

/// <summary>Lantern: gain 1 extra energy on the first turn. (MegaCrit Lantern.)</summary>
public sealed class Lantern : RelicModel
{
    public override string Id => "Lantern";
    public override void OnPlayerTurnStart(CombatState combat) { if (combat.TurnNumber == 1) Cmd.GainEnergy(combat, 1); }
}

/// <summary>Very Hot Cocoa: gain 4 extra energy on the first turn. (MegaCrit VeryHotCocoa.)</summary>
public sealed class VeryHotCocoa : RelicModel
{
    public override string Id => "VeryHotCocoa";
    public override void OnPlayerTurnStart(CombatState combat) { if (combat.TurnNumber == 1) Cmd.GainEnergy(combat, 4); }
}

/// <summary>Candelabra: gain 2 extra energy on the second turn. (MegaCrit Candelabra.)</summary>
public sealed class Candelabra : RelicModel
{
    public override string Id => "Candelabra";
    public override void OnPlayerTurnStart(CombatState combat) { if (combat.TurnNumber == 2) Cmd.GainEnergy(combat, 2); }
}

/// <summary>Chandelier: gain 3 extra energy on the third turn. (MegaCrit Chandelier.)</summary>
public sealed class Chandelier : RelicModel
{
    public override string Id => "Chandelier";
    public override void OnPlayerTurnStart(CombatState combat) { if (combat.TurnNumber == 3) Cmd.GainEnergy(combat, 3); }
}

/// <summary>Festive Popper: on the first turn, deal 9 damage to ALL enemies. Unpowered (Strength does not add).
/// (MegaCrit FestivePopper.)</summary>
public sealed class FestivePopper : RelicModel
{
    public override string Id => "FestivePopper";
    public override void OnPlayerTurnStart(CombatState combat)
    {
        if (combat.TurnNumber != 1) return;
        foreach (var m in combat.LivingMonsters.ToList())
            if (m.IsAlive) Cmd.Attack(combat, combat.Player, m, 9, ValueProp.Unpowered, null);
    }
}

// ---- Passive modifiers (install a hidden relic power at combat start) ----

/// <summary>Ectoplasm: gain 1 additional energy at the start of each turn (and can't gain gold — gold is
/// HP-neutral, so the downside is inert for the solver). (MegaCrit Ectoplasm.)</summary>
public sealed class Ectoplasm : RelicModel
{
    public override string Id => "Ectoplasm";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new RelicMaxEnergyPower(), 1, combat.Player);
}

/// <summary>Strike Dummy: cards with "Strike" in their name deal 3 additional damage. (MegaCrit StrikeDummy.)</summary>
public sealed class StrikeDummy : RelicModel
{
    public override string Id => "StrikeDummy";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new RelicStrikeDamagePower(), 3, combat.Player);
}

/// <summary>Fake Strike Dummy: cards with "Strike" in their name deal 1 additional damage. (MegaCrit FakeStrikeDummy.)</summary>
public sealed class FakeStrikeDummy : RelicModel
{
    public override string Id => "FakeStrikeDummy";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new RelicStrikeDamagePower(), 1, combat.Player);
}

/// <summary>Miniature Cannon: upgraded attacks deal 3 additional damage. (MegaCrit MiniatureCannon.)</summary>
public sealed class MiniatureCannon : RelicModel
{
    public override string Id => "MiniatureCannon";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new RelicUpgradedDamagePower(), 3, combat.Player);
}

// ---- Batch 2: HP-loss reducers + passive modifiers (existing power hooks) ----

/// <summary>Tungsten Rod: whenever you would lose HP, lose 1 less. (MegaCrit TungstenRod.)</summary>
public sealed class TungstenRod : RelicModel
{
    public override string Id => "TungstenRod";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new RelicHpLossReductionPower(), 1, combat.Player);
}

/// <summary>The Boot: when you would deal 4 or less unblocked attack damage to an enemy, deal 5 instead.
/// (MegaCrit TheBoot.)</summary>
public sealed class TheBoot : RelicModel
{
    public override string Id => "TheBoot";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new RelicMinDamagePower(), 5, combat.Player);
}

/// <summary>Spiked Gauntlets: gain 1 additional energy each turn, but Power cards cost 1 more. Both the upside
/// (max energy) and the downside (Power surcharge) are modelled. (MegaCrit SpikedGauntlets.)</summary>
public sealed class SpikedGauntlets : RelicModel
{
    public override string Id => "SpikedGauntlets";
    public override void OnCombatStart(CombatState combat)
    {
        Cmd.ApplyPower(combat, combat.Player, new RelicMaxEnergyPower(), 1, combat.Player);
        Cmd.ApplyPower(combat, combat.Player, new RelicPowerCostSurchargePower(), 1, combat.Player);
    }
}

/// <summary>Pael's Blood: draw 1 additional card at the start of each turn. (MegaCrit PaelsBlood.)</summary>
public sealed class PaelsBlood : RelicModel
{
    public override string Id => "PaelsBlood";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new RelicDrawPower(), 1, combat.Player);
}

/// <summary>Blessed Antler: gain 1 additional energy each turn, but start each combat with 3 Dazed shuffled
/// into your draw pile. The Dazed dilution (a downside) is modelled so the +1 energy isn't free. (MegaCrit
/// BlessedAntler.)</summary>
public sealed class BlessedAntler : RelicModel
{
    public override string Id => "BlessedAntler";
    public override void OnCombatStart(CombatState combat)
    {
        Cmd.ApplyPower(combat, combat.Player, new RelicMaxEnergyPower(), 1, combat.Player);
        for (int i = 0; i < 3; i++) combat.Player.DrawPile.Add(new Dazed());
    }
}
