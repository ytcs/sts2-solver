using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>Deal 6 damage. Upgrade: +3. (MegaCrit StrikeIronclad)</summary>
public sealed class StrikeIronclad : CardModel
{
    public override string Name => "StrikeIronclad";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;

    public int Damage => 6 + 3 * Upgrades;

    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}
/// <summary>Gain 5 Block. Upgrade: +3. (MegaCrit DefendIronclad)</summary>
public sealed class DefendIronclad : CardModel
{
    public override string Name => "DefendIronclad";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.Self;

    public int Block => 5 + 3 * Upgrades;

    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}
/// <summary>Deal 32 damage. Upgrade: +10. (MegaCrit Bludgeon)</summary>
public sealed class Bludgeon : CardModel
{
    public override string Name => "Bludgeon";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 32 + 10 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}
/// <summary>Gain 2 Strength. Upgrade: +1. Power card. (MegaCrit Inflame)</summary>
public sealed class Inflame : CardModel
{
    public override string Name => "Inflame";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Strength => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new StrengthPower(), Strength, combat.Player);
}
/// <summary>Gain 5 Block. Deal 5 damage. Upgrade: +2 each. (MegaCrit Iron Wave)</summary>
public sealed class IronWave : CardModel
{
    public override string Name => "IronWave";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 5 + 2 * Upgrades;
    public int Block => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
    }
}
/// <summary>Deal 5 damage twice. Upgrade: +1 each hit. (MegaCrit Twin Strike)</summary>
public sealed class TwinStrike : CardModel
{
    public override string Name => "TwinStrike";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Damage => 5 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, 2, ValueProp.Move, this);
}
/// <summary>Deal 13 damage. Apply 1 Weak and 1 Vulnerable. Upgrade: +1 each debuff. (MegaCrit Uppercut)</summary>
public sealed class Uppercut : CardModel
{
    public override string Name => "Uppercut";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 13;
    public int Debuff => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
        {
            Cmd.ApplyPower(combat, play.Target!, new WeakPower(), Debuff, combat.Player);
            Cmd.ApplyPower(combat, play.Target!, new VulnerablePower(), Debuff, combat.Player);
        }
    }
}
/// <summary>Lose 2 HP (unblockable). Deal 15 damage. Upgrade: +3 damage. (MegaCrit Hemokinesis)</summary>
public sealed class Hemokinesis : CardModel
{
    public override string Name => "Hemokinesis";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int SelfLoss => 2;
    public int Damage => 15 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.LoseHp(combat, combat.Player, SelfLoss);
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
    }
}
/// <summary>Deal 4 damage to ALL enemies. Apply 1 Vulnerable to all. Upgrade: +3 damage. (MegaCrit Thunderclap)</summary>
public sealed class Thunderclap : CardModel
{
    public override string Name => "Thunderclap";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 4 + 3 * Upgrades;
    public int Vulnerable => 1;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
        {
            Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
            if (m.IsAlive) Cmd.ApplyPower(combat, m, new VulnerablePower(), Vulnerable, combat.Player);
        }
    }
}
/// <summary>Deal 12 damage to ALL enemies. Upgrade: +2. (MegaCrit Stomp)</summary>
public sealed class Stomp : CardModel
{
    public override string Name => "Stomp";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 12 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
    }
}
/// <summary>Deal 10 damage. If the target is Vulnerable, double its Vulnerable. Exhaust. Upgrade: +4.
/// (MegaCrit Molten Fist)</summary>
public sealed class MoltenFist : CardModel
{
    public override string Name => "MoltenFist";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Damage => 10 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
        {
            int v = play.Target!.GetPowerAmount("Vulnerable");
            if (v > 0) Cmd.ApplyPower(combat, play.Target!, new VulnerablePower(), v, combat.Player);
        }
    }
}
/// <summary>Deal 16 damage to ALL enemies. Upgrade: +4. (MegaCrit Howl From Beyond)</summary>
public sealed class HowlFromBeyond : CardModel
{
    public override string Name => "HowlFromBeyond";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 16 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
    }
}
/// <summary>Deal 20 damage. Apply 5 Vulnerable. Upgrade: +10 damage, +2 Vulnerable. (MegaCrit Break)</summary>
public sealed class Break : CardModel
{
    public override string Name => "Break";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;       // "Ancient" in-game; cosmetic for combat
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 20 + 10 * Upgrades;
    public int Vulnerable => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new VulnerablePower(), Vulnerable, combat.Player);
    }
}
/// <summary>Lose 1 HP (unblockable). Deal 9 damage to ALL enemies. Upgrade: +4 damage.
/// (MegaCrit Breakthrough)</summary>
public sealed class Breakthrough : CardModel
{
    public override string Name => "Breakthrough";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AllEnemies;
    public int SelfLoss => 1;
    public int Damage => 9 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.LoseHp(combat, combat.Player, SelfLoss);
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
    }
}
/// <summary>Gain 7 Block. Apply 1 Vulnerable. Upgrade: +1 Block, +1 Vulnerable. (MegaCrit Taunt)</summary>
public sealed class Taunt : CardModel
{
    public override string Name => "Taunt";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;   // Skill that targets an enemy (applies Vuln)
    public int Block => 7 + Upgrades;
    public int Vulnerable => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new VulnerablePower(), Vulnerable, combat.Player);
    }
}
/// <summary>Apply 3 Vulnerable. Exhaust. Upgrade: +1 Vulnerable. (MegaCrit Tremble)</summary>
public sealed class Tremble : CardModel
{
    public override string Name => "Tremble";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Vulnerable => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new VulnerablePower(), Vulnerable, combat.Player);
    }
}
/// <summary>Deal damage equal to your current Block. Upgrade: costs 0. The base damage is your Block at
/// play time; Strength/Vulnerable then apply through the normal pipeline (matching the game). (MegaCrit
/// Body Slam)</summary>
public sealed class BodySlam : CardModel
{
    public override string Name => "BodySlam";
    public override int BaseCost => 1 - (Upgrades > 0 ? 1 : 0);
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, combat.Player.Block, ValueProp.Move, this);
}
/// <summary>Deal 4 damage, plus 2 per Vulnerable on the target. Upgrade: +1 per Vulnerable. (MegaCrit Bully)</summary>
public sealed class Bully : CardModel
{
    public override string Name => "Bully";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Base => 4;
    public int PerVuln => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int dmg = Base + PerVuln * play.Target!.GetPowerAmount("Vulnerable");
        Cmd.Attack(combat, combat.Player, play.Target!, dmg, ValueProp.Move, this);
    }
}
/// <summary>Deal 6 damage, plus 3 per card in your exhaust pile. Upgrade: +1 per exhausted card.
/// (MegaCrit Ashen Strike)</summary>
public sealed class AshenStrike : CardModel
{
    public override string Name => "AshenStrike";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Base => 6;
    public int PerExhausted => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int dmg = Base + PerExhausted * combat.Player.ExhaustPile.Count;
        Cmd.Attack(combat, combat.Player, play.Target!, dmg, ValueProp.Move, this);
    }
}
/// <summary>Gain 12 Block. Gain Flame Barrier 4: whenever you are attacked this round, deal 4 damage
/// back. Upgrade: +4 Block, +2 retaliation. (MegaCrit Flame Barrier)</summary>
public sealed class FlameBarrier : CardModel
{
    public override string Name => "FlameBarrier";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 12 + 4 * Upgrades;
    public int DamageBack => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new FlameBarrierPower(), DamageBack, combat.Player);
    }
}
/// <summary>Deal 15 damage. Reduce the target's Strength by 10 until its turn ends. Upgrade: +5 each.
/// (MegaCrit Mangle)</summary>
public sealed class Mangle : CardModel
{
    public override string Name => "Mangle";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 15 + 5 * Upgrades;
    public int StrengthLoss => 10 + 5 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new ManglePower(), StrengthLoss, combat.Player);
    }
}
/// <summary>Deal 7 damage. Gain 2 Strength for the rest of this turn. Upgrade: +2 damage, +1 Strength.
/// The temporary Strength is applied after this card's hit, so it boosts only later attacks. (MegaCrit
/// Setup Strike)</summary>
public sealed class SetupStrike : CardModel
{
    public override string Name => "SetupStrike";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Damage => 7 + 2 * Upgrades;
    public int Strength => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new SetupStrikePower(), Strength, combat.Player);
    }
}
/// <summary>This turn, gain 3 Block each time you play an Attack. Upgrade: +2. (MegaCrit Rage)</summary>
public sealed class Rage : CardModel
{
    public override string Name => "Rage";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 3 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new RagePower(), Block, combat.Player);
}
/// <summary>At the start of each turn, gain 2 Strength. Power. Upgrade: +1. (MegaCrit Demon Form)</summary>
public sealed class DemonForm : CardModel
{
    public override string Name => "DemonForm";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Strength => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new DemonFormPower(), Strength, combat.Player);
}
/// <summary>Gain 30 Block. Exhaust. Upgrade: +10. (MegaCrit Impervious)</summary>
public sealed class Impervious : CardModel
{
    public override string Name => "Impervious";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Block => 30 + 10 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}
/// <summary>Lose 2 HP (unblockable). Gain 16 Block. Upgrade: +4 Block. (MegaCrit Blood Wall)</summary>
public sealed class BloodWall : CardModel
{
    public override string Name => "BloodWall";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int SelfLoss => 2;
    public int Block => 16 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.LoseHp(combat, combat.Player, SelfLoss);
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
    }
}
/// <summary>Gain 5 Block and Colossus 1: through the next enemy turn, powered attacks against you from
/// Vulnerable enemies deal half damage. Upgrade: +3 Block. (MegaCrit Colossus)</summary>
public sealed class Colossus : CardModel
{
    public override string Name => "Colossus";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 5 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new ColossusPower(), 1, combat.Player);
    }
}
/// <summary>Power: at the end of each of your turns gain 4 Block, the amount decaying by 1 per turn.
/// Upgrade: +2. (MegaCrit Stone Armor — PlatingPower.)</summary>
public sealed class StoneArmor : CardModel
{
    public override string Name => "StoneArmor";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Plating => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new PlatingPower(), Plating, combat.Player);
}
/// <summary>Apply 1 Vulnerable. Gain Strength equal to the target's total Vulnerable. Exhaust.
/// Upgrade: +1 Vulnerable. (MegaCrit Dominate)</summary>
public sealed class Dominate : CardModel
{
    public override string Name => "Dominate";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Vulnerable => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, play.Target!, new VulnerablePower(), Vulnerable, combat.Player);
        int total = play.Target!.GetPowerAmount("Vulnerable");
        if (total > 0) Cmd.ApplyPower(combat, combat.Player, new StrengthPower(), total, combat.Player);
    }
}
/// <summary>Heal 10 HP. Exhaust. Upgrade: +3. (MegaCrit Not Yet)</summary>
public sealed class NotYet : CardModel
{
    public override string Name => "NotYet";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int HealAmount => 10 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => combat.Player.Heal(HealAmount);
}
/// <summary>Deal 2 damage to ALL enemies 4 times. Upgrade: +1 hit. (MegaCrit Conflagration)</summary>
public sealed class Conflagration : CardModel
{
    public override string Name => "Conflagration";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 2;
    public int Hits => 4 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.AttackMulti(combat, combat.Player, m, Damage, Hits, ValueProp.Move, this);
    }
}
/// <summary>Power: whenever you lose HP from a card, gain 1 Strength. Upgrade: +1. (MegaCrit Rupture)</summary>
public sealed class Rupture : CardModel
{
    public override string Name => "Rupture";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Strength => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new RupturePower(), Strength, combat.Player);
}
/// <summary>Power: whenever you gain Block, deal 5 damage to a random enemy. Upgrade: +2. (MegaCrit Juggernaut)</summary>
public sealed class Juggernaut : CardModel
{
    public override string Name => "Juggernaut";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Damage => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new JuggernautPower(), Damage, combat.Player);
}
/// <summary>Power: Block is no longer cleared at the start of your turn. Upgrade: costs 2. (MegaCrit Barricade)</summary>
public sealed class Barricade : CardModel
{
    public override string Name => "Barricade";
    public override int BaseCost => 3 - (Upgrades > 0 ? 1 : 0);
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new BarricadePower(), 1, combat.Player);
}
/// <summary>Deal 10 damage. If this kills a (non-minion) enemy, gain 3 Max HP. Exhaust. Upgrade: +2 damage,
/// +1 Max HP. (MegaCrit Feed)</summary>
public sealed class Feed : CardModel
{
    public override string Name => "Feed";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Damage => 10 + 2 * Upgrades;
    public int MaxHpGain => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        bool wasAlive = play.Target!.IsAlive;
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (wasAlive && !play.Target!.IsAlive) combat.Player.GainMaxHp(MaxHpGain);
    }
}
/// <summary>Gain 8 Block. Draw 1 card. Upgrade: +3 Block. (MegaCrit Shrug It Off)</summary>
public sealed class ShrugItOff : CardModel
{
    public override string Name => "ShrugItOff";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 8 + 3 * Upgrades;
    public int Cards => 1;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.Draw(combat, Cards);
    }
}
/// <summary>Deal 9 damage. Draw 1 card. Upgrade: +1 damage, +1 card. (MegaCrit Pommel Strike)</summary>
public sealed class PommelStrike : CardModel
{
    public override string Name => "PommelStrike";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Damage => 9 + Upgrades;
    public int Cards => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.Draw(combat, Cards);
    }
}
/// <summary>Draw 3 cards. You cannot draw additional cards this turn. Upgrade: +1 card. (MegaCrit Battle Trance)</summary>
public sealed class BattleTrance : CardModel
{
    public override string Name => "BattleTrance";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Cards => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Draw(combat, Cards);
        Cmd.ApplyPower(combat, combat.Player, new NoDrawPower(), 1, combat.Player);
    }
}
/// <summary>Power: whenever a card is Exhausted, gain 3 Block. Upgrade: +1. (MegaCrit Feel No Pain)</summary>
public sealed class FeelNoPain : CardModel
{
    public override string Name => "FeelNoPain";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new FeelNoPainPower(), Block, combat.Player);
}
/// <summary>Power: whenever a card is Exhausted, draw 1 card. Upgrade: costs 1. (MegaCrit Dark Embrace)</summary>
public sealed class DarkEmbrace : CardModel
{
    public override string Name => "DarkEmbrace";
    public override int BaseCost => 2 - (Upgrades > 0 ? 1 : 0);
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new DarkEmbracePower(), 1, combat.Player);
}
/// <summary>Lose 3 HP (unblockable). Gain 2 Energy. Upgrade: +1 Energy. (MegaCrit Bloodletting)</summary>
public sealed class Bloodletting : CardModel
{
    public override string Name => "Bloodletting";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int SelfLoss => 3;
    public int Energy => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.LoseHp(combat, combat.Player, SelfLoss);
        Cmd.GainEnergy(combat, Energy);
    }
}
/// <summary>Lose 6 HP (unblockable). Gain 2 Energy. Draw 3 cards. Exhaust. Upgrade: +2 cards.
/// (MegaCrit Offering)</summary>
public sealed class Offering : CardModel
{
    public override string Name => "Offering";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int SelfLoss => 6;
    public int Energy => 2;
    public int Cards => 3 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.LoseHp(combat, combat.Player, SelfLoss);
        Cmd.GainEnergy(combat, Energy);
        Cmd.Draw(combat, Cards);
    }
}
/// <summary>X-cost: deal 5 damage to ALL enemies X times, where X is all your remaining energy. Upgrade:
/// +3 damage. (MegaCrit Whirlwind)</summary>
public sealed class Whirlwind : CardModel
{
    public override string Name => "Whirlwind";
    public override int BaseCost => 0;
    public override bool IsXCost => true;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 5 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.AttackMulti(combat, combat.Player, m, Damage, play.XValue, ValueProp.Move, this);
    }
}
/// <summary>Deal 6 damage. Add a copy of this card to your discard pile. Upgrade: +2 damage.
/// (MegaCrit Anger)</summary>
public sealed class Anger : CardModel
{
    public override string Name => "Anger";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 6 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        combat.Player.DiscardPile.Add((CardModel)Clone());   // a fresh copy joins the discard pile
    }
}
/// <summary>Deal 9 damage. Put a card from your discard pile on top of your draw pile. Upgrade: +3 damage.
/// The topdeck target is a real player CHOICE (promoted to a search decision via <see cref="Choices"/>); with
/// no choice supplied (trace replay) it defaults to the most-recently discarded card. (MegaCrit Headbutt)</summary>
public sealed class Headbutt : CardModel
{
    public override string Name => "Headbutt";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 9 + 3 * Upgrades;

    /// <summary>Which discard-pile card to topdeck — one option per DISTINCT discard card (identical cards
    /// collapse by StateKey, so e.g. four Strikes are one choice).</summary>
    public override IEnumerable<string> Choices(CombatState combat)
        => combat.Player.DiscardPile.Select(c => c.StateKey()).Distinct();

    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        var discard = combat.Player.DiscardPile;
        if (discard.Count == 0) return;
        // Topdeck the chosen discard card (last index matching the choice key); default to the most recent.
        int idx = play.ChoiceKey == null ? discard.Count - 1
                                         : discard.FindLastIndex(c => c.StateKey() == play.ChoiceKey);
        if (idx < 0) idx = discard.Count - 1;
        var card = discard[idx];
        discard.RemoveAt(idx);
        combat.Player.DrawPile.Insert(0, card);
    }
}
/// <summary>Deal 3 damage to a random enemy 3 times. Upgrade: +1 hit. Each hit re-targets a living enemy;
/// with a single enemy this is deterministic (the only case validated). (MegaCrit Sword Boomerang)</summary>
public sealed class SwordBoomerang : CardModel
{
    public override string Name => "SwordBoomerang";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.RandomEnemy;
    public int Damage => 3;
    public int Hits => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        for (int i = 0; i < Hits; i++)
        {
            var t = combat.LivingMonsters.FirstOrDefault();
            if (t == null) break;
            Cmd.Attack(combat, combat.Player, t, Damage, ValueProp.Move, this);
        }
    }
}
/// <summary>Power: gain 1 Max Energy (this combat). Upgrade: +1. (MegaCrit Pyre)</summary>
public sealed class Pyre : CardModel
{
    public override string Name => "Pyre";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Energy => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new PyrePower(), Energy, combat.Player);
}
/// <summary>Gain 8 Block. If a card was Exhausted this turn, gain it twice (16). Upgrade: +3 Block each.
/// (MegaCrit Evil Eye)</summary>
public sealed class EvilEye : CardModel
{
    public override string Name => "EvilEye";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 8 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int gains = combat.CardExhaustedThisTurn ? 2 : 1;
        for (int i = 0; i < gains; i++)
            Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
    }
}
/// <summary>If a card was Exhausted this turn, gain 3 Energy. Exhaust. Upgrade: +1 Energy.
/// (MegaCrit Forgotten Ritual)</summary>
public sealed class ForgottenRitual : CardModel
{
    public override string Name => "ForgottenRitual";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Energy => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.CardExhaustedThisTurn) Cmd.GainEnergy(combat, Energy);
    }
}
/// <summary>This turn, your next Attack is played twice. Upgrade: +1 (next two Attacks). (MegaCrit One-Two Punch)</summary>
public sealed class OneTwoPunch : CardModel
{
    public override string Name => "OneTwoPunch";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Charges => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new OneTwoPunchPower(), Charges, combat.Player);
}
/// <summary>Deal 14 damage. Your next Attack this combat costs 0 (Free Attack). Upgrade: +6 damage.
/// (MegaCrit Unrelenting)</summary>
public sealed class Unrelenting : CardModel
{
    public override string Name => "Unrelenting";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 14 + 6 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new FreeAttackPower(), 1, combat.Player);
    }
}
/// <summary>Power: Skills cost 0 and are Exhausted when played. Upgrade: costs 2. (MegaCrit Corruption)</summary>
public sealed class Corruption : CardModel
{
    public override string Name => "Corruption";
    public override int BaseCost => 3 - (Upgrades > 0 ? 1 : 0);
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new CorruptionPower(), 1, combat.Player);
}
/// <summary>Deal 5 damage. If you lost HP this turn, hit twice instead. Upgrade: +1 hit (thrice).
/// (MegaCrit Spite)</summary>
public sealed class Spite : CardModel
{
    public override string Name => "Spite";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 5;
    public int Hits => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int hits = combat.PlayerLostHpThisTurn ? Hits : 1;
        Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, hits, ValueProp.Move, this);
    }
}
/// <summary>Power: your attacks make Vulnerable enemies take +25% (multiplier ×1.5 → ×1.75). Upgrade: +25%.
/// (MegaCrit Cruelty)</summary>
public sealed class Cruelty : CardModel
{
    public override string Name => "Cruelty";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Amount => 25 + 25 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new CrueltyPower(), Amount, combat.Player);
}
/// <summary>Power: at the start of each turn, lose HP (1 per copy played) then gain 8 Block. Upgrade:
/// +2 Block. (MegaCrit Crimson Mantle)</summary>
public sealed class CrimsonMantle : CardModel
{
    public override string Name => "CrimsonMantle";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Block => 8 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new CrimsonMantlePower(), Block, combat.Player);
}
/// <summary>Power: the first Block you gain from a card each turn is doubled. Upgrade: costs 1.
/// (MegaCrit Unmovable)</summary>
public sealed class Unmovable : CardModel
{
    public override string Name => "Unmovable";
    public override int BaseCost => 2 - (Upgrades > 0 ? 1 : 0);
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new UnmovablePower(), 1, combat.Player);
}
/// <summary>Power: every 3rd Attack you play each turn, add a copy of it to your hand. Upgrade: Innate.
/// (MegaCrit Juggling)</summary>
public sealed class Juggling : CardModel
{
    public override string Name => "Juggling";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new JugglingPower(), 1, combat.Player);
}
/// <summary>Exhaust your whole hand. Deal 7 damage to a target for each card Exhausted. Exhaust. Upgrade:
/// +3 damage per hit. (MegaCrit Fiend Fire)</summary>
public sealed class FiendFire : CardModel
{
    public override string Name => "FiendFire";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Damage => 7 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        var rest = combat.Player.Hand.ToList();          // the rest of the hand (Fiend Fire already removed)
        int count = rest.Count;
        foreach (var c in rest) Cmd.ExhaustFromHand(combat, c);
        if (count > 0)
            Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, count, ValueProp.Move, this);
    }
}
/// <summary>Exhaust all non-Attack cards in your hand. Gain 5 Block for each. Exhaust. Upgrade: +2 Block.
/// (MegaCrit Second Wind)</summary>
public sealed class SecondWind : CardModel
{
    public override string Name => "SecondWind";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Block => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var c in combat.Player.Hand.Where(x => x.Type != CardType.Attack).ToList())
        {
            Cmd.ExhaustFromHand(combat, c);
            Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        }
    }
}
/// <summary>Gain 7 Block. Exhaust a card from your hand (random; the choice is HP-neutral). Exhaust.
/// Upgrade: +2 Block. (MegaCrit True Grit)</summary>
public sealed class TrueGrit : CardModel
{
    public override string Name => "TrueGrit";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Block => 7 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        var card = combat.Player.Hand.FirstOrDefault();
        if (card != null) Cmd.ExhaustFromHand(combat, card);
    }
}
/// <summary>Deal 18 damage. Exhaust a card from your hand (random; HP-neutral). Exhaust. Upgrade:
/// +6 damage. (MegaCrit Cinder)</summary>
public sealed class Cinder : CardModel
{
    public override string Name => "Cinder";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Damage => 18 + 6 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        var card = combat.Player.Hand.FirstOrDefault();
        if (card != null) Cmd.ExhaustFromHand(combat, card);
    }
}
/// <summary>Exhaust a card from your hand (choice is HP-neutral). Draw 2 cards. Upgrade: +1 card.
/// (MegaCrit Burning Pact)</summary>
public sealed class BurningPact : CardModel
{
    public override string Name => "BurningPact";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Cards => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        var card = combat.Player.Hand.FirstOrDefault();
        if (card != null) Cmd.ExhaustFromHand(combat, card);
        Cmd.Draw(combat, Cards);
    }
}
/// <summary>Lose 1 HP (unblockable). Exhaust a card from your hand (HP-neutral choice). Gain 1 Strength.
/// Upgrade: +1 Strength. (MegaCrit Brand)</summary>
public sealed class Brand : CardModel
{
    public override string Name => "Brand";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int SelfLoss => 1;
    public int Strength => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.LoseHp(combat, combat.Player, SelfLoss);
        var card = combat.Player.Hand.FirstOrDefault();
        if (card != null) Cmd.ExhaustFromHand(combat, card);
        Cmd.ApplyPower(combat, combat.Player, new StrengthPower(), Strength, combat.Player);
    }
}
/// <summary>Deal 6 damage. Draw until you draw a non-Attack card. Gain gold (meta — ignored in combat).
/// Upgrade: +3 damage. (MegaCrit Pillage) — the conditional draw is externally supplied during replay, so
/// it is modelled as a single draw call (no-op without an ambient Rng); the damage is what validation checks.</summary>
public sealed class Pillage : CardModel
{
    public override string Name => "Pillage";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.Draw(combat, 1);
    }
}
/// <summary>Gain Energy equal to the number of Attack cards in your hand. You cannot gain energy again
/// this turn. Upgrade: costs 1. (MegaCrit Expect a Fight)</summary>
public sealed class ExpectAFight : CardModel
{
    public override string Name => "ExpectAFight";
    public override int BaseCost => 2 - (Upgrades > 0 ? 1 : 0);
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int attacks = combat.Player.Hand.Count(c => c.Type == CardType.Attack);
        Cmd.GainEnergy(combat, attacks);
        Cmd.ApplyPower(combat, combat.Player, new NoEnergyGainPower(), 1, combat.Player);
    }
}
/// <summary>Power: whenever you apply Vulnerable, draw 1 card. Upgrade: +1 card. (MegaCrit Vicious)</summary>
public sealed class Vicious : CardModel
{
    public override string Name => "Vicious";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Cards => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new ViciousPower(), Cards, combat.Player);
}
/// <summary>Power: at the start of each turn, lose HP (1 per copy played) and deal 6 damage to ALL enemies.
/// Upgrade: +3 damage. (MegaCrit Inferno)</summary>
public sealed class Inferno : CardModel
{
    public override string Name => "Inferno";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Damage => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new InfernoPower(), Damage, combat.Player);
}
/// <summary>Draw 2 cards. (MegaCrit Drum of Battle — the decompiled OnPlay only draws; its Energy var is
/// vestigial. The live trace confirms the no-energy reading.)</summary>
public sealed class DrumOfBattle : CardModel
{
    public override string Name => "DrumOfBattle";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Cards => 2;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Draw(combat, Cards);
}
/// <summary>Deal 8 damage. Hit twice if the target is Vulnerable. Upgrade: +2 damage. (MegaCrit Dismantle)</summary>
public sealed class Dismantle : CardModel
{
    public override string Name => "Dismantle";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 8 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int hits = play.Target!.HasPower("Vulnerable") ? 2 : 1;
        Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, hits, ValueProp.Move, this);
    }
}
/// <summary>Deal 6 damage, +2 for EVERY card with the Strike tag in your deck (including this one).
/// Upgrade: +1 per Strike. (MegaCrit Perfected Strike) — the count is taken from the live piles; the
/// validator's hand-override drifts the deck over turns, so this is unit-tested rather than live-traced.</summary>
public sealed class PerfectedStrike : CardModel
{
    public override string Name => "PerfectedStrike";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Base => 6;
    public int PerStrike => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        var p = combat.Player;
        // +1 for this card itself (it's been removed from hand into the transient play slot).
        int strikes = 1 + p.Hand.Concat(p.DrawPile).Concat(p.DiscardPile).Concat(p.ExhaustPile).Count(c => c.IsStrike);
        int dmg = Base + PerStrike * strikes;
        Cmd.Attack(combat, combat.Player, play.Target!, dmg, ValueProp.Move, this);
    }
}
/// <summary>Deal 9 damage. Each time this card is played, its damage permanently rises by 5 (for the rest
/// of combat). Upgrade: +4 per play. (MegaCrit Rampage) — the escalation is per-card-instance; the
/// validator reconstructs cards each turn, so this is unit-tested rather than live-traced.</summary>
public sealed class Rampage : CardModel
{
    public override string Name => "Rampage";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Base => 9;
    public int Increase => 5 + 4 * Upgrades;
    private int _extra;
    public override bool Stateful => true;   // _extra escalates per play, so each search state needs its own instance
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Base + _extra, ValueProp.Move, this);
        _extra += Increase;
    }
    public override string StateKey() => Upgrades > 0 ? $"Rampage+{Upgrades}/{_extra}" : $"Rampage/{_extra}";
}
/// <summary>Add a random Attack to your hand (it costs 0 this turn). Exhaust. (MegaCrit Infernal Blade) —
/// the generated attack is random/unrecorded and HP-neutral, so this is modelled as exhaust-self only; any
/// generated card the autopilot plays is reconstructed by the validator.</summary>
public sealed class InfernalBlade : CardModel
{
    public override string Name => "InfernalBlade";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // adds a random 0-cost attack (no HP effect)
}
/// <summary>Exhaust your whole hand, then add that many random cards to your hand. (MegaCrit Stoke) — the
/// generated cards are random/unrecorded; modelled as exhaust-the-hand (HP-neutral; any generated card the
/// autopilot plays is reconstructed by the validator).</summary>
public sealed class Stoke : CardModel
{
    public override string Name => "Stoke";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var c in combat.Player.Hand.ToList()) Cmd.ExhaustFromHand(combat, c);
    }
}
/// <summary>Gain 5 Block. Upgrade a card in your hand for the rest of combat (all cards if upgraded).
/// (MegaCrit Armaments) — the upgrade-target pops an in-game prompt, so this is unit-tested via the block +
/// an arbitrary upgrade; the upgrade itself is HP-neutral unless that card is later played.
///
/// SOUNDNESS: like Apotheosis, this MUST NOT mutate a card instance in place — the immutable majority of
/// cards are SHARED across cloned search states (Player.Clone only deep-clones <see cref="CardModel.Stateful"/>
/// cards), so bumping a shared card's Upgrades would change its StateKey in every sibling branch and corrupt
/// the draw enumerator's pile bookkeeping. We REPLACE the chosen hand card(s) with a freshly cloned, upgraded
/// instance this state alone owns, leaving the shared original untouched.</summary>
public sealed class Armaments : CardModel
{
    public override string Name => "Armaments";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 5;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        // Upgrade by REPLACING each chosen hand card with a private, freshly-cloned upgraded copy — never
        // mutate the (shared, non-Stateful) instance in place, or sibling search branches that share it get
        // corrupted (the draw enumerator then throws "Pile missing card …"). Already-upgraded cards are skipped
        // so an upgraded Armaments can't double-upgrade. Upgraded+ hits the whole hand; base hits one.
        var hand = combat.Player.Hand;
        if (Upgrades > 0)
        {
            for (int i = 0; i < hand.Count; i++)
                if (hand[i].Upgrades == 0)
                    hand[i] = hand[i].Clone().Upgraded(1);
        }
        else
        {
            // Upgrade one arbitrary unupgraded hand card (the in-game choice is HP-neutral for replay).
            for (int i = 0; i < hand.Count; i++)
                if (hand[i].Upgrades == 0) { hand[i] = hand[i].Clone().Upgraded(1); break; }
        }
    }
}
/// <summary>Deal 17 damage to ALL enemies — but only if 3+ cards are in your exhaust pile. Upgrade: +6.
/// (MegaCrit Pact's End)</summary>
public sealed class PactsEnd : CardModel
{
    public override string Name => "PactsEnd";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 17 + 6 * Upgrades;
    public int Threshold => 3;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Player.ExhaustPile.Count < Threshold) return;
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
    }
}
/// <summary>Deal 4 damage twice, then Exhaust a random Attack from your hand and deal its damage too.
/// Exhaust. Upgrade: +2 damage. (MegaCrit Thrash) — the random exhaust + bonus damage are RNG/unrecorded,
/// so this is unit-tested via the base 4×2 + exhausting an arbitrary hand Attack.</summary>
public sealed class Thrash : CardModel
{
    public override string Name => "Thrash";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Damage => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, 2, ValueProp.Move, this);
        var atk = combat.Player.Hand.FirstOrDefault(c => c.Type == CardType.Attack);
        if (atk != null) Cmd.ExhaustFromHand(combat, atk);   // bonus damage from the exhausted attack is RNG/unmodelled
    }
}
/// <summary>Power: at the start of each turn, add random Attacks from your discard pile to hand (upgraded).
/// Upgrade: Innate. (MegaCrit Aggression) — the card movement is RNG/HP-neutral; ported as an inert power.</summary>
public sealed class Aggression : CardModel
{
    public override string Name => "Aggression";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new AggressionPower(), 1, combat.Player);
}
/// <summary>Power: each turn, auto-play a random Attack from your hand. Upgrade: +1. (MegaCrit Stampede) —
/// the auto-play is RNG; ported as an inert power (auto-plays are replayed from the trace).</summary>
public sealed class Stampede : CardModel
{
    public override string Name => "Stampede";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new StampedePower(), Amount, combat.Player);
}
/// <summary>Power: whenever you draw a Strike, play it for free. Upgrade: +1. (MegaCrit Hellraiser) —
/// the auto-play-on-draw is RNG; ported as an inert power.</summary>
public sealed class Hellraiser : CardModel
{
    public override string Name => "Hellraiser";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new HellraiserPower(), 1, combat.Player);
}
/// <summary>Transform all Attacks in your hand into Giant Rocks. (MegaCrit Primal Force) — Giant Rock is a
/// colorless card outside the Ironclad set; the transform is HP-neutral until a Rock is played, so it is
/// modelled as a no-op (the played Rock, if any, is reconstructed by the validator). Unit-tested as inert.</summary>
public sealed class PrimalForce : CardModel
{
    public override string Name => "PrimalForce";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play) { }
}
/// <summary>Play the top card of your draw pile, then Exhaust it. (MegaCrit Havoc) — the auto-played card
/// depends on draw order (RNG) and is recorded as an auto-play, so this is modelled as a no-op (auto-plays
/// are replayed from the trace). Unit-tested as inert.</summary>
public sealed class Havoc : CardModel
{
    public override string Name => "Havoc";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play) { }
}
/// <summary>X-cost: play the top X cards of your draw pile (auto-play). Upgrade: +1 card. (MegaCrit Cascade)
/// — the auto-plays depend on draw order (RNG) and are replayed from the trace, so this is modelled as a
/// no-op (X energy is still spent). Unit-tested as inert.</summary>
public sealed class Cascade : CardModel
{
    public override string Name => "Cascade";
    public override int BaseCost => 0;
    public override bool IsXCost => true;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play) { }
}
/// <summary>Lose 1 HP (unblockable). Gain Block equal to your current Block (doubling it). Exhaust.
/// Multiplayer-only — ported for catalog completeness; targets an ally (self in single-player).
/// (MegaCrit Demonic Shield)</summary>
public sealed class DemonicShield : CardModel
{
    public override string Name => "DemonicShield";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.LoseHp(combat, combat.Player, 1);
        Cmd.GainBlock(combat, combat.Player, combat.Player.Block, ValueProp.Move, this);
    }
}
/// <summary>Power: multiplayer-only support buff (Guarded to allies + damage reduction). Ported for catalog
/// completeness; inert in single-player. (MegaCrit Tank)</summary>
public sealed class Tank : CardModel
{
    public override string Name => "Tank";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new TankPower(), 1, combat.Player);
}
/// <summary>Deal 5 damage twice. Gain 3 Strength. The enemy gains 1 Strength too. Upgrade: +1 damage,
/// +1 Strength. (MegaCrit Fight Me!)</summary>
public sealed class FightMe : CardModel
{
    public override string Name => "FightMe";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 5 + Upgrades;
    public int SelfStrength => 3 + Upgrades;
    public int EnemyStrength => 1;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, 2, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new StrengthPower(), SelfStrength, combat.Player);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new StrengthPower(), EnemyStrength, combat.Player);
    }
}
/// <summary>Deal 5 damage, once plus once more for every time you have taken unblocked damage this combat.
/// Upgrade: +2 damage. (MegaCrit Tear Asunder)</summary>
public sealed class TearAsunder : CardModel
{
    public override string Name => "TearAsunder";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int hits = 1 + combat.PlayerUnblockedHitsCount;
        Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, hits, ValueProp.Move, this);
    }
}
/// <summary>Deal 8 damage. Apply 2 Vulnerable. Upgrade: +2 damage, +1 Vulnerable. (MegaCrit Bash)</summary>
public sealed class Bash : CardModel
{
    public override string Name => "Bash";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.AnyEnemy;

    public int Damage => 8 + 2 * Upgrades;
    public int Vulnerable => 2 + (Upgrades > 0 ? 1 : 0);

    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new VulnerablePower(), Vulnerable, combat.Player);
    }
}
