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
