using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Silent character cards. Shared status cards live in Core/StatusCards.cs;
// generic keyword powers (Poison, Weak, …) in Core/CommonPowers.cs; Silent-only
// powers in SilentPowers.cs; registration in SilentCatalog.cs.
// ===========================================================================

/// <summary>Deal 6 damage. Upgrade: +3. (MegaCrit StrikeSilent)</summary>
public sealed class StrikeSilent : CardModel
{
    public override string Name => "StrikeSilent";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Gain 5 Block. Upgrade: +3. (MegaCrit DefendSilent)</summary>
public sealed class DefendSilent : CardModel
{
    public override string Name => "DefendSilent";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.Self;
    public int Block => 5 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Deal 3 damage. Apply 1 Weak. Cost 0. Upgrade: +1 each. (MegaCrit Neutralize — Silent starter)</summary>
public sealed class Neutralize : CardModel
{
    public override string Name => "Neutralize";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 3 + Upgrades;
    public int Weak => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new WeakPower(), Weak, combat.Player);
    }
}

/// <summary>Gain 8 Block. (Discard a card — deferred: the discard isn't modelled yet, so this is the
/// block-only effect.) Upgrade: +3 Block. (MegaCrit Survivor — Silent starter)</summary>
// NOTE: Survivor's "discard 1 card" half is intentionally omitted until hand-discard selection is
// supported. Kept here for completeness so the starter deck can be built, but excluded from validation
// of the discard behaviour. (Listed in DeferredSilentCards.)

/// <summary>Deal 6 damage. Cost 0. Upgrade: +3. (MegaCrit Slice)</summary>
public sealed class Slice : CardModel
{
    public override string Name => "Slice";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Gain 4 Block. Cost 0. Upgrade: +3. (MegaCrit Deflect)</summary>
public sealed class Deflect : CardModel
{
    public override string Name => "Deflect";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 4 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Gain 10 Block. Deal 10 damage. Upgrade: +3 each. (MegaCrit Dash)</summary>
public sealed class Dash : CardModel
{
    public override string Name => "Dash";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 10 + 3 * Upgrades;
    public int Block => 10 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
    }
}

/// <summary>Deal 8 damage. Apply 1 Weak. Upgrade: +2 damage, +1 Weak. (MegaCrit SuckerPunch)</summary>
public sealed class SuckerPunch : CardModel
{
    public override string Name => "SuckerPunch";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 8 + 2 * Upgrades;
    public int Weak => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new WeakPower(), Weak, combat.Player);
    }
}

/// <summary>Gain 11 Block. Apply 2 Weak. Upgrade: +3 Block, +1 Weak. (MegaCrit LegSweep)</summary>
public sealed class LegSweep : CardModel
{
    public override string Name => "LegSweep";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;   // Skill that targets an enemy (applies Weak)
    public int Block => 11 + 3 * Upgrades;
    public int Weak => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new WeakPower(), Weak, combat.Player);
    }
}

/// <summary>Deal 4 damage to ALL enemies twice. Upgrade: +2 damage. (MegaCrit DaggerSpray)</summary>
public sealed class DaggerSpray : CardModel
{
    public override string Name => "DaggerSpray";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 4 + 2 * Upgrades;
    public int Hits => 2;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.AttackMulti(combat, combat.Player, m, Damage, Hits, ValueProp.Move, this);
    }
}

/// <summary>Deal 6 damage. Apply 3 Poison. Upgrade: +2 damage, +1 Poison. (MegaCrit PoisonedStab)</summary>
public sealed class PoisonedStab : CardModel
{
    public override string Name => "PoisonedStab";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 6 + 2 * Upgrades;
    public int Poison => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new PoisonPower(), Poison, combat.Player);
    }
}

/// <summary>Apply 5 Poison. Upgrade: +2. (MegaCrit DeadlyPoison)</summary>
public sealed class DeadlyPoison : CardModel
{
    public override string Name => "DeadlyPoison";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Poison => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new PoisonPower(), Poison, combat.Player);
    }
}

/// <summary>Gain 2 Dexterity. Power. Upgrade: +1. (MegaCrit Footwork)</summary>
public sealed class Footwork : CardModel
{
    public override string Name => "Footwork";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Dexterity => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new DexterityPower(), Dexterity, combat.Player);
}

/// <summary>Apply 4 Poison to ALL enemies. Upgrade: +2. (MegaCrit Haze)</summary>
public sealed class Haze : CardModel
{
    public override string Name => "Haze";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AllEnemies;
    public int Poison => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.ApplyPower(combat, m, new PoisonPower(), Poison, combat.Player);
    }
}

/// <summary>Deal 5 damage for each Skill in your hand. Upgrade: +2 damage per hit. (MegaCrit Flechettes)
/// The played Flechettes is an Attack and is already out of hand, so it never counts itself.</summary>
public sealed class Flechettes : CardModel
{
    public override string Name => "Flechettes";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int hits = combat.Player.Hand.Count(c => c.Type == CardType.Skill);
        if (hits > 0)
            Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, hits, ValueProp.Move, this);
    }
}

// ===========================================================================
// Batch 2 — Shivs, poison-synergy powers, and more primitive attacks/skills.
// Card-generation (Shivs) is deterministic add-to-hand (no chance node).
// ===========================================================================

/// <summary>A generated 0-cost token: deal 4 damage, Exhaust. Upgrade: +2. (MegaCrit Shiv) Created in
/// hand by Cloak and Dagger / Blade Dance (Fan of Knives' all-enemy variant is not yet modelled).</summary>
public sealed class Shiv : CardModel
{
    public override string Name => "Shiv";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Special;     // "Token" in-game; cosmetic for combat
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Damage => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Add <c>n</c> freshly-created Shivs to the player's hand (or discard if the hand is full),
/// mirroring the game's AddGeneratedCardsToCombat into the hand pile.</summary>
internal static class SilentCardHelpers
{
    public static void AddShivsToHand(CombatState combat, int n)
    {
        var p = combat.Player;
        for (int i = 0; i < n; i++)
            (p.Hand.Count < Player.MaxHandSize ? p.Hand : p.DiscardPile).Add(new Shiv());
    }
}

/// <summary>Gain 6 Block. Add 1 Shiv to your hand. Upgrade: +1 Shiv. (MegaCrit Cloak and Dagger)</summary>
public sealed class CloakAndDagger : CardModel
{
    public override string Name => "CloakAndDagger";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 6;
    public int Shivs => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        SilentCardHelpers.AddShivsToHand(combat, Shivs);
    }
}

/// <summary>Add 3 Shivs to your hand. Exhaust. Upgrade: +1 Shiv. (MegaCrit Blade Dance)</summary>
public sealed class BladeDance : CardModel
{
    public override string Name => "BladeDance";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Shivs => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => SilentCardHelpers.AddShivsToHand(combat, Shivs);
}

/// <summary>Deal 15 damage. Upgrade: +4. (MegaCrit Pinpoint)</summary>
public sealed class Pinpoint : CardModel
{
    public override string Name => "Pinpoint";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 15 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 6 damage to ALL enemies. Upgrade: +2. (MegaCrit Flick Flack)</summary>
public sealed class FlickFlack : CardModel
{
    public override string Name => "FlickFlack";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 6 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
    }
}

/// <summary>Apply 1 Weak to ALL enemies. Exhaust. Upgrade: no longer Exhausts. Cost 0. (MegaCrit Scare)</summary>
public sealed class Scare : CardModel
{
    public override string Name => "Scare";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AllEnemies;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public int Weak => 1;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.ApplyPower(combat, m, new WeakPower(), Weak, combat.Player);
    }
}

/// <summary>Apply 7 Poison. Upgrade: +3. (Retain is not modelled — HP-neutral; the recorder's hands cover
/// it during validation.) (MegaCrit Snakebite)</summary>
public sealed class Snakebite : CardModel
{
    public override string Name => "Snakebite";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Poison => 7 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new PoisonPower(), Poison, combat.Player);
    }
}

/// <summary>If the target is already Poisoned, apply 9 more Poison. Upgrade: +3. (MegaCrit Bubble Bubble)</summary>
public sealed class BubbleBubble : CardModel
{
    public override string Name => "BubbleBubble";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Poison => 9 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (play.Target!.IsAlive && play.Target!.HasPower("Poison"))
            Cmd.ApplyPower(combat, play.Target!, new PoisonPower(), Poison, combat.Player);
    }
}

/// <summary>Deal 11 damage. Apply 3 Weak. Cost 0. Upgrade: +6 damage, +2 Weak. (Innate not modelled —
/// affects only the opening hand.) (MegaCrit Suppress)</summary>
public sealed class Suppress : CardModel
{
    public override string Name => "Suppress";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;       // "Ancient" in-game; cosmetic for combat
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 11 + 6 * Upgrades;
    public int Weak => 3 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new WeakPower(), Weak, combat.Player);
    }
}

/// <summary>Gain 4 Block now, and 4 Block at the start of your next turn. Upgrade: +2 each. The next-turn
/// amount equals the Block actually gained now (so Dexterity carries over). (MegaCrit Dodge and Roll)</summary>
public sealed class DodgeAndRoll : CardModel
{
    public override string Name => "DodgeAndRoll";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int before = combat.Player.Block;
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        int gained = combat.Player.Block - before;
        Cmd.ApplyPower(combat, combat.Player, new BlockNextTurnPower(), gained, combat.Player);
    }
}

/// <summary>Gain 5 Block. Your Block is not removed next turn (Blur 1). Upgrade: +3 Block. (MegaCrit Blur)</summary>
public sealed class Blur : CardModel
{
    public override string Name => "Blur";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 5 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new BlurPower(), 1, combat.Player);
    }
}

/// <summary>Power: whenever you play a card, gain 1 Block. Upgrade: becomes Innate (not modelled — opening
/// hand only). (MegaCrit Afterimage)</summary>
public sealed class Afterimage : CardModel
{
    public override string Name => "Afterimage";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Block => 1;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new AfterimagePower(), Block, combat.Player);
}

/// <summary>Power: at the start of each of your turns, apply 2 Poison to ALL enemies. Upgrade: +1.
/// (MegaCrit Noxious Fumes)</summary>
public sealed class NoxiousFumes : CardModel
{
    public override string Name => "NoxiousFumes";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Poison => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new NoxiousFumesPower(), Poison, combat.Player);
}

/// <summary>Power: whenever you deal unblocked attack damage, apply 1 Poison. Upgrade: +1. (MegaCrit Envenom)</summary>
public sealed class Envenom : CardModel
{
    public override string Name => "Envenom";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Poison => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new EnvenomPower(), Poison, combat.Player);
}

/// <summary>Reduce ALL enemies' Strength by 6 until their turn ends. Exhaust. Upgrade: +2. (MegaCrit
/// Piercing Wail)</summary>
public sealed class PiercingWail : CardModel
{
    public override string Name => "PiercingWail";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AllEnemies;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int StrengthLoss => 6 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.ApplyPower(combat, m, new PiercingWailPower(), StrengthLoss, combat.Player);
    }
}
