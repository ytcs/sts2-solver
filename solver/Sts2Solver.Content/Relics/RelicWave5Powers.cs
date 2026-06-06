using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// Hidden relic powers for batch-5 relics that react to damage / stars / play-count via the existing power hooks.
// Each carries cloned + hashed state (a once-per-combat flag, a per-turn flag, or a counter) and is installed by
// its relic at combat start; inert for any deck without that relic. SOUNDNESS: every effect is deterministic.

/// <summary>Centennial Puzzle: the FIRST time you take unblocked damage in a combat, draw 3 cards (once).
/// (MegaCrit CentennialPuzzle.)</summary>
public sealed class CentennialPuzzlePower : PowerModel
{
    private bool _used;
    public override string Id => "RelicCentennialPuzzle";
    public override PowerType Type => PowerType.Buff;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (_used || target != Owner || unblockedDamage <= 0) return;
        _used = true;
        Cmd.Draw(combat, 3);
    }

    public override PowerModel Clone() { var c = (CentennialPuzzlePower)base.Clone(); c._used = _used; return c; }
    public override string StateKey() => _used ? "RelicCentennialPuzzle!" : "RelicCentennialPuzzle";
    public override long HashValue() => base.HashValue() ^ (_used ? 0x5BD1E9955BD1E995L : 0L);
}

/// <summary>Demon Tongue: the FIRST time you take unblocked damage each TURN, heal that much. (MegaCrit DemonTongue.)</summary>
public sealed class DemonTonguePower : PowerModel
{
    private bool _healedThisTurn;
    public override string Id => "RelicDemonTongue";
    public override PowerType Type => PowerType.Buff;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (_healedThisTurn || target != Owner || unblockedDamage <= 0) return;
        _healedThisTurn = true;
        Owner.Heal(unblockedDamage);
    }

    public override void AfterSideTurnStart(CombatState combat, CombatSide side) { if (side == Owner.Side) _healedThisTurn = false; }

    public override PowerModel Clone() { var c = (DemonTonguePower)base.Clone(); c._healedThisTurn = _healedThisTurn; return c; }
    public override string StateKey() => _healedThisTurn ? "RelicDemonTongue!" : "RelicDemonTongue";
    public override long HashValue() => base.HashValue() ^ (_healedThisTurn ? 0x27D4EB2F165667C5L : 0L);
}

/// <summary>Galactic Dust: every 10 Stars spent, gain 10 Block. (MegaCrit GalacticDust.)</summary>
public sealed class GalacticDustPower : PowerModel
{
    private int _stars;                               // Stars spent since the last 10-block grant (0..9)
    public override string Id => "RelicGalacticDust";
    public override PowerType Type => PowerType.Buff;

    public override void AfterStarsSpent(CombatState combat, int amount)
    {
        if (Owner != combat.Player || amount <= 0) return;
        _stars += amount;
        while (_stars >= 10) { _stars -= 10; Cmd.GainBlock(combat, Owner, 10, ValueProp.Unpowered, null); }
    }

    public override PowerModel Clone() { var c = (GalacticDustPower)base.Clone(); c._stars = _stars; return c; }
    public override string StateKey() => $"RelicGalacticDust#{_stars}";
    public override long HashValue() => ((long)Id.GetHashCode() << 20) ^ (uint)_stars;
}

/// <summary>Mini Regent: the first time you spend Stars each TURN, gain 1 Strength. (MegaCrit MiniRegent.)</summary>
public sealed class MiniRegentPower : PowerModel
{
    private bool _grantedThisTurn;
    public override string Id => "RelicMiniRegent";
    public override PowerType Type => PowerType.Buff;

    public override void AfterStarsSpent(CombatState combat, int amount)
    {
        if (_grantedThisTurn || Owner != combat.Player || amount <= 0) return;
        _grantedThisTurn = true;
        Cmd.ApplyPower(combat, Owner, new StrengthPower(), 1, Owner);
    }

    public override void AfterSideTurnStart(CombatState combat, CombatSide side) { if (side == Owner.Side) _grantedThisTurn = false; }

    public override PowerModel Clone() { var c = (MiniRegentPower)base.Clone(); c._grantedThisTurn = _grantedThisTurn; return c; }
    public override string StateKey() => _grantedThisTurn ? "RelicMiniRegent!" : "RelicMiniRegent";
    public override long HashValue() => base.HashValue() ^ (_grantedThisTurn ? 0x165667B19E3779F9L : 0L);
}

/// <summary>Beating Remnant: cap the HP you can lose to 20 per turn. Routed through ModifyHpLost (the cap) plus
/// AfterDamageReceived (accumulating this turn's loss), reset each turn. (MegaCrit BeatingRemnant.)</summary>
public sealed class BeatingRemnantPower : PowerModel
{
    private int _lostThisTurn;
    private const int Cap = 20;
    public override string Id => "RelicBeatingRemnant";
    public override PowerType Type => PowerType.Buff;

    public override int ModifyHpLost(Creature target, int hpLost, ValueProp props, Creature? dealer)
        => target == Owner ? Math.Min(hpLost, Math.Max(0, Cap - _lostThisTurn)) : hpLost;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    { if (target == Owner) _lostThisTurn += unblockedDamage; }

    public override void AfterSideTurnStart(CombatState combat, CombatSide side) { if (side == Owner.Side) _lostThisTurn = 0; }

    public override PowerModel Clone() { var c = (BeatingRemnantPower)base.Clone(); c._lostThisTurn = _lostThisTurn; return c; }
    public override string StateKey() => $"RelicBeatingRemnant#{_lostThisTurn}";
    public override long HashValue() => ((long)Id.GetHashCode() << 20) ^ (uint)_lostThisTurn;
}

/// <summary>Vambrace: the FIRST block-granting card you play each combat grants double Block. (MegaCrit Vambrace.)</summary>
public sealed class VambracePower : PowerModel
{
    private bool _used;
    public override string Id => "RelicVambrace";
    public override PowerType Type => PowerType.Buff;

    public override decimal ModifyBlockMultiplicative(Creature target, decimal block, ValueProp props, CardModel? cardSource)
        => !_used && target == Owner && props.IsPoweredBlock() && cardSource != null ? 2m : 1m;

    public override void AfterBlockGained(CombatState combat, Creature creature, int amount, ValueProp props, CardModel? cardSource)
    { if (!_used && creature == Owner && cardSource != null && amount > 0) _used = true; }

    public override PowerModel Clone() { var c = (VambracePower)base.Clone(); c._used = _used; return c; }
    public override string StateKey() => _used ? "RelicVambrace!" : "RelicVambrace";
    public override long HashValue() => base.HashValue() ^ (_used ? 0x2545F4914F6CDD1DL : 0L);
}

/// <summary>Ruined Helmet: the FIRST time you gain Strength in a combat, gain that much again (the gain is
/// doubled). Watches the post-apply broadcast for a positive Strength application to the owner, then adds an
/// equal Strength once and disarms. The re-apply is guarded by <c>_used</c> (set before it) so it can't recurse,
/// and the broadcast iterates a snapshot — re-entrancy-safe. (MegaCrit RuinedHelmet.)</summary>
public sealed class RuinedHelmetPower : PowerModel
{
    private bool _used;
    public override string Id => "RelicRuinedHelmet";
    public override PowerType Type => PowerType.Buff;

    public override void AfterPowerApplied(CombatState combat, Creature target, PowerModel power, int amount, Creature? applier)
    {
        if (_used || target != Owner || power.Id != "Strength" || amount <= 0) return;
        _used = true;                                                  // disarm BEFORE re-applying (no recursion)
        Cmd.ApplyPower(combat, Owner, new StrengthPower(), amount, Owner);   // double the first gain
    }

    public override PowerModel Clone() { var c = (RuinedHelmetPower)base.Clone(); c._used = _used; return c; }
    public override string StateKey() => _used ? "RelicRuinedHelmet!" : "RelicRuinedHelmet";
    public override long HashValue() => base.HashValue() ^ (_used ? 0x3C6EF372FE94F82BL : 0L);
}

/// <summary>Throwing Axe: the FIRST card you play each combat is played an extra time. (MegaCrit ThrowingAxe.)</summary>
public sealed class ThrowingAxePower : PowerModel
{
    private bool _used;
    public override string Id => "RelicThrowingAxe";
    public override PowerType Type => PowerType.Buff;

    public override int ModifyCardPlayCount(CardModel card) => _used ? 0 : 1;
    public override void AfterModifyingCardPlayCount(CombatState combat, CardModel card) => _used = true;

    public override PowerModel Clone() { var c = (ThrowingAxePower)base.Clone(); c._used = _used; return c; }
    public override string StateKey() => _used ? "RelicThrowingAxe!" : "RelicThrowingAxe";
    public override long HashValue() => base.HashValue() ^ (_used ? 0x5BD1E9955BD1E995L : 0L);
}
