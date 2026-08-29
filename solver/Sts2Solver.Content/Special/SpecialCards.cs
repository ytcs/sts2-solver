using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Event & Ancient special-pool cards (CardRarity.Event / CardRarity.Ancient).
// These belong to no single character (acquired from events / special rewards),
// so — like the Colorless pool — they register in their own table
// (SpecialCardFactories, Special/SpecialCatalog.cs) and join the deck-buildable
// CardPool. New powers they grant live in Special/SpecialPowers.cs.
//
// Faithfulness follows the established convention: where a card's only unported
// half is RNG card-generation, an HP-neutral selection/card-flow prompt, or a
// meta (gold/stars/reward) effect, it is ported as its HP-faithful subset with
// the reason documented inline. The few cards needing an unported SUBSYSTEM are
// deferred at the bottom of this file with a clear note.
// ===========================================================================

// ==========================================================================
// EVENT cards
// ==========================================================================

/// <summary>Deal 14 damage. Cost 0. Upgrade: +4. (Byrdpip pet is cosmetic — the damage is a normal attack.)
/// (MegaCrit ByrdSwoop)</summary>
public sealed class ByrdSwoop : CardModel
{
    public override string Name => "ByrdSwoop";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Event;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 14 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 3 damage to ALL enemies 4 times. Cost 1. Upgrade: +1 damage per hit. (MegaCrit Exterminate)
/// </summary>
public sealed class Exterminate : CardModel
{
    public override string Name => "Exterminate";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Event;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 3 + Upgrades;
    public int Hits => 4;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.AttackMulti(combat, combat.Player, m, Damage, Hits, ValueProp.Move, this);
    }
}

/// <summary>Deal 2 damage 3 times. Cost 1. Upgrade: +1 hit (4 times). (MegaCrit Peck)</summary>
public sealed class Peck : CardModel
{
    public override string Name => "Peck";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Event;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 2;
    public int Hits => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, Hits, ValueProp.Move, this);
}

/// <summary>Deal 10 damage. Apply 2 Vulnerable. Cost 1. Upgrade: +2 damage, +1 Vulnerable. (MegaCrit Squash)
/// </summary>
public sealed class Squash : CardModel
{
    public override string Name => "Squash";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Event;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 10 + 2 * Upgrades;
    public int Vulnerable => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new VulnerablePower(), Vulnerable, combat.Player);
    }
}

/// <summary>Deal 7 damage to a random enemy 2 times. Cost 1. Upgrade: +2 damage. (MegaCrit RipAndTear) —
/// each hit targets a (random) living enemy; modelled deterministically against the first living enemy,
/// which is exact for single-enemy fights (the common case) and an approximation of the random split for
/// multi-enemy fights.</summary>
public sealed class RipAndTear : CardModel
{
    public override string Name => "RipAndTear";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Event;
    public override TargetType Target => TargetType.RandomEnemy;
    public int Damage => 7 + 2 * Upgrades;
    public int Hits => 2;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        for (int i = 0; i < Hits; i++)
        {
            var target = combat.LivingMonsters.FirstOrDefault();
            if (target == null) break;
            Cmd.Attack(combat, combat.Player, target, Damage, ValueProp.Move, this);
        }
    }
}

/// <summary>Double your current Block. Cost 2. Upgrade: costs 1. (MegaCrit Entrench — gains Block equal to
/// your current Block, unpowered.)</summary>
public sealed class Entrench : CardModel
{
    public override string Name => "Entrench";
    public override int BaseCost => Upgrades > 0 ? 1 : 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Event;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, combat.Player.Block, ValueProp.Unpowered | ValueProp.Move, this);
}

/// <summary>Gain Block equal to the number of cards in your discard pile. Cost 1. Upgrade: +3 Block.
/// (MegaCrit Stack)</summary>
public sealed class Stack : CardModel
{
    public override string Name => "Stack";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Event;
    public override TargetType Target => TargetType.Self;
    public int BonusBlock => 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, BonusBlock + combat.Player.DiscardPile.Count, ValueProp.Move, this);
}

/// <summary>Gain 2 energy next turn. Cost 1. Upgrade: +1 energy (3). (MegaCrit Outmaneuver)</summary>
public sealed class Outmaneuver : CardModel
{
    public override string Name => "Outmaneuver";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Event;
    public override TargetType Target => TargetType.Self;
    public int Energy => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new EnergyNextTurnPower(), Energy, combat.Player);
}

/// <summary>Gain 5 Strength this turn. Cost 0. Upgrade: +2 Strength. (MegaCrit FeedingFrenzy — a one-turn
/// temporary Strength, undone at end of turn.)</summary>
public sealed class FeedingFrenzy : CardModel
{
    public override string Name => "FeedingFrenzy";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Event;
    public override TargetType Target => TargetType.Self;
    public int Strength => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new FeedingFrenzyPower(), Strength, combat.Player);
}

/// <summary>Deal 9 damage. The next card you play this turn goes on top of your draw pile. Cost 1. Upgrade:
/// +3 damage. (MegaCrit Rebound — the card-flow half is HP-neutral, modelled by the inert ReboundPower.)
/// </summary>
public sealed class Rebound : CardModel
{
    public override string Name => "Rebound";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Event;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 9 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new ReboundPower(), 1, combat.Player);
    }
}

/// <summary>Power: at the start of each turn, add a random Common card to your hand. Cost 1. Upgrade: Innate.
/// (MegaCrit HelloWorld — the generated card is RNG/HP-neutral and reconstructed on replay, so the power is an
/// inert marker; Innate only seeds the opening hand, not modelled.)</summary>
public sealed class HelloWorld : CardModel
{
    public override string Name => "HelloWorld";
    public override bool Innate => Upgrades > 0;   // Upgrade: Innate (guaranteed in the opening hand)
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Event;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new HelloWorldPower(), 1, combat.Player);
}

/// <summary>Gain 5 Block. Gain 5 Block at the start of your next 2 turns. Cost 2. Upgrade: +2 Block.
/// (MegaCrit ToricToughness — the retained block is re-granted by ToricToughnessPower; it stores the ACTUAL
/// block gained, after any Frail/Dexterity, matching the game.)</summary>
public sealed class ToricToughness : CardModel
{
    public override string Name => "ToricToughness";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Event;
    public override TargetType Target => TargetType.Self;
    public int Block => 5 + 2 * Upgrades;
    public int Turns => 2;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int before = combat.Player.Block;
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        int gained = combat.Player.Block - before;
        Cmd.ApplyPower(combat, combat.Player, new ToricToughnessPower(), Turns, combat.Player);
        (combat.Player.GetPower("ToricToughness") as ToricToughnessPower)?.SetBlock(gained);
    }
}

/// <summary>Add a random Skill to your hand (it costs 0 this turn). Exhaust. Cost 1. Upgrade: costs 0.
/// (MegaCrit Distraction) — the generated Skill is RNG/unrecorded and HP-neutral until played (reconstructed
/// by the validator), so this is modelled as exhaust-self only, mirroring Metamorphosis / Infernal Blade.
/// </summary>
public sealed class Distraction : CardModel
{
    public override string Name => "Distraction";
    public override int BaseCost => Upgrades > 0 ? 0 : 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Event;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // generated Skill is RNG/HP-neutral
}

/// <summary>Choose an Attack or Power in your hand. Add a copy of it to your hand. Cost 1. Upgrade: +1 copy.
/// (MegaCrit DualWield) — the chosen card and its copies are an HP-neutral selection that the recorder/
/// validator reconstructs as the copies are played (they carry their original's effect), so this is modelled
/// as inert, mirroring Metamorphosis / Distraction.</summary>
public sealed class DualWield : CardModel
{
    public override string Name => "DualWield";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Event;
    public override TargetType Target => TargetType.Self;
    public int Copies => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // copy is a selection prompt, reconstructed on replay
}

// ==========================================================================
// ANCIENT cards
// ==========================================================================

// MeteorShower (Ancient) is a Regent Stars card; it lives in Content/Regent/RegentCards.cs (where its StarCost
// is modelled). Removed from the special pool to keep a single canonical definition.

/// <summary>Deal 5 damage twice. Each time this card is played, its damage rises by 1 for the rest of combat.
/// Cost 1. Upgrade: +1 damage, +1 increase. (MegaCrit Maul) — the game buffs EVERY Maul on a play; we buff
/// only the played instance (Rampage-style self-escalation), which is exact for the common single-Maul deck
/// and avoids the shared-instance aliasing that cross-instance mutation would reintroduce. Stateful.</summary>
public sealed class Maul : CardModel
{
    public override string Name => "Maul";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Ancient;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Base => 5 + Upgrades;
    public int Increase => 2 + Upgrades;
    public int Hits => 2;
    private int _extra;
    public override bool Stateful => true;   // _extra escalates per play, so each search state needs its own instance
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.AttackMulti(combat, combat.Player, play.Target!, Base + _extra, Hits, ValueProp.Move, this);
        _extra += Increase;
    }
    public override string StateKey() => Upgrades > 0 ? $"Maul+{Upgrades}/{_extra}" : $"Maul/{_extra}";
}

/// <summary>Deal 10 damage. Return up to 2 cards from your discard pile to your hand. Exhaust. Cost 1.
/// Upgrade: +4 damage, +1 card. (MegaCrit NeowsFury — the return is an HP-neutral selection/card-flow prompt
/// reconstructed on replay, so only the damage is modelled.)</summary>
public sealed class NeowsFury : CardModel
{
    public override string Name => "NeowsFury";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Ancient;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Damage => 10 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 33 damage. Stun the target (it skips its next turn). Exhaust. Cost 3. Upgrade: +11 damage.
/// (MegaCrit Whistle) — the stun is modelled via the engine's bounded one-turn stun (Cmd.Stun): the target's
/// telegraphed move is DELAYED a turn (never permanently disabled), so the player correctly avoids one enemy
/// action and the model can't over-credit.</summary>
public sealed class Whistle : CardModel
{
    public override string Name => "Whistle";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Ancient;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Damage => 33 + 11 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target is Monster m) Cmd.Stun(combat, m);
    }
}

/// <summary>Gain 2 energy. Draw 2 cards. Lose 1 Max HP. Cost 0. Upgrade: +1 energy, +1 card. (MegaCrit
/// BrightestFlame — the draw is real only under an ambient Rng; the Max-HP loss lowers the heal cap and, if
/// already at max, costs 1 current HP.)</summary>
public sealed class BrightestFlame : CardModel
{
    public override string Name => "BrightestFlame";
    public override bool LoopRiskDraw => true;   // cost 0 + draws + returns to discard ⇒ replayable in-turn
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Ancient;
    public override TargetType Target => TargetType.Self;
    public int Energy => 2 + Upgrades;
    public int Cards => 2 + Upgrades;
    public int MaxHpLoss => 2;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainEnergy(combat, Energy);
        Cmd.Draw(combat, Cards);
        combat.Player.LoseMaxHp(MaxHpLoss);
    }
}

/// <summary>Gain 15 Block. Draw 2 cards and gain 2 energy at the start of your next turn. Exhaust. Cost 3.
/// Upgrade: +2 Block, +1 card, +1 energy. (MegaCrit Relax — the next-turn draw/energy use DrawNextTurnPower /
/// EnergyNextTurnPower.)</summary>
public sealed class Relax : CardModel
{
    public override string Name => "Relax";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Ancient;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Block => 16 + 2 * Upgrades;
    public int Cards => 2 + Upgrades;
    public int Energy => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new DrawNextTurnPower(), Cards, combat.Player);
        Cmd.ApplyPower(combat, combat.Player, new EnergyNextTurnPower(), Energy, combat.Player);
    }
}

/// <summary>Gain 1 Intangible (all HP loss reduced to 1 until your next turn). Ethereal. Exhaust. Cost 1.
/// Upgrade: removes Ethereal. (MegaCrit Apparition)</summary>
public sealed class Apparition : CardModel
{
    public override string Name => "Apparition";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Ancient;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override bool Ethereal => Upgrades == 0;
    public int Intangible => 1;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new IntangiblePower(), Intangible, combat.Player);
}

// WraithForm is a Silent power card; its card + the Silent-only WraithFormPower live in Content/Silent/
// (the Silent agent ported it from the canonical SilentCardPool). Removed from the special pool to keep a
// single canonical definition. (Apparition above stays here and grants the shared Core IntangiblePower.)

/// <summary>Power: after combat, gain extra card-removal rewards. Eternal. Cost 2. Upgrade: costs 1.
/// (MegaCrit ForbiddenGrimoire — the reward is a meta effect with no in-combat consequence, modelled via the
/// inert ForbiddenGrimoirePower.)</summary>
public sealed class ForbiddenGrimoire : CardModel
{
    public override string Name => "ForbiddenGrimoire";
    public override int BaseCost => Upgrades > 0 ? 1 : 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Ancient;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new ForbiddenGrimoirePower(), 1, combat.Player);
}

// TheSealedThrone (Ancient) is a Regent Stars card; it lives in Content/Regent/RegentCards.cs (where its
// StarCost and Stars-on-play are modelled). Removed from the special pool to keep one canonical definition.

// ==========================================================================
// DEFERRED — need an unported SUBSYSTEM (not just an HP-neutral half):
//   * MadScience (Event)      — TinkerTime variable card: its type (Attack 12 /
//                               Skill 8 block / Power+rider) and rider are chosen
//                               at acquisition and stored in the run save; our
//                               BuildCard is name+upgrade only, so the concrete
//                               effect is ambiguous without that saved config.
//   * BiasedCognition (Ancient) — Defect Focus + orbs (orbs unmodelled).
//   * Quadcast (Ancient)        — evokes orbs (orbs unmodelled).
//   * Protector (Ancient)       — Osty pet attack (companion creature unmodelled).
// ==========================================================================
