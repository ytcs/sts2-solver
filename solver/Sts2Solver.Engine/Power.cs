namespace Sts2Solver.Engine;

/// <summary>
/// Base for all powers (buffs/debuffs/statuses). Mirrors MegaCrit.Sts2.Core.Models.PowerModel:
/// stacks are tracked as an integer Amount, and behaviour is expressed through overridable hooks
/// that fire at well-defined points in the combat pipeline (Appendix A of the plan).
///
/// Hooks default to no-ops / identity so a concrete power only overrides what it touches.
/// </summary>
public abstract class PowerModel
{
    /// <summary>Stable identifier used for state hashing and lookup (e.g. "Vulnerable").</summary>
    public abstract string Id { get; }

    public abstract PowerType Type { get; }

    /// <summary>If false, Amount is clamped at 0 and the power is removed when it would go below.</summary>
    public virtual bool AllowNegative => false;

    /// <summary>If true, the owner's hand is NOT discarded at end of their turn (Well-Laid Plans, Runic
    /// Pyramid). Ethereal cards are also kept — the game skips the entire flush. (Game: AbstractModel.ShouldFlush.)</summary>
    public virtual bool PreventsHandFlush => false;

    /// <summary>Additive bonus to the player's max energy while owned (Pyre). Summed over the player's
    /// powers when energy resets at turn start. (Game: PowerModel.ModifyMaxEnergy.)</summary>
    public virtual int ModifyMaxEnergy(Creature player) => 0;

    /// <summary>Modifies the player's turn-start hand-draw count (MachineLearning adds Amount). Chained over the
    /// player's powers in <see cref="CombatManager.TurnStartDrawCount"/>. (Game: PowerModel.ModifyHandDraw.)</summary>
    public virtual int ModifyHandDraw(Creature player, int count) => count;

    /// <summary>Additive bonus to the Vulnerable damage multiplier when the OWNER (an attacker) deals a
    /// powered attack to a Vulnerable target. Cruelty adds Amount/100. Summed by VulnerablePower over the
    /// dealer's powers. (Game: PowerModel.ModifyVulnerableMultiplier.)</summary>
    public virtual decimal VulnerableMultiplierBonus() => 0m;

    public int Amount { get; set; }

    /// <summary>Set by the engine when the power is attached to a creature.</summary>
    public Creature Owner { get; set; } = null!;

    /// <summary>When true, the NEXT owner-turn-end duration tick is skipped (then this clears). Mirrors the
    /// game's <c>SkipNextDurationTick</c>: a debuff freshly applied at the end of a turn (Doubt's Weak,
    /// Shame's Frail) must not be consumed by that same turn's end tick, so it survives to affect the
    /// following turn. Honoured by the ticking debuffs (Weak/Vulnerable/Frail) in <see cref="AfterSideTurnEnd"/>.</summary>
    public bool SkipNextTick;

    // ---- Damage modification hooks (run during DamagePipeline) ----

    /// <summary>Flat damage added. Strength adds its Amount here (dealer side, powered attacks).</summary>
    public virtual decimal ModifyDamageAdditive(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource) => 0m;

    /// <summary>Multiplier applied to damage. Vulnerable ×1.5 (target side), Weak ×0.75 (dealer side).</summary>
    public virtual decimal ModifyDamageMultiplicative(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource) => 1m;

    /// <summary>Modifies the final HP loss (post-block) before it is applied to the target. HardenedShell
    /// caps the owner's total HP loss per turn here. (Mirrors the game's ModifyHpLostBeforeOsty.)</summary>
    public virtual int ModifyHpLost(Creature target, int hpLost, ValueProp props, Creature? dealer) => hpLost;

    /// <summary>Redirects the UNBLOCKED remainder of an attack (after the original target's block has been
    /// absorbed) onto a different creature, which takes it as direct HP loss. DieForYouPower (on Osty) sends
    /// the post-block portion of powered attacks aimed at its owner-player onto Osty instead — so the player's
    /// block still soaks the hit and only the leftover reaches Osty. Return the (possibly new) HP-loss target.
    /// (Mirrors the game's PowerModel.ModifyUnblockedDamageTarget, called after DamageBlockInternal.)</summary>
    public virtual Creature ModifyUnblockedDamageTarget(Creature target, int unblocked, ValueProp props, Creature? dealer) => target;

    // ---- Block modification hooks (run during BlockPipeline) ----

    /// <summary>Flat block added. Dexterity adds its Amount here.</summary>
    public virtual decimal ModifyBlockAdditive(Creature target, decimal block, ValueProp props, CardModel? cardSource) => 0m;

    /// <summary>Multiplier applied to block. Frail ×0.75.</summary>
    public virtual decimal ModifyBlockMultiplicative(Creature target, decimal block, ValueProp props, CardModel? cardSource) => 1m;

    // ---- Turn lifecycle hooks ----

    /// <summary>Fires when this power is applied/stacked onto its owner (Ritual uses it to skip the
    /// turn-end it was applied on).</summary>
    public virtual void AfterApplied(CombatState combat, Creature? applier) { }

    /// <summary>Fires (on every power in combat) after any power is applied to any creature, with the
    /// applied power, its amount, and the applier. Vicious draws cards when the owner applies Vulnerable.
    /// (Game: PowerModel.AfterPowerAmountChanged.)</summary>
    public virtual void AfterPowerApplied(CombatState combat, Creature target, PowerModel power, int amount, Creature? applier) { }

    /// <summary>Offered each incoming debuff being applied to this power's owner, before it lands. Return
    /// true to negate it (and consume a charge). ArtifactPower uses this. (Mirrors the game's
    /// TryModifyPowerAmountReceived / Artifact.)</summary>
    public virtual bool TryAbsorbDebuff(CombatState combat, PowerModel incoming) => false;

    /// <summary>Fires (on every power in combat) just BEFORE a card's effect resolves (after cost is paid).
    /// Danse Macabre / Spirit of Ash gain block here; Veilpiercer consumes a charge. Because it runs before
    /// OnPlay, a Power card does not trigger the very power it is applying. (Game: Hook.BeforeCardPlayed.)</summary>
    public virtual void BeforeCardPlayed(CombatState combat, CardModel card) { }

    /// <summary>Fires (on every power in combat) after the player resolves a card's effect, before it
    /// moves to its result pile. SlowPower increments its counter here. Mirrors the game's
    /// Hook.AfterCardPlayed, which runs after OnPlay — so a card never boosts its own damage.</summary>
    public virtual void AfterCardPlayed(CombatState combat, CardModel card) { }

    /// <summary>Transforms the Vulnerable damage multiplier when this power's OWNER is the target being
    /// hit (Debilitate strengthens it). Consulted by VulnerablePower over the target's own powers.</summary>
    public virtual decimal TransformVulnerableMultiplier(decimal mult) => mult;

    /// <summary>Transforms the Weak damage multiplier when this power's OWNER is the weakened dealer
    /// (Debilitate strengthens it). Consulted by WeakPower over the dealer's own powers.</summary>
    public virtual decimal TransformWeakMultiplier(decimal mult) => mult;

    /// <summary>Fires (on every power in combat) when a creature dies. Ravenous uses it to devour a
    /// dead ally — granting its owner Strength.</summary>
    public virtual void AfterCreatureDeath(CombatState combat, Creature dead) { }

    /// <summary>Last chance for a power to PREVENT its owner's death when it is brought to 0 HP: return true to
    /// veto the death and handle a revive in place (TestSubject's Adaptable heals it to its next form). When any
    /// power vetoes, the standard on-death cleanup / <see cref="AfterCreatureDeath"/> hooks are skipped — the
    /// monster survives. Called only for the dying monster's own powers.</summary>
    public virtual bool VetoLethalDamage(CombatState combat, Monster owner) => false;

    /// <summary>Fires (on every power in combat) after a creature is hit, with the unblocked amount
    /// (0 if fully blocked), the dealer (null for non-attack sources like poison) and the value props.
    /// Shriek uses it (on unblocked damage) to stun+Terror at an HP threshold; PersonalHive uses it
    /// (on any powered attack received) to add Dazed to the attacker's pile. (Game: AfterDamageReceived.)</summary>
    public virtual void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props) { }

    /// <summary>Fires (on every power in combat) after a dealer resolves a single attack. VigorPower
    /// uses it to consume itself once its bonus has been spent on a powered attack (mirrors the game's
    /// PowerModel.AfterAttack). Note: a multi-hit attack fires this per hit.</summary>
    public virtual void AfterAttackDealt(CombatState combat, Creature dealer, ValueProp props) { }

    /// <summary>Fires (on every power in combat) after a creature gains block, with the amount actually
    /// gained (post-modifiers). Juggernaut uses it to deal damage to an enemy whenever its owner blocks.
    /// (Game: PowerModel.AfterBlockGained.)</summary>
    public virtual void AfterBlockGained(CombatState combat, Creature creature, int amount, ValueProp props, CardModel? cardSource) { }

    /// <summary>Fires (on every power in combat) when a player card is moved to the exhaust pile, with
    /// whether the exhaust was an Ethereal card expiring at turn end. FeelNoPain gains block here;
    /// DarkEmbrace draws on a non-Ethereal exhaust. (Game: PowerModel.AfterCardExhausted.)</summary>
    public virtual void AfterCardExhausted(CombatState combat, CardModel card, bool causedByEthereal) { }

    /// <summary>Modifies a card's energy cost at play time (Free Attack zeroes the next Attack; Corruption
    /// zeroes Skills). Return the (possibly lower) cost. A power that lowers it gets AfterModifyingCardCost.
    /// (Game: PowerModel.TryModifyEnergyCostInCombatLate.)</summary>
    public virtual int ModifyCardCost(CardModel card, int cost) => cost;

    /// <summary>Fires after a card whose cost this power lowered is played — Free Attack consumes a charge
    /// here. (Game: the Free Attack consume side of BeforeCardPlayed.)</summary>
    public virtual void AfterModifyingCardCost(CombatState combat, CardModel card) { }

    /// <summary>If true for the played card, it is exhausted instead of discarded (Corruption exhausts
    /// Skills). (Game: PowerModel.ModifyCardPlayResultPileTypeAndPosition.)</summary>
    public virtual bool OverrideResultPileToExhaust(CardModel card) => false;

    /// <summary>Modifies a card's STAR cost at play time (VoidForm zeroes it for the first N cards). Return
    /// the (possibly lower) cost. (Game: PowerModel.TryModifyStarCost.)</summary>
    public virtual int ModifyStarCost(CardModel card, int cost) => cost;

    /// <summary>Fires (on every power in combat) after the player gains stars (BlackHole deals damage to all
    /// enemies). The amount is the stars gained. (Game: Hook.AfterStarsGained.)</summary>
    public virtual void AfterStarsGained(CombatState combat, int amount) { }

    /// <summary>Fires (on every power in combat) after the player spends stars on a card play (ChildOfTheStars
    /// gains block, BlackHole deals damage). The amount is the stars spent. (Game: Hook.AfterStarsSpent.)</summary>
    public virtual void AfterStarsSpent(CombatState combat, int amount) { }

    /// <summary>Fires (on every power in combat) after the player spends energy on a card play, with the
    /// amount spent. Orbit grants energy for every 4 cumulative energy spent. (Game: Hook.AfterEnergySpent.)</summary>
    public virtual void AfterEnergySpent(CombatState combat, int amount) { }

    /// <summary>Additional times the card's effect should resolve beyond the first (One-Two Punch adds 1
    /// for the owner's Attacks). (Game: PowerModel.ModifyCardPlayCount.)</summary>
    public virtual int ModifyCardPlayCount(CardModel card) => 0;

    /// <summary>Fires after a card's play count was increased by this power, for each such power. One-Two
    /// Punch consumes a charge here. (Game: PowerModel.AfterModifyingCardPlayCount.)</summary>
    public virtual void AfterModifyingCardPlayCount(CombatState combat, CardModel card) { }

    /// <summary>A per-turn cap on the number of cards the OWNER may play while this power is held — the engine
    /// mins it into <see cref="CombatState.EffectivePlayCap"/> alongside the card-side caps. CeremonialBeast's
    /// Ringing returns 1 (the boss's BEAST_CRY restricts the player to one card that turn). Default = no cap.
    /// SOUNDNESS: a play cap is HARM to the player, so honouring it CLOSES an optimistic gap. (Game: the player
    /// side of RingingPower.ShouldPlay, which blocks any further Ringing-afflicted card once one has been played.)</summary>
    public virtual int PlayCapThisTurn() => int.MaxValue;

    /// <summary>If any power on a creature returns true, that creature's block is NOT cleared at the start
    /// of its turn (Barricade). (Game: PowerModel.ShouldClearBlock, inverted.)</summary>
    public virtual bool PreventsBlockClear => false;

    /// <summary>Fires (on every power in combat) for each card the player draws MID-TURN (via
    /// <see cref="Cmd.Draw"/>), with whether it came from the turn-start hand draw (always false here — the
    /// exact solver models the hand draw as a chance node, not through this hook). CorrosiveWave applies
    /// Poison on every draw; Speedster deals damage on each non-hand draw. Real only with an ambient Rng
    /// (otherwise mid-turn draws are no-ops); inert in pure search. (Game: PowerModel.AfterCardDrawn.)</summary>
    public virtual void AfterCardDrawn(CombatState combat, CardModel card, bool fromHandDraw) { }

    /// <summary>Fires (on every power in combat) after an orb's evoke effect resolves, with the evoked orb.
    /// ThunderPower deals damage to all enemies whenever the player evokes a Lightning orb. Fired by
    /// <see cref="OrbOps"/> after every evoke (front/back/overflow). (Game: PowerModel.AfterOrbEvoked.)</summary>
    public virtual void AfterOrbEvoked(CombatState combat, OrbModel orb) { }

    /// <summary>Fires (on every power in combat) after the player generates a card into a pile mid-combat
    /// (status cards from BoostAway/FightThrough/GunkUp/Overclock/Turbo; random orbs). SmokestackPower deals
    /// damage and TrashToTreasurePower channels an orb when a Status card is generated. Routed through
    /// <see cref="Cmd.GenerateStatusCard"/>. (Game: PowerModel.AfterCardGeneratedForCombat.)</summary>
    public virtual void AfterCardGenerated(CombatState combat, CardModel card) { }

    /// <summary>Fires after a side's turn begins, for creatures on that side. Poison ticks here.</summary>
    public virtual void AfterSideTurnStart(CombatState combat, CombatSide side) { }

    /// <summary>Fires after a side's turn ends. Most debuffs tick down here (on the enemy turn end);
    /// Ritual grants Strength here.</summary>
    public virtual void AfterSideTurnEnd(CombatState combat, CombatSide side) { }

    /// <summary>Deep copy for state cloning. Concrete powers with extra fields override and call CopyBaseTo.</summary>
    public virtual PowerModel Clone()
    {
        var copy = (PowerModel)MemberwiseClone();
        // Owner is re-linked by the cloning Creature, so leave the reference; the cloner fixes it.
        return copy;
    }

    /// <summary>Contribution to the canonical state key. The pending-skip flag is folded in only when set,
    /// so it never perturbs the common (skip-free) case.</summary>
    public virtual string StateKey() => SkipNextTick ? $"{Id}={Amount}!" : $"{Id}={Amount}";

    /// <summary>Allocation-free contribution to the structural hash (id + amount + any extra state).</summary>
    public virtual long HashValue() => (((long)Id.GetHashCode() << 24) ^ (uint)Amount) ^ (SkipNextTick ? 0x5bd1e9955bd1e995L : 0L);
}
