using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// Combat-affecting relic pool — batch 4 (Wave 4). Stateful "every-Nth-play" / once-per-combat relics. Each
// installs a hidden counter power (RelicCounterPowers.cs) at combat start; the power carries the cloned + hashed
// counter and fires through the standard power AfterCardPlayed/AfterSideTurnStart hooks. The relic itself only
// uses OnCombatStart (so it needs no event-hook gate). SOUNDNESS: deterministic ⇒ exact for the objective.

/// <summary>Kunai: every 3rd Attack played in a turn, gain 1 Dexterity. (MegaCrit Kunai.)</summary>
public sealed class Kunai : RelicModel
{
    public override string Id => "Kunai";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new KunaiPower(), 1, combat.Player);
}

/// <summary>Shuriken: every 3rd Attack played in a turn, gain 1 Strength. (MegaCrit Shuriken.)</summary>
public sealed class Shuriken : RelicModel
{
    public override string Id => "Shuriken";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new ShurikenPower(), 1, combat.Player);
}

/// <summary>Ornamental Fan: every 3rd Attack played in a turn, gain 4 Block. (MegaCrit OrnamentalFan.)</summary>
public sealed class OrnamentalFan : RelicModel
{
    public override string Id => "OrnamentalFan";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new OrnamentalFanPower(), 1, combat.Player);
}

/// <summary>Letter Opener: every 3rd Skill played in a turn, deal 5 damage to ALL enemies. (MegaCrit LetterOpener.)</summary>
public sealed class LetterOpener : RelicModel
{
    public override string Id => "LetterOpener";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new LetterOpenerPower(), 1, combat.Player);
}

/// <summary>Nunchaku: every 10th Attack played, gain 1 energy. (MegaCrit Nunchaku.)</summary>
public sealed class Nunchaku : RelicModel
{
    public override string Id => "Nunchaku";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new NunchakuPower(), 1, combat.Player);
}

/// <summary>Tuning Fork: every 10th Skill played, gain 7 Block. (MegaCrit TuningFork.)</summary>
public sealed class TuningFork : RelicModel
{
    public override string Id => "TuningFork";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new TuningForkPower(), 1, combat.Player);
}

/// <summary>Iron Club: every 4th card played, draw 1 card. (MegaCrit IronClub.)</summary>
public sealed class IronClub : RelicModel
{
    public override string Id => "IronClub";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new IronClubPower(), 1, combat.Player);
}

/// <summary>Permafrost: the first Power you play each combat, gain 7 Block. (MegaCrit Permafrost.)</summary>
public sealed class Permafrost : RelicModel
{
    public override string Id => "Permafrost";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new PermafrostPower(), 1, combat.Player);
}

/// <summary>Rainbow Ring: the first turn each turn you play an Attack, a Skill and a Power, gain 1 Strength and
/// 1 Dexterity. (MegaCrit RainbowRing.)</summary>
public sealed class RainbowRing : RelicModel
{
    public override string Id => "RainbowRing";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new RainbowRingPower(), 1, combat.Player);
}
