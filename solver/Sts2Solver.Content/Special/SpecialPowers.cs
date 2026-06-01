using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Powers for the Event/Ancient special-pool cards (Special/SpecialCards.cs).
// Generic keyword powers (Strength, Weak, Dexterity, Vulnerable, …) live in
// Core/CommonPowers.cs; this file holds the ones unique to the special cards.
// ===========================================================================

/// <summary>At the start of the owner's next turn (after energy resets) gain <c>Amount</c> energy, then the
/// power is removed. Mirrors the game's EnergyNextTurnPower (AfterEnergyReset). Granted by Outmaneuver and
/// Relax. (MegaCrit EnergyNextTurnPower.)</summary>
public sealed class EnergyNextTurnPower : PowerModel
{
    public override string Id => "EnergyNextTurn";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;          // fires after BeginPlayerTurn's energy reset → adds on top
        Cmd.GainEnergy(combat, Amount);
        Owner.RemovePower(Id);
    }
}

/// <summary>Intangible: every instance of HP loss the owner would take is capped to 1 while owned; the
/// counter decrements by 1 at the end of each enemy turn, and at 0 the power is gone. Modelled via the
/// engine's ModifyHpLost cap (same mechanism as SlipperyPower), with the counter on the base Amount so the
/// base StateKey/HashValue already serialise it. Granted by Apparition and Wraith Form. (MegaCrit
/// IntangiblePower — the net effect of its ModifyDamageCap/ModifyHpLost-to-1.)</summary>
public sealed class IntangiblePower : PowerModel
{
    public override string Id => "Intangible";
    public override PowerType Type => PowerType.Buff;

    public override int ModifyHpLost(Creature target, int hpLost, ValueProp props, Creature? dealer)
        => target == Owner && Amount > 0 && hpLost >= 1 ? 1 : hpLost;

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == CombatSide.Enemy) { Amount--; this.NormalizeOrRemove(Owner); }   // game: decrement on enemy turn end
    }
}

/// <summary>Wraith Form aftermath: at the start of each of the owner's turns, lose <c>Amount</c> Dexterity
/// (a growing block penalty — the cost of staying Intangible). The counter persists (it does not tick
/// itself). (MegaCrit WraithFormPower.)</summary>
public sealed class WraithFormPower : PowerModel
{
    public override string Id => "WraithForm";
    public override PowerType Type => PowerType.Debuff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) Cmd.ApplyPower(combat, Owner, new DexterityPower(), -Amount, Owner);
    }
}

/// <summary>Feeding Frenzy: gain <c>Amount</c> Strength for the rest of this turn (then it is undone at the
/// owner's turn end). A one-turn self-buff — a TemporaryStrengthPower with positive sign, exactly as the game
/// defines FeedingFrenzyPower. (MegaCrit FeedingFrenzyPower : TemporaryStrengthPower.)</summary>
public sealed class FeedingFrenzyPower : TemporaryStrengthPower
{
    public override string Id => "FeedingFrenzy";
    protected override int Sign => 1;
}

/// <summary>Toric Toughness: at the start of each of the owner's next <c>Amount</c> turns (after block is
/// cleared), re-gain <c>_block</c> Block (unpowered — the stored amount already had its modifiers applied
/// when the card was played), then the counter ticks down. Mirrors the game's ToricToughnessPower
/// (AfterBlockCleared re-grant). The stored block is mutable per-combat state, carried through Clone /
/// StateKey. (MegaCrit ToricToughnessPower.)</summary>
public sealed class ToricToughnessPower : PowerModel
{
    public override string Id => "ToricToughness";
    public override PowerType Type => PowerType.Buff;

    private int _block;
    public void SetBlock(int block) => _block = block;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;   // fires after BeginPlayerTurn clears block → re-grant the stored block
        Cmd.GainBlock(combat, Owner, _block, ValueProp.Unpowered | ValueProp.Move, null);
        Amount--; this.NormalizeOrRemove(Owner);
    }

    public override PowerModel Clone()
    {
        var c = (ToricToughnessPower)base.Clone();
        c._block = _block;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}/{_block}";
    public override long HashValue() => base.HashValue() ^ ((long)_block * 0x100000001B3L);
}

/// <summary>Rebound: the next card you play this turn goes to the top of your draw pile instead of the
/// discard pile. Pure card-flow (HP-neutral) — modelled as an inert marker; the recorder/validator replays
/// the resulting draw order. (MegaCrit ReboundPower.)</summary>
public sealed class ReboundPower : PowerModel
{
    public override string Id => "Rebound";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Hello World: at the start of each turn add a random Common card to your hand. The generated card
/// is RNG/unrecorded and HP-neutral until played (any the autopilot plays is reconstructed by the validator),
/// so this is an inert marker — mirroring Mayhem / Distraction. (MegaCrit HelloWorldPower.)</summary>
public sealed class HelloWorldPower : PowerModel
{
    public override string Id => "HelloWorld";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Forbidden Grimoire: grants extra card-removal rewards after combat — a meta (between-combats)
/// effect with no in-combat consequence, so modelled as an inert marker. (MegaCrit ForbiddenGrimoirePower.)
/// </summary>
public sealed class ForbiddenGrimoirePower : PowerModel
{
    public override string Id => "ForbiddenGrimoire";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>The Sealed Throne: gain Stars whenever you play a card — Stars are a meta currency with no
/// combat-HP effect, so modelled as an inert marker. (MegaCrit TheSealedThronePower.)</summary>
public sealed class TheSealedThronePower : PowerModel
{
    public override string Id => "TheSealedThrone";
    public override PowerType Type => PowerType.Buff;
}
