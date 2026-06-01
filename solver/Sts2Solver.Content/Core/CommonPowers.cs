using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>Incoming damage ×1.5 while owned. Ticks down at the owner's turn end. (MegaCrit VulnerablePower)</summary>
public sealed class VulnerablePower : PowerModel
{
    public override string Id => "Vulnerable";
    public override PowerType Type => PowerType.Debuff;

    public override decimal ModifyDamageMultiplicative(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (target != Owner) return 1m;
        if (!props.IsPoweredAttack()) return 1m;
        decimal mult = 1.5m;
        if (dealer != null)                                // Cruelty (on the attacker) lifts the multiplier
            foreach (var p in dealer.Powers) mult += p.VulnerableMultiplierBonus();
        return mult;
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;                                        // a debuff ticks down at its OWNER's turn end
        if (SkipNextTick) { SkipNextTick = false; return; }                    // skip the end-tick on the turn it was applied
        Amount--; this.NormalizeOrRemove(Owner);
    }
}
/// <summary>Outgoing damage ×0.75. Ticks down at the owner's turn end. (MegaCrit WeakPower)</summary>
public sealed class WeakPower : PowerModel
{
    public override string Id => "Weak";
    public override PowerType Type => PowerType.Debuff;

    public override decimal ModifyDamageMultiplicative(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (dealer != Owner) return 1m;
        if (!props.IsPoweredAttack()) return 1m;
        return 0.75m;
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;                                        // a debuff ticks down at its OWNER's turn end
        if (SkipNextTick) { SkipNextTick = false; return; }                    // skip the end-tick on the turn it was applied
        Amount--; this.NormalizeOrRemove(Owner);
    }
}
/// <summary>Block gained ×0.75. Ticks down at the owner's turn end. (MegaCrit FrailPower)</summary>
public sealed class FrailPower : PowerModel
{
    public override string Id => "Frail";
    public override PowerType Type => PowerType.Debuff;

    public override decimal ModifyBlockMultiplicative(Creature target, decimal block, ValueProp props, CardModel? cardSource)
    {
        if (target != Owner) return 1m;
        if (!props.IsPoweredBlock()) return 1m;
        return 0.75m;
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;                                        // a debuff ticks down at its OWNER's turn end
        if (SkipNextTick) { SkipNextTick = false; return; }                    // skip the end-tick on the turn it was applied
        Amount--; this.NormalizeOrRemove(Owner);
    }
}
/// <summary>Adds flat damage equal to Amount on powered attacks dealt by the owner. (MegaCrit StrengthPower)</summary>
public sealed class StrengthPower : PowerModel
{
    public override string Id => "Strength";
    public override PowerType Type => PowerType.Buff;
    public override bool AllowNegative => true;

    public override decimal ModifyDamageAdditive(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (dealer != Owner) return 0m;
        if (!props.IsPoweredAttack()) return 0m;
        return Amount;
    }
}
/// <summary>Adds flat block equal to Amount on powered block gained by the owner. (MegaCrit DexterityPower)</summary>
public sealed class DexterityPower : PowerModel
{
    public override string Id => "Dexterity";
    public override PowerType Type => PowerType.Buff;
    public override bool AllowNegative => true;

    public override decimal ModifyBlockAdditive(Creature target, decimal block, ValueProp props, CardModel? cardSource)
    {
        if (target != Owner) return 0m;
        if (!props.IsPoweredBlock()) return 0m;
        return Amount;
    }
}
/// <summary>Base for "temporary Strength" powers. Applying amount N immediately pushes Sign×N *actual*
/// Strength onto the owner (tracked in <c>_appliedStrength</c>, handling stacks via the delta); at the
/// owner's own turn end the pushed Strength is undone and the marker removed. Sign &gt; 0 = a one-turn
/// self buff; Sign &lt; 0 = an enemy debuff that weakens its upcoming turn. Mirrors the game's
/// TemporaryStrengthPower. Because the marker is applied on the player's turn and cleared at the owner's
/// turn end, it is never present at a validator checkpoint (turn-start / after-enemy) — only its effect
/// on damage is observed (enemy hit harder/softer → monster or player HP).</summary>
public abstract class TemporaryStrengthPower : PowerModel
{
    protected abstract int Sign { get; }
    public override PowerType Type => Sign > 0 ? PowerType.Buff : PowerType.Debuff;
    public override bool AllowNegative => false;          // the marker amount is always positive (the magnitude)

    private int _appliedStrength;                          // actual Strength delta already pushed to the owner

    public override void AfterApplied(CombatState combat, Creature? applier)
    {
        int want = Sign * Amount;
        int delta = want - _appliedStrength;
        if (delta != 0) Cmd.ApplyPower(combat, Owner, new StrengthPower(), delta, applier);
        _appliedStrength = want;
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;                   // only undoes at the OWNER's own turn end
        if (_appliedStrength != 0) Cmd.ApplyPower(combat, Owner, new StrengthPower(), -_appliedStrength, Owner);
        Owner.RemovePower(Id);
    }

    public override PowerModel Clone()
    {
        var c = (TemporaryStrengthPower)base.Clone();
        c._appliedStrength = _appliedStrength;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}/{_appliedStrength}";
    public override long HashValue() => base.HashValue() ^ ((long)_appliedStrength * 0x100000001B3L);
}

/// <summary>Poison. At the start of the owner's turn it deals <c>Amount</c> unblockable, unpowered
/// damage, then decrements by 1. STS2 differs from STS1 only in that it can tick multiple times in one
/// turn: the trigger count is <c>min(Amount, 1 + Σ Accelerant on the owner's opponents)</c> — each tick
/// deals the (now smaller) live Amount and decrements again. With no Accelerant in play this is the
/// familiar "deal Amount, lose 1 stack per turn". (MegaCrit PoisonPower.)</summary>
public sealed class PoisonPower : PowerModel
{
    public override string Id => "Poison";
    public override PowerType Type => PowerType.Debuff;

    /// <summary>min(Amount, 1 + total Accelerant on living opponents). Accelerant is itself a Silent
    /// power; until it is modelled the sum is 0, so this is just 1 while poisoned.</summary>
    private int TriggerCount(CombatState combat)
    {
        int accelerant = combat.AllCreatures
            .Where(c => c.Side != Owner.Side && c.IsAlive)
            .Sum(c => c.GetPowerAmount("Accelerant"));
        return Math.Min(Amount, 1 + accelerant);
    }

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        int iterations = TriggerCount(combat);
        for (int i = 0; i < iterations; i++)
        {
            Cmd.LoseHp(combat, Owner, Amount);     // unblockable + unpowered
            if (!Owner.IsAlive) break;
            Amount--;
            if (this.NormalizeOrRemove(Owner)) break;
        }
    }
}

/// <summary>At the start of the owner's next turn (after energy resets) gain <c>Amount</c> energy, then the
/// power is removed. Mirrors the game's EnergyNextTurnPower (AfterEnergyReset). Cross-character: granted by
/// the Event cards Outmaneuver/Relax and the Regent's Hegemony/RefineBlade/Convergence, so it lives in Core.
/// (MegaCrit EnergyNextTurnPower.)</summary>
public sealed class EnergyNextTurnPower : PowerModel
{
    public override string Id => "EnergyNextTurn";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;          // fires after BeginPlayerTurn's energy reset → adds on top
        Cmd.GainEnergy(combat, Amount);
        Owner.RemovePower(Id);
    }
}
