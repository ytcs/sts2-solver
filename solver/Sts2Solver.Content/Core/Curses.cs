using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Curse cards (CardRarity.Curse / CardType.Curse). Acquired from events, fights
// and relics — never deliberately deck-built — so they register in Catalog's
// CommonCardFactories (buildable by name for a real run deck) but stay OUT of
// the deck-buildable CardPool, exactly like the shared status cards and
// AscendersBane (which lives in Core/StatusCards.cs).
//
// Combat fidelity:
//   * Most curses are pure draw-dilution (Unplayable, no effect) — modelled
//     like Dazed: an Unplayable card that just clogs the hand/draw.
//   * A few act when held at end of turn (BadLuck/Decay/Regret self-damage,
//     Doubt/Shame self-debuff) — modelled via OnTurnEndInHand, like Infection.
//   * Two impose a play-restriction the search's move generator does not model
//     (Enthralled's hand lockout, Normality's 3-cards-per-turn cap). Their
//     restriction is documented-deferred; they degrade to inert dilution, which
//     UNDER-states their harm (flagged inline) — porting the restriction needs
//     a move-legality hook in the search, out of scope here.
// Deck-level keywords that don't change combat HP (Eternal, Innate, Retain) are
// not modelled, matching the existing convention (Innate only seeds the opening
// hand; Retain only changes card flow).
// ===========================================================================

// --------------------------------------------------------------------------
// Curses that act when held in hand at end of turn
// --------------------------------------------------------------------------

/// <summary>Curse: Unplayable + Eternal. While in hand at end of your turn, lose 13 HP (unblockable).
/// Upgrade: none. (MegaCrit BadLuck)</summary>
public sealed class BadLuck : CardModel
{
    public override string Name => "BadLuck";
    public override int BaseCost => -1;
    public override CardType Type => CardType.Curse;
    public override CardRarity Rarity => CardRarity.Curse;
    public override TargetType Target => TargetType.None;
    public override bool Unplayable => true;
    public override bool HasTurnEndInHandEffect => true;
    public int HpLoss => 13;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // never played
    public override void OnTurnEndInHand(CombatState combat)
        => Cmd.Attack(combat, combat.Player, combat.Player, HpLoss, ValueProp.Unblockable | ValueProp.Unpowered | ValueProp.Move, this);
}

/// <summary>Curse: Unplayable. While in hand at end of your turn, take 2 damage (blockable, unpowered).
/// (MegaCrit Decay)</summary>
public sealed class Decay : CardModel
{
    public override string Name => "Decay";
    public override int BaseCost => -1;
    public override CardType Type => CardType.Curse;
    public override CardRarity Rarity => CardRarity.Curse;
    public override TargetType Target => TargetType.None;
    public override bool Unplayable => true;
    public override bool HasTurnEndInHandEffect => true;
    public int Damage => 2;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // never played
    public override void OnTurnEndInHand(CombatState combat)
        => Cmd.Attack(combat, combat.Player, combat.Player, Damage, ValueProp.Unpowered | ValueProp.Move, this);
}

/// <summary>Curse: Unplayable. While in hand at end of your turn, lose HP equal to the number of cards in
/// your hand (unblockable). (MegaCrit Regret)</summary>
public sealed class Regret : CardModel
{
    public override string Name => "Regret";
    public override int BaseCost => -1;
    public override CardType Type => CardType.Curse;
    public override CardRarity Rarity => CardRarity.Curse;
    public override TargetType Target => TargetType.None;
    public override bool Unplayable => true;
    public override bool HasTurnEndInHandEffect => true;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // never played
    public override void OnTurnEndInHand(CombatState combat)
        // Hand is still intact when end-of-turn-in-hand effects fire (the engine snapshots it first), so the
        // live count is the game's "number of cards in hand" (this curse included).
        => Cmd.Attack(combat, combat.Player, combat.Player, combat.Player.Hand.Count,
            ValueProp.Unblockable | ValueProp.Unpowered | ValueProp.Move, this);
}

/// <summary>Curse: Unplayable. While in hand at end of your turn, gain 1 Weak. (MegaCrit Doubt) — the Weak
/// skips the duration tick on the turn it lands (SkipNextDurationTick), so it weakens your NEXT turn.</summary>
public sealed class Doubt : CardModel
{
    public override string Name => "Doubt";
    public override int BaseCost => -1;
    public override CardType Type => CardType.Curse;
    public override CardRarity Rarity => CardRarity.Curse;
    public override TargetType Target => TargetType.None;
    public override bool Unplayable => true;
    public override bool HasTurnEndInHandEffect => true;
    public int Weak => 1;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // never played
    public override void OnTurnEndInHand(CombatState combat)
    {
        bool had = combat.Player.HasPower("Weak");
        Cmd.ApplyPower(combat, combat.Player, new WeakPower(), Weak, combat.Player);
        if (!had && combat.Player.GetPower("Weak") is { } w) w.SkipNextTick = true;
    }
}

/// <summary>Curse: Unplayable. While in hand at end of your turn, gain 1 Frail. (MegaCrit Shame) — the Frail
/// skips the duration tick on the turn it lands, so it weakens your NEXT turn's block.</summary>
public sealed class Shame : CardModel
{
    public override string Name => "Shame";
    public override int BaseCost => -1;
    public override CardType Type => CardType.Curse;
    public override CardRarity Rarity => CardRarity.Curse;
    public override TargetType Target => TargetType.None;
    public override bool Unplayable => true;
    public override bool HasTurnEndInHandEffect => true;
    public int Frail => 1;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // never played
    public override void OnTurnEndInHand(CombatState combat)
    {
        bool had = combat.Player.HasPower("Frail");
        Cmd.ApplyPower(combat, combat.Player, new FrailPower(), Frail, combat.Player);
        if (!had && combat.Player.GetPower("Frail") is { } f) f.SkipNextTick = true;
    }
}

// --------------------------------------------------------------------------
// Pure draw-dilution curses (Unplayable, no combat effect)
// --------------------------------------------------------------------------

/// <summary>Base for an Unplayable, no-combat-effect curse: pure draw dilution (clogs the hand/draw like
/// Dazed). <see cref="Ethereal"/> curses additionally exhaust if still in hand at end of turn.</summary>
public abstract class DilutionCurse : CardModel
{
    public override int BaseCost => -1;
    public override CardType Type => CardType.Curse;
    public override CardRarity Rarity => CardRarity.Curse;
    public override TargetType Target => TargetType.None;
    public override bool Unplayable => true;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // never played
}

/// <summary>Curse: Unplayable + Ethereal (exhausts at end of turn if still held). No combat effect — pure
/// dilution. (MegaCrit Clumsy)</summary>
public sealed class Clumsy : DilutionCurse
{
    public override string Name => "Clumsy";
    public override bool Ethereal => true;
}

/// <summary>Curse: Unplayable + Eternal. No combat effect — pure dilution. (MegaCrit CurseOfTheBell)</summary>
public sealed class CurseOfTheBell : DilutionCurse
{
    public override string Name => "CurseOfTheBell";
}

/// <summary>Curse: Unplayable + Ethereal + Innate + Eternal. No combat effect — pure dilution (Ethereal so
/// it exhausts at end of turn; Innate only seeds the opening hand, not modelled). (MegaCrit Folly)</summary>
public sealed class Folly : DilutionCurse
{
    public override string Name => "Folly";
    public override bool Ethereal => true;
}

/// <summary>Curse: Unplayable + Eternal. No combat effect — pure dilution. (MegaCrit Greed)</summary>
public sealed class Greed : DilutionCurse
{
    public override string Name => "Greed";
}

/// <summary>Curse: Unplayable. No combat effect — pure dilution. (MegaCrit Injury)</summary>
public sealed class Injury : DilutionCurse
{
    public override string Name => "Injury";
}

/// <summary>Curse: Unplayable + Retain. No combat effect — pure dilution (Retain only changes card flow,
/// not modelled). (MegaCrit PoorSleep)</summary>
public sealed class PoorSleep : DilutionCurse
{
    public override string Name => "PoorSleep";
}

/// <summary>Curse: Unplayable + Innate. No combat effect — pure dilution (Innate only seeds the opening
/// hand, not modelled). (MegaCrit Writhe)</summary>
public sealed class Writhe : DilutionCurse
{
    public override string Name => "Writhe";
}

/// <summary>Curse: Unplayable. Removes itself from the deck after 5 combats — a meta (between-combats)
/// effect with no in-combat consequence, so modelled as pure dilution. (MegaCrit Guilty)</summary>
public sealed class Guilty : DilutionCurse
{
    public override string Name => "Guilty";
}

/// <summary>Curse: Unplayable. While in hand at end of your turn, lose 10 Gold — a meta resource with no
/// combat-HP effect, so modelled as pure dilution. (MegaCrit Debt)</summary>
public sealed class Debt : DilutionCurse
{
    public override string Name => "Debt";
}

/// <summary>Curse: Unplayable. You cannot play more than 3 cards per turn. The per-turn play cap is a
/// move-legality restriction the search does not model, so this degrades to inert dilution (its harm is
/// UNDER-stated). (MegaCrit Normality)</summary>
public sealed class Normality : DilutionCurse
{
    public override string Name => "Normality";
}

// --------------------------------------------------------------------------
// Playable curses (no combat effect when played)
// --------------------------------------------------------------------------

/// <summary>Curse: cost 1, Exhaust. Playing it does nothing but remove it (exhaust) for the rest of combat.
/// No combat effect. (MegaCrit SporeMind)</summary>
public sealed class SporeMind : CardModel
{
    public override string Name => "SporeMind";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Curse;
    public override CardRarity Rarity => CardRarity.Curse;
    public override TargetType Target => TargetType.None;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // exhausts itself; no effect
}

/// <summary>Curse: cost 2, Eternal. While in your hand you cannot play your other cards (only Enthralled,
/// which discards it and lifts the lockout). The hand-lockout is a move-legality restriction the search does
/// not model, so this degrades to a playable cost-2 no-op (its harm is UNDER-stated). (MegaCrit Enthralled)</summary>
public sealed class Enthralled : CardModel
{
    public override string Name => "Enthralled";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Curse;
    public override CardRarity Rarity => CardRarity.Curse;
    public override TargetType Target => TargetType.None;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // lockout unmodelled; playing just discards it
}
