using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Powers for the Event/Ancient special-pool cards (Special/SpecialCards.cs).
// Generic keyword powers (Strength, Weak, Dexterity, Vulnerable, …) live in
// Core/CommonPowers.cs; this file holds the ones unique to the special cards.
// ===========================================================================

// EnergyNextTurnPower is cross-character (Event cards + Regent), so it now lives in Core/CommonPowers.cs.
// IntangiblePower is cross-character too (Apparition here + the Silent's Wraith Form), so it also lives in
// Core/CommonPowers.cs. WraithFormPower (the Dexterity-loss aftermath) is Silent-only and lives in
// Silent/SilentPowers.cs alongside the Wraith Form card.

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

// TheSealedThrone (Ancient) is a Regent Stars card; its card + power live in Content/Regent/ (Regent models
// the Stars resource, so its TheSealedThronePower actually gains Stars rather than being an inert marker).
