using System;
using System.Collections.Generic;
using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// Concrete card enchantments, ported from sts2.dll v0.111.0 (MegaCrit.Sts2.Core.Models.Enchantments).
// Each mirrors one game enchantment's combat effect via the CardEnchantment hooks. The powered-attack /
// powered-block guards live in Cmd (it only calls DamageAdditive/Multiplier/BlockAdditive for powered actions),
// so these just return their magnitude. Eligibility (CanEnchant) is the game's; we don't re-check it — the save
// only ever carries a legal enchant — but it's noted per class.
//
// NOT ported here (warned as unmodelled by the advisor): Swift / Imbued / SlumberingEssence (need the deferred-
// draw, opening-hand auto-play, and per-turn-held-cost machinery respectively); Slither (RNG cost), Goopy
// (cross-combat ramp), PerfectFit (draw-order only), Clone (no combat effect).

/// <summary>Sharp (Attack): +Amount damage. Game: EnchantDamageAdditive => Amount.</summary>
public sealed class SharpEnchant : CardEnchantment
{
    public override string Id => "Sharp";
    public override int DamageAdditive(ValueProp props) => Amount;
}

/// <summary>Inky (any): apply 1 Weak on play. v0.111.0 removed the +1 damage rider (still applies Weak).</summary>
public sealed class InkyEnchant : CardEnchantment
{
    public override string Id => "Inky";
    public override void OnPlay(CombatState combat, CardPlay play, CardModel card)
    {
        if (card.Target == TargetType.AllEnemies)
        {
            foreach (var m in combat.LivingMonsters.ToList())
                Cmd.ApplyPower(combat, m, new WeakPower(), 1, combat.Player);
        }
        else if (play.Target is { IsAlive: true } t)
            Cmd.ApplyPower(combat, t, new WeakPower(), 1, combat.Player);
    }
}

/// <summary>Instinct (Attack): ×2 damage. Game: EnchantDamageMultiplicative => 2.</summary>
public sealed class InstinctEnchant : CardEnchantment
{
    public override string Id => "Instinct";
    public override double DamageMultiplier(ValueProp props) => 2.0;
}

/// <summary>Corrupted (Attack): ×1.5 damage, and lose 2 HP on play. Game: ×1.5 + 2 Unblockable/Unpowered self.</summary>
public sealed class CorruptedEnchant : CardEnchantment
{
    public override string Id => "Corrupted";
    public override double DamageMultiplier(ValueProp props) => 1.5;
    public override void OnPlay(CombatState combat, CardPlay play, CardModel card)
        => Cmd.Attack(combat, combat.Player, combat.Player, 2,
            ValueProp.Unblockable | ValueProp.Unpowered | ValueProp.Move, null);
}

/// <summary>TezcatarasEmber (Attack): cost → 0 and +3 damage (Eternal not modelled — no single-combat effect).</summary>
public sealed class TezcatarasEmberEnchant : CardEnchantment
{
    public override string Id => "TezcatarasEmber";
    public override bool SetsCostZero => true;
    public override int DamageAdditive(ValueProp props) => 3;
}

/// <summary>Nimble (gains-block card): +Amount block. Game: EnchantBlockAdditive => Amount.</summary>
public sealed class NimbleEnchant : CardEnchantment
{
    public override string Id => "Nimble";
    public override int BlockAdditive() => Amount;
}

/// <summary>Adroit (any): gain Amount block on play. Game: OnPlay GainBlock(Amount).</summary>
public sealed class AdroitEnchant : CardEnchantment
{
    public override string Id => "Adroit";
    public override void OnPlay(CombatState combat, CardPlay play, CardModel card)
        => Cmd.GainBlock(combat, combat.Player, Amount, ValueProp.Move, card);
}

/// <summary>Steady: the card gains Retain. Game: OnEnchant AddKeyword(Retain).</summary>
public sealed class SteadyEnchant : CardEnchantment
{
    public override string Id => "Steady";
    public override bool AddsRetain => true;
}

/// <summary>RoyallyApproved (Attack/Skill): the card gains Innate + Retain. Game: OnEnchant adds both.</summary>
public sealed class RoyallyApprovedEnchant : CardEnchantment
{
    public override string Id => "RoyallyApproved";
    public override bool AddsInnate => true;
    public override bool AddsRetain => true;
}

/// <summary>SoulsPower (Exhaust card): the card no longer exhausts. Game: OnEnchant RemoveKeyword(Exhaust).</summary>
public sealed class SoulsPowerEnchant : CardEnchantment
{
    public override string Id => "SoulsPower";
    public override bool RemovesExhaust => true;
}

/// <summary>Sown: gain Amount energy the FIRST time the card is played each combat. Game: OnPlay (Normal) →
/// GainEnergy(Amount), then disable.</summary>
public sealed class SownEnchant : CardEnchantment
{
    private bool _used;
    public override string Id => "Sown";
    public override bool Stateful => true;
    public override void OnPlay(CombatState combat, CardPlay play, CardModel card)
    {
        if (_used) return;
        combat.Player.Energy += Amount;
        _used = true;
    }
    public override string Key() => _used ? $"Sown!" : $"Sown:{Amount}";
}

/// <summary>Vigorous (Attack): +Amount damage on the FIRST play each combat only. Game: EnchantDamageAdditive
/// while Normal; AfterCardPlayed disables.</summary>
public sealed class VigorousEnchant : CardEnchantment
{
    private bool _used;
    public override string Id => "Vigorous";
    public override bool Stateful => true;
    public override int DamageAdditive(ValueProp props) => _used ? 0 : Amount;
    public override void AfterPlayed(CombatState combat, CardModel card) => _used = true;
    public override string Key() => _used ? "Vigorous!" : $"Vigorous:{Amount}";
}

/// <summary>Momentum (Attack): each play permanently adds Amount damage (the ramp applies from the NEXT play —
/// the enchant rider runs after the card's own attack). Game: OnPlay ExtraDamage += Amount; additive => ExtraDamage.</summary>
public sealed class MomentumEnchant : CardEnchantment
{
    private int _extra;
    public override string Id => "Momentum";
    public override bool Stateful => true;
    public override int DamageAdditive(ValueProp props) => _extra;
    public override void AfterPlayed(CombatState combat, CardModel card) => _extra += Amount;
    public override string Key() => $"Momentum:{Amount}/{_extra}";
}

/// <summary>Spiral (Basic Strike/Defend): the card resolves one extra time, every play. Game: EnchantPlayCount +1.</summary>
public sealed class SpiralEnchant : CardEnchantment
{
    public override string Id => "Spiral";
    public override int ExtraPlayCount => 1;
}

/// <summary>Glam (any): the card resolves one extra time on its FIRST play each combat. Game: EnchantPlayCount
/// +1 until played, then disables.</summary>
public sealed class GlamEnchant : CardEnchantment
{
    private bool _used;
    public override string Id => "Glam";
    public override bool Stateful => true;
    public override int ExtraPlayCount => _used ? 0 : 1;
    public override void AfterPlayed(CombatState combat, CardModel card) => _used = true;
    public override string Key() => _used ? "Glam!" : "Glam";
}

/// <summary>Maps a game enchantment class name (e.g. "Sharp") to a solver enchantment carrying the save amount.
/// The clean, single-combat-sound subset of the 22 game enchantments; the rest are warned as unmodelled.</summary>
public static class EnchantmentCatalog
{
    private static readonly Dictionary<string, Func<int, CardEnchantment>> Factories = new(StringComparer.Ordinal)
    {
        ["Sharp"]           = a => new SharpEnchant { Amount = a },
        ["Inky"]            = a => new InkyEnchant { Amount = a },
        ["Instinct"]        = a => new InstinctEnchant { Amount = a },
        ["Corrupted"]       = a => new CorruptedEnchant { Amount = a },
        ["TezcatarasEmber"] = a => new TezcatarasEmberEnchant { Amount = a },
        ["Nimble"]          = a => new NimbleEnchant { Amount = a },
        ["Adroit"]          = a => new AdroitEnchant { Amount = a },
        ["Steady"]          = a => new SteadyEnchant { Amount = a },
        ["RoyallyApproved"] = a => new RoyallyApprovedEnchant { Amount = a },
        ["SoulsPower"]      = a => new SoulsPowerEnchant { Amount = a },
        ["Sown"]            = a => new SownEnchant { Amount = a },
        ["Vigorous"]        = a => new VigorousEnchant { Amount = a },
        ["Momentum"]        = a => new MomentumEnchant { Amount = a },
        ["Spiral"]          = a => new SpiralEnchant { Amount = a },
        ["Glam"]            = a => new GlamEnchant { Amount = a },
    };

    public static bool IsModelled(string enchantName) => Factories.ContainsKey(enchantName);

    public static CardEnchantment Build(string enchantName, int amount) => Factories[enchantName](amount);
}
