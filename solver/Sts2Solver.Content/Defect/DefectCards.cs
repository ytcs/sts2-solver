using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// The Defect (orb-channeling character). Cards channel / evoke orbs (see the
// engine Orbs.cs subsystem) and scale on Focus and orb count. This is the first
// batch — starters + a spread across all five orb types + Focus + slots — added
// to as the rest of the 88-card pool is ported.
//
// Orb damage/block is unpowered (ValueProp.Unpowered) and resolves inside the
// orb itself; cards here just channel/evoke and deal their own card damage.
// ===========================================================================

// ---- Starters ----

/// <summary>Deal 6 damage. Upgrade: +3. (MegaCrit StrikeDefect)</summary>
public sealed class StrikeDefect : CardModel
{
    public override string Name => "StrikeDefect";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Damage => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Gain 5 Block. Upgrade: +3. (MegaCrit DefendDefect)</summary>
public sealed class DefendDefect : CardModel
{
    public override string Name => "DefendDefect";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.Self;
    public override bool IsDefend => true;   // Defend tag (Fasten boost)
    public int Block => 5 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Channel 1 Lightning orb. Cost 1. Upgrade: cost 0. (MegaCrit Zap)</summary>
public sealed class Zap : CardModel
{
    public override string Name => "Zap";
    public override int BaseCost => 1;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);   // upgrade: cost −1
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => OrbOps.Channel(combat, new LightningOrb());
}

/// <summary>Evoke your next (oldest) Orb twice (its effect fires twice, then it leaves the queue). Cost 1.
/// Upgrade: cost 0. (MegaCrit Dualcast — EvokeNext without dequeue, then EvokeNext with dequeue.)</summary>
public sealed class Dualcast : CardModel
{
    public override string Name => "Dualcast";
    public override int BaseCost => 1;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Player.Orbs.Count == 0) return;
        OrbOps.EvokeFront(combat, dequeue: false);   // fire the front orb's evoke effect…
        OrbOps.EvokeFront(combat, dequeue: true);    // …again, this time removing it
    }
}

// ---- Common ----

/// <summary>Deal 7 damage. Channel 1 Lightning orb. Upgrade: +3 damage. (MegaCrit BallLightning)</summary>
public sealed class BallLightning : CardModel
{
    public override string Name => "BallLightning";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 7 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        OrbOps.Channel(combat, new LightningOrb());
    }
}

/// <summary>Channel 1 Frost orb, then draw 1 card. Upgrade: draw 2. (MegaCrit Coolheaded). The draw acts on
/// no hand state, so it defers cleanly in search.</summary>
public sealed class Coolheaded : CardModel
{
    public override string Name => "Coolheaded";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Cards => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        OrbOps.Channel(combat, new FrostOrb());
        Cmd.Draw(combat, Cards);
    }
}

/// <summary>Deal 6 damage. Channel 1 Frost orb. Upgrade: +3 damage. (MegaCrit ColdSnap)</summary>
public sealed class ColdSnap : CardModel
{
    public override string Name => "ColdSnap";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        OrbOps.Channel(combat, new FrostOrb());
    }
}

/// <summary>Deal 3 damage. Apply 1 Vulnerable. Cost 0. Upgrade: +1 damage, +1 Vulnerable. (MegaCrit BeamCell)</summary>
public sealed class BeamCell : CardModel
{
    public override string Name => "BeamCell";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 3 + Upgrades;
    public int Vulnerable => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive) Cmd.ApplyPower(combat, play.Target, new VulnerablePower(), Vulnerable, combat.Player);
    }
}

/// <summary>Deal 5 damage, once per Orb you have. Upgrade: +2 damage. (MegaCrit Barrage — hit count = orbs
/// currently in the queue.)</summary>
public sealed class Barrage : CardModel
{
    public override string Name => "Barrage";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, combat.Player.Orbs.Count, ValueProp.Move, this);
}

// ---- Uncommon ----

/// <summary>Channel a Frost orb for each enemy. Exhaust. Cost 0. Upgrade: no longer Exhausts. (MegaCrit Chill)</summary>
public sealed class Chill : CardModel
{
    public override string Name => "Chill";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var _ in combat.LivingMonsters.ToList()) OrbOps.Channel(combat, new FrostOrb());
    }
}

/// <summary>Gain 6 Block. Channel 2 Frost orbs. Cost 2. Upgrade: +3 Block. (MegaCrit Glacier)</summary>
public sealed class Glacier : CardModel
{
    public override string Name => "Glacier";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        OrbOps.Channel(combat, new FrostOrb());
        OrbOps.Channel(combat, new FrostOrb());
    }
}

/// <summary>Power: gain 2 Orb slots. Cost 1. Upgrade: 3 slots. (MegaCrit Capacitor)</summary>
public sealed class Capacitor : CardModel
{
    public override string Name => "Capacitor";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Slots => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => OrbOps.AddSlots(combat, Slots);
}

/// <summary>Channel 1 Dark orb, then trigger every Dark orb's passive once (twice upgraded) — front-loading
/// their accumulated evoke damage. Cost 1. (MegaCrit Darkness)</summary>
public sealed class Darkness : CardModel
{
    public override string Name => "Darkness";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Triggers => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        OrbOps.Channel(combat, new DarkOrb());
        foreach (var orb in combat.Player.Orbs.OfType<DarkOrb>().ToList())
            for (int i = 0; i < Triggers; i++) orb.Passive(combat);
    }
}

// ---- Rare ----

/// <summary>Power: gain 1 Focus. Cost 1. Upgrade: 2 Focus. (MegaCrit Defragment)</summary>
public sealed class Defragment : CardModel
{
    public override string Name => "Defragment";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Focus => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new FocusPower(), Focus, combat.Player);
}

// ===========================================================================
// Batch 2 — pure orb/damage/block/draw/energy cards (no new powers or
// mechanisms). All numbers verified 1:1 against the decompile.
// ===========================================================================

// ---- Block-only skills ----

/// <summary>Gain 10 Block. Innate, Exhaust. Cost 0. Upgrade: +3 Block. (MegaCrit BootSequence)</summary>
public sealed class BootSequence : CardModel
{
    public override string Name => "BootSequence";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override bool Innate => true;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Block => 10 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Gain 9 Block. Cost 1. Upgrade: +3 Block. (MegaCrit Leap)</summary>
public sealed class Leap : CardModel
{
    public override string Name => "Leap";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 9 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Gain 5 Block. Channel 1 Glass orb. Cost 1. Upgrade: +3 Block. (MegaCrit Glasswork)</summary>
public sealed class Glasswork : CardModel
{
    public override string Name => "Glasswork";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 5 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        OrbOps.Channel(combat, new GlassOrb());
    }
}

/// <summary>Gain 11 Block. Channel 1 Dark orb. Cost 2. Upgrade: +4 Block. (MegaCrit ShadowShield)</summary>
public sealed class ShadowShield : CardModel
{
    public override string Name => "ShadowShield";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 11 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        OrbOps.Channel(combat, new DarkOrb());
    }
}

/// <summary>Gain 7 Block. Gain 1 energy next turn. Cost 1. Upgrade: +3 Block. (MegaCrit ChargeBattery)</summary>
public sealed class ChargeBattery : CardModel
{
    public override string Name => "ChargeBattery";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 7 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new EnergyNextTurnPower(), 1, combat.Player);
    }
}

// ---- Draw / energy skills ----

/// <summary>Draw 3 cards. Cost 1. Upgrade: draw 4. (MegaCrit Skim)</summary>
public sealed class Skim : CardModel
{
    public override string Name => "Skim";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Cards => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => Cmd.Draw(combat, Cards);
}

/// <summary>Gain 4 energy. Exhaust. Cost 0. Upgrade: gain 6 energy. (MegaCrit Supercritical)</summary>
public sealed class Supercritical : CardModel
{
    public override string Name => "Supercritical";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Energy => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => Cmd.GainEnergy(combat, Energy);
}

// ---- Orb-channeling skills ----

/// <summary>Channel 1 Plasma orb. Exhaust. Cost 1. Upgrade: no longer Exhausts. (MegaCrit Fusion)</summary>
public sealed class Fusion : CardModel
{
    public override string Name => "Fusion";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) => OrbOps.Channel(combat, new PlasmaOrb());
}

/// <summary>Channel 1 Lightning, 1 Frost, and 1 Dark orb. Exhaust. Cost 2. Upgrade: no longer Exhausts.
/// (MegaCrit Rainbow)</summary>
public sealed class Rainbow : CardModel
{
    public override string Name => "Rainbow";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        OrbOps.Channel(combat, new LightningOrb());
        OrbOps.Channel(combat, new FrostOrb());
        OrbOps.Channel(combat, new DarkOrb());
    }
}

// ---- Attacks ----

/// <summary>Deal 9 damage twice. Channel 2 Glass orbs. Cost 3. Upgrade: +3 damage. (MegaCrit Refract)</summary>
public sealed class Refract : CardModel
{
    public override string Name => "Refract";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 9 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, 2, ValueProp.Move, this);
        OrbOps.Channel(combat, new GlassOrb());
        OrbOps.Channel(combat, new GlassOrb());
    }
}

/// <summary>Deal 19 damage. Channel 3 Frost orbs. Cost 3. Upgrade: +5 damage. (MegaCrit IceLance)</summary>
public sealed class IceLance : CardModel
{
    public override string Name => "IceLance";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 19 + 5 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        for (int i = 0; i < 3; i++) OrbOps.Channel(combat, new FrostOrb());
    }
}

/// <summary>Deal 24 damage. Channel 3 Plasma orbs. Cost 5. Upgrade: +6 damage. A Strike. (MegaCrit MeteorStrike)</summary>
public sealed class MeteorStrike : CardModel
{
    public override string Name => "MeteorStrike";
    public override int BaseCost => 5;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Damage => 24 + 6 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        for (int i = 0; i < 3; i++) OrbOps.Channel(combat, new PlasmaOrb());
    }
}

/// <summary>Deal 6 damage to ALL enemies. Draw 1 card. Cost 1. Upgrade: +3 damage. (MegaCrit SweepingBeam)</summary>
public sealed class SweepingBeam : CardModel
{
    public override string Name => "SweepingBeam";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
        Cmd.Draw(combat, 1);
    }
}

/// <summary>Deal 10 damage. Apply 2 Weak. Channel 1 Dark orb. Cost 2. Upgrade: +3 damage, +1 Weak.
/// (MegaCrit Null)</summary>
public sealed class Null : CardModel
{
    public override string Name => "Null";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 10 + 3 * Upgrades;
    public int Weak => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive) Cmd.ApplyPower(combat, play.Target, new WeakPower(), Weak, combat.Player);
        OrbOps.Channel(combat, new DarkOrb());
    }
}

// ===========================================================================
// Batch 3 — power cards (apply a new orb-reactive / turn-boundary PowerModel,
// see DefectPowers.cs) + the temporary-Focus / FreePower / replay cards.
// All numbers verified 1:1 against the decompile.
// ===========================================================================

/// <summary>Power: whenever you evoke a Lightning orb, deal 6 to all enemies. Cost 1. Upgrade: +2. (Thunder)</summary>
public sealed class Thunder : CardModel
{
    public override string Name => "Thunder";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Damage => 6 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new ThunderPower(), Damage, combat.Player);
}

/// <summary>Power: at turn end, if you have a Frost orb, deal 6 to all enemies. Cost 1. Upgrade: +2. (Hailstorm)</summary>
public sealed class Hailstorm : CardModel
{
    public override string Name => "Hailstorm";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Damage => 6 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new HailstormPower(), Damage, combat.Player);
}

/// <summary>Power: whenever you play a Power, channel 1 Lightning orb. Cost 1. Upgrade: +1. (Storm)</summary>
public sealed class Storm : CardModel
{
    public override string Name => "Storm";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Orbs => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new StormPower(), Orbs, combat.Player);
}

/// <summary>Power: whenever you play a Power, gain 1 energy. Cost 1. Upgrade: cost 0. (Subroutine)</summary>
public sealed class Subroutine : CardModel
{
    public override string Name => "Subroutine";
    public override int BaseCost => 1;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new SubroutinePower(), 1, combat.Player);
}

/// <summary>Power: at turn start, gain 2 Block per distinct orb type you have. Cost 1. Upgrade: +1. (Coolant)</summary>
public sealed class Coolant : CardModel
{
    public override string Name => "Coolant";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Block => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new CoolantPower(), Block, combat.Player);
}

/// <summary>Power: whenever you generate a Status card, deal 5 to all enemies. Cost 1. Upgrade: +2. (Smokestack)</summary>
public sealed class Smokestack : CardModel
{
    public override string Name => "Smokestack";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Damage => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new SmokestackPower(), Damage, combat.Player);
}

/// <summary>Power: at turn start, trigger your front orb's passive 1 extra time. Cost 1. Upgrade: +1. (Loop)</summary>
public sealed class Loop : CardModel
{
    public override string Name => "Loop";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Triggers => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new LoopPower(), Triggers, combat.Player);
}

/// <summary>Power: at turn start, channel 1 Glass orb. Upgrade: also channel 1 Glass now. Cost 1. (Spinner)</summary>
public sealed class Spinner : CardModel
{
    public override string Name => "Spinner";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (Upgrades > 0) OrbOps.Channel(combat, new GlassOrb());
        Cmd.ApplyPower(combat, combat.Player, new SpinnerPower(), 1, combat.Player);
    }
}

/// <summary>Gain 4 Block. Power: at the start of each turn, channel 1 Lightning orb (for 2 turns). Cost 1.
/// Upgrade: +3 Block. (LightningRod)</summary>
public sealed class LightningRod : CardModel
{
    public override string Name => "LightningRod";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 4 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new LightningRodPower(), 2, combat.Player);
    }
}

/// <summary>Power: gain 4 Focus. At the start of each turn, lose 1 Focus. Cost 1. Upgrade: +1 Focus.
/// (BiasedCognition — an Ancient card.)</summary>
public sealed class BiasedCognition : CardModel
{
    public override string Name => "BiasedCognition";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Ancient;
    public override TargetType Target => TargetType.Self;
    public int Focus => 4 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, combat.Player, new FocusPower(), Focus, combat.Player);
        Cmd.ApplyPower(combat, combat.Player, new BiasedCognitionPower(), 1, combat.Player);
    }
}

/// <summary>Power: channel 2 Dark orbs. At turn end, evoke your newest orb. Cost 2. Upgrade: channel 3 Dark.
/// (ConsumingShadow)</summary>
public sealed class ConsumingShadow : CardModel
{
    public override string Name => "ConsumingShadow";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int DarkOrbs => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        for (int i = 0; i < DarkOrbs; i++) OrbOps.Channel(combat, new DarkOrb());
        Cmd.ApplyPower(combat, combat.Player, new ConsumingShadowPower(), 1, combat.Player);
    }
}

/// <summary>Power: prevent the next instance of damage. Cost 2. Upgrade: +1. (Buffer)</summary>
public sealed class Buffer : CardModel
{
    public override string Name => "Buffer";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Charges => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new BufferPower(), Charges, combat.Player);
}

/// <summary>Power: the first Status card you draw each turn, draw 2 cards. Cost 1. Upgrade: +1. (Iteration)</summary>
public sealed class Iteration : CardModel
{
    public override string Name => "Iteration";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Cards => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new IterationPower(), Cards, combat.Player);
}

/// <summary>Gain 2 Focus until end of turn. Exhaust. Cost 0. Upgrade: no longer Exhausts. (Hotfix)</summary>
public sealed class Hotfix : CardModel
{
    public override string Name => "Hotfix";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) => TemporaryFocusPower.Grant(combat, 2);
}

/// <summary>Deal 9 damage. Gain 1 Focus until end of turn. Cost 1. Upgrade: +2 damage, +1 Focus. A Strike.
/// (FocusedStrike)</summary>
public sealed class FocusedStrike : CardModel
{
    public override string Name => "FocusedStrike";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Damage => 9 + 2 * Upgrades;
    public int Focus => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        TemporaryFocusPower.Grant(combat, Focus);
    }
}

/// <summary>Gain (2 × distinct orb types) Focus until end of turn. Exhaust. Cost 1. Upgrade: no longer
/// Exhausts. (Synchronize)</summary>
public sealed class Synchronize : CardModel
{
    public override string Name => "Synchronize";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int distinct = combat.Player.Orbs.Select(o => o.Name).Distinct().Count();
        TemporaryFocusPower.Grant(combat, 2 * distinct);
    }
}

/// <summary>Deal 14 damage. The next Power you play this combat costs 0. Cost 2. Upgrade: +6 damage. (Synthesis)</summary>
public sealed class Synthesis : CardModel
{
    public override string Name => "Synthesis";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 14 + 6 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new FreePowerPower(), 1, combat.Player);
    }
}

/// <summary>The next Power card you play is played twice. Exhaust. Cost 1. Upgrade: cost 0. (SignalBoost)</summary>
public sealed class SignalBoost : CardModel
{
    public override string Name => "SignalBoost";
    public override int BaseCost => 1;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new SignalBoostPower(), 1, combat.Player);
}

/// <summary>Power: the first card you play each turn is played twice. Ethereal. Cost 3. Upgrade: no longer
/// Ethereal. (EchoForm)</summary>
public sealed class EchoForm : CardModel
{
    public override string Name => "EchoForm";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override bool Ethereal => Upgrades == 0;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new EchoFormPower(), 1, combat.Player);
}

/// <summary>Power: draw 1 additional card each turn. Cost 1. Upgrade: Innate. INERT draw bonus (sound — see
/// MachineLearningPower). (MachineLearning)</summary>
public sealed class MachineLearning : CardModel
{
    public override string Name => "MachineLearning";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override bool Innate => Upgrades > 0;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new MachineLearningPower(), 1, combat.Player);
}

/// <summary>Power: whenever you generate a Status card, channel a random orb. Cost 1. Upgrade: Innate. INERT
/// random orb-gen (sound — see TrashToTreasurePower). (TrashToTreasure)</summary>
public sealed class TrashToTreasure : CardModel
{
    public override string Name => "TrashToTreasure";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override bool Innate => Upgrades > 0;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new TrashToTreasurePower(), 1, combat.Player);
}

/// <summary>Power: before each turn's draw, add a random Power card to hand. Cost 3. Upgrade: cost 2. INERT
/// RNG card-gen (sound — see CreativeAiPower). (CreativeAi)</summary>
public sealed class CreativeAi : CardModel
{
    public override string Name => "CreativeAi";
    public override int BaseCost => 3;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new CreativeAiPower(), 1, combat.Player);
}

/// <summary>Power: when you play a 0-cost Attack, return it to your hand (once per turn). Cost 2. Upgrade:
/// cost 1. INERT return-to-hand (sound — see FeralPower). (Feral)</summary>
public sealed class Feral : CardModel
{
    public override string Name => "Feral";
    public override int BaseCost => 2;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new FeralPower(), 1, combat.Player);
}

// ===========================================================================
// Batch 4 — the remaining pool: status-generating attacks/skills, X-cost orb
// cards, gated-counter scalers (Voltaic/HelixDrill), Stateful cost-mutators,
// and a handful of sound pessimistic approximations for RNG/loop/choice cards.
// All numbers verified 1:1 against the decompile.
// ===========================================================================

internal static class DefectFx
{
    /// <summary>Exhaust a specific card wherever it sits in the player's piles (FlakCannon / Scavenge),
    /// firing the on-exhaust hook. No-op if not found.</summary>
    public static void ExhaustCard(CombatState combat, CardModel card)
    {
        var p = combat.Player;
        if (p.Hand.Remove(card) || p.DrawPile.Remove(card) || p.DiscardPile.Remove(card))
        {
            p.ExhaustPile.Add(card);
            combat.CardExhaustedThisTurn = true;
            foreach (var pw in combat.AllPowers.ToList()) pw.AfterCardExhausted(combat, card, false);
        }
    }

    public static int DistinctOrbs(CombatState combat) => combat.Player.Orbs.Select(o => o.Name).Distinct().Count();
}

// ---- Status-generating cards ----

/// <summary>Gain 6 Block. Add a Dazed to your discard. Cost 0. Upgrade: +3 Block. (BoostAway)</summary>
public sealed class BoostAway : CardModel
{
    public override string Name => "BoostAway";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.GenerateStatusCard(combat, new Dazed(), combat.Player.DiscardPile);
    }
}

/// <summary>Gain 13 Block. Add 2 Wounds to your discard. Cost 1. Upgrade: +4 Block. (FightThrough)</summary>
public sealed class FightThrough : CardModel
{
    public override string Name => "FightThrough";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 13 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        for (int i = 0; i < 2; i++) Cmd.GenerateStatusCard(combat, new Wound(), combat.Player.DiscardPile);
    }
}

/// <summary>Deal 4 damage 3 times. Add a Slimed to your discard. Cost 1. Upgrade: +1 damage. (GunkUp)</summary>
public sealed class GunkUp : CardModel
{
    public override string Name => "GunkUp";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 4 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, 3, ValueProp.Move, this);
        Cmd.GenerateStatusCard(combat, new Slimed(), combat.Player.DiscardPile);
    }
}

/// <summary>Draw 2 cards. Add a Burn to your discard. Cost 0. Upgrade: draw 3. (Overclock)</summary>
public sealed class Overclock : CardModel
{
    public override string Name => "Overclock";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Cards => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Draw(combat, Cards);
        Cmd.GenerateStatusCard(combat, new Burn(), combat.Player.DiscardPile);
    }
}

/// <summary>Gain 2 energy. Add a Void to your discard. Cost 0. Upgrade: gain 3 energy. (Turbo)</summary>
public sealed class Turbo : CardModel
{
    public override string Name => "Turbo";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Energy => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainEnergy(combat, Energy);
        Cmd.GenerateStatusCard(combat, new Void(), combat.Player.DiscardPile);
    }
}

// ---- Other attacks / skills ----

/// <summary>Deal 3 damage. Apply 1 Weak if the enemy intends to attack. Cost 0. Upgrade: +1 damage, +1 Weak.
/// (GoForTheEyes)</summary>
public sealed class GoForTheEyes : CardModel
{
    public override string Name => "GoForTheEyes";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 3 + Upgrades;
    public int Weak => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target is Monster m && m.IsAlive && m.IntendsToAttack)
            Cmd.ApplyPower(combat, m, new WeakPower(), Weak, combat.Player);
    }
}

/// <summary>Deal 28 damage to ALL enemies. Lose 3 Focus. Cost 2. Upgrade: +8 damage. (Hyperbeam)</summary>
public sealed class Hyperbeam : CardModel
{
    public override string Name => "Hyperbeam";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 28 + 8 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new FocusPower(), -3, combat.Player);
    }
}

/// <summary>Deal 24 damage. If it kills the target, gain 3 energy. Cost 3. Upgrade: +8 damage. (Sunder)</summary>
public sealed class Sunder : CardModel
{
    public override string Name => "Sunder";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 24 + 8 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (!play.Target!.IsAlive) Cmd.GainEnergy(combat, 3);
    }
}

/// <summary>Deal 13 damage. Draw 1 card. Cost 2. Upgrade: +1 damage, draw 2. (RocketPunch — the
/// status-generation cost-reduction is unmodelled: it would need an in-hand card-generation hook; omitting it
/// keeps the card at full cost, a sound pessimistic gap.)</summary>
public sealed class RocketPunch : CardModel
{
    public override string Name => "RocketPunch";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 13 + Upgrades;
    public int Cards => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.Draw(combat, Cards);
    }
}

/// <summary>Gain energy equal to your current energy (double it). Exhaust. Cost 1. Upgrade: cost 0.
/// (DoubleEnergy)</summary>
public sealed class DoubleEnergy : CardModel
{
    public override string Name => "DoubleEnergy";
    public override int BaseCost => 1;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) => Cmd.GainEnergy(combat, combat.Player.Energy);
}

/// <summary>Gain 2 energy. Exhaust. Cost 1. Upgrade: 3 energy. Multiplayer-only in the real game; here it
/// simply grants the player energy. (EnergySurge)</summary>
public sealed class EnergySurge : CardModel
{
    public override string Name => "EnergySurge";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Energy => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => Cmd.GainEnergy(combat, Energy);
}

/// <summary>Channel 1 Plasma orb (to an ally — here, yourself). Exhaust. Cost 1. Upgrade: no longer Exhausts.
/// Multiplayer-only in the real game. (Ignition)</summary>
public sealed class Ignition : CardModel
{
    public override string Name => "Ignition";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) => OrbOps.Channel(combat, new PlasmaOrb());
}

/// <summary>Power: remove 1 orb slot. Gain 2 Strength and 2 Dexterity. Cost 2. Upgrade: +1 each. (BulkUp)</summary>
public sealed class BulkUp : CardModel
{
    public override string Name => "BulkUp";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        OrbOps.RemoveSlots(combat, 1);
        Cmd.ApplyPower(combat, combat.Player, new StrengthPower(), Amount, combat.Player);
        Cmd.ApplyPower(combat, combat.Player, new DexterityPower(), Amount, combat.Player);
    }
}

/// <summary>Gain 6 Block. Transform all Status cards in your hand into Fuel. Cost 1. Upgrade: +1 Block, the
/// Fuel is upgraded. (Compact)</summary>
public sealed class Compact : CardModel
{
    public override string Name => "Compact";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 6 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        var hand = combat.Player.Hand;
        for (int i = 0; i < hand.Count; i++)
            if (hand[i].Type == CardType.Status)
                hand[i] = (CardModel)new Fuel().Upgraded(Upgrades > 0 ? 1 : 0);
    }
}

/// <summary>Deal 7 damage. Draw 1 card per distinct orb type you have. Cost 1. Upgrade: +3 damage.
/// (CompileDriver)</summary>
public sealed class CompileDriver : CardModel
{
    public override string Name => "CompileDriver";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 7 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.Draw(combat, DefectFx.DistinctOrbs(combat));
    }
}

/// <summary>Deal 18 damage. Add a copy of this card (costing 0) to your discard. Cost 2. Upgrade: +5 damage.
/// A Strike. (AdaptiveStrike) — the generated copy is a 0-cost Stateful instance.</summary>
public sealed class AdaptiveStrike : CardModel
{
    public override string Name => "AdaptiveStrike";
    public override int BaseCost => 2;
    private bool _free;
    public override int Cost => _free ? 0 : BaseCost;
    public override bool Stateful => true;   // ALWAYS — a conditional flag would let unplayed copies be shared by ref then mutated
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Damage => 18 + 5 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        var copy = (AdaptiveStrike)Clone();
        copy._free = true;
        combat.Player.DiscardPile.Add(copy);
    }
    public override string StateKey() => (_free ? "AdaptiveStrike0" : "AdaptiveStrike") + (Upgrades > 0 ? $"+{Upgrades}" : "");
}

/// <summary>Deal 10 damage. Return all 0-cost non-Attack cards from your discard to your hand. Cost 2.
/// Upgrade: +4 damage. (AllForOne — filter: 0-cost, not X-cost, type Skill/Power/Status.)</summary>
public sealed class AllForOne : CardModel
{
    public override string Name => "AllForOne";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 10 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        var ret = combat.Player.DiscardPile.Where(c =>
            !c.IsXCost && c.EffectiveCost(combat) == 0 &&
            (c.Type == CardType.Skill || c.Type == CardType.Power || c.Type == CardType.Status)).ToList();
        foreach (var c in ret) { combat.Player.DiscardPile.Remove(c); combat.Player.Hand.Add(c); }
    }
}

/// <summary>Gain 3 Block, Exhaust, choose a card from your discard to return to your hand. Cost 1. Upgrade:
/// +2 Block, no longer Exhausts. (Hologram) — the choice is modelled as a fixed default (the oldest discarded
/// card), which is pessimistic-but-sound (the real player would pick at least as well).</summary>
public sealed class Hologram : CardModel
{
    public override string Name => "Hologram";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public int Block => 3 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        if (combat.Player.DiscardPile.Count > 0)
        {
            var c = combat.Player.DiscardPile[0];
            combat.Player.DiscardPile.RemoveAt(0);
            combat.Player.Hand.Add(c);
        }
    }
}

/// <summary>Exhaust 1 card from your hand. Gain 2 energy next turn. Cost 1. Upgrade: +1 energy. (Scavenge) —
/// the exhaust choice is a fixed default (the first hand card), pessimistic-but-sound.</summary>
public sealed class Scavenge : CardModel
{
    public override string Name => "Scavenge";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Energy => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Player.Hand.Count > 0) DefectFx.ExhaustCard(combat, combat.Player.Hand[0]);
        Cmd.ApplyPower(combat, combat.Player, new EnergyNextTurnPower(), Energy, combat.Player);
    }
}

/// <summary>Deal 7 damage. Draw 4 cards, then discard the drawn cards that don't cost 0. Cost 1. Upgrade:
/// +3 damage, draw 5. (Scrape) — modelled only under a concrete Rng (rollout / replay); in pure search the
/// draw-then-selective-discard is omitted (sound: deferring the draw without the discard would be optimistic,
/// so the whole draw is skipped — a pessimistic under-credit).</summary>
public sealed class Scrape : CardModel
{
    public override string Name => "Scrape";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 7 + 3 * Upgrades;
    public int Cards => 4 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (combat.Rng == null) return;                      // search: skip the draw (sound under-credit)
        int before = combat.Player.Hand.Count;
        Cmd.Draw(combat, Cards);
        foreach (var c in combat.Player.Hand.Skip(before).ToList())
            if (c.IsXCost || c.EffectiveCost(combat) != 0)
            {
                combat.Player.Hand.Remove(c);
                combat.Player.DiscardPile.Add(c);
            }
    }
}

/// <summary>Shuffle your hand into your draw pile, then draw 4. Exhaust. Cost 0. Upgrade: draw 6. (Reboot) —
/// draw-pile order is irrelevant to the solver's hypergeometric draw model, so the shuffle is a no-op move.</summary>
public sealed class Reboot : CardModel
{
    public override string Name => "Reboot";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Cards => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        combat.Player.DrawPile.AddRange(combat.Player.Hand);
        combat.Player.Hand.Clear();
        Cmd.Draw(combat, Cards);
    }
}

/// <summary>Deal 3 damage. Add this card (costing 0) to your hand — no: set this card's cost to 0 for the
/// rest of combat. Cost 1. Upgrade: +3 damage. A Strike. (MomentumStrike) — Stateful cost-zero flag.</summary>
public sealed class MomentumStrike : CardModel
{
    public override string Name => "MomentumStrike";
    public override int BaseCost => 1;
    private bool _zeroed;
    public override int Cost => _zeroed ? 0 : BaseCost;
    public override bool Stateful => true;   // ALWAYS — playing it mutates _zeroed; a shared-by-ref copy would corrupt siblings
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Damage => 10 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        _zeroed = true;
    }
    public override string StateKey() => (_zeroed ? "MomentumStrike0" : "MomentumStrike") + (Upgrades > 0 ? $"+{Upgrades}" : "");
}

/// <summary>Add 1 orb slot. Draw 1 card. This card's cost increases by 1 this combat. Cost 0. Upgrade: draw 2.
/// (Modded) — Stateful escalating cost.</summary>
public sealed class Modded : CardModel
{
    public override string Name => "Modded";
    public override int BaseCost => 0;
    private int _bump;
    public override int Cost => _bump;
    public override bool Stateful => true;   // ALWAYS — playing it mutates _bump; a shared-by-ref copy would corrupt siblings
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Cards => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        OrbOps.AddSlots(combat, 1);
        Cmd.Draw(combat, Cards);
        _bump++;
    }
    public override string StateKey() => $"Modded{(_bump > 0 ? "#" + _bump : "")}{(Upgrades > 0 ? "+" + Upgrades : "")}";
}

/// <summary>Gain 1 Block. Exhaust. Cost 1. (GeneticAlgorithm) — its block PERMANENTLY grows by 3 each play,
/// but that scaling persists across COMBATS (the card exhausts after a single in-combat play), so within a
/// single-combat solve it is just "Gain 1 Block, Exhaust"; the cross-combat scaling is out of scope.</summary>
public sealed class GeneticAlgorithm : CardModel
{
    public override string Name => "GeneticAlgorithm";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, 1, ValueProp.Move, this);
}

/// <summary>Deal 3 damage. Increase ALL Claws' damage by 2 this combat. Cost 0. Upgrade: +1 damage, +1
/// increase. (Claw) — Stateful per-instance damage bonus, buffed across every Claw in the deck on each play.</summary>
public sealed class Claw : CardModel
{
    public override string Name => "Claw";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool Stateful => true;
    private int _bonus;
    public int Damage => 3 + Upgrades + _bonus;
    public int Increase => 2 + Upgrades;
    internal void BuffFromClawPlay(int n) => _bonus += n;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        var p = combat.Player;
        foreach (var c in p.Hand.Concat(p.DrawPile).Concat(p.DiscardPile).Concat(p.ExhaustPile).OfType<Claw>())
            c.BuffFromClawPlay(Increase);
    }
    public override string StateKey() => $"Claw{(Upgrades > 0 ? "+" + Upgrades : "")}{(_bonus != 0 ? "/" + _bonus : "")}";
}

/// <summary>Deal 3 damage once per energy you spent this turn. Cost 0. Upgrade: +2 damage. (HelixDrill) —
/// hit count = energy spent this turn (gated counter).</summary>
public sealed class HelixDrill : CardModel
{
    public override string Name => "HelixDrill";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool TracksEnergySpentThisTurn => true;
    public int Damage => 3 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, combat.EnergySpentThisTurn, ValueProp.Move, this);
}

/// <summary>Deal 8 damage once per Status card you hold (in any pile but exhaust). Exhaust all those Status
/// cards. Cost 2. Upgrade: +3 damage. Hits a random enemy (first, by the search default). (FlakCannon)</summary>
public sealed class FlakCannon : CardModel
{
    public override string Name => "FlakCannon";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    // Game: TargetType.RandomEnemy, each hit TargetingRandomOpponents. Modelled (like RipAndTear/Ricochet) as
    // the first living enemy per hit — NOT AnyEnemy, which would let the search pick the best target to
    // concentrate the multi-hit burst (an over-credit in multi-enemy fights). Identical for single-enemy.
    public override TargetType Target => TargetType.RandomEnemy;
    public int Damage => 8 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        var p = combat.Player;
        var statuses = p.Hand.Concat(p.DrawPile).Concat(p.DiscardPile).Where(c => c.Type == CardType.Status).ToList();
        int hits = statuses.Count;
        foreach (var s in statuses) DefectFx.ExhaustCard(combat, s);
        for (int i = 0; i < hits; i++)
        {
            var target = combat.LivingMonsters.FirstOrDefault();   // re-evaluated per hit: overkill spills to the next enemy
            if (target == null) break;
            Cmd.Attack(combat, combat.Player, target, Damage, ValueProp.Move, this);
        }
    }
}

/// <summary>Deal 5 damage. If you've played fewer than 3 cards this turn, draw 1 card. Cost 0. Upgrade:
/// +1 damage, +1 to the play threshold. (Ftl) — the play-count gate reads the per-turn play counter, so an
/// Ftl deck hashes it (LoopRiskDraw).</summary>
public sealed class Ftl : CardModel
{
    public override string Name => "Ftl";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool LoopRiskDraw => true;   // its draw gate reads PlaysThisTurn → must be hashed
    public int Damage => 5 + Upgrades;
    public int PlayMax => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (combat.PlaysThisTurn < PlayMax) Cmd.Draw(combat, 1);
    }
}

/// <summary>Deal 3 damage. Trigger the passive of each of your Lightning orbs. Cost 0. Upgrade: +1 damage,
/// trigger twice. (TeslaCoil)</summary>
public sealed class TeslaCoil : CardModel
{
    public override string Name => "TeslaCoil";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        int triggers = Upgrades > 0 ? 2 : 1;
        foreach (var orb in combat.Player.Orbs.OfType<LightningOrb>().ToList())
            for (int i = 0; i < triggers && !combat.IsCombatOver; i++) orb.Passive(combat);
    }
}

// ---- Orb evoke / channel cards ----

/// <summary>Evoke your front orb 4 times. Cost 1. Upgrade: cost 0. (Quadcast — an Ancient card.)</summary>
public sealed class Quadcast : CardModel
{
    public override string Name => "Quadcast";
    public override int BaseCost => 1;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Ancient;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Player.Orbs.Count == 0) return;
        for (int i = 0; i < 4; i++) OrbOps.EvokeFront(combat, dequeue: i == 3);
    }
}

/// <summary>Deal 7 damage to ALL enemies. Evoke every orb twice (each then leaves the queue). Cost 1.
/// Upgrade: +4 damage. (Shatter)</summary>
public sealed class Shatter : CardModel
{
    public override string Name => "Shatter";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 7 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
        int n = combat.Player.Orbs.Count;
        for (int i = 0; i < n; i++)
        {
            OrbOps.EvokeFront(combat, dequeue: false);
            OrbOps.EvokeFront(combat, dequeue: true);
        }
    }
}

/// <summary>X-cost: evoke your front orb X times. Upgrade: X+1. (MultiCast)</summary>
public sealed class MultiCast : CardModel
{
    public override string Name => "MultiCast";
    public override int BaseCost => 0;
    public override bool IsXCost => true;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Player.Orbs.Count == 0) return;
        int n = play.XValue + (Upgrades > 0 ? 1 : 0);
        for (int i = 0; i < n; i++) OrbOps.EvokeFront(combat, dequeue: i == n - 1);
    }
}

/// <summary>X-cost: channel X Lightning orbs. Upgrade: X+1. (Tempest)</summary>
public sealed class Tempest : CardModel
{
    public override string Name => "Tempest";
    public override int BaseCost => 0;
    public override bool IsXCost => true;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int n = play.XValue + (Upgrades > 0 ? 1 : 0);
        for (int i = 0; i < n; i++) OrbOps.Channel(combat, new LightningOrb());
    }
}

/// <summary>Channel a Lightning orb for each Lightning orb you've channeled this combat. Exhaust. Cost 3.
/// Upgrade: no longer Exhausts. (Voltaic — gated combat counter.)</summary>
public sealed class Voltaic : CardModel
{
    public override string Name => "Voltaic";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override bool TracksLightningChanneledThisCombat => true;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int n = combat.LightningsChanneledThisCombat;
        for (int i = 0; i < n; i++) OrbOps.Channel(combat, new LightningOrb());
    }
}

/// <summary>Deal 6 damage twice, then auto-play a random Attack from your draw pile. Cost 2. Upgrade:
/// +2 damage. (Uproar) — the random auto-play is a RNG card selection, never a search decision, so it is left
/// inert (a sound pessimistic under-credit); only the 2-hit damage is modelled.</summary>
public sealed class Uproar : CardModel
{
    public override string Name => "Uproar";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 6 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, 2, ValueProp.Move, this);
}

/// <summary>Channel a random orb (1×, +1/upg). Cost 1. (Chaos) — random orb-type selection is never a search
/// decision (picking the best would be optimistically unsound), so the channel is left INERT, a sound
/// pessimistic under-credit. (Documented out-of-scope, like the other RNG-generation cards.)</summary>
public sealed class Chaos : CardModel
{
    public override string Name => "Chaos";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play) { /* inert: random orb-gen (sound) */ }
}

/// <summary>Add a random Power card to your hand (free this turn). Exhaust. Cost 1. Upgrade: cost 0.
/// (WhiteNoise) — full-pool RNG card-generation is documented out-of-scope; the generation is left INERT
/// (sound pessimistic under-credit).</summary>
public sealed class WhiteNoise : CardModel
{
    public override string Name => "WhiteNoise";
    public override int BaseCost => 1;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) { /* inert: random power-gen (sound) */ }
}
