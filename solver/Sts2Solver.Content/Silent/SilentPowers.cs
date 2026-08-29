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
/// <summary>Caltrops: whenever the owner is hit by a powered attack, deal <c>Amount</c> damage back to the
/// dealer (unpowered — no Strength/Vulnerable — but blockable). Permanent (does not decay), unlike Flame
/// Barrier. (MegaCrit CaltropsPower — Thorns.)</summary>
public sealed class CaltropsPower : PowerModel
{
    public override string Id => "Caltrops";
    public override PowerType Type => PowerType.Buff;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (target == Owner && dealer != null && dealer.IsAlive && props.IsPoweredAttack())
            Cmd.Attack(combat, Owner, dealer, Amount, ValueProp.Unpowered, null);
    }
}
/// <summary>Thorns: whenever the owner is hit by a powered attack, deal <c>Amount</c> damage back to the
/// dealer (unpowered — no Strength/Vulnerable — but blockable). Permanent. The game's generic ThornsPower
/// (granted by Abrasive); mechanically identical to <see cref="CaltropsPower"/> but a distinct stack id.
/// (MegaCrit ThornsPower.)</summary>
public sealed class ThornsPower : PowerModel
{
    public override string Id => "Thorns";
    public override PowerType Type => PowerType.Buff;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (target == Owner && dealer != null && dealer.IsAlive && props.IsPoweredAttack())
            Cmd.Attack(combat, Owner, dealer, Amount, ValueProp.Unpowered, null);
    }
}
/// <summary>Free Skill: the owner's next <c>Amount</c> Skills cost 0 energy. A counter consuming one charge
/// per Skill it actually discounts (mirrors <see cref="FreeAttackPower"/> for Skills). (MegaCrit
/// FreeSkillPower — Pounce.)</summary>
public sealed class FreeSkillPower : PowerModel
{
    public override string Id => "FreeSkill";
    public override PowerType Type => PowerType.Buff;

    public override int ModifyCardCost(CardModel card, int cost)
        => (card.Type == CardType.Skill && Amount > 0 && cost > 0) ? 0 : cost;

    public override void AfterModifyingCardCost(CombatState combat, CardModel card)
    {
        Amount--;
        this.NormalizeOrRemove(Owner);
    }
}
/// <summary>At the start of the owner's next turn (after the normal draw), draw <c>Amount</c> extra cards,
/// then the power is removed. Real only with an ambient Rng (otherwise a no-op; the validator replays the
/// recorded hand). HP-neutral. (MegaCrit Predator's "draw 2 additional cards next turn".)</summary>
public sealed class DrawNextTurnPower : PowerModel
{
    public override string Id => "DrawNextTurn";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        Cmd.Draw(combat, Amount);
        Owner.RemovePower(Id);
    }
}

// ===========================================================================
// Batch 5 powers — supporting the remaining Silent cards (Wave 2). Several are
// inert markers in single-player (multiplayer-only, or grant an HP-neutral
// keyword we don't model); these are documented as such.
// ===========================================================================

/// <summary>Accelerant: each living opponent's Poison ticks one extra time per turn per stack (the engine's
/// <see cref="PoisonPower"/> already reads this — <c>TriggerCount = min(Amount, 1 + Σ Accelerant)</c>), so
/// this power needs no behaviour of its own. (MegaCrit AccelerantPower.)</summary>
public sealed class AccelerantPower : PowerModel
{
    public override string Id => "Accelerant";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Accuracy: the owner's Shivs deal <c>Amount</c> additional damage (powered attacks only).
/// (MegaCrit AccuracyPower.)</summary>
public sealed class AccuracyPower : PowerModel
{
    public override string Id => "Accuracy";
    public override PowerType Type => PowerType.Buff;

    public override decimal ModifyDamageAdditive(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (dealer != Owner || !props.IsPoweredAttack()) return 0m;
        return cardSource is Shiv ? Amount : 0m;
    }
}

/// <summary>Temporary Dexterity: grants (<c>Sign·Amount</c>) Dexterity immediately and removes exactly that
/// much at the owner's own turn end, then removes itself. Same construction as <see cref="TemporaryStrengthPower"/>.
/// (MegaCrit TemporaryDexterityPower.)</summary>
public abstract class TemporaryDexterityPower : PowerModel
{
    protected abstract int Sign { get; }
    public override PowerType Type => Sign > 0 ? PowerType.Buff : PowerType.Debuff;
    public override bool AllowNegative => false;          // the marker amount is the positive magnitude

    private int _appliedDex;                               // actual Dexterity delta already pushed to the owner

    public override void AfterApplied(CombatState combat, Creature? applier)
    {
        int want = Sign * Amount;
        int delta = want - _appliedDex;
        if (delta != 0) Cmd.ApplyPower(combat, Owner, new DexterityPower(), delta, applier);
        _appliedDex = want;
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        if (_appliedDex != 0) Cmd.ApplyPower(combat, Owner, new DexterityPower(), -_appliedDex, Owner);
        Owner.RemovePower(Id);
    }

    public override PowerModel Clone()
    {
        var c = (TemporaryDexterityPower)base.Clone();
        c._appliedDex = _appliedDex;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}/{_appliedDex}";
    public override long HashValue() => base.HashValue() ^ ((long)_appliedDex * 0x100000001B3L);
}

/// <summary>Anticipate: temporary Dexterity until the end of your turn. (MegaCrit AnticipatePower.)</summary>
public sealed class AnticipatePower : TemporaryDexterityPower
{
    public override string Id => "Anticipate";
    protected override int Sign => 1;
}

/// <summary>Strangle (debuff on an enemy): whenever you play a card, deal <c>Amount</c> unblockable damage to
/// that enemy; removed at the end of the enemy's turn. The card that applied Strangle does not trigger it (the
/// game snapshots eligibility before OnPlay — mirrored with a one-shot skip set on apply). (MegaCrit
/// StranglePower.)</summary>
public sealed class StranglePower : PowerModel
{
    public override string Id => "Strangle";
    public override PowerType Type => PowerType.Debuff;

    private bool _skipApplyingCard;
    public override void AfterApplied(CombatState combat, Creature? applier) => _skipApplyingCard = true;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (_skipApplyingCard) { _skipApplyingCard = false; return; }   // the Strangle card itself
        if (Owner.IsAlive) Cmd.LoseHp(combat, Owner, Amount);           // unblockable + unpowered
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) Owner.RemovePower(Id);                  // removed at the enemy owner's turn end
    }

    public override PowerModel Clone()
    {
        var c = (StranglePower)base.Clone();
        c._skipApplyingCard = _skipApplyingCard;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}{(_skipApplyingCard ? "*" : "")}";
    public override long HashValue() => base.HashValue() ^ (_skipApplyingCard ? 0x9E3779B1L : 0L);
}

/// <summary>Serpent Form: whenever you play a card, deal <c>Amount</c> damage (unpowered, blockable) to a
/// random enemy. Permanent. The Serpent Form card itself does not trigger it (one-shot skip on apply).
/// (MegaCrit SerpentFormPower.)</summary>
public sealed class SerpentFormPower : PowerModel
{
    public override string Id => "SerpentForm";
    public override PowerType Type => PowerType.Buff;

    private bool _skipApplyingCard;
    public override void AfterApplied(CombatState combat, Creature? applier) => _skipApplyingCard = true;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (_skipApplyingCard) { _skipApplyingCard = false; return; }
        var living = combat.LivingMonsters.ToList();
        if (living.Count == 0) return;
        var t = combat.Rng != null ? living[combat.Rng.NextInt(living.Count)] : living[0];
        Cmd.Attack(combat, Owner, t, Amount, ValueProp.Unpowered, null);
    }

    public override PowerModel Clone()
    {
        var c = (SerpentFormPower)base.Clone();
        c._skipApplyingCard = _skipApplyingCard;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}{(_skipApplyingCard ? "*" : "")}";
    public override long HashValue() => base.HashValue() ^ (_skipApplyingCard ? 0x85EBCA6BL : 0L);
}

/// <summary>Outbreak: every third Poison the owner applies (cumulative across all enemies), deal
/// <c>Amount</c> damage to ALL enemies. (MegaCrit OutbreakPower.)</summary>
public sealed class OutbreakPower : PowerModel
{
    public override string Id => "Outbreak";
    public override PowerType Type => PowerType.Buff;

    private int _timesPoisoned;

    public override void AfterPowerApplied(CombatState combat, Creature target, PowerModel power, int amount, Creature? applier)
    {
        if (applier != Owner || amount <= 0 || power.Id != "Poison") return;
        if (++_timesPoisoned >= 3)
        {
            _timesPoisoned %= 3;
            foreach (var m in combat.LivingMonsters.ToList())
                Cmd.Attack(combat, Owner, m, Amount, ValueProp.Unpowered, null);
        }
    }

    public override PowerModel Clone()
    {
        var c = (OutbreakPower)base.Clone();
        c._timesPoisoned = _timesPoisoned;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}/{_timesPoisoned}";
    public override long HashValue() => base.HashValue() ^ ((long)_timesPoisoned * 0xC2B2AE35L);
}

/// <summary>Infinite Blades: at the start of each of your turns, add <c>Amount</c> Shiv(s) to your hand.
/// (MegaCrit InfiniteBladesPower.)</summary>
public sealed class InfiniteBladesPower : PowerModel
{
    public override string Id => "InfiniteBlades";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) SilentCardHelpers.AddShivsToHand(combat, Amount);
    }
}

/// <summary>Phantom Blades: the FIRST Shiv you play each turn deals <c>Amount</c> additional damage. (The
/// game also gives all Shivs Retain — HP-neutral, not modelled.) (MegaCrit PhantomBladesPower.)</summary>
public sealed class PhantomBladesPower : PowerModel
{
    public override string Id => "PhantomBlades";
    public override PowerType Type => PowerType.Buff;

    private bool _shivPlayedThisTurn;

    public override decimal ModifyDamageAdditive(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (dealer != Owner || !props.IsPoweredAttack() || cardSource is not Shiv) return 0m;
        return _shivPlayedThisTurn ? 0m : Amount;
    }

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (card is Shiv) _shivPlayedThisTurn = true;
    }

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) _shivPlayedThisTurn = false;
    }

    public override PowerModel Clone()
    {
        var c = (PhantomBladesPower)base.Clone();
        c._shivPlayedThisTurn = _shivPlayedThisTurn;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}{(_shivPlayedThisTurn ? "*" : "")}";
    public override long HashValue() => base.HashValue() ^ (_shivPlayedThisTurn ? 0x27D4EB2FL : 0L);
}

/// <summary>Tracking: the owner's card attacks against Weak enemies deal ×(1 + Amount/100) damage (powered
/// attacks only). v0.108.0: Amount is 50 → ×1.5 (was ×2). (MegaCrit TrackingPower.)</summary>
public sealed class TrackingPower : PowerModel
{
    public override string Id => "Tracking";
    public override PowerType Type => PowerType.Buff;

    public override decimal ModifyDamageMultiplicative(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (dealer != Owner || !props.IsPoweredAttack() || cardSource == null) return 1m;
        if (target == null || !target.HasPower("Weak")) return 1m;
        return 1m + Amount / 100m;
    }
}

/// <summary>Double Damage: the owner's card attacks deal ×2 damage; decrements one charge at the owner's
/// turn end. (MegaCrit DoubleDamagePower — granted by Shadow Step.)</summary>
public sealed class DoubleDamagePower : PowerModel
{
    public override string Id => "DoubleDamage";
    public override PowerType Type => PowerType.Buff;

    public override decimal ModifyDamageMultiplicative(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (dealer != Owner || !props.IsPoweredAttack() || cardSource == null) return 1m;
        return 2m;
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) { Amount--; this.NormalizeOrRemove(Owner); }
    }
}

/// <summary>Shadow Step: at the start of your next turn, gain Double Damage <c>Amount</c> and remove this
/// power. (MegaCrit ShadowStepPower.)</summary>
public sealed class ShadowStepPower : PowerModel
{
    public override string Id => "ShadowStep";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        Cmd.ApplyPower(combat, Owner, new DoubleDamagePower(), Amount, Owner);
        Owner.RemovePower(Id);
    }
}

/// <summary>Shadowmeld: while owned (this turn), block the owner gains is multiplied by 2^<c>Amount</c>;
/// removed at the owner's turn end. (MegaCrit ShadowmeldPower.)</summary>
public sealed class ShadowmeldPower : PowerModel
{
    public override string Id => "Shadowmeld";
    public override PowerType Type => PowerType.Buff;

    public override decimal ModifyBlockMultiplicative(Creature target, decimal block, ValueProp props, CardModel? cardSource)
        => target == Owner ? (decimal)Math.Pow(2.0, Amount) : 1m;

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) Owner.RemovePower(Id);
    }
}

/// <summary>Burst: the next <c>Amount</c> Skills you play are played twice; removed at your turn end.
/// (MegaCrit BurstPower.)</summary>
public sealed class BurstPower : PowerModel
{
    public override string Id => "Burst";
    public override PowerType Type => PowerType.Buff;

    public override int ModifyCardPlayCount(CardModel card)
        => (Amount > 0 && card.Type == CardType.Skill) ? 1 : 0;

    public override void AfterModifyingCardPlayCount(CombatState combat, CardModel card)
    {
        Amount--;
        this.NormalizeOrRemove(Owner);
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) Owner.RemovePower(Id);
    }
}

/// <summary>Fan of Knives: an inert marker power (the card's effect is the 4 Shivs it creates on play; the
/// power carries no combat behaviour of its own). (MegaCrit FanOfKnivesPower.)</summary>
public sealed class FanOfKnivesPower : PowerModel
{
    public override string Id => "FanOfKnives";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Master Planner: whenever you play a Skill, it gains Sly. Sly (auto-play on discard) is not
/// modelled — see SilentCards.cs — so this is an inert (HP-neutral) marker. (MegaCrit MasterPlannerPower.)</summary>
public sealed class MasterPlannerPower : PowerModel
{
    public override string Id => "MasterPlanner";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Well-Laid Plans (v0.109+): the owner's hand is not discarded at end of turn.
/// (MegaCrit WellLaidPlansPower.ShouldFlush → false for the owner.)</summary>
public sealed class WellLaidPlansPower : PowerModel
{
    public override string Id => "WellLaidPlans";
    public override PowerType Type => PowerType.Buff;
    public override bool PreventsHandFlush => true;
}

/// <summary>Sneaky: whenever an ALLY plays an Attack, gain <c>Amount</c> Block. Requires another player, so
/// it is inert in single-player. (MegaCrit SneakyPower — multiplayer.)</summary>
public sealed class SneakyPower : PowerModel
{
    public override string Id => "Sneaky";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Flanking (debuff on an enemy): attacks from players OTHER than the applier deal ×<c>Amount</c>
/// damage to it. The applier's own attacks are unaffected, so it is inert in single-player. (MegaCrit
/// FlankingPower — multiplayer.)</summary>
public sealed class FlankingPower : PowerModel
{
    public override string Id => "Flanking";
    public override PowerType Type => PowerType.Debuff;
}

// ===========================================================================
// Batch 6 powers — Intangible + Wraith Form (Wave 3).
// ===========================================================================

// IntangiblePower is shared (the Event-pool Apparition + the Silent's Wraith Form), so it lives in
// Core/CommonPowers.cs (the canonical version keeps the Amount>0 guard so an expired-but-not-yet-removed
// charge stops capping). WraithFormPower below is Silent-only and stays here.

/// <summary>Wraith Form: at the start of each of your turns, lose <c>Amount</c> Dexterity (the downside of
/// Wraith Form's Intangible). Permanent. (MegaCrit WraithFormPower.)</summary>
public sealed class WraithFormPower : PowerModel
{
    public override string Id => "WraithForm";
    public override PowerType Type => PowerType.Debuff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side)
            Cmd.ApplyPower(combat, Owner, new DexterityPower(), -Amount, Owner);
    }
}

/// <summary>Bullet Time: every card the owner plays this turn costs 0; removed at the owner's turn end. The
/// game flags the specific cards in hand at play time; with the accompanying NoDraw (no new cards) this is
/// equivalent to zeroing every card the player plays for the rest of the turn. (MegaCrit Bullet Time —
/// modelled via a cost hook rather than a per-card "free this turn" flag.)</summary>
public sealed class BulletTimePower : PowerModel
{
    public override string Id => "BulletTime";
    public override PowerType Type => PowerType.Buff;

    public override int ModifyCardCost(CardModel card, int cost) => 0;

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) Owner.RemovePower(Id);
    }
}

// ===========================================================================
// Batch 8 powers — the formerly-deferred set, now ported to the project's
// "real with a driver / degrade in pure search" bar (matching the existing
// draw cards): Nightmare (deferred card copies), Tools of the Trade (turn-start
// draw filter), and the mid-turn-draw triggers Corrosive Wave / Speedster.
// ===========================================================================

/// <summary>Nightmare: at the start of your next turn, add <c>Amount</c> copies of the chosen card to your
/// hand, then remove this power. The chosen card is captured at play time (a default — player choice not
/// modelled). (MegaCrit NightmarePower.)</summary>
public sealed class NightmarePower : PowerModel
{
    public override string Id => "Nightmare";
    public override PowerType Type => PowerType.Buff;

    /// <summary>The card to triplicate next turn (a private clone captured when Nightmare was played).</summary>
    public CardModel? Selected;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side || Selected == null) return;
        var p = combat.Player;
        for (int i = 0; i < Amount; i++)
            (p.Hand.Count < Player.MaxHandSize ? p.Hand : p.DiscardPile).Add(Selected.Clone());
        Owner.RemovePower(Id);
    }

    public override PowerModel Clone()
    {
        var c = (NightmarePower)base.Clone();
        c.Selected = Selected?.Clone();
        return c;
    }

    public override string StateKey() => $"Nightmare={Amount}:{Selected?.StateKey() ?? "-"}";
    public override long HashValue() => base.HashValue() ^ ((long)(Selected?.StateKey().GetHashCode() ?? 7) * 0x9E3779B1L);
}

/// <summary>Tools of the Trade: at the start of each of your turns, draw <c>Amount</c> extra card(s) and
/// discard <c>Amount</c> (a default — player choice not modelled). HP-neutral card filtering; the draw is
/// real only with an ambient Rng (otherwise a no-op). (MegaCrit ToolsOfTheTradePower.)</summary>
public sealed class ToolsOfTheTradePower : PowerModel
{
    public override string Id => "ToolsOfTheTrade";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        int drew = Cmd.Draw(combat, Amount);                 // no-op without an Rng
        if (drew > 0) SilentCardHelpers.DiscardDefault(combat, Amount);
    }
}

/// <summary>Corrosive Wave: whenever you draw a card (this turn), apply <c>Amount</c> Poison to ALL enemies;
/// removed at the end of your turn. Real only on mid-turn draws with an ambient Rng (inert in pure search).
/// (MegaCrit CorrosiveWavePower.)</summary>
public sealed class CorrosiveWavePower : PowerModel
{
    public override string Id => "CorrosiveWave";
    public override PowerType Type => PowerType.Buff;

    public override void AfterCardDrawn(CombatState combat, CardModel card, bool fromHandDraw)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.ApplyPower(combat, m, new PoisonPower(), Amount, Owner);
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) Owner.RemovePower(Id);
    }
}

/// <summary>Speedster: whenever you draw a card mid-turn (not the turn-start hand draw), deal <c>Amount</c>
/// damage to ALL enemies. Permanent. Real only with an ambient Rng (inert in pure search). (MegaCrit
/// SpeedsterPower.)</summary>
public sealed class SpeedsterPower : PowerModel
{
    public override string Id => "Speedster";
    public override PowerType Type => PowerType.Buff;

    public override void AfterCardDrawn(CombatState combat, CardModel card, bool fromHandDraw)
    {
        if (fromHandDraw) return;
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, Owner, m, Amount, ValueProp.Unpowered, null);
    }
}
