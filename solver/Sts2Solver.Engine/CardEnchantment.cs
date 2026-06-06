namespace Sts2Solver.Engine;

/// <summary>
/// A permanent per-card modifier carried over from the run (the game's <c>EnchantmentModel</c>). A card has at
/// most one, plus a save-supplied <see cref="Amount"/> (the scalable magnitude — +N damage, +N block, draw N…).
/// Mirrors the game's enchant hooks: damage/block riders on the card's OWN powered attack/block, keyword grants,
/// cost changes, an on-play rider, and a bonus play count. Concrete enchantments live in Content (they apply
/// powers); this base + its hooks live here so <see cref="CardModel"/>, <see cref="Cmd"/> and
/// <see cref="CombatManager"/> can consult them generically.
///
/// SOUNDNESS: the enchant is folded into the card's <see cref="CardModel.StateKey"/> (see <see cref="Key"/>), so
/// search memoisation stays correct. A <see cref="Stateful"/> enchant (per-combat one-shot flag or a per-play
/// ramp) is deep-cloned with its card so sibling search branches never share mutable state.
/// </summary>
public abstract class CardEnchantment
{
    /// <summary>The save-supplied magnitude (game: <c>SerializableEnchantment.amount</c>). 0 for the fixed-effect
    /// enchantments whose effect doesn't scale (Steady/SoulsPower/…).</summary>
    public int Amount;

    /// <summary>Stable name (the game enchantment class name, e.g. "Sharp"), used in <see cref="Key"/>.</summary>
    public abstract string Id { get; }

    /// <summary>Flat damage added to THIS card's powered attacks (game: <c>EnchantDamageAdditive</c>).</summary>
    public virtual int DamageAdditive(ValueProp props) => 0;

    /// <summary>Multiplier on THIS card's powered attacks, applied after additive modifiers (game:
    /// <c>EnchantDamageMultiplicative</c>).</summary>
    public virtual double DamageMultiplier(ValueProp props) => 1.0;

    /// <summary>Flat block added to THIS card's powered block gains (game: <c>EnchantBlockAdditive</c>).</summary>
    public virtual int BlockAdditive() => 0;

    /// <summary>Card gains Retain (stays in hand at end of turn). Game: OnEnchant <c>AddKeyword(Retain)</c>.</summary>
    public virtual bool AddsRetain => false;

    /// <summary>Card gains Innate (guaranteed in the opening hand). Game: OnEnchant <c>AddKeyword(Innate)</c>.</summary>
    public virtual bool AddsInnate => false;

    /// <summary>Card no longer exhausts on play (game: OnEnchant <c>RemoveKeyword(Exhaust)</c>, SoulsPower).</summary>
    public virtual bool RemovesExhaust => false;

    /// <summary>Card costs 0 (game: TezcatarasEmber upgrades the cost down to 0).</summary>
    public virtual bool SetsCostZero => false;

    /// <summary>Additive cost change (negative = cheaper). Default none.</summary>
    public virtual int CostDelta(CombatState combat, CardModel card) => 0;

    /// <summary>A rider that runs alongside the card's own <see cref="CardModel.OnPlay"/>, once per resolution
    /// (so it fires again on a replayed play). Runs AFTER the card's effect — matching the game's Inky (its Weak
    /// lands after the attack, never weakening the attack's own damage).</summary>
    public virtual void OnPlay(CombatState combat, CardPlay play, CardModel card) { }

    /// <summary>Extra times the card's effect resolves this play (game: <c>EnchantPlayCount</c> − 1). Reuses the
    /// engine's existing bonus-play loop (One-Two Punch). Read BEFORE the play, so a one-shot replay can disable
    /// itself in <see cref="AfterPlayed"/>.</summary>
    public virtual int ExtraPlayCount => 0;

    /// <summary>Fired once after the card finishes playing — for one-shot disabling and per-play ramps.</summary>
    public virtual void AfterPlayed(CombatState combat, CardModel card) { }

    /// <summary>True when the enchant carries MUTABLE per-combat state (a one-shot flag or a ramp). Such cards
    /// must deep-clone and must not cache their key/hash. See <see cref="CardModel.Stateful"/>.</summary>
    public virtual bool Stateful => false;

    public virtual CardEnchantment Clone() => (CardEnchantment)MemberwiseClone();

    /// <summary>Identity contribution to the owning card's <see cref="CardModel.StateKey"/> — the id, the amount,
    /// and (for <see cref="Stateful"/> enchants) the live flag/ramp so distinct search states stay distinct.</summary>
    public virtual string Key() => Amount != 0 ? $"{Id}:{Amount}" : Id;
}
