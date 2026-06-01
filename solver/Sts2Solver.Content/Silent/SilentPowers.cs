using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Silent-only powers. Generic keyword powers shared with other characters
// (Poison, Weak, Vulnerable, Strength, Dexterity, …) live in Core/CommonPowers.cs.
// ===========================================================================

/// <summary>At the start of the owner's turn, apply <c>Amount</c> Poison to every living enemy. Applied
/// mid-turn, so the first application lands at the NEXT turn start (like Demon Form). (MegaCrit
/// NoxiousFumesPower.)</summary>
public sealed class NoxiousFumesPower : PowerModel
{
    public override string Id => "NoxiousFumes";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.ApplyPower(combat, m, new PoisonPower(), Amount, Owner);
    }
}
/// <summary>Whenever the owner deals unblocked damage with a powered attack, apply <c>Amount</c> Poison
/// to the struck target. Multi-hit attacks apply it per unblocked hit (mirrors the game's per-hit
/// AfterDamageGiven). (MegaCrit EnvenomPower.)</summary>
public sealed class EnvenomPower : PowerModel
{
    public override string Id => "Envenom";
    public override PowerType Type => PowerType.Buff;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (dealer != Owner || unblockedDamage <= 0 || !props.IsPoweredAttack()) return;
        if (target.IsAlive)
            Cmd.ApplyPower(combat, target, new PoisonPower(), Amount, Owner);
    }
}
/// <summary>Whenever the owner plays a card, gain <c>Amount</c> Block (unpowered — no Dexterity). The card
/// that grants Afterimage does not trigger it (the game records eligibility before OnPlay, when the power
/// isn't attached yet); we mirror that with a one-shot skip set when the power is applied. (MegaCrit
/// AfterimagePower.)</summary>
public sealed class AfterimagePower : PowerModel
{
    public override string Id => "Afterimage";
    public override PowerType Type => PowerType.Buff;

    private bool _skipApplyingCard;

    public override void AfterApplied(CombatState combat, Creature? applier) => _skipApplyingCard = true;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (_skipApplyingCard) { _skipApplyingCard = false; return; }   // the Afterimage card itself
        Cmd.GainBlock(combat, Owner, Amount, ValueProp.Unpowered, null);
    }

    public override PowerModel Clone()
    {
        var c = (AfterimagePower)base.Clone();
        c._skipApplyingCard = _skipApplyingCard;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}{(_skipApplyingCard ? "*" : "")}";
    public override long HashValue() => base.HashValue() ^ (_skipApplyingCard ? 0x1B873593L : 0L);
}
/// <summary>Blur: the owner's Block is not cleared at the start of its next turn. Counter; decrements one
/// per owner turn start, so Blur 1 carries this turn's block through one extra turn. (MegaCrit BlurPower —
/// modelled via the engine's PreventsBlockClear flag.)</summary>
public sealed class BlurPower : PowerModel
{
    public override string Id => "Blur";
    public override PowerType Type => PowerType.Buff;

    public override bool PreventsBlockClear => true;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        // Block-clear is skipped earlier in BeginPlayerTurn (PreventsBlockClear); now spend a charge.
        if (side == Owner.Side) { Amount--; this.NormalizeOrRemove(Owner); }
    }
}
/// <summary>At the start of the owner's next turn (after block clears), gain <c>Amount</c> Block
/// (unpowered), then the power is removed. The amount is the block actually gained by the card that
/// applied it. (MegaCrit BlockNextTurnPower — Dodge and Roll.)</summary>
public sealed class BlockNextTurnPower : PowerModel
{
    public override string Id => "BlockNextTurn";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        Cmd.GainBlock(combat, Owner, Amount, ValueProp.Unpowered, null);
        Owner.RemovePower(Id);
    }
}
/// <summary>Piercing Wail: temporarily reduces the target enemy's Strength by <c>Amount</c> until its turn
/// ends, weakening its upcoming attack (then restored). Same mechanic as Mangle. (MegaCrit
/// PiercingWailPower — a TemporaryStrengthPower with negative sign.)</summary>
public sealed class PiercingWailPower : TemporaryStrengthPower
{
    public override string Id => "PiercingWail";
    protected override int Sign => -1;
}
