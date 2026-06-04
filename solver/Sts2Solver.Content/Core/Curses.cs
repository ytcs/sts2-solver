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
//   * Two impose a play-restriction (Enthralled's hand lockout, Normality's
//     3-cards-per-turn cap). Both are now MODELLED 1:1 by every search move
//     generator (exact Solver, MCTS, rollout/heuristic) via
//     CombatState.EffectivePlayCap() (Normality → PlayCapWhileInHand 3) and
//     CardPlayAllowed() (Enthralled → LocksHandWhileInHand), so their HARM is
//     exact — the search can no longer over-credit by ignoring the restriction.
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
        // Cmd.ApplyPower sets SkipNextTick for any debuff applied to the player, so the Weak survives this
        // turn-end (it ticks at the enemy turn end) and weakens the next turn — no manual skip needed.
        => Cmd.ApplyPower(combat, combat.Player, new WeakPower(), Weak, combat.Player);
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
        // Cmd.ApplyPower sets SkipNextTick for any debuff applied to the player, so the Frail survives this
        // turn-end (it ticks at the enemy turn end) and reduces the next turn's block — no manual skip needed.
        => Cmd.ApplyPower(combat, combat.Player, new FrailPower(), Frail, combat.Player);
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
/// it exhausts at end of turn; Innate guarantees it clogs the opening hand). (MegaCrit Folly)</summary>
public sealed class Folly : DilutionCurse
{
    public override string Name => "Folly";
    public override bool Ethereal => true;
    public override bool Innate => true;   // canonical Innate: guaranteed in the opening hand
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

/// <summary>Curse: Unplayable + Retain. No combat effect — pure dilution; Retain keeps it clogging the hand
/// rather than cycling through discard. (MegaCrit PoorSleep)</summary>
public sealed class PoorSleep : DilutionCurse
{
    public override string Name => "PoorSleep";
    public override bool Retain => true;   // canonical Retain: stays in hand rather than being discarded+redrawn
}

/// <summary>Curse: Unplayable + Innate. No combat effect — pure dilution; Innate guarantees it clogs the
/// opening hand. (MegaCrit Writhe)</summary>
public sealed class Writhe : DilutionCurse
{
    public override string Name => "Writhe";
    public override bool Innate => true;   // canonical Innate: guaranteed in the opening hand
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

/// <summary>Curse: Unplayable. While held in hand you cannot play more than 3 cards per turn. Modelled as a
/// real per-turn play cap (<see cref="CardModel.PlayCapWhileInHand"/>) enforced by every search move generator
/// via <see cref="CombatState.EffectivePlayCap"/> — its harm is now exact, not under-stated. (MegaCrit
/// Normality)</summary>
public sealed class Normality : DilutionCurse
{
    public override string Name => "Normality";
    public override int PlayCapWhileInHand => 3;
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

/// <summary>Curse: cost 2, Eternal. While in your hand you cannot play your other cards — only Enthralled
/// itself, which discards it and lifts the lockout for the turn. Modelled as a real hand-lockout
/// (<see cref="CardModel.LocksHandWhileInHand"/>) enforced by every search move generator via
/// <see cref="CombatState.CardPlayAllowed"/> — its harm is now exact, not under-stated. (MegaCrit Enthralled)</summary>
public sealed class Enthralled : CardModel
{
    public override string Name => "Enthralled";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Curse;
    public override CardRarity Rarity => CardRarity.Curse;
    public override TargetType Target => TargetType.None;
    public override bool LocksHandWhileInHand => true;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // playing it just discards it, lifting the lock
}
