using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// Combat-affecting relic pool — batch 5. Two kinds: (a) "every N turns" relics, which need NO state — they read
// combat.TurnNumber directly (the game's per-turn counter fires on turns N, 2N, … ≡ TurnNumber % N == 0); and
// (b) damage/stars/play-count reactors, which install a hidden hashed-state power (RelicWave5Powers.cs).
// SOUNDNESS: deterministic ⇒ exact for the objective.

// ---- "Every N turns" relics (stateless — read combat.TurnNumber) ----

/// <summary>Happy Flower: gain 1 energy every 3rd turn. (MegaCrit HappyFlower.)</summary>
public sealed class HappyFlower : RelicModel
{
    public override string Id => "HappyFlower";
    public override void OnPlayerTurnStart(CombatState combat) { if (combat.TurnNumber % 3 == 0) Cmd.GainEnergy(combat, 1); }
}

/// <summary>Fake Happy Flower: gain 1 energy every 5th turn. (MegaCrit FakeHappyFlower.)</summary>
public sealed class FakeHappyFlower : RelicModel
{
    public override string Id => "FakeHappyFlower";
    public override void OnPlayerTurnStart(CombatState combat) { if (combat.TurnNumber % 5 == 0) Cmd.GainEnergy(combat, 1); }
}

/// <summary>Pendulum: draw 1 additional card every 3rd turn. (MegaCrit Pendulum.)</summary>
public sealed class Pendulum : EventRelic
{
    public override string Id => "Pendulum";
    public override int ModifyHandDraw(CombatState combat, int count) => combat.TurnNumber % 3 == 0 ? count + 1 : count;
}

/// <summary>Pollinous Core: draw 2 additional cards every 4th turn. (MegaCrit PollinousCore.)</summary>
public sealed class PollinousCore : EventRelic
{
    public override string Id => "PollinousCore";
    public override int ModifyHandDraw(CombatState combat, int count) => combat.TurnNumber % 4 == 0 ? count + 2 : count;
}

// ---- Damage / stars / play-count reactors (install a hidden hashed-state power) ----

/// <summary>Centennial Puzzle: the first unblocked damage you take in a combat draws 3 cards. (MegaCrit CentennialPuzzle.)</summary>
public sealed class CentennialPuzzle : RelicModel
{
    public override string Id => "CentennialPuzzle";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new CentennialPuzzlePower(), 1, combat.Player);
}

/// <summary>Demon Tongue: the first unblocked damage you take each turn, heal that much. (MegaCrit DemonTongue.)</summary>
public sealed class DemonTongue : RelicModel
{
    public override string Id => "DemonTongue";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new DemonTonguePower(), 1, combat.Player);
}

/// <summary>Galactic Dust: every 10 Stars spent, gain 10 Block. (MegaCrit GalacticDust.)</summary>
public sealed class GalacticDust : RelicModel
{
    public override string Id => "GalacticDust";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new GalacticDustPower(), 1, combat.Player);
}

/// <summary>Mini Regent: the first Stars you spend each turn grants 1 Strength. (MegaCrit MiniRegent.)</summary>
public sealed class MiniRegent : RelicModel
{
    public override string Id => "MiniRegent";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new MiniRegentPower(), 1, combat.Player);
}

/// <summary>Beating Remnant: you can lose at most 20 HP per turn. (MegaCrit BeatingRemnant.)</summary>
public sealed class BeatingRemnant : RelicModel
{
    public override string Id => "BeatingRemnant";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new BeatingRemnantPower(), 1, combat.Player);
}

/// <summary>Vambrace: the first block-granting card you play each combat grants double Block. (MegaCrit Vambrace.)</summary>
public sealed class Vambrace : RelicModel
{
    public override string Id => "Vambrace";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new VambracePower(), 1, combat.Player);
}

/// <summary>Throwing Axe: the first card you play each combat is played an extra time. (MegaCrit ThrowingAxe.)</summary>
public sealed class ThrowingAxe : RelicModel
{
    public override string Id => "ThrowingAxe";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new ThrowingAxePower(), 1, combat.Player);
}
