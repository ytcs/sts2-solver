using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Regent-only powers. Generic keyword powers shared with other characters
// (Strength/Dexterity/Weak/Vulnerable/Vigor/Plating/BlockNextTurn/DrawNextTurn,
// the TemporaryStrengthPower base) are reused from Core/Ironclad/Silent/Monsters.
//
// Many Regent powers scale off subsystems this solver does not model (RNG card
// generation, hand-draw-count modification, gold, multiplayer). Per the project
// convention those are ported as documented-inert markers: the power is applied
// (so it shows in state) but its generation/draw/gold effect is a no-op. The
// HP-relevant powers (Stars payback, Forge, temp-Strength, Sovereign-Blade
// modifiers, resource-next-turn) are faithful.
// ===========================================================================

// ---- Sovereign Blade modifiers (read by SovereignBlade.OnPlay; see RegentCards.cs) ----

/// <summary>Counter buff. Each time the owner plays a Sovereign Blade it gains <c>Amount</c> Block (the
/// game computes this as a CalculatedBlock with the Parry amount as multiplier). Parry itself has no
/// turn hooks — Sovereign Blade reads <see cref="Creature.GetPowerAmount"/>("Parry") at play time.
/// (MegaCrit ParryPower.)</summary>
public sealed class ParryPower : PowerModel
{
    public override string Id => "Parry";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Single-stack marker buff: while owned, the owner's Sovereign Blade hits ALL enemies instead of
/// one. Read by SovereignBlade.OnPlay. (MegaCrit SeekingEdgePower.)</summary>
public sealed class SeekingEdgePower : PowerModel
{
    public override string Id => "SeekingEdge";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Counter buff: while owned, the owner's Sovereign Blade replays <c>Amount</c> extra times. Read
/// by SovereignBlade.OnPlay (the game increments SovereignBlade.BaseReplayCount; reading the live power
/// amount is equivalent for a single blade). (MegaCrit SwordSagePower.)</summary>
public sealed class SwordSagePower : PowerModel
{
    public override string Id => "SwordSage";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Debuff applied to an enemy: the owner's Sovereign Blade powered attacks against this enemy deal
/// ×2 damage. Decrements one stack at the enemy's (owner's) turn end, so a 1-stack Conqueror covers the
/// blades played until the enemy's next turn ends. (MegaCrit ConquerorPower.)</summary>
public sealed class ConquerorPower : PowerModel
{
    public override string Id => "Conqueror";
    public override PowerType Type => PowerType.Debuff;

    public override decimal ModifyDamageMultiplicative(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (cardSource is not SovereignBlade) return 1m;
        if (!props.IsPoweredAttack()) return 1m;
        if (target != Owner) return 1m;
        return 2m;
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) { Amount--; this.NormalizeOrRemove(Owner); }
    }
}

// ---- Stars-payback powers ----

/// <summary>Counter buff: whenever the owner SPENDS stars (to play a star-cost card), gain
/// <c>Amount × starsSpent</c> Block (unpowered — no Dexterity). (MegaCrit ChildOfTheStarsPower.)</summary>
public sealed class ChildOfTheStarsPower : PowerModel
{
    public override string Id => "ChildOfTheStars";
    public override PowerType Type => PowerType.Buff;

    public override void AfterStarsSpent(CombatState combat, int amount)
    {
        if (amount > 0) Cmd.GainBlock(combat, Owner, Amount * amount, ValueProp.Unpowered, null);
    }
}

/// <summary>Counter buff: whenever the owner GAINS or SPENDS stars, deal <c>Amount</c> damage to all
/// enemies (unpowered, blockable). (MegaCrit BlackHolePower — triggers on both star events.)</summary>
public sealed class BlackHolePower : PowerModel
{
    public override string Id => "BlackHole";
    public override PowerType Type => PowerType.Buff;

    private void Blast(CombatState combat)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, Owner, m, Amount, ValueProp.Unpowered, null);
    }

    public override void AfterStarsGained(CombatState combat, int amount) { if (amount > 0) Blast(combat); }
    public override void AfterStarsSpent(CombatState combat, int amount) { if (amount > 0) Blast(combat); }
}

// ---- Resource-per-turn / next-turn powers ----

/// <summary>Counter buff: at the start of the owner's turn, gain <c>Amount</c> stars. Applied mid-turn, so
/// the first grant lands at the NEXT turn start (like Demon Form). (MegaCrit GenesisPower — the game fires
/// on AfterEnergyReset, which for the player coincides with turn start.)</summary>
public sealed class GenesisPower : PowerModel
{
    public override string Id => "Genesis";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) Cmd.GainStars(combat, Amount);
    }
}

/// <summary>Counter buff: at the start of the owner's turn, Forge <c>Amount</c> (build the Sovereign Blade).
/// Applied mid-turn, so the first forge lands at the NEXT turn start. (MegaCrit FurnacePower.)</summary>
public sealed class FurnacePower : PowerModel
{
    public override string Id => "Furnace";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) RegentForge.Forge(combat, Amount);
    }
}

/// <summary>Counter buff: gain <c>Amount</c> energy for every 4 cumulative energy the owner spends on cards.
/// Tracks fractional progress across plays. (MegaCrit OrbitPower.)</summary>
public sealed class OrbitPower : PowerModel
{
    public override string Id => "Orbit";
    public override PowerType Type => PowerType.Buff;

    private int _progress;   // energy spent toward the next 4-energy trigger

    public override void AfterEnergySpent(CombatState combat, int amount)
    {
        _progress += amount;
        while (_progress >= 4) { _progress -= 4; Cmd.GainEnergy(combat, Amount); }
    }

    public override PowerModel Clone()
    {
        var c = (OrbitPower)base.Clone();
        c._progress = _progress;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}+{_progress}";
    public override long HashValue() => base.HashValue() ^ ((long)_progress * 0x100000001B3L);
}

// EnergyNextTurnPower (used by Hegemony/RefineBlade/Convergence and the Event cards Outmaneuver/Relax) is
// cross-character, so it lives in Core/CommonPowers.cs.

/// <summary>Counter buff: at the start of the owner's next turn, gain <c>Amount</c> stars, then removed.
/// (MegaCrit StarNextTurnPower — HiddenCache, Convergence.)</summary>
public sealed class StarNextTurnPower : PowerModel
{
    public override string Id => "StarNextTurn";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        Cmd.GainStars(combat, Amount);
        Owner.RemovePower(Id);
    }
}

/// <summary>Counter buff: at the start of the owner's next turn, draw <c>Amount</c> extra cards, then
/// removed. Real only with an ambient Rng (otherwise a no-op; the solver enumerates draws at a fixed count
/// per turn, so the extra draw is HP-neutral here). (MegaCrit DrawCardsNextTurnPower — Glow.)</summary>
public sealed class DrawCardsNextTurnPower : PowerModel
{
    public override string Id => "DrawCardsNextTurn";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        Cmd.Draw(combat, Amount);
        Owner.RemovePower(Id);
    }
}

// ---- Strength / temporary-Strength ----

/// <summary>Buff: whenever the owner deals a powered attack, permanently reduce the struck enemy's Strength
/// by <c>Amount</c> (the game applies a stacking MonarchsGazeStrengthDown debuff; applying negative
/// Strength is equivalent for damage). Multi-hit attacks apply it per hit (mirrors the per-result game
/// hook). (MegaCrit MonarchsGazePower.)</summary>
public sealed class MonarchsGazePower : PowerModel
{
    public override string Id => "MonarchsGaze";
    public override PowerType Type => PowerType.Buff;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (dealer != Owner || !props.IsPoweredAttack()) return;
        if (target.IsEnemy && target.IsAlive)
            Cmd.ApplyPower(combat, target, new StrengthPower(), -Amount, Owner);
    }
}

/// <summary>Crush Under: enemies lose <c>Amount</c> Strength until the end of their turn (weakening their
/// upcoming attack, then restored). (MegaCrit CrushUnderPower — a negative TemporaryStrengthPower.)</summary>
public sealed class CrushUnderPower : TemporaryStrengthPower
{
    public override string Id => "CrushUnder";
    protected override int Sign => -1;
}

/// <summary>Dying Star: enemies lose <c>Amount</c> Strength until the end of their turn. (MegaCrit
/// DyingStarPower — a negative TemporaryStrengthPower.)</summary>
public sealed class DyingStarPower : TemporaryStrengthPower
{
    public override string Id => "DyingStar";
    protected override int Sign => -1;
}

/// <summary>Monologue: whenever the owner plays a card this turn, gain <c>Amount</c> Strength; at the
/// owner's turn end, lose all Strength gained this way (and the power is removed). The Monologue card that
/// applies it does not trigger it (eligibility is recorded before OnPlay, when the power isn't attached);
/// we mirror that with a one-shot skip set on apply. (MegaCrit MonologuePower.)</summary>
public sealed class MonologuePower : PowerModel
{
    public override string Id => "Monologue";
    public override PowerType Type => PowerType.Buff;

    private bool _skipApplyingCard;
    private int _accumulated;

    public override void AfterApplied(CombatState combat, Creature? applier) => _skipApplyingCard = true;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (_skipApplyingCard) { _skipApplyingCard = false; return; }   // the Monologue card itself
        Cmd.ApplyPower(combat, Owner, new StrengthPower(), Amount, Owner);
        _accumulated += Amount;
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        if (_accumulated != 0) Cmd.ApplyPower(combat, Owner, new StrengthPower(), -_accumulated, Owner);
        Owner.RemovePower(Id);
    }

    public override PowerModel Clone()
    {
        var c = (MonologuePower)base.Clone();
        c._skipApplyingCard = _skipApplyingCard;
        c._accumulated = _accumulated;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}/{_accumulated}{(_skipApplyingCard ? "*" : "")}";
    public override long HashValue() => base.HashValue() ^ ((long)_accumulated * 0x100000001B3L) ^ (_skipApplyingCard ? 0x5bd1e995L : 0L);
}

/// <summary>The Sealed Throne (Ancient): whenever the owner plays a card, gain <c>Amount</c> stars. (MegaCrit
/// TheSealedThronePower — the game fires before the play; firing after is equivalent for HP purposes.)</summary>
public sealed class TheSealedThronePower : PowerModel
{
    public override string Id => "TheSealedThrone";
    public override PowerType Type => PowerType.Buff;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        Cmd.GainStars(combat, Amount);
    }
}

// ---- Documented-inert markers (effects depend on unported subsystems) ----

/// <summary>Arsenal: gain <c>Amount</c> Strength each time a card is generated into combat. Card generation
/// (colorless / token gen) is not modelled in this solver, so this is an inert marker. (MegaCrit
/// ArsenalPower.)</summary>
public sealed class ArsenalPower : PowerModel
{
    public override string Id => "Arsenal";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Pillar of Creation: gain <c>Amount</c> Block each time a card is generated into combat. Card
/// generation is not modelled, so this is an inert marker. (MegaCrit PillarOfCreationPower.)</summary>
public sealed class PillarOfCreationPower : PowerModel
{
    public override string Id => "PillarOfCreation";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Spectrum Shift: at the start of each turn, add <c>Amount</c> generated colorless cards to hand.
/// Card generation is not modelled (HP-neutral), so this is an inert marker. (MegaCrit SpectrumShiftPower.)</summary>
public sealed class SpectrumShiftPower : PowerModel
{
    public override string Id => "SpectrumShift";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Pale Blue Dot: draw <c>Amount</c> extra cards next turn if you played ≥5 cards this turn. The
/// solver draws a fixed count per turn, so the extra draw is not modelled (HP-neutral). Inert marker.
/// (MegaCrit PaleBlueDotPower.)</summary>
public sealed class PaleBlueDotPower : PowerModel
{
    public override string Id => "PaleBlueDot";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Tyranny: each turn draw <c>Amount</c> more cards and exhaust <c>Amount</c> cards. Turn-start
/// draw-count change and forced exhaust are not modelled (the solver's draw count is fixed). Inert marker.
/// (MegaCrit TyrannyPower.)</summary>
public sealed class TyrannyPower : PowerModel
{
    public override string Id => "Tyranny";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Royalties: gain <c>Amount</c> gold at combat end. Gold is out of scope (HP-neutral). Inert
/// marker. (MegaCrit RoyaltiesPower.)</summary>
public sealed class RoyaltiesPower : PowerModel
{
    public override string Id => "Royalties";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Reflect: when the owner blocks a powered attack, deal the blocked amount back to the dealer.
/// The engine's damage-received hook exposes only the UNBLOCKED amount, not the blocked portion, so this
/// thorns-on-block effect is not modelled (offensive only, HP-neutral for the player). Inert marker.
/// (MegaCrit ReflectPower.)</summary>
public sealed class ReflectPower : PowerModel
{
    public override string Id => "Reflect";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Foregone Conclusion: at next hand draw, choose <c>Amount</c> cards from the draw pile to draw.
/// Card selection over the draw pile is HP-neutral (the solver enumerates draws). Inert marker. (MegaCrit
/// ForegoneConclusionPower.)</summary>
public sealed class ForegoneConclusionPower : PowerModel
{
    public override string Id => "ForegoneConclusion";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Convergence's Retain: the owner's hand is retained (not discarded) next turn. Hand retention
/// across the turn boundary is not modelled (the solver redraws each turn). Inert marker. (MegaCrit
/// RetainHandPower.)</summary>
public sealed class RetainHandPower : PowerModel
{
    public override string Id => "RetainHand";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Void Form: the next <c>Amount</c> cards each turn cost 0 energy and 0 stars. Integrating a cost
/// discount with the solver's fast playability gate is deferred, so the discount is not applied (the power
/// is an inert marker — conservative: it never grants free plays the gate doesn't know about). (MegaCrit
/// VoidFormPower.)</summary>
public sealed class VoidFormPower : PowerModel
{
    public override string Id => "VoidForm";
    public override PowerType Type => PowerType.Buff;
}
