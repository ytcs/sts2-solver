using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// Combat-affecting relic pool — batch 3 (Wave 3). Stateless deterministic relics, implemented directly through
// the stateless relic event hooks (OnCombatStart / OnPlayerTurnStart / BeforeCardPlayed / AfterCardPlayed /
// AfterCardExhausted / AfterSideTurnEnd / ModifyHandDraw / ModifyPowerAmountGiven). No per-combat mutable state,
// so nothing to clone/hash. SOUNDNESS: every effect is deterministic ⇒ exact for the objective; AoE damage hits
// all living enemies (deterministic); downsides (Royal Poison self-damage, Big Mushroom −2 draw) are modelled.

internal static class RelicAoe
{
    /// <summary>Deal <paramref name="dmg"/> unpowered damage to every living enemy (relic AoE pings).</summary>
    public static void DamageAllEnemies(CombatState combat, int dmg)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            if (m.IsAlive) Cmd.Attack(combat, combat.Player, m, dmg, ValueProp.Unpowered, null);
    }
}

// ---- Combat-start grants ----

/// <summary>Fake Anchor: gain 4 Block at the start of combat. (MegaCrit FakeAnchor.)</summary>
public sealed class FakeAnchor : RelicModel
{
    public override string Id => "FakeAnchor";
    public override void OnCombatStart(CombatState combat) => Cmd.GainBlock(combat, combat.Player, 4, ValueProp.Unpowered, null);
}

/// <summary>Twisted Funnel: at combat start apply 4 Poison to ALL enemies. (MegaCrit TwistedFunnel.)</summary>
public sealed class TwistedFunnel : RelicModel
{
    public override string Id => "TwistedFunnel";
    public override void OnCombatStart(CombatState combat)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.ApplyPower(combat, m, new PoisonPower(), 4, combat.Player);
    }
}

/// <summary>Blood-Soaked Rose: gain 1 additional energy each turn. (The Enthralled curse it adds on pickup is a
/// deck card that ranwid already reads from the save, so only the energy is an in-combat effect.) (MegaCrit
/// BloodSoakedRose.)</summary>
public sealed class BloodSoakedRose : RelicModel
{
    public override string Id => "BloodSoakedRose";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new RelicMaxEnergyPower(), 1, combat.Player);
}

/// <summary>Prismatic Gem: gain 1 additional energy each turn. (Its card-reward effect is out of combat scope.)
/// (MegaCrit PrismaticGem.)</summary>
public sealed class PrismaticGem : RelicModel
{
    public override string Id => "PrismaticGem";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new RelicMaxEnergyPower(), 1, combat.Player);
}

/// <summary>Sozu: gain 1 additional energy each turn. (Its potion lockout is out of scope — potions aren't
/// modelled.) (MegaCrit Sozu.)</summary>
public sealed class Sozu : RelicModel
{
    public override string Id => "Sozu";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new RelicMaxEnergyPower(), 1, combat.Player);
}

/// <summary>Fiddle: draw 2 additional cards at the start of each turn. (MegaCrit Fiddle.)</summary>
public sealed class Fiddle : RelicModel
{
    public override string Id => "Fiddle";
    public override void OnCombatStart(CombatState combat) => Cmd.ApplyPower(combat, combat.Player, new RelicDrawPower(), 2, combat.Player);
}

// ---- Turn-numbered / recurring turn-start effects ----

/// <summary>Fake Blood Vial: heal 1 HP on the first turn. (MegaCrit FakeBloodVial.)</summary>
public sealed class FakeBloodVial : RelicModel
{
    public override string Id => "FakeBloodVial";
    public override void OnPlayerTurnStart(CombatState combat) { if (combat.TurnNumber == 1) combat.Player.Heal(1); }
}

/// <summary>Divine Destiny: gain 6 Stars on the first turn. (MegaCrit DivineDestiny.)</summary>
public sealed class DivineDestiny : RelicModel
{
    public override string Id => "DivineDestiny";
    public override void OnPlayerTurnStart(CombatState combat) { if (combat.TurnNumber == 1) Cmd.GainStars(combat, 6); }
}

/// <summary>Bread: lose 2 energy on turn 1; gain 1 extra energy every turn after. (MegaCrit Bread.)</summary>
public sealed class Bread : RelicModel
{
    public override string Id => "Bread";
    public override void OnPlayerTurnStart(CombatState combat)
    {
        if (combat.TurnNumber == 1) combat.Player.LoseEnergy(2);
        else Cmd.GainEnergy(combat, 1);
    }
}

/// <summary>Pael's Flesh: gain 1 extra energy from turn 3 onward. (MegaCrit PaelsFlesh.)</summary>
public sealed class PaelsFlesh : RelicModel
{
    public override string Id => "PaelsFlesh";
    public override void OnPlayerTurnStart(CombatState combat) { if (combat.TurnNumber >= 3) Cmd.GainEnergy(combat, 1); }
}

/// <summary>Captain's Wheel: at the start of turn 3 (after block clears), gain 18 Block. (MegaCrit CaptainsWheel.)</summary>
public sealed class CaptainsWheel : RelicModel
{
    public override string Id => "CaptainsWheel";
    public override void OnPlayerTurnStart(CombatState combat) { if (combat.TurnNumber == 3) Cmd.GainBlock(combat, combat.Player, 18, ValueProp.Unpowered, null); }
}

/// <summary>Horn Cleat: at the start of turn 2 (after block clears), gain 14 Block. (MegaCrit HornCleat.)</summary>
public sealed class HornCleat : RelicModel
{
    public override string Id => "HornCleat";
    public override void OnPlayerTurnStart(CombatState combat) { if (combat.TurnNumber == 2) Cmd.GainBlock(combat, combat.Player, 14, ValueProp.Unpowered, null); }
}

/// <summary>Sparkling Rouge: at the start of turn 3, gain 1 Strength and 1 Dexterity. (MegaCrit SparklingRouge.)</summary>
public sealed class SparklingRouge : RelicModel
{
    public override string Id => "SparklingRouge";
    public override void OnPlayerTurnStart(CombatState combat)
    {
        if (combat.TurnNumber != 3) return;
        Cmd.ApplyPower(combat, combat.Player, new StrengthPower(), 1, combat.Player);
        Cmd.ApplyPower(combat, combat.Player, new DexterityPower(), 1, combat.Player);
    }
}

/// <summary>Mercury Hourglass: at the start of each turn, deal 3 damage to ALL enemies. (MegaCrit MercuryHourglass.)</summary>
public sealed class MercuryHourglass : RelicModel
{
    public override string Id => "MercuryHourglass";
    public override void OnPlayerTurnStart(CombatState combat) => RelicAoe.DamageAllEnemies(combat, 3);
}

/// <summary>Mr Struggles: at the start of each turn, deal damage equal to the turn number to ALL enemies.
/// (MegaCrit MrStruggles.)</summary>
public sealed class MrStruggles : RelicModel
{
    public override string Id => "MrStruggles";
    public override void OnPlayerTurnStart(CombatState combat) => RelicAoe.DamageAllEnemies(combat, combat.TurnNumber);
}

/// <summary>Royal Poison: on the first turn, lose 4 HP (unblockable). A downside event relic, modelled so it
/// can't read as harmless. (MegaCrit RoyalPoison.)</summary>
public sealed class RoyalPoison : RelicModel
{
    public override string Id => "RoyalPoison";
    public override void OnPlayerTurnStart(CombatState combat) { if (combat.TurnNumber == 1) Cmd.LoseHp(combat, combat.Player, 4); }
}

/// <summary>Runic Capacitor: gain 3 orb slots on the first turn (Defect). Inert for non-orb characters.
/// (MegaCrit RunicCapacitor.)</summary>
public sealed class RunicCapacitor : RelicModel
{
    public override string Id => "RunicCapacitor";
    public override void OnPlayerTurnStart(CombatState combat) { if (combat.TurnNumber == 1) combat.Player.OrbSlots += 3; }
}

// ---- Turn-start draw modifiers (relic ModifyHandDraw — combat-available, so turn-conditioned) ----

/// <summary>Bag of Preparation: draw 2 additional cards on the first turn. (MegaCrit BagOfPreparation.)</summary>
public sealed class BagOfPreparation : EventRelic
{
    public override string Id => "BagOfPreparation";
    public override int ModifyHandDraw(CombatState combat, int count) => combat.TurnNumber <= 1 ? count + 2 : count;
}

/// <summary>Ring of the Snake: draw 2 additional cards on the first turn. (MegaCrit RingOfTheSnake.)</summary>
public sealed class RingOfTheSnake : EventRelic
{
    public override string Id => "RingOfTheSnake";
    public override int ModifyHandDraw(CombatState combat, int count) => combat.TurnNumber <= 1 ? count + 2 : count;
}

/// <summary>Ring of the Drake: draw 2 additional cards on each of the first 3 turns. (MegaCrit RingOfTheDrake.)</summary>
public sealed class RingOfTheDrake : EventRelic
{
    public override string Id => "RingOfTheDrake";
    public override int ModifyHandDraw(CombatState combat, int count) => combat.TurnNumber <= 3 ? count + 2 : count;
}

/// <summary>Big Mushroom: draw 2 FEWER cards on the first turn. (Its +20 max HP is a one-time pickup, already
/// reflected in the save's HP, so only the draw downside is an in-combat effect.) (MegaCrit BigMushroom.)</summary>
public sealed class BigMushroom : EventRelic
{
    public override string Id => "BigMushroom";
    public override int ModifyHandDraw(CombatState combat, int count) => combat.TurnNumber <= 1 ? count - 2 : count;
}

// ---- End-of-turn effects (relic AfterSideTurnEnd, player side) ----

/// <summary>Orichalcum: if you end your turn with no Block, gain 6 Block. (MegaCrit Orichalcum.)</summary>
public sealed class Orichalcum : EventRelic
{
    public override string Id => "Orichalcum";
    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    { if (side == CombatSide.Player && combat.Player.Block == 0) Cmd.GainBlock(combat, combat.Player, 6, ValueProp.Unpowered, null); }
}

/// <summary>Fake Orichalcum: if you end your turn with no Block, gain 3 Block. (MegaCrit FakeOrichalcum.)</summary>
public sealed class FakeOrichalcum : EventRelic
{
    public override string Id => "FakeOrichalcum";
    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    { if (side == CombatSide.Player && combat.Player.Block == 0) Cmd.GainBlock(combat, combat.Player, 3, ValueProp.Unpowered, null); }
}

/// <summary>Cloak Clasp: at the end of your turn, gain Block equal to the number of cards in your hand.
/// (MegaCrit CloakClasp.)</summary>
public sealed class CloakClasp : EventRelic
{
    public override string Id => "CloakClasp";
    public override void BeforeSideTurnEnd(CombatState combat)
    { if (combat.Player.Hand.Count > 0) Cmd.GainBlock(combat, combat.Player, combat.Player.Hand.Count, ValueProp.Unpowered, null); }
}

/// <summary>Ripple Basin: if you played no Attacks this turn, gain 4 Block at the end of your turn. (MegaCrit
/// RippleBasin.)</summary>
public sealed class RippleBasin : EventRelic
{
    public override string Id => "RippleBasin";
    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    { if (side == CombatSide.Player && combat.AttacksPlayedThisTurn == 0) Cmd.GainBlock(combat, combat.Player, 4, ValueProp.Unpowered, null); }
}

/// <summary>Screaming Flagon: if your hand is empty at the end of your turn, deal 20 damage to ALL enemies.
/// (MegaCrit ScreamingFlagon.)</summary>
public sealed class ScreamingFlagon : EventRelic
{
    public override string Id => "ScreamingFlagon";
    public override void BeforeSideTurnEnd(CombatState combat)
    { if (combat.Player.Hand.Count == 0) RelicAoe.DamageAllEnemies(combat, 20); }
}

/// <summary>Stone Calendar: at the end of turn 7, deal 52 damage to ALL enemies. (MegaCrit StoneCalendar.)</summary>
public sealed class StoneCalendar : EventRelic
{
    public override string Id => "StoneCalendar";
    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    { if (side == CombatSide.Player && combat.TurnNumber == 7) RelicAoe.DamageAllEnemies(combat, 52); }
}

/// <summary>Lunar Pastry: at the end of each turn, gain 1 Star. (MegaCrit LunarPastry.)</summary>
public sealed class LunarPastry : EventRelic
{
    public override string Id => "LunarPastry";
    public override void AfterSideTurnEnd(CombatState combat, CombatSide side) { if (side == CombatSide.Player) Cmd.GainStars(combat, 1); }
}

// ---- On-card-played effects (relic BeforeCardPlayed / AfterCardPlayed) ----

/// <summary>Intimidating Helmet: when you play a card costing 2 or more, gain 4 Block. (MegaCrit IntimidatingHelmet.)</summary>
public sealed class IntimidatingHelmet : EventRelic
{
    public override string Id => "IntimidatingHelmet";
    public override void BeforeCardPlayed(CombatState combat, CardModel card)
    { if (CombatManager.ResolveCardCost(combat, card) >= 2) Cmd.GainBlock(combat, combat.Player, 4, ValueProp.Unpowered, null); }
}

/// <summary>Ivory Tile: when you play a card costing 3 or more, gain 1 energy. (MegaCrit IvoryTile.)</summary>
public sealed class IvoryTile : EventRelic
{
    public override string Id => "IvoryTile";
    public override void BeforeCardPlayed(CombatState combat, CardModel card)
    { if (CombatManager.ResolveCardCost(combat, card) >= 3) Cmd.GainEnergy(combat, 1); }
}

/// <summary>Daughter of the Wind: whenever you play an Attack, gain 1 Block. (MegaCrit DaughterOfTheWind.)</summary>
public sealed class DaughterOfTheWind : EventRelic
{
    public override string Id => "DaughterOfTheWind";
    public override void AfterCardPlayed(CombatState combat, CardModel card)
    { if (card.Type == CardType.Attack) Cmd.GainBlock(combat, combat.Player, 1, ValueProp.Unpowered, null); }
}

/// <summary>Lost Wisp: whenever you play a Power, deal 8 damage to ALL enemies. (MegaCrit LostWisp.)</summary>
public sealed class LostWisp : EventRelic
{
    public override string Id => "LostWisp";
    public override void AfterCardPlayed(CombatState combat, CardModel card)
    { if (card.Type == CardType.Power) RelicAoe.DamageAllEnemies(combat, 8); }
}

/// <summary>Game Piece: whenever you play a Power, draw 1 card. (MegaCrit GamePiece.)</summary>
public sealed class GamePiece : EventRelic
{
    public override string Id => "GamePiece";
    public override void AfterCardPlayed(CombatState combat, CardModel card)
    { if (card.Type == CardType.Power) Cmd.Draw(combat, 1); }
}

// ---- On-exhaust / on-apply effects ----

/// <summary>Charon's Ashes: whenever a card is exhausted, deal 3 damage to ALL enemies. (MegaCrit CharonsAshes.)</summary>
public sealed class CharonsAshes : EventRelic
{
    public override string Id => "CharonsAshes";
    public override void AfterCardExhausted(CombatState combat, CardModel card, bool causedByEthereal) => RelicAoe.DamageAllEnemies(combat, 3);
}

/// <summary>Snecko Skull: the Poison you apply is increased by 1. (MegaCrit SneckoSkull.)</summary>
public sealed class SneckoSkull : EventRelic
{
    public override string Id => "SneckoSkull";
    public override int ModifyPowerAmountGiven(PowerModel power, Creature? applier, int amount)
        => power.Id == "Poison" && applier != null && applier.Side == CombatSide.Player ? amount + 1 : amount;
}
