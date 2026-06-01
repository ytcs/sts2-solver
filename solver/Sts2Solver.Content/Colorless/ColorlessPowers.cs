using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Colorless-only powers. Generic keyword powers shared with other characters
// (Poison, Weak, Vulnerable, Strength, Dexterity, …) live in Core/CommonPowers.cs.
// ===========================================================================

/// <summary>Dark Shackles: temporarily reduces the target enemy's Strength by <c>Amount</c> until its turn
/// ends, weakening its upcoming attack (then restored). Same mechanic as Mangle / Piercing Wail. (MegaCrit
/// DarkShacklesPower — a TemporaryStrengthPower with negative sign.)</summary>
public sealed class DarkShacklesPower : TemporaryStrengthPower
{
    public override string Id => "DarkShackles";
    protected override int Sign => -1;
}

/// <summary>Panache: counts cards played; every <c>5th</c> card played this combat deals <c>Amount</c>
/// damage to ALL enemies. The counter is per-combat mutable state, so the card that grants Panache is
/// Stateful via this power's own StateKey (powers are cloned with combat state). (MegaCrit PanachePower.)
/// </summary>
public sealed class PanachePower : PowerModel
{
    public override string Id => "Panache";
    public override PowerType Type => PowerType.Buff;

    public const int Period = 5;
    private int _counter;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        _counter++;
        if (_counter < Period) return;
        _counter = 0;
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, Owner, m, Amount, ValueProp.Move, null);
    }

    public override PowerModel Clone()
    {
        var c = (PanachePower)base.Clone();
        c._counter = _counter;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}/{_counter}";
    public override long HashValue() => base.HashValue() ^ ((long)_counter * 0x100000001B3L);
}

/// <summary>Panic Button aftermath: the owner cannot gain Block for the next <c>Amount</c> turns. Block
/// gains are multiplied by 0; the counter decrements at the owner's turn end. (MegaCrit — modelled via the
/// engine's block-multiplier + a per-turn countdown.)</summary>
public sealed class NoBlockPower : PowerModel
{
    public override string Id => "NoBlock";
    public override PowerType Type => PowerType.Debuff;

    public override decimal ModifyBlockMultiplicative(Creature target, decimal block, ValueProp props, CardModel? cardSource)
        => target == Owner ? 0m : 1m;

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) { Amount--; this.NormalizeOrRemove(Owner); }
    }
}

/// <summary>The Bomb: at the END of the owner's turn the countdown drops by 1; when it reaches 0 it deals
/// <c>BombDamage</c> to ALL enemies (then the power is removed). The countdown is tracked in <c>_turns</c>
/// (mutable per-combat state, carried through Clone / StateKey). (MegaCrit TheBombPower.)</summary>
public sealed class TheBombPower : PowerModel
{
    public override string Id => "TheBomb";
    public override PowerType Type => PowerType.Buff;

    // Amount carries the explosion damage; _turns is the remaining countdown (set on application).
    private int _turns;
    public void SetCountdown(int turns) => _turns = turns;

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        _turns--;
        if (_turns > 0) return;
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, Owner, m, Amount, ValueProp.Move, null);
        Owner.RemovePower(Id);
    }

    public override PowerModel Clone()
    {
        var c = (TheBombPower)base.Clone();
        c._turns = _turns;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}/{_turns}";
    public override long HashValue() => base.HashValue() ^ ((long)_turns * 0x1000193L);
}

/// <summary>Mayhem: at the start of each of the owner's turns, the top card of the draw pile is auto-played
/// <c>Amount</c> times. The auto-played card depends on draw order (RNG) and is recorded as an auto-play in
/// the trace, so this is modelled as an inert marker (auto-plays are replayed from the trace). (MegaCrit
/// MayhemPower — ported as an inert power, mirroring Aggression/Stampede/Hellraiser in IroncladCards.)</summary>
public sealed class MayhemPower : PowerModel
{
    public override string Id => "Mayhem";
    public override PowerType Type => PowerType.Buff;
}
