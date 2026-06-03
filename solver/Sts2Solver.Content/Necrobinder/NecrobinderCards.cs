using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// The Necrobinder's 88-card pool plus the two tokens (Soul, SweepingGaze). HP-faithful: damage, block,
// summons, Doom, debuffs and the Osty interactions are modelled exactly; subsystems the engine does not
// simulate — mid-combat card generation (Soul/SweepingGaze/CallOfTheVoid), card selection/transform from
// piles, the draw COUNT, upgrades-in-combat and out-of-combat rewards — are documented inert inline (they
// are HP-neutral, or are reconstructed by the trace validator when replaying recorded runs).

// ─────────────────────────── Basics / starters ───────────────────────────

/// <summary>Deal 6 damage. Upgrade: +3. (Strike)</summary>
public sealed class StrikeNecrobinder : CardModel
{
    public override string Name => "StrikeNecrobinder";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Damage => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Gain 5 block. Upgrade: +3. (Defend)</summary>
public sealed class DefendNecrobinder : CardModel
{
    public override string Name => "DefendNecrobinder";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.Self;
    public override bool IsDefend => true;   // Defend tag (Fasten boost)
    public int Block => 5 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Summon 5. Upgrade: +2. (Bodyguard)</summary>
public sealed class Bodyguard : CardModel
{
    public override string Name => "Bodyguard";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.Self;
    public int Summon => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => NecroOsty.Summon(combat, Summon);
}

/// <summary>Osty attack: deal 6 + Osty's current HP. Upgrade base +3. (Unleash)</summary>
public sealed class Unleash : CardModel
{
    public override string Name => "Unleash";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsOstyAttack => true;
    public int Base => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Player.IsOstyMissing) return;
        NecroOsty.OstyHit(combat, play.Target!, Base + combat.Player.Osty!.CurrentHp, this);
    }
}

// ─────────────────────────── Attacks ───────────────────────────

/// <summary>Deal 8. Apply Doom equal to the damage dealt. Upgrade: +2. (Blight Strike)</summary>
public sealed class BlightStrike : CardModel
{
    public override string Name => "BlightStrike";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Damage => 8 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int dealt = Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (dealt > 0 && play.Target!.IsAlive) Cmd.ApplyPower(combat, play.Target, new DoomPower(), dealt, combat.Player);
    }
}

/// <summary>Deal 33 to ALL enemies. Costs 2 less per Ethereal card played this combat. Upgrade: cost 9→7.
/// (Banshee's Cry)</summary>
public sealed class BansheesCry : CardModel
{
    public override string Name => "BansheesCry";
    public override int BaseCost => 9;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AllEnemies;
    public override int Cost => 9 - 2 * Upgrades;
    public override int EffectiveCost(CombatState combat) => Math.Max(0, Cost - 2 * combat.EtherealPlayedThisCombat);
    public int Damage => 33;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
    }
}

/// <summary>Deal 52. Upgrade: +11. (Bury)</summary>
public sealed class Bury : CardModel
{
    public override string Name => "Bury";
    public override int BaseCost => 4;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 52 + 11 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 3 unblockable damage. Add 3 Souls to the draw pile. Upgrade: damage +1, Souls +1. The
/// Soul generation is inert (mid-combat card generation is not modelled). (Capture Spirit)</summary>
public sealed class CaptureSpirit : CardModel
{
    public override string Name => "CaptureSpirit";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyDamage(combat, play.Target!, Damage, ValueProp.Unblockable | ValueProp.Unpowered, combat.Player);
}

/// <summary>Deal 8 damage. Apply 1 Vulnerable. Ethereal. Upgrade: +1 / +1. (Fear)</summary>
public sealed class Fear : CardModel
{
    public override string Name => "Fear";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool Ethereal => true;
    public int Damage => 7 + Upgrades;
    public int Vulnerable => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive) Cmd.ApplyPower(combat, play.Target, new VulnerablePower(), Vulnerable, combat.Player);
    }
}

/// <summary>Deal 13. Ethereal. Upgrade: +4. (Defile)</summary>
public sealed class Defile : CardModel
{
    public override string Name => "Defile";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool Ethereal => true;
    public int Damage => 13 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 10 damage. Apply 2 Debilitate. Upgrade: +2 / +1. (Debilitate)</summary>
public sealed class Debilitate : CardModel
{
    public override string Name => "Debilitate";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 10 + 2 * Upgrades;
    public int Power => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive) Cmd.ApplyPower(combat, play.Target, new DebilitatePower(), Power, combat.Player);
    }
}

/// <summary>Deal 8 + (4 × cards drawn by effects this turn). Upgrade: base +1, extra +2. The draw-scaling
/// term is 0 in search (mid-combat draw is a chance node, not modelled deterministically). (Death March)</summary>
public sealed class DeathMarch : CardModel
{
    public override string Name => "DeathMarch";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Base => 8 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Base, ValueProp.Move, this);
}

/// <summary>Deal 11, X times (X = energy spent). Retain. Upgrade: +3. (Eradicate)</summary>
public sealed class Eradicate : CardModel
{
    public override string Name => "Eradicate";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsXCost => true;
    public override bool Retain => true;
    public int Damage => 11 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, play.XValue, ValueProp.Move, this);
}

/// <summary>Deal 10. Apply HangPower so Hang cards deal more, doubling each play. Upgrade: +3. (Hang)</summary>
public sealed class Hang : CardModel
{
    public override string Name => "Hang";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 10 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (!play.Target!.IsAlive) return;
        int add = Math.Max(2, play.Target.GetPowerAmount("Hang"));   // game: Max(2, current) -> doubles
        Cmd.ApplyPower(combat, play.Target, new HangPower(), add, combat.Player);
    }
}

/// <summary>Deal 7. Spread the target's debuffs to all other enemies. 0-cost. Upgrade: +2, Retain. (Misery)</summary>
public sealed class Misery : CardModel
{
    public override string Name => "Misery";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool Retain => Upgrades > 0;
    public int Damage => 7 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        var target = play.Target!;
        var debuffs = target.Powers
            .Where(p => p.Type == PowerType.Debuff && p is not TemporaryStrengthPower)
            .Select(p => (power: p, amount: p.Amount)).ToList();
        Cmd.Attack(combat, combat.Player, target, Damage, ValueProp.Move, this);
        foreach (var m in combat.LivingMonsters.ToList())
        {
            if (m == target) continue;
            foreach (var (power, amount) in debuffs)
                Cmd.ApplyPower(combat, m, power.Clone(), amount, combat.Player);
        }
    }
}

/// <summary>Deal 5, X times (X = Ethereal cards played this combat). Upgrade: +2. (Pull from Below)</summary>
public sealed class PullFromBelow : CardModel
{
    public override string Name => "PullFromBelow";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, combat.EtherealPlayedThisCombat, ValueProp.Move, this);
}

/// <summary>Deal 27. Retain. Upgrade: +6. (Reap)</summary>
public sealed class Reap : CardModel
{
    public override string Name => "Reap";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool Retain => true;
    public int Damage => 27 + 6 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 9. Add 1 Soul to the draw pile (inert generation). Upgrade: +2. (Reave)</summary>
public sealed class Reave : CardModel
{
    public override string Name => "Reave";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 9 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 13. Upgrade: +5. (Severance — its 3-Soul generation is inert.)</summary>
public sealed class Severance : CardModel
{
    public override string Name => "Severance";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 13 + 5 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 8 to ALL enemies. Retain. Upgrade: +3. (Sow)</summary>
public sealed class Sow : CardModel
{
    public override string Name => "Sow";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AllEnemies;
    public override bool Retain => true;
    public int Damage => 8 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
    }
}

/// <summary>Deal 9 + (2 × Souls in your exhaust pile). Upgrade: extra +1. (Soul Storm)</summary>
public sealed class SoulStorm : CardModel
{
    public override string Name => "SoulStorm";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Base => 9;
    public int Extra => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int souls = combat.Player.ExhaustPile.Count(c => c.Name == "Soul");
        Cmd.Attack(combat, combat.Player, play.Target!, Base + Extra * souls, ValueProp.Move, this);
    }
}

/// <summary>Deal 13, growing +3 each play this combat (permanent). Exhaust. Upgrade: increase +1. (The Scythe)</summary>
public sealed class TheScythe : CardModel
{
    public override string Name => "TheScythe";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override bool Stateful => true;

    private int _bonus;
    public int Increase => 3 + Upgrades;
    public int Damage => 13 + _bonus;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        _bonus += Increase;
    }
    public override CardModel Clone() { var c = (TheScythe)base.Clone(); c._bonus = _bonus; return c; }
    public override string StateKey() => _bonus > 0 ? $"TheScythe#{_bonus}{(Upgrades > 0 ? $"+{Upgrades}" : "")}" : base.StateKey();
}

/// <summary>Deal damage equal to the target's Doom. Exhaust. Upgrade: Retain. (Time's Up)</summary>
public sealed class TimesUp : CardModel
{
    public override string Name => "TimesUp";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public override bool Retain => Upgrades > 0;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, play.Target!.GetPowerAmount("Doom"), ValueProp.Move, this);
}

/// <summary>Deal 10. Apply 1 Veilpiercer (next Ethereal card costs 0). Upgrade: +3. (Veilpiercer)</summary>
public sealed class Veilpiercer : CardModel
{
    public override string Name => "Veilpiercer";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 10 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new VeilpiercerPower(), 1, combat.Player);
    }
}

/// <summary>Deal 9, Strike, Ethereal. Make a hand card Ethereal (selection inert). Upgrade: +3. (Sculpting Strike)</summary>
public sealed class SculptingStrike : CardModel
{
    public override string Name => "SculptingStrike";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public override bool Ethereal => true;
    public int Damage => 9 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 10. Drain power from upgrades in discard (inert). Upgrade: +2. (Drain Power)</summary>
public sealed class DrainPower : CardModel
{
    public override string Name => "DrainPower";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 10 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 4. Return a card from discard to hand (selection inert). Exhaust (removed on upgrade).
/// Upgrade: +2. (Graveblast)</summary>
public sealed class Graveblast : CardModel
{
    public override string Name => "Graveblast";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public int Damage => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

// ─────────────────────────── Osty attacks ───────────────────────────

/// <summary>Osty attack: 9 to ALL enemies, gain 9 block, then Osty dies (NecroMastery reflects its HP).
/// Upgrade: +3 / +3. (Bone Shards)</summary>
public sealed class BoneShards : CardModel
{
    public override string Name => "BoneShards";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AllEnemies;
    public override bool IsOstyAttack => true;
    public int Damage => 9 + 3 * Upgrades;
    public int Block => 9 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Player.IsOstyMissing) return;
        NecroOsty.OstyHitAll(combat, Damage, this);
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        if (combat.Player.IsOstyAlive) Cmd.Kill(combat, combat.Player.Osty!);
    }
}

/// <summary>Osty attack: 3. The first time each turn, draw 1 (inert). 0-cost. Upgrade: +3. (Fetch)</summary>
public sealed class Fetch : CardModel
{
    public override string Name => "Fetch";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsOstyAttack => true;
    public int Damage => 3 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => NecroOsty.OstyHit(combat, play.Target!, Damage, this);
}

/// <summary>Osty attack: 12. Costs 0 if Osty has already attacked this turn. Upgrade: +4. (Flatten)</summary>
public sealed class Flatten : CardModel
{
    public override string Name => "Flatten";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsOstyAttack => true;
    public override int EffectiveCost(CombatState combat) => combat.OstyAttacksThisTurn > 0 ? 0 : Cost;
    public int Damage => 12 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => NecroOsty.OstyHit(combat, play.Target!, Damage, this);
}

/// <summary>Osty attack: 11 to ALL enemies. Apply 2 Vulnerable to all. Playable only with Osty present.
/// Upgrade: +2 / +1. (High Five)</summary>
public sealed class HighFive : CardModel
{
    public override string Name => "HighFive";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AllEnemies;
    public override bool IsOstyAttack => true;
    public int Damage => 11 + 2 * Upgrades;
    public int Vulnerable => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Player.IsOstyMissing) return;
        NecroOsty.OstyHitAll(combat, Damage, this);
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.ApplyPower(combat, m, new VulnerablePower(), Vulnerable, combat.Player);
    }
}

/// <summary>Osty attack: 6. 0-cost. Upgrade: +3. (Poke)</summary>
public sealed class Poke : CardModel
{
    public override string Name => "Poke";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsOstyAttack => true;
    public int Damage => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => NecroOsty.OstyHit(combat, play.Target!, Damage, this);
}

/// <summary>Osty attack: 10 + Osty's MAX HP. Upgrade: cost 1→0, base +5. (Protector)</summary>
public sealed class Protector : CardModel
{
    public override string Name => "Protector";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Ancient;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsOstyAttack => true;
    public override int Cost => Math.Max(0, 1 - Upgrades);
    public int Base => 10 + 5 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Player.IsOstyMissing) return;
        NecroOsty.OstyHit(combat, play.Target!, Base + combat.Player.Osty!.MaxHp, this);
    }
}

/// <summary>Osty attack: 7, hitting (1 + Osty attacks already made this turn) times. Upgrade: +2. (Rattle)</summary>
public sealed class Rattle : CardModel
{
    public override string Name => "Rattle";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsOstyAttack => true;
    public int Damage => 7 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => NecroOsty.OstyHit(combat, play.Target!, Damage, this, 1 + combat.OstyAttacksThisTurn);
}

/// <summary>Osty attack: 4. (If 2+ energy was spent, returns to hand — inert card-advantage loop.) 0-cost.
/// Upgrade: +2. (Right Hand Hand)</summary>
public sealed class RightHandHand : CardModel
{
    public override string Name => "RightHandHand";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsOstyAttack => true;
    public int Damage => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => NecroOsty.OstyHit(combat, play.Target!, Damage, this);
}

/// <summary>Osty attack: 5. Apply 2 Sic 'Em (Osty hits on it summon). Upgrade: +1 / +1. (Sic 'Em)</summary>
public sealed class SicEm : CardModel
{
    public override string Name => "SicEm";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsOstyAttack => true;
    public int Damage => 5 + Upgrades;
    public int Power => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        NecroOsty.OstyHit(combat, play.Target!, Damage, this);
        if (play.Target!.IsAlive) Cmd.ApplyPower(combat, play.Target, new SicEmPower(), Power, combat.Player);
    }
}

/// <summary>Osty attack: 7. Make a hand card Retain (selection inert). Upgrade: +3. (Snap)</summary>
public sealed class Snap : CardModel
{
    public override string Name => "Snap";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsOstyAttack => true;
    public int Damage => 7 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => NecroOsty.OstyHit(combat, play.Target!, Damage, this);
}

/// <summary>Osty attack: 25 + (5 × other OstyAttack cards in your deck). Upgrade: base +5, extra +1. (Squeeze)</summary>
public sealed class Squeeze : CardModel
{
    public override string Name => "Squeeze";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsOstyAttack => true;
    public int Base => 25 + 5 * Upgrades;
    public int Extra => 5 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Player.IsOstyMissing) return;
        var p = combat.Player;
        int others = p.Hand.Concat(p.DrawPile).Concat(p.DiscardPile).Concat(p.ExhaustPile)
            .Count(c => c.IsOstyAttack && !ReferenceEquals(c, this));
        NecroOsty.OstyHit(combat, play.Target!, Base + Extra * others, this);
    }
}

// ─────────────────────────── Skills: summon / block ───────────────────────────

/// <summary>Summon 6. Exhaust. Upgrade: +3. (Afterlife)</summary>
public sealed class Afterlife : CardModel
{
    public override string Name => "Afterlife";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Summon => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => NecroOsty.Summon(combat, Summon);
}

/// <summary>Summon 3, then exhaust a card from the draw pile (selection inert). Exhaust. Upgrade: +2. (Cleanse)</summary>
public sealed class Cleanse : CardModel
{
    public override string Name => "Cleanse";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Summon => 3 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => NecroOsty.Summon(combat, Summon);
}

/// <summary>Summon 3, X times (X = energy spent). Add X Souls (inert). Exhaust. Upgrade: summon +1. (Dirge)</summary>
public sealed class Dirge : CardModel
{
    public override string Name => "Dirge";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override bool IsXCost => true;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Summon => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        for (int i = 0; i < play.XValue; i++) NecroOsty.Summon(combat, Summon);
    }
}

/// <summary>Summon 20. Exhaust. Upgrade: +5. (Reanimate)</summary>
public sealed class Reanimate : CardModel
{
    public override string Name => "Reanimate";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Summon => 20 + 5 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => NecroOsty.Summon(combat, Summon);
}

/// <summary>Summon 4. Gain 7 block. Upgrade: +1 / +2. (Pull Aggro)</summary>
public sealed class PullAggro : CardModel
{
    public override string Name => "PullAggro";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Summon => 4 + Upgrades;
    public int Block => 7 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        NecroOsty.Summon(combat, Summon);
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
    }
}

/// <summary>Summon 3. Heal Osty 5. Retain. Upgrade: +2 / +2. (Spur)</summary>
public sealed class Spur : CardModel
{
    public override string Name => "Spur";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override bool Retain => true;
    public int Summon => 3 + 2 * Upgrades;
    public int HealAmount => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        NecroOsty.Summon(combat, Summon);
        combat.Player.Osty?.Heal(HealAmount);
    }
}

/// <summary>If Osty is alive, gain block equal to twice its MAX HP, then Osty dies (NecroMastery reflects
/// its HP). Retain. Upgrade: cost 1→0. (Sacrifice)</summary>
public sealed class Sacrifice : CardModel
{
    public override string Name => "Sacrifice";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override bool Retain => true;
    public override int Cost => Math.Max(0, 1 - Upgrades);
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Player.IsOstyMissing) return;
        int block = combat.Player.Osty!.MaxHp * 2;
        Cmd.Kill(combat, combat.Player.Osty!);
        Cmd.GainBlock(combat, combat.Player, block, ValueProp.Move, this);
    }
}

/// <summary>Gain 8 block. Add 1 Soul (inert generation). Upgrade: +3. (Grave Warden)</summary>
public sealed class GraveWarden : CardModel
{
    public override string Name => "GraveWarden";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 8 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Gain 11 block. Gain 1 energy next turn. Upgrade: +2 / +1. (Delay)</summary>
public sealed class Delay : CardModel
{
    public override string Name => "Delay";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 11 + 2 * Upgrades;
    public int EnergyNext => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new EnergyNextTurnPower(), EnergyNext, combat.Player);
    }
}

/// <summary>Gain 13 block. (When destroyed, costs 1 less — deck mechanic, inert.) Upgrade: +4. (Melancholy)</summary>
public sealed class Melancholy : CardModel
{
    public override string Name => "Melancholy";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 13 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Gain 6 block, once — three times if you applied Doom this turn. Upgrade: +1. (Death's Door)</summary>
public sealed class DeathsDoor : CardModel
{
    public override string Name => "DeathsDoor";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 6 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int gains = 1 + (combat.DoomAppliedThisTurn ? 2 : 0);
        for (int i = 0; i < gains; i++) Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
    }
}

/// <summary>Gain 7 block. Add a copy of this card to your discard pile. 0-cost. Upgrade: +2. (Undeath)</summary>
public sealed class Undeath : CardModel
{
    public override string Name => "Undeath";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Block => 7 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        combat.Player.DiscardPile.Add(new Undeath().Upgraded(Upgrades));
    }
}

/// <summary>Gain 6 block. Apply 1 Weak. Ethereal. Upgrade: +3. (Defy)</summary>
public sealed class Defy : CardModel
{
    public override string Name => "Defy";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool Ethereal => true;
    public int Block => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        if (play.Target is { IsAlive: true }) Cmd.ApplyPower(combat, play.Target, new WeakPower(), 1, combat.Player);
    }
}

/// <summary>Gain 5 block. Apply 7 Doom to ALL enemies. Upgrade: +1 / +4. (Negative Pulse)</summary>
public sealed class NegativePulse : CardModel
{
    public override string Name => "NegativePulse";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AllEnemies;
    public int Block => 5 + Upgrades;
    public int Doom => 7 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.ApplyPower(combat, m, new DoomPower(), Doom, combat.Player);
    }
}

// ─────────────────────────── Skills: Doom / debuff / utility ───────────────────────────

/// <summary>Apply 21 Doom and 1 Weak to ALL enemies. Upgrade: Doom +5. (Deathbringer)</summary>
public sealed class Deathbringer : CardModel
{
    public override string Name => "Deathbringer";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AllEnemies;
    public int Doom => 21 + 5 * Upgrades;
    public int Weak => 1;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
        {
            Cmd.ApplyPower(combat, m, new DoomPower(), Doom, combat.Player);
            Cmd.ApplyPower(combat, m, new WeakPower(), Weak, combat.Player);
        }
    }
}

/// <summary>Apply 29 Doom to ALL enemies, then kill any doomed enemy. Upgrade: +8. (End of Days)</summary>
public sealed class EndOfDays : CardModel
{
    public override string Name => "EndOfDays";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AllEnemies;
    public int Doom => 29 + 8 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.ApplyPower(combat, m, new DoomPower(), Doom, combat.Player);
        DoomPower.KillDoomed(combat, combat.LivingMonsters);
    }
}

/// <summary>Apply 8 Strength loss to an enemy this turn (Ethereal). Upgrade: +3. (Enfeebling Touch)</summary>
public sealed class EnfeeblingTouch : CardModel
{
    public override string Name => "EnfeeblingTouch";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool Ethereal => true;
    public int StrengthLoss => 8 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, play.Target!, new EnfeeblingTouchPower(), StrengthLoss, combat.Player);
}

/// <summary>Apply Doom = 10 + 5×floor(target Doom / 10). Upgrade: base +5. (No Escape)</summary>
public sealed class NoEscape : CardModel
{
    public override string Name => "NoEscape";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Base => 10 + 5 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int doom = play.Target!.GetPowerAmount("Doom");
        Cmd.ApplyPower(combat, play.Target, new DoomPower(), Base + 5 * (doom / 10), combat.Player);
    }
}

/// <summary>Apply 3 Oblivion (each card you play this turn applies Doom). 0-cost. Upgrade: +1. (Oblivion)</summary>
public sealed class Oblivion : CardModel
{
    public override string Name => "Oblivion";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Amount => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, play.Target!, new OblivionPower(), Amount, combat.Player);
}

/// <summary>Apply 13 Doom. Draw 1 (inert). Upgrade: +3 / +1. (Scourge)</summary>
public sealed class Scourge : CardModel
{
    public override string Name => "Scourge";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Doom => 13 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, play.Target!, new DoomPower(), Doom, combat.Player);
}

/// <summary>Apply 2 Weak and 2 Vulnerable. Exhaust. Upgrade: +1. (Putrefy)</summary>
public sealed class Putrefy : CardModel
{
    public override string Name => "Putrefy";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Amount => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, play.Target!, new WeakPower(), Amount, combat.Player);
        if (play.Target!.IsAlive) Cmd.ApplyPower(combat, play.Target, new VulnerablePower(), Amount, combat.Player);
    }
}

/// <summary>You and an enemy each lose 2 Strength (enemy upgradeable). Exhaust. Upgrade: enemy +1. (Shared Fate)</summary>
public sealed class SharedFate : CardModel
{
    public override string Name => "SharedFate";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int PlayerLoss => 2;
    public int EnemyLoss => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, combat.Player, new StrengthPower(), -PlayerLoss, combat.Player);
        Cmd.ApplyPower(combat, play.Target!, new StrengthPower(), -EnemyLoss, combat.Player);
    }
}

/// <summary>Exhaust your whole hand; if you exhausted 9+ cards, gain 1 Intangible. Exhaust. Upgrade: cost
/// 2→1. (Eidolon)</summary>
public sealed class Eidolon : CardModel
{
    public override string Name => "Eidolon";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override int Cost => Math.Max(0, 2 - Upgrades);
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int n = 0;
        foreach (var c in combat.Player.Hand.ToList()) { Cmd.ExhaustFromHand(combat, c); n++; }
        if (n >= 9) Cmd.ApplyPower(combat, combat.Player, new IntangiblePower(), 1, combat.Player);
    }
}

/// <summary>Gain 1 energy. Exhaust. Upgrade: Retain. (Wisp)</summary>
public sealed class Wisp : CardModel
{
    public override string Name => "Wisp";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override bool Retain => Upgrades > 0;
    public override void OnPlay(CombatState combat, CardPlay play) => Cmd.GainEnergy(combat, 1);
}

/// <summary>Draw 3. Ethereal. Upgrade: +1. Draw is inert in search (chance node). (Parse)</summary>
public sealed class Parse : CardModel
{
    public override string Name => "Parse";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override bool Ethereal => true;
    public int Cards => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => Cmd.Draw(combat, Cards);
}

/// <summary>Pull a card from discard to hand (selection inert). Upgrade: Retain. (Dredge)</summary>
public sealed class Dredge : CardModel
{
    public override string Name => "Dredge";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override bool Retain => Upgrades > 0;
    public override void OnPlay(CombatState combat, CardPlay play) { /* card selection — inert */ }
}

/// <summary>Transform draw-pile cards into Souls (transform — inert). Ethereal. Upgrade: cost 1→0. (Seance)</summary>
public sealed class Seance : CardModel
{
    public override string Name => "Seance";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override bool Ethereal => true;
    public override int Cost => Math.Max(0, 1 - Upgrades);
    public override void OnPlay(CombatState combat, CardPlay play) { /* card transform — inert */ }
}

/// <summary>Add 1 to a hand card's cost and give it +1 Replay (deck/replay mechanic — inert). Exhaust
/// (removed on upgrade). (Transfigure)</summary>
public sealed class Transfigure : CardModel
{
    public override string Name => "Transfigure";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) { /* replay-count mechanic — inert */ }
}

/// <summary>Multiplayer-only: gift allies Souls (inert in single-player). Exhaust. (Glimpse Beyond)</summary>
public sealed class GlimpseBeyond : CardModel
{
    public override string Name => "GlimpseBeyond";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AllAllies;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) { /* multiplayer-only — inert */ }
}

/// <summary>Multiplayer-only: summon 6 for every ally. In single-player, summons 6 for you. Exhaust.
/// Upgrade: +2. (Legion of Bone)</summary>
public sealed class LegionOfBone : CardModel
{
    public override string Name => "LegionOfBone";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AllAllies;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Summon => 6 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => NecroOsty.Summon(combat, Summon);
}

// ─────────────────────────── Powers ───────────────────────────

/// <summary>Power: Osty's attacks deal +4. Upgrade: +2. (Calcify)</summary>
public sealed class Calcify : CardModel
{
    public override string Name => "Calcify";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new CalcifyPower(), Amount, combat.Player);
}

/// <summary>Power: at the start of each turn, apply 6 Doom to an enemy. Upgrade: +3. (Countdown)</summary>
public sealed class Countdown : CardModel
{
    public override string Name => "Countdown";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new CountdownPower(), Amount, combat.Player);
}

/// <summary>Power: before playing a card costing 2+, gain 4 block. Upgrade: +2. (Danse Macabre)</summary>
public sealed class DanseMacabre : CardModel
{
    public override string Name => "DanseMacabre";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new DanseMacabrePower(), Amount, combat.Player);
}

/// <summary>Power: +1 card drawn (inert) and +1 max energy each turn. Ethereal. Upgrade: cost 3→2. (Demesne)</summary>
public sealed class Demesne : CardModel
{
    public override string Name => "Demesne";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override bool Ethereal => true;
    public override int Cost => Math.Max(0, 3 - Upgrades);
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new DemesnePower(), 1, combat.Player);
}

/// <summary>Power: playing a Soul summons 1. Upgrade: +1. (Devour Life)</summary>
public sealed class DevourLife : CardModel
{
    public override string Name => "DevourLife";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Amount => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new DevourLifePower(), Amount, combat.Player);
}

/// <summary>Power: lose 2 Strength, gain 1 energy each turn. Upgrade: lose 1 less Strength. (Friendship)</summary>
public sealed class Friendship : CardModel
{
    public override string Name => "Friendship";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int StrengthLoss => 2 - Upgrades;
    public int Energy => 1;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, combat.Player, new StrengthPower(), -StrengthLoss, combat.Player);
        Cmd.ApplyPower(combat, combat.Player, new FriendshipPower(), Energy, combat.Player);
    }
}

/// <summary>Power: playing a Soul deals 6 unblockable to an enemy. Upgrade: +2. (Haunt)</summary>
public sealed class Haunt : CardModel
{
    public override string Name => "Haunt";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 6 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new HauntPower(), Amount, combat.Player);
}

/// <summary>Power: your first Attack each turn deals +50% damage. Upgrade: +25%. (Lethality)</summary>
public sealed class Lethality : CardModel
{
    public override string Name => "Lethality";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override bool Ethereal => true;
    public int Amount => 50 + 25 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new LethalityPower(), Amount, combat.Player);
}

/// <summary>Power: summon 5 and gain NecroMastery (Osty's HP loss is reflected to all enemies).
/// Upgrade: summon +3. (Necro Mastery)</summary>
public sealed class NecroMastery : CardModel
{
    public override string Name => "NecroMastery";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Summon => 5 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        NecroOsty.Summon(combat, Summon);
        Cmd.ApplyPower(combat, combat.Player, new NecroMasteryPower(), 1, combat.Player);
    }
}

/// <summary>Power: gain 3 energy, draw 2 (inert), and each turn gain 3 Doom yourself. Upgrade: energy +1.
/// (Neurosurge)</summary>
public sealed class Neurosurge : CardModel
{
    public override string Name => "Neurosurge";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Energy => 3 + Upgrades;
    public int Amount => 3;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainEnergy(combat, Energy);
        Cmd.Draw(combat, 2);
        Cmd.ApplyPower(combat, combat.Player, new NeurosurgePower(), Amount, combat.Player);
    }
}

/// <summary>Power: after a powered attack from you or Osty, apply Doom = damage dealt. (Reaper Form)</summary>
public sealed class ReaperForm : CardModel
{
    public override string Name => "ReaperForm";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override bool Retain => Upgrades > 0;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new ReaperFormPower(), 1, combat.Player);
}

/// <summary>Power: applying Doom gains 2 block. Upgrade: +1. (Shroud)</summary>
public sealed class Shroud : CardModel
{
    public override string Name => "Shroud";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new ShroudPower(), Amount, combat.Player);
}

/// <summary>Power: applying a (non-temporary) debuff to an enemy deals 9 to it. Upgrade: +4. (Sleight of Flesh)</summary>
public sealed class SleightOfFlesh : CardModel
{
    public override string Name => "SleightOfFlesh";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 9 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new SleightOfFleshPower(), Amount, combat.Player);
}

/// <summary>Power: before playing an Ethereal card, gain 4 block. Upgrade: +1. (Spirit of Ash)</summary>
public sealed class SpiritOfAsh : CardModel
{
    public override string Name => "SpiritOfAsh";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Amount => 4 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new SpiritOfAshPower(), Amount, combat.Player);
}

/// <summary>Power: gain 4 energy this turn; cards cost 1 more this turn. Upgrade: energy +2. (Borrowed Time)</summary>
public sealed class BorrowedTime : CardModel
{
    public override string Name => "BorrowedTime";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Energy => 4 + 2 * Upgrades;
    public int ExtraCost => 1;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainEnergy(combat, Energy);
        Cmd.ApplyPower(combat, combat.Player, new BorrowedTimePower(), ExtraCost, combat.Player);
    }
}

/// <summary>Power: each turn, add 1 random Ethereal card to hand (inert). Rare. Upgrade: Innate (inert).
/// (Call of the Void)</summary>
public sealed class CallOfTheVoid : CardModel
{
    public override string Name => "CallOfTheVoid";
    public override bool Innate => Upgrades > 0;   // Upgrade: Innate (guaranteed in the opening hand)
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new CallOfTheVoidPower(), 1, combat.Player);
}

/// <summary>Power: each turn, add a SweepingGaze to hand (inert generation). Upgrade: cost 2→1. (Sentry Mode)</summary>
public sealed class SentryMode : CardModel
{
    public override string Name => "SentryMode";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override int Cost => Math.Max(0, 2 - Upgrades);
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new SentryModePower(), 1, combat.Player);
}

// ForbiddenGrimoire (Ancient) is the same inert card as the Event/Ancient pool's — it lives in
// Content/Special/ (and is registered there), not duplicated in the Necrobinder module.

/// <summary>Power: each turn, when you draw an Ethereal card, draw 1 (inert). Upgrade: cost 1→0. (Pagestorm)</summary>
public sealed class Pagestorm : CardModel
{
    public override string Name => "Pagestorm";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override int Cost => Math.Max(0, 1 - Upgrades);
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new PagestormPower(), 1, combat.Player);
}

/// <summary>Summon 2 and gain 2 energy at the start of next turn. Upgrade: +1 / +1. (Invoke)</summary>
public sealed class Invoke : CardModel
{
    public override string Name => "Invoke";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Summon => 2 + Upgrades;
    public int Energy => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, combat.Player, new SummonNextTurnPower(), Summon, combat.Player);
        Cmd.ApplyPower(combat, combat.Player, new EnergyNextTurnPower(), Energy, combat.Player);
    }
}

// ─────────────────────────── Tokens ───────────────────────────

/// <summary>Token: draw 2. Exhaust. Triggers Haunt/Devour Life. Upgrade: +1. Draw is inert in search. (Soul)</summary>
public sealed class Soul : CardModel
{
    public override string Name => "Soul";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Token;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Cards => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => Cmd.Draw(combat, Cards);
}

/// <summary>Token: Osty attack, 10 to a random enemy. Ethereal, Exhaust. Upgrade: +5. (Sweeping Gaze)</summary>
public sealed class SweepingGaze : CardModel
{
    public override string Name => "SweepingGaze";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Token;
    public override TargetType Target => TargetType.RandomEnemy;
    public override bool IsOstyAttack => true;
    public override bool Ethereal => true;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Damage => 10 + 5 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        var target = play.Target ?? combat.LivingMonsters.FirstOrDefault();
        if (target != null) NecroOsty.OstyHit(combat, target, Damage, this);
    }
}
