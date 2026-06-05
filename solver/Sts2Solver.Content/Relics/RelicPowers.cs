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

/// <summary>The owner loses Amount less HP from every source (Tungsten Rod -1). Routed through the engine's
/// post-block ModifyHpLost hook (the same one Intangible uses). (Game: relic ModifyHpLostAfterOsty.)</summary>
public sealed class RelicHpLossReductionPower : PowerModel
{
    public override string Id => "RelicHpLossReduction";
    public override PowerType Type => PowerType.Buff;
    public override int ModifyHpLost(Creature target, int hpLost, ValueProp props, Creature? dealer)
        => target == Owner ? Math.Max(0, hpLost - Amount) : hpLost;
}

/// <summary>The owner's powered attacks that would deal a small amount of unblocked damage to an enemy are
/// raised to a minimum of Amount (The Boot: 1–4 → 5). Routed through ModifyHpLost (post-block), which fires
/// for damage dealt to enemies too. (Game: relic ModifyHpLostAfterOstyLate.)</summary>
public sealed class RelicMinDamagePower : PowerModel
{
    public override string Id => "RelicMinDamage";
    public override PowerType Type => PowerType.Buff;
    public override int ModifyHpLost(Creature target, int hpLost, ValueProp props, Creature? dealer)
    {
        if (dealer != Owner || target == Owner || !props.IsPoweredAttack()) return hpLost;
        return hpLost >= 1 && hpLost < Amount ? Amount : hpLost;
    }
}

/// <summary>The owner's Power cards cost Amount more energy (Spiked Gauntlets' downside; its +1 energy upside
/// is the separate RelicMaxEnergyPower). Routed through the engine's ModifyCardCost pipeline, so the surcharge
/// is visible to affordability in search. (Game: relic TryModifyEnergyCostInCombat for CardType.Power.)</summary>
public sealed class RelicPowerCostSurchargePower : PowerModel
{
    public override string Id => "RelicPowerCostSurcharge";
    public override PowerType Type => PowerType.Buff;
    public override int ModifyCardCost(CardModel card, int cost)
        => card.Type == CardType.Power ? cost + Amount : cost;
}

/// <summary>The owner draws Amount extra card(s) at the start of every turn (Pael's Blood +1). Summed by the
/// engine's turn-start draw count, exactly like the MachineLearning power. (Game: relic ModifyHandDraw.)</summary>
public sealed class RelicDrawPower : PowerModel
{
    public override string Id => "RelicDraw";
    public override PowerType Type => PowerType.Buff;
    public override int ModifyHandDraw(Creature player, int count) => player == Owner ? count + Amount : count;
}
