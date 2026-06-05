using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// Hidden "relic powers": passive combat modifiers carried by a relic. The game implements these relics with
// RelicModel.ModifyMaxEnergy / ModifyDamageAdditive hooks; rather than duplicate that pipeline on RelicModel we
// route the same effect through a PowerModel installed on the player at combat start. The engine already sums
// these power hooks (energy reset, damage pipeline) and already clones + hashes powers, so an installed relic
// power is sound w.r.t. search/memoisation for free, and inert for any deck that doesn't carry the relic.

/// <summary>Passive +max-energy carried by an energy relic (Ectoplasm +1). Installed on the player at combat
/// start; summed into the per-turn energy reset via the standard power pipeline. (Game: the relic's
/// RelicModel.ModifyMaxEnergy.)</summary>
public sealed class RelicMaxEnergyPower : PowerModel
{
    public override string Id => "RelicMaxEnergy";
    public override PowerType Type => PowerType.Buff;
    public override int ModifyMaxEnergy(Creature player) => player == Owner ? Amount : 0;
}

/// <summary>Flat extra damage on the owner's powered Strike attacks (Strike Dummy +3 / Fake Strike Dummy +1).
/// Mirrors the game's relic ModifyDamageAdditive gated on CardTag.Strike + IsPoweredAttack.</summary>
public sealed class RelicStrikeDamagePower : PowerModel
{
    public override string Id => "RelicStrikeDamage";
    public override PowerType Type => PowerType.Buff;
    public override decimal ModifyDamageAdditive(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (dealer != Owner || !props.IsPoweredAttack()) return 0m;
        return cardSource?.IsStrike == true ? Amount : 0m;
    }
}

/// <summary>Flat extra damage on the owner's powered attacks from UPGRADED cards (Miniature Cannon +3).
/// Mirrors the game's relic ModifyDamageAdditive gated on cardSource.IsUpgraded + IsPoweredAttack.</summary>
public sealed class RelicUpgradedDamagePower : PowerModel
{
    public override string Id => "RelicUpgradedDamage";
    public override PowerType Type => PowerType.Buff;
    public override decimal ModifyDamageAdditive(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (dealer != Owner || !props.IsPoweredAttack()) return 0m;
        return cardSource is { Upgrades: > 0 } ? Amount : 0m;
    }
}
