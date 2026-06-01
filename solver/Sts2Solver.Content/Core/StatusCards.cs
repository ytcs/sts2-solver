using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>Status card (unplayable). At end of the player's turn, every copy held in hand deals 3
/// damage to the player — unpowered (no Strength/Vulnerable) but blockable. Added to the discard pile
/// by Phrog Parasite's Infect and Wriggler's Wriggle. (MegaCrit Infection)</summary>
public sealed class Infection : CardModel
{
    public override string Name => "Infection";
    public override int BaseCost => -1;                 // unplayable; cost is irrelevant
    public override CardType Type => CardType.Status;
    public override CardRarity Rarity => CardRarity.Status;
    public override TargetType Target => TargetType.None;

    public override bool Unplayable => true;
    public override bool HasTurnEndInHandEffect => true;

    public int Damage => 3;

    public override void OnPlay(CombatState combat, CardPlay play) { }  // never played

    public override void OnTurnEndInHand(CombatState combat)
        => Cmd.Attack(combat, combat.Player, combat.Player, Damage, ValueProp.Unpowered | ValueProp.Move, this);
}
/// <summary>Status card (unplayable). At end of the player's turn, every copy held in hand deals 2
/// damage to the player — unpowered but blockable. MechaKnight's Flamethrower adds 4 to the hand.
/// (MegaCrit Burn)</summary>
public sealed class Burn : CardModel
{
    public override string Name => "Burn";
    public override int BaseCost => -1;                 // unplayable
    public override CardType Type => CardType.Status;
    public override CardRarity Rarity => CardRarity.Status;
    public override TargetType Target => TargetType.None;

    public override bool Unplayable => true;
    public override bool HasTurnEndInHandEffect => true;

    public int Damage => 2;

    public override void OnPlay(CombatState combat, CardPlay play) { }  // never played

    public override void OnTurnEndInHand(CombatState combat)
        => Cmd.Attack(combat, combat.Player, combat.Player, Damage, ValueProp.Unpowered | ValueProp.Move, this);
}
/// <summary>Status card: Unplayable + Ethereal (exhausts at end of turn if still in hand). No effect
/// while held — pure draw dilution. Shuffled into the draw pile by Entomancer's Personal Hive.
/// (MegaCrit Dazed)</summary>
public sealed class Dazed : CardModel
{
    public override string Name => "Dazed";
    public override int BaseCost => -1;
    public override CardType Type => CardType.Status;
    public override CardRarity Rarity => CardRarity.Status;
    public override TargetType Target => TargetType.None;

    public override bool Unplayable => true;
    public override bool Ethereal => true;

    public override void OnPlay(CombatState combat, CardPlay play) { }  // never played
}

/// <summary>Curse: Unplayable + Ethereal (exhausts at end of turn if still in hand) + Eternal (can't be
/// removed from the deck between combats). The starting curse added to every deck at Ascension 5+
/// (AscendersBane), so it appears in the opening hand/draw of every A10 run. No combat effect — pure draw
/// dilution. (MegaCrit AscendersBane)</summary>
public sealed class AscendersBane : CardModel
{
    public override string Name => "AscendersBane";
    public override int BaseCost => -1;
    public override CardType Type => CardType.Curse;
    public override CardRarity Rarity => CardRarity.Curse;
    public override TargetType Target => TargetType.None;

    public override bool Unplayable => true;
    public override bool Ethereal => true;

    public override void OnPlay(CombatState combat, CardPlay play) { }  // never played
}
