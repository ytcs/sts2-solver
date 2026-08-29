using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Silent character cards. Shared status cards live in Core/StatusCards.cs;
// generic keyword powers (Poison, Weak, …) in Core/CommonPowers.cs; Silent-only
// powers in SilentPowers.cs; registration in SilentCatalog.cs.
// ===========================================================================

/// <summary>Deal 6 damage. Upgrade: +3. (MegaCrit StrikeSilent)</summary>
public sealed class StrikeSilent : CardModel
{
    public override string Name => "StrikeSilent";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Gain 5 Block. Upgrade: +3. (MegaCrit DefendSilent)</summary>
public sealed class DefendSilent : CardModel
{
    public override string Name => "DefendSilent";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.Self;
    public override bool IsDefend => true;   // Defend tag (Fasten boost)
    public int Block => 5 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Deal 3 damage. Apply 1 Weak. Cost 0. Upgrade: +1 each. (MegaCrit Neutralize — Silent starter)</summary>
public sealed class Neutralize : CardModel
{
    public override string Name => "Neutralize";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 3 + Upgrades;
    public int Weak => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new WeakPower(), Weak, combat.Player);
    }
}

/// <summary>Gain 8 Block. (Discard a card — deferred: the discard isn't modelled yet, so this is the
/// block-only effect.) Upgrade: +3 Block. (MegaCrit Survivor — Silent starter)</summary>
// NOTE: Survivor's "discard 1 card" half is intentionally omitted until hand-discard selection is
// supported. Kept here for completeness so the starter deck can be built, but excluded from validation
// of the discard behaviour. (Listed in DeferredSilentCards.)

/// <summary>Deal 6 damage. Cost 0. Upgrade: +3. (MegaCrit Slice)</summary>
public sealed class Slice : CardModel
{
    public override string Name => "Slice";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Gain 4 Block. Cost 0. Upgrade: +3. (MegaCrit Deflect)</summary>
public sealed class Deflect : CardModel
{
    public override string Name => "Deflect";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 4 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Gain 10 Block. Deal 10 damage. Upgrade: +3 each. (MegaCrit Dash)</summary>
public sealed class Dash : CardModel
{
    public override string Name => "Dash";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 10 + 3 * Upgrades;
    public int Block => 10 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
    }
}

/// <summary>Deal 8 damage. Apply 1 Weak. Upgrade: +2 damage, +1 Weak. (MegaCrit SuckerPunch)</summary>
public sealed class SuckerPunch : CardModel
{
    public override string Name => "SuckerPunch";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 8 + 2 * Upgrades;
    public int Weak => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new WeakPower(), Weak, combat.Player);
    }
}

/// <summary>Gain 11 Block. Apply 2 Weak. Upgrade: +3 Block, +1 Weak. (MegaCrit LegSweep)</summary>
public sealed class LegSweep : CardModel
{
    public override string Name => "LegSweep";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;   // Skill that targets an enemy (applies Weak)
    public int Block => 11 + 3 * Upgrades;
    public int Weak => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new WeakPower(), Weak, combat.Player);
    }
}

/// <summary>Deal 4 damage to ALL enemies twice. Upgrade: +2 damage. (MegaCrit DaggerSpray)</summary>
public sealed class DaggerSpray : CardModel
{
    public override string Name => "DaggerSpray";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 4 + 2 * Upgrades;
    public int Hits => 2;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.AttackMulti(combat, combat.Player, m, Damage, Hits, ValueProp.Move, this);
    }
}

/// <summary>Deal 6 damage. Apply 3 Poison. Upgrade: +2 damage, +1 Poison. (MegaCrit PoisonedStab)</summary>
public sealed class PoisonedStab : CardModel
{
    public override string Name => "PoisonedStab";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 6 + 2 * Upgrades;
    public int Poison => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new PoisonPower(), Poison, combat.Player);
    }
}

/// <summary>Apply 5 Poison. Upgrade: +2. (MegaCrit DeadlyPoison)</summary>
public sealed class DeadlyPoison : CardModel
{
    public override string Name => "DeadlyPoison";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Poison => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new PoisonPower(), Poison, combat.Player);
    }
}

/// <summary>Gain 2 Dexterity. Power. Upgrade: +1. (MegaCrit Footwork)</summary>
public sealed class Footwork : CardModel
{
    public override string Name => "Footwork";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Dexterity => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new DexterityPower(), Dexterity, combat.Player);
}

/// <summary>Apply 4 Poison and 1 Weak to ALL enemies. Cost 2. Upgrade: +2 Poison, +1 Weak.
/// (MegaCrit Haze — reworked v0.110.0; no longer Sly.)</summary>
public sealed class Haze : CardModel
{
    public override string Name => "Haze";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AllEnemies;
    public int Poison => 4 + 2 * Upgrades;
    public int Weak => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
        {
            Cmd.ApplyPower(combat, m, new PoisonPower(), Poison, combat.Player);
            Cmd.ApplyPower(combat, m, new WeakPower(), Weak, combat.Player);
        }
    }
}

/// <summary>Deal 5 damage for each Skill in your hand. Upgrade: +2 damage per hit. (MegaCrit Flechettes)
/// The played Flechettes is an Attack and is already out of hand, so it never counts itself.</summary>
public sealed class Flechettes : CardModel
{
    public override string Name => "Flechettes";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int hits = combat.Player.Hand.Count(c => c.Type == CardType.Skill);
        if (hits > 0)
            Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, hits, ValueProp.Move, this);
    }
}

// ===========================================================================
// Batch 2 — Shivs, poison-synergy powers, and more primitive attacks/skills.
// Card-generation (Shivs) is deterministic add-to-hand (no chance node).
// ===========================================================================

/// <summary>A generated 0-cost token: deal 4 damage, Exhaust. Upgrade: +2. (MegaCrit Shiv) Created in
/// hand by Cloak and Dagger / Blade Dance (Fan of Knives' all-enemy variant is not yet modelled).</summary>
public sealed class Shiv : CardModel
{
    public override string Name => "Shiv";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Special;     // "Token" in-game; cosmetic for combat
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    /// <summary>Inky enchantment (from Blade of Ink): applies 1 Weak on play. v0.111.0: no extra damage.</summary>
    public bool Inky;
    public int Damage => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (Inky && play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new WeakPower(), 1, combat.Player);
    }
    // An Inky Shiv is a distinct card identity for hashing/memoisation.
    public override string StateKey() => base.StateKey() + (Inky ? "i" : "");
}

/// <summary>Add <c>n</c> freshly-created Shivs to the player's hand (or discard if the hand is full),
/// mirroring the game's AddGeneratedCardsToCombat into the hand pile.</summary>
internal static class SilentCardHelpers
{
    public static void AddShivsToHand(CombatState combat, int n, int upgrades = 0)
    {
        var p = combat.Player;
        for (int i = 0; i < n; i++)
        {
            var shiv = new Shiv();
            if (upgrades > 0) shiv.Upgraded(upgrades);
            (p.Hand.Count < Player.MaxHandSize ? p.Hand : p.DiscardPile).Add(shiv);
        }
    }

    /// <summary>Discard every card currently in the player's hand (moving it to the discard pile) and return
    /// the count discarded. Used by hand-dumping cards (Storm of Steel, Calculated Gamble). Any Sly card among
    /// the discarded hand auto-plays for free after the discard (<see cref="CombatManager.TriggerSlyOnDiscard"/>).</summary>
    public static int DiscardWholeHand(CombatState combat)
    {
        var hand = combat.Player.Hand.ToList();
        combat.Player.Hand.Clear();
        combat.Player.DiscardPile.AddRange(hand);
        combat.CardsDiscardedThisTurn += hand.Count;   // mid-turn discards (Memento Mori scales on this)
        CombatManager.TriggerSlyOnDiscard(combat, hand);   // Sly cards auto-play after the discard completes
        return hand.Count;
    }

    /// <summary>Discard up to <paramref name="count"/> cards from the front of hand to the discard pile. The
    /// game lets the player CHOOSE which to discard; we use a fixed default (player choice not modelled),
    /// matching the engine's other selection cards (Armaments / Burning Pact / Headbutt). Returns the count.
    /// Any Sly card discarded auto-plays for free afterward (<see cref="CombatManager.TriggerSlyOnDiscard"/>).</summary>
    public static int DiscardDefault(CombatState combat, int count)
    {
        var hand = combat.Player.Hand;
        int n = Math.Min(count, hand.Count);
        var discarded = new List<CardModel>(n);
        for (int i = 0; i < n; i++)
        {
            var card = hand[0];
            hand.RemoveAt(0);
            combat.Player.DiscardPile.Add(card);
            discarded.Add(card);
        }
        combat.CardsDiscardedThisTurn += n;   // feeds Memento Mori
        CombatManager.TriggerSlyOnDiscard(combat, discarded);   // Sly cards auto-play after the discard completes
        return n;
    }
}

/// <summary>Gain 6 Block. Add 1 Shiv to your hand. Upgrade: +1 Shiv. (MegaCrit Cloak and Dagger)</summary>
public sealed class CloakAndDagger : CardModel
{
    public override string Name => "CloakAndDagger";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 6;
    public int Shivs => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        SilentCardHelpers.AddShivsToHand(combat, Shivs);
    }
}

/// <summary>Add 3 Shivs to your hand. Exhaust. Upgrade: +1 Shiv. (MegaCrit Blade Dance)</summary>
public sealed class BladeDance : CardModel
{
    public override string Name => "BladeDance";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Shivs => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => SilentCardHelpers.AddShivsToHand(combat, Shivs);
}

/// <summary>Deal 15 damage. Upgrade: +4. (MegaCrit Pinpoint)</summary>
public sealed class Pinpoint : CardModel
{
    public override string Name => "Pinpoint";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 15 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 6 damage to ALL enemies. Upgrade: +2. (MegaCrit Flick Flack)</summary>
public sealed class FlickFlack : CardModel
{
    public override string Name => "FlickFlack";
    public override bool IsSly => true;   // Sly: auto-plays for free when discarded mid-turn
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 7 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
    }
}

/// <summary>Next turn, gain 1 Energy. Cost 0. Upgrade: +1 Energy. (MegaCrit Sidestep — renamed from Scare
/// in v0.110.0.)</summary>
public sealed class Sidestep : CardModel
{
    public override string Name => "Sidestep";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Energy => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new EnergyNextTurnPower(), Energy, combat.Player);
}

/// <summary>Apply 7 Poison. Retain. Upgrade: +3. (MegaCrit Snakebite)</summary>
public sealed class Snakebite : CardModel
{
    public override string Name => "Snakebite";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool Retain => true;   // not discarded at end of turn — carries into the next turn
    public int Poison => 7 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new PoisonPower(), Poison, combat.Player);
    }
}

/// <summary>If the target is already Poisoned, apply 9 more Poison. Upgrade: +3. (MegaCrit Bubble Bubble)</summary>
public sealed class BubbleBubble : CardModel
{
    public override string Name => "BubbleBubble";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Poison => 9 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (play.Target!.IsAlive && play.Target!.HasPower("Poison"))
            Cmd.ApplyPower(combat, play.Target!, new PoisonPower(), Poison, combat.Player);
    }
}

/// <summary>Deal 11 damage. Apply 3 Weak. Cost 0. Innate. Upgrade: +6 damage, +2 Weak. (Innate: guaranteed
/// in the opening hand.) (MegaCrit Suppress)</summary>
public sealed class Suppress : CardModel
{
    public override string Name => "Suppress";
    public override bool Innate => true;   // guaranteed in the opening hand
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;       // "Ancient" in-game; cosmetic for combat
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 11 + 6 * Upgrades;
    public int Weak => 3 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new WeakPower(), Weak, combat.Player);
    }
}

/// <summary>Gain 4 Block now, and 4 Block at the start of your next turn. Upgrade: +2 each. The next-turn
/// amount equals the Block actually gained now (so Dexterity carries over). (MegaCrit Dodge and Roll)</summary>
public sealed class DodgeAndRoll : CardModel
{
    public override string Name => "DodgeAndRoll";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int before = combat.Player.Block;
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        int gained = combat.Player.Block - before;
        Cmd.ApplyPower(combat, combat.Player, new BlockNextTurnPower(), gained, combat.Player);
    }
}

/// <summary>Gain 5 Block. Your Block is not removed next turn (Blur 1). Upgrade: +3 Block. (MegaCrit Blur)</summary>
public sealed class Blur : CardModel
{
    public override string Name => "Blur";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 5 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new BlurPower(), 1, combat.Player);
    }
}

/// <summary>Power: whenever you play a card, gain 1 Block. Upgrade: becomes Innate (guaranteed in the
/// opening hand). (MegaCrit Afterimage)</summary>
public sealed class Afterimage : CardModel
{
    public override string Name => "Afterimage";
    public override bool Innate => Upgrades > 0;   // Upgrade: Innate (guaranteed in the opening hand)
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Block => 1;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new AfterimagePower(), Block, combat.Player);
}

/// <summary>Power: at the start of each of your turns, apply 2 Poison to ALL enemies. Upgrade: +1.
/// (MegaCrit Noxious Fumes)</summary>
public sealed class NoxiousFumes : CardModel
{
    public override string Name => "NoxiousFumes";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Poison => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new NoxiousFumesPower(), Poison, combat.Player);
}

/// <summary>Power: whenever you deal unblocked attack damage, apply 1 Poison. Upgrade: +1. (MegaCrit Envenom)</summary>
public sealed class Envenom : CardModel
{
    public override string Name => "Envenom";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Poison => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new EnvenomPower(), Poison, combat.Player);
}

/// <summary>Reduce ALL enemies' Strength by 6 until their turn ends. Exhaust. Upgrade: +2. (MegaCrit
/// Piercing Wail)</summary>
public sealed class PiercingWail : CardModel
{
    public override string Name => "PiercingWail";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AllEnemies;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int StrengthLoss => 6 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.ApplyPower(combat, m, new PiercingWailPower(), StrengthLoss, combat.Player);
    }
}

// ===========================================================================
// Batch 3 — remaining Silent cards: primitive attacks/skills, X-cost, thorns,
// and a draw-next-turn power. Cards needing card-selection-from-hand,
// double-play, cost-set-on-hand, or conditional-on-drawn-card mechanics are
// SKIPPED (see the manifest) because no faithful primitive exists yet.
// ===========================================================================

/// <summary>Gain 8 Block, then discard 1 card of your choice. Upgrade: +3 Block. (MegaCrit Survivor — Silent
/// starter) The forced discard is a player MAX over the hand: in search it's a <see cref="PendingDiscard"/>
/// decision layer (no draw, so no chance node); with an ambient Rng it discards a heuristic default. Modelling
/// the discard (vs omitting it) is sound — omitting a forced discard would leave the player an extra in-turn
/// card the real game removes.</summary>
public sealed class Survivor : CardModel
{
    public override string Name => "Survivor";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.Self;
    public int Block => 8 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        if (combat.Rng == null) Cmd.DeferDiscardOfChoice(combat, 1);   // search: discard-choice player MAX
        else SilentCardHelpers.DiscardDefault(combat, 1);
    }
}

/// <summary>Deal 11 damage. Exhaust. Cost 0. Innate. Upgrade: +4. (Innate: guaranteed in the opening
/// hand.) (MegaCrit Backstab)</summary>
public sealed class Backstab : CardModel
{
    public override string Name => "Backstab";
    public override bool Innate => true;   // guaranteed in the opening hand
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Damage => 11 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 9 damage, draw 1 card, then discard 1 card of your choice. Upgrade: +3 damage. (MegaCrit
/// Dagger Throw) In search the draw is a chance node and the discard a post-draw player MAX over the resulting
/// hand (<see cref="PostDrawDiscardCount"/> — you may discard the just-drawn card, as in-game); with an ambient
/// Rng it draws then discards a heuristic default. Modelling the forced discard (vs omitting it) is sound.</summary>
public sealed class DaggerThrow : CardModel
{
    public override string Name => "DaggerThrow";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 9 + 3 * Upgrades;
    public override bool HasPostDraw => true;
    public override int PostDrawDiscardCount => 1;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (combat.Rng == null) { Cmd.DeferDrawThenResolve(combat, 1, this); return; }   // search: defer draw + discard-choice
        int drew = Cmd.Draw(combat, 1);
        if (drew > 0) SilentCardHelpers.DiscardDefault(combat, 1);
    }
}

/// <summary>Deal 15 damage. Draw 2 additional cards next turn. Upgrade: +3 damage. (MegaCrit Predator)
/// The bonus draw is modelled via <see cref="DrawNextTurnPower"/>, which draws at the next player turn start
/// (real only with an ambient Rng; HP-neutral either way).</summary>
public sealed class Predator : CardModel
{
    public override string Name => "Predator";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 15 + 5 * Upgrades;   // decompile: Damage.UpgradeValueBy(5)
    public int BonusDraw => 2;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new DrawNextTurnPower(), BonusDraw, combat.Player);
    }
}

/// <summary>Apply 3 Poison to a random enemy 3 times. Upgrade: +1 Poison per hit. (MegaCrit Bouncing Flask)
/// Each bounce picks a living enemy at random via the ambient Rng (deterministic single-target when only one
/// enemy lives — the validated case, like Sword Boomerang).</summary>
public sealed class BouncingFlask : CardModel
{
    public override string Name => "BouncingFlask";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.RandomEnemy;
    public int Poison => 3 + Upgrades;
    public int Bounces => 3;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        for (int i = 0; i < Bounces; i++)
        {
            var living = combat.LivingMonsters.ToList();
            if (living.Count == 0) break;
            var t = combat.Rng != null ? living[combat.Rng.NextInt(living.Count)] : living[0];
            Cmd.ApplyPower(combat, t, new PoisonPower(), Poison, combat.Player);
        }
    }
}

/// <summary>Power: whenever you take attack damage, deal 3 damage back to the attacker (Thorns).
/// Upgrade: +2. (MegaCrit Caltrops)</summary>
public sealed class Caltrops : CardModel
{
    public override string Name => "Caltrops";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Thorns => 3 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new CaltropsPower(), Thorns, combat.Player);
}

/// <summary>If your draw pile is empty, deal 60 damage to ALL enemies. Exhaust. Cost 0. Upgrade: +20.
/// (MegaCrit Grand Finale)</summary>
public sealed class GrandFinale : CardModel
{
    public override string Name => "GrandFinale";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 60 + 20 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Player.DrawPile.Count != 0) return;
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
    }
}

/// <summary>X-cost: deal 8 damage to the target X times, where X is all your remaining energy. Upgrade:
/// +2 damage per hit. (MegaCrit Skewer)</summary>
public sealed class Skewer : CardModel
{
    public override string Name => "Skewer";
    public override int BaseCost => 0;
    public override bool IsXCost => true;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 8 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, play.XValue, ValueProp.Move, this);
}

/// <summary>Gain 1 Energy. Draw 2 cards. Exhaust. Cost 0. Upgrade: +1 Energy. (MegaCrit Adrenaline) The draw
/// is a single draw call (no-op without an ambient Rng — the validator replays the recorded hand); the
/// energy gain is a real primitive. HP-neutral.</summary>
public sealed class Adrenaline : CardModel
{
    public override string Name => "Adrenaline";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Energy => 1 + Upgrades;
    public int Cards => 2;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainEnergy(combat, Energy);
        Cmd.Draw(combat, Cards);
    }
}

/// <summary>Gain 5 Block. Draw 2 cards. Upgrade: +3 Block, +1 card. (MegaCrit Backflip) The draw is a
/// single draw call (no-op without an ambient Rng — the validator replays the recorded hand); the block is
/// the HP-affecting part that is validated.</summary>
public sealed class Backflip : CardModel
{
    public override string Name => "Backflip";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 5 + 3 * Upgrades;
    public int Cards => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.Draw(combat, Cards);
    }
}

/// <summary>Draw 2 cards. They gain Retain this turn. Upgrade: draw 3. (MegaCrit Expertise — reworked
/// v0.109.0.)</summary>
public sealed class Expertise : CardModel
{
    public override string Name => "Expertise";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override bool HasPostDraw => true;
    public int Cards => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Rng == null) { Cmd.DeferDrawThenResolve(combat, Cards, this); return; }
        int drawn = Cmd.Draw(combat, Cards);
        RetainLastDrawn(combat, drawn);
    }
    public override void OnPostDraw(CombatState combat, int drawn) => RetainLastDrawn(combat, drawn);
    private static void RetainLastDrawn(CombatState combat, int drawn)
    {
        var hand = combat.Player.Hand;
        int n = Math.Min(drawn, hand.Count);
        for (int i = 0; i < n; i++) hand[hand.Count - 1 - i].SingleTurnRetain = true;
    }
}

// ===========================================================================
// Batch 4 — remaining primitive-portable cards: straightforward attacks/skills/
// powers expressible with existing primitives plus two new simple counter powers
// (Thorns / FreeSkill in SilentPowers.cs). The "Sly" keyword (auto-play when this
// card is discarded during your turn) is HP-neutral here — we don't model the
// hand-discard-selection that would trigger it, matching the existing deferrals.
// ===========================================================================

/// <summary>Power: gain 1 Dexterity and 4 Thorns. Upgrade: +2 Thorns. (Sly: auto-plays its power for free when discarded.)
/// (MegaCrit Abrasive)</summary>
public sealed class Abrasive : CardModel
{
    public override string Name => "Abrasive";
    public override bool IsSly => true;   // Sly: auto-plays for free when discarded mid-turn
    public override int BaseCost => 3;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Dexterity => 1;
    public int Thorns => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, combat.Player, new DexterityPower(), Dexterity, combat.Player);
        Cmd.ApplyPower(combat, combat.Player, new ThornsPower(), Thorns, combat.Player);
    }
}

/// <summary>Deal 10 damage. Apply 1 Vulnerable. Cost 0. Exhaust. Upgrade: +3 damage, +1 Vulnerable.
/// (Innate not modelled — affects only the opening hand.) (MegaCrit Assassinate)</summary>
public sealed class Assassinate : CardModel
{
    public override string Name => "Assassinate";
    public override bool Innate => true;   // guaranteed in the opening hand
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Damage => 10 + 3 * Upgrades;
    public int Vulnerable => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new VulnerablePower(), Vulnerable, combat.Player);
    }
}

/// <summary>Remove ALL of the target's Block, remove its Artifact, then apply 2 Vulnerable. Cost 0. Exhaust.
/// Upgrade: +1 Vulnerable. (MegaCrit Expose)</summary>
public sealed class Expose : CardModel
{
    public override string Name => "Expose";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Vulnerable => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        var t = play.Target!;
        if (!t.IsAlive) return;
        t.ClearBlock();
        if (t.HasPower("Artifact")) t.RemovePower("Artifact");   // game removes the Artifact power outright
        Cmd.ApplyPower(combat, t, new VulnerablePower(), Vulnerable, combat.Player);
    }
}

/// <summary>Deal 3 damage. Add 2 Shivs to your hand. Upgrade: +3 damage. (Strike-tagged.)
/// (MegaCrit LeadingStrike)</summary>
public sealed class LeadingStrike : CardModel
{
    public override string Name => "LeadingStrike";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Damage => 3 + 3 * Upgrades;
    public int Shivs => 2;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        SilentCardHelpers.AddShivsToHand(combat, Shivs);
    }
}

/// <summary>X-cost: reduce the target's Strength by X and apply X Weak, where X is all your remaining energy.
/// Exhaust. Upgrade: X+1 for both. (MegaCrit Malaise)</summary>
public sealed class Malaise : CardModel
{
    public override string Name => "Malaise";
    public override int BaseCost => 0;
    public override bool IsXCost => true;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int amount = play.XValue + (Upgrades > 0 ? 1 : 0);
        if (amount <= 0 || !play.Target!.IsAlive) return;
        Cmd.ApplyPower(combat, play.Target!, new StrengthPower(), -amount, combat.Player);   // Strength allows negative
        Cmd.ApplyPower(combat, play.Target!, new WeakPower(), amount, combat.Player);
    }
}

/// <summary>Deal 14 damage. Your next Skill this combat costs 0. Upgrade: +6 damage. (MegaCrit Pounce)</summary>
public sealed class Pounce : CardModel
{
    public override string Name => "Pounce";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 14 + 6 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new FreeSkillPower(), 1, combat.Player);
    }
}

/// <summary>Draw 2 cards. Cost 3. (Sly: auto-plays for free when discarded mid-turn.) Upgrade: draw 3. (MegaCrit Reflex) The draw is a
/// single draw call (no-op without an ambient Rng — the validator replays the recorded hand). HP-neutral.</summary>
public sealed class Reflex : CardModel
{
    public override string Name => "Reflex";
    public override bool IsSly => true;   // Sly: auto-plays (draw 2) for free when discarded mid-turn
    public override int BaseCost => 3;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Cards => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => Cmd.Draw(combat, Cards);
}

/// <summary>Deal 3 damage to a random enemy 4 times. (Sly: auto-plays for free when discarded mid-turn.) Upgrade: 5 hits. (MegaCrit
/// Ricochet) Each hit re-rolls a living target via the ambient Rng (deterministic single-target when only
/// one enemy lives — the validated case, like Bouncing Flask / Sword Boomerang).</summary>
public sealed class Ricochet : CardModel
{
    public override string Name => "Ricochet";
    public override bool IsSly => true;   // Sly: auto-plays for free when discarded mid-turn
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.RandomEnemy;
    public int Damage => 3;
    public int Hits => 4 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        for (int i = 0; i < Hits; i++)
        {
            var living = combat.LivingMonsters.ToList();
            if (living.Count == 0) break;
            var t = combat.Rng != null ? living[combat.Rng.NextInt(living.Count)] : living[0];
            Cmd.Attack(combat, combat.Player, t, Damage, ValueProp.Move, this);
        }
    }
}

/// <summary>Gain 1 Energy. Cost 3. (Sly: auto-plays for free when discarded mid-turn.) Upgrade: +1 Energy. (MegaCrit Tactician)</summary>
public sealed class Tactician : CardModel
{
    public override string Name => "Tactician";
    public override bool IsSly => true;   // Sly: auto-plays (gain energy) for free when discarded mid-turn
    public override int BaseCost => 3;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Energy => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => Cmd.GainEnergy(combat, Energy);
}

/// <summary>Gain 6 Block. Cost 2. (Sly: auto-plays for free when discarded mid-turn.) Upgrade: +3 Block. (MegaCrit Untouchable)</summary>
public sealed class Untouchable : CardModel
{
    public override string Name => "Untouchable";
    public override bool IsSly => true;   // Sly: auto-plays (gain block) for free when discarded mid-turn
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Discard your hand, then add that many Shivs to your hand. Upgrade: the Shivs are upgraded.
/// (MegaCrit Storm of Steel) Card generation is deterministic (no chance node); the discarded hand goes
/// to the discard pile. Sly cards in the discarded hand auto-play for free.</summary>
public sealed class StormOfSteel : CardModel
{
    public override string Name => "StormOfSteel";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int n = SilentCardHelpers.DiscardWholeHand(combat);
        SilentCardHelpers.AddShivsToHand(combat, n, Upgrades > 0 ? 1 : 0);
    }
}

/// <summary>Discard your hand, then draw that many cards. Cost 0. Exhaust. Upgrade: Retain (not modelled).
/// (MegaCrit Calculated Gamble) Pure HP-neutral card cycling: the redraw is a chance node the search does
/// not model, so this is a no-op without an ambient Rng (the discard would otherwise destroy the hand with
/// no compensating draw). With an Rng (a concrete driver / the validator's replay) it discards then draws.</summary>
public sealed class CalculatedGamble : CardModel
{
    public override string Name => "CalculatedGamble";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override bool Retain => Upgrades > 0;   // Upgrade: Retain (kept in hand if unplayed)
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Rng == null) return;   // search/replay: redraw is an unmodelled chance node — leave hand intact
        int n = SilentCardHelpers.DiscardWholeHand(combat);
        Cmd.Draw(combat, n);
    }
}

// ===========================================================================
// Batch 5 — Silent powers + Shiv-synergy cards (Wave 2). Each applies a new
// Silent power (see SilentPowers.cs). Multiplayer-only / Sly / Retain effects
// are inert in single-player and documented as such.
// ===========================================================================

/// <summary>Power: each enemy's Poison ticks one extra time per turn. Upgrade: +1. (MegaCrit Accelerant)</summary>
public sealed class Accelerant : CardModel
{
    public override string Name => "Accelerant";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new AccelerantPower(), Amount, combat.Player);
}

/// <summary>Power: your Shivs deal 4 additional damage. Upgrade: +2. (MegaCrit Accuracy)</summary>
public sealed class Accuracy : CardModel
{
    public override string Name => "Accuracy";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new AccuracyPower(), Amount, combat.Player);
}

/// <summary>Gain 2 Dexterity until the end of your turn. Cost 0. Upgrade: +1. (MegaCrit Anticipate)</summary>
public sealed class Anticipate : CardModel
{
    public override string Name => "Anticipate";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Dexterity => 2 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new AnticipatePower(), Dexterity, combat.Player);
}

/// <summary>Deal 8 damage. Apply Strangle 2 (each card you play deals 2 to this enemy until its turn ends).
/// Upgrade: +2 damage, +1 Strangle. (MegaCrit Strangle)</summary>
public sealed class Strangle : CardModel
{
    public override string Name => "Strangle";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 8 + 2 * Upgrades;
    public int Strang => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new StranglePower(), Strang, combat.Player);
    }
}

/// <summary>Power: at the start of each turn, add 1 Shiv to your hand. Upgrade: Innate (guaranteed in the opening hand).
/// (MegaCrit Infinite Blades)</summary>
public sealed class InfiniteBlades : CardModel
{
    public override string Name => "InfiniteBlades";
    public override bool Innate => Upgrades > 0;   // Upgrade: Innate (guaranteed in the opening hand)
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new InfiniteBladesPower(), 1, combat.Player);
}

/// <summary>Power: the first Shiv you play each turn deals 9 additional damage. Upgrade: +3.
/// (MegaCrit Phantom Blades)</summary>
public sealed class PhantomBlades : CardModel
{
    public override string Name => "PhantomBlades";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 9 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new PhantomBladesPower(), Amount, combat.Player);
}

/// <summary>Apply 9 Poison to ALL enemies, then trigger Poison immediately. Cost 3 Rare Skill.
/// Upgrade: +3 Poison. (MegaCrit Outbreak — reworked v0.110.0 from a Power.)</summary>
public sealed class Outbreak : CardModel
{
    public override string Name => "Outbreak";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AllEnemies;
    public int Poison => 9 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.ApplyPower(combat, m, new PoisonPower(), Poison, combat.Player);
        foreach (var m in combat.LivingMonsters.ToList())
            if (m.GetPower("Poison") is PoisonPower p) p.Trigger(combat);
    }
}

/// <summary>Power: whenever you play a card, deal 4 damage to a random enemy. Upgrade: +2. (MegaCrit
/// Serpent Form)</summary>
public sealed class SerpentForm : CardModel
{
    public override string Name => "SerpentForm";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Amount => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new SerpentFormPower(), Amount, combat.Player);
}

/// <summary>Power: Weak enemies take 50% more damage from your Attacks. Cost 2. Upgrade: cost 1.
/// (MegaCrit Tracking — v0.108.0 nerf from double damage.)</summary>
public sealed class Tracking : CardModel
{
    public override string Name => "Tracking";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);   // upgrade: cost 1
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new TrackingPower(), 50, combat.Player);
}

/// <summary>Power: whenever you play a Skill, it gains Sly (HP-neutral — not modelled). Cost 2. Upgrade:
/// cost 1. (MegaCrit Master Planner)</summary>
public sealed class MasterPlanner : CardModel
{
    public override string Name => "MasterPlanner";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new MasterPlannerPower(), 1, combat.Player);
}

/// <summary>Double the Block you gain this turn (×2 per stack). Cost 1. Exhaust? No — discards normally.
/// Upgrade: cost 0. (MegaCrit Shadowmeld)</summary>
public sealed class Shadowmeld : CardModel
{
    public override string Name => "Shadowmeld";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new ShadowmeldPower(), 1, combat.Player);
}

/// <summary>The next Skill you play this turn is played twice. Cost 1. Upgrade: the next 2 Skills.
/// (MegaCrit Burst)</summary>
public sealed class Burst : CardModel
{
    public override string Name => "Burst";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Skills => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new BurstPower(), Skills, combat.Player);
}

/// <summary>Power: add 4 Shivs to your hand. Cost 2. Upgrade: 5 Shivs. (MegaCrit Fan of Knives) The applied
/// FanOfKnivesPower is an inert marker; the Shivs are the effect.</summary>
public sealed class FanOfKnives : CardModel
{
    public override string Name => "FanOfKnives";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Shivs => 4 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, combat.Player, new FanOfKnivesPower(), 1, combat.Player);
        SilentCardHelpers.AddShivsToHand(combat, Shivs);
    }
}

/// <summary>Power: at the end of your turn, you no longer discard your Hand. Cost 2. Upgrade: cost 1.
/// (MegaCrit Well-Laid Plans — reworked v0.109.0, cost 2(1) as of v0.110.0.)</summary>
public sealed class WellLaidPlans : CardModel
{
    public override string Name => "WellLaidPlans";
    public override int BaseCost => 2;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new WellLaidPlansPower(), 1, combat.Player);
}

/// <summary>Power: whenever an ally plays an Attack, gain 1 Block (multiplayer-only — inert in single-player).
/// (Sly: auto-plays for free when discarded mid-turn.) Cost 2. Upgrade: +1. (MegaCrit Sneaky)</summary>
public sealed class Sneaky : CardModel
{
    public override string Name => "Sneaky";
    public override bool IsSly => true;   // Sly: auto-plays for free when discarded (SneakyPower is MP-only ⇒ inert)
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Amount => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new SneakyPower(), Amount, combat.Player);
}

/// <summary>Apply Flanking 2 to an enemy (your allies' attacks deal double to it — multiplayer-only, inert in
/// single-player). Cost 2. Upgrade: cost 1. (MegaCrit Flanking)</summary>
public sealed class Flanking : CardModel
{
    public override string Name => "Flanking";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new FlankingPower(), 2, combat.Player);
    }
}

/// <summary>Add 2 Shivs to your hand (SP: "all players" → you). Cost 2. Upgrade: cost 1. (MegaCrit BladeSymphony.)</summary>
public sealed class BladeSymphony : CardModel
{
    public override string Name => "BladeSymphony";
    public override int BaseCost => 2;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => SilentCardHelpers.AddShivsToHand(combat, 2);
}

/// <summary>Another player's Attacks apply 3 Poison this turn. Inert in single-player. Cost 0. (MegaCrit Concoct.)</summary>
public sealed class Concoct : CardModel
{
    public override string Name => "Concoct";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play) { }
}

/// <summary>Another player gains 6 Dexterity this turn. Inert in single-player. Cost 0. Retain. (MegaCrit Fade.)</summary>
public sealed class Fade : CardModel
{
    public override string Name => "Fade";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override bool Retain => true;
    public override void OnPlay(CombatState combat, CardPlay play) { }
}

/// <summary>Discard your hand. At the start of your next turn, your attacks deal double damage. Cost 1.
/// Upgrade: cost 0. (MegaCrit Shadow Step) The discard is deterministic (cards go to the discard pile); the
/// Sly auto-play-on-discard trigger is not modelled.</summary>
public sealed class ShadowStep : CardModel
{
    public override string Name => "ShadowStep";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        SilentCardHelpers.DiscardWholeHand(combat);
        Cmd.ApplyPower(combat, combat.Player, new ShadowStepPower(), 1, combat.Player);
    }
}

/// <summary>Add 2 Shivs to your hand, each Inky (deal +1 damage, apply 1 Weak on play). Cost 1. Upgrade:
/// 3 Shivs. (MegaCrit Blade of Ink) The Inky enchantment is modelled directly on the generated Shivs.</summary>
public sealed class BladeOfInk : CardModel
{
    public override string Name => "BladeOfInk";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Shivs => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        var p = combat.Player;
        for (int i = 0; i < Shivs; i++)
        {
            var shiv = new Shiv { Inky = true };
            (p.Hand.Count < Player.MaxHandSize ? p.Hand : p.DiscardPile).Add(shiv);
        }
    }
}

// ===========================================================================
// Batch 6 — counter/conditional attacks + Intangible (Wave 3). These read
// per-turn counters on CombatState (Finisher / Memento Mori), the hand size
// (Precise Cut), enemy Poison (Mirage), or kill results (Echoing Slash).
// ===========================================================================

/// <summary>Deal 6 damage for each Attack you've already played this turn (it never counts itself). Upgrade:
/// +2 damage per hit. (MegaCrit Finisher)</summary>
public sealed class Finisher : CardModel
{
    public override string Name => "Finisher";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 6 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int hits = combat.AttacksPlayedThisTurn;   // attacks finished this turn; Finisher itself not yet counted
        if (hits > 0)
            Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, hits, ValueProp.Move, this);
    }
}

/// <summary>Deal (9 + 4 × cards discarded this turn) damage. Upgrade: +2 base, +1 per discard. (MegaCrit
/// Memento Mori) Only mid-turn discards (Storm of Steel / Shadow Step / Calculated Gamble) are counted — the
/// discard decks that would normally feed this are themselves not modelled (see the manifest).</summary>
public sealed class MementoMori : CardModel
{
    public override string Name => "MementoMori";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Base => 9 + 2 * Upgrades;
    public int Per => 4 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Base + Per * combat.CardsDiscardedThisTurn, ValueProp.Move, this);
}

/// <summary>Deal (13 − 2 × cards in your hand) damage. Cost 0. Upgrade: base 16. (MegaCrit Precise Cut)
/// The hand is measured after Precise Cut itself has left it; the attack pipeline floors damage at 0.</summary>
public sealed class PreciseCut : CardModel
{
    public override string Name => "PreciseCut";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Base => 13 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Base - 2 * combat.Player.Hand.Count, ValueProp.Move, this);
}

/// <summary>Gain Block equal to the total Poison on all enemies. Cost 1. Exhaust. Upgrade: loses Exhaust.
/// (MegaCrit Mirage — v0.111.0: exhausts unupgraded; no longer drops cost.)</summary>
public sealed class Mirage : CardModel
{
    public override string Name => "Mirage";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int block = combat.LivingMonsters.Sum(m => m.GetPowerAmount("Poison"));
        if (block > 0) Cmd.GainBlock(combat, combat.Player, block, ValueProp.Move, this);
    }
}

/// <summary>Deal 10 damage to ALL enemies. Whenever this kills an enemy, repeat the attack on all remaining
/// enemies. Upgrade: +3 damage. (MegaCrit Echoing Slash)</summary>
public sealed class EchoingSlash : CardModel
{
    public override string Name => "EchoingSlash";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 10 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int attacks = 1;
        while (attacks > 0)
        {
            attacks--;
            int killed = 0;
            foreach (var m in combat.LivingMonsters.ToList())
            {
                bool aliveBefore = m.IsAlive;
                Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
                if (aliveBefore && !m.IsAlive) killed++;
            }
            attacks += killed;     // each kill triggers another full sweep of the survivors
        }
    }
}

/// <summary>Power: gain 2 Intangible (each instance of HP loss is reduced to 1) and Wraith Form 1 (lose 1
/// Dexterity at the start of each turn). Cost 3. Upgrade: +1 Intangible. (MegaCrit Wraith Form)</summary>
public sealed class WraithForm : CardModel
{
    public override string Name => "WraithForm";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;   // "Ancient" in-game; cosmetic for combat
    public override TargetType Target => TargetType.Self;
    public int Intangible => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, combat.Player, new IntangiblePower(), Intangible, combat.Player);
        Cmd.ApplyPower(combat, combat.Player, new WraithFormPower(), 1, combat.Player);
    }
}

// ===========================================================================
// Batch 7 — remaining feasible cards (Wave 4): a self-cost-reducing Stateful
// card, a cost-zeroing skill, and two whose only unmodelled half is HP-neutral
// (a Sly-grant selection / an out-of-combat card reward).
// ===========================================================================

/// <summary>Deal 10 damage. Exhaust. If this kills the target, gain a card reward (out-of-combat, HP-neutral
/// — not modelled). Upgrade: +5 damage. (MegaCrit The Hunt)</summary>
public sealed class TheHunt : CardModel
{
    public override string Name => "TheHunt";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Damage => 10 + 5 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Gain 7 Block, then give a Skill in your hand Sly (HP-neutral — the Sly selection isn't modelled).
/// Cost 1. Upgrade: +3 Block. (MegaCrit Hand Trick)</summary>
public sealed class HandTrick : CardModel
{
    public override string Name => "HandTrick";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 7 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>This turn, every card you play costs 0; you can't draw additional cards this turn. Cost 3.
/// Upgrade: cost 2. (MegaCrit Bullet Time) Applies <see cref="BulletTimePower"/> + NoDraw.</summary>
public sealed class BulletTime : CardModel
{
    public override string Name => "BulletTime";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, combat.Player, new BulletTimePower(), 1, combat.Player);
        Cmd.ApplyPower(combat, combat.Player, new NoDrawPower(), 1, combat.Player);
    }
}

/// <summary>Add 3 Shivs to your hand. This card's cost is reduced by 1 each time you play it this combat.
/// Cost 2. Upgrade: 4 Shivs. (MegaCrit Up My Sleeve) A <see cref="CardModel.Stateful"/> card: its escalating
/// cost reduction is per-combat mutable state, so it is deep-cloned per search state (like Rampage).</summary>
public sealed class UpMySleeve : CardModel
{
    public override string Name => "UpMySleeve";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override bool Stateful => true;

    private int _costReduction;
    public override int Cost => Math.Max(0, BaseCost - _costReduction);
    public int Shivs => 3 + Upgrades;

    public override void OnPlay(CombatState combat, CardPlay play)
    {
        SilentCardHelpers.AddShivsToHand(combat, Shivs);
        _costReduction++;            // permanent for the combat, cumulative
    }

    // Stateful: the escalating cost is part of the card's identity for hashing/memoisation.
    public override string StateKey()
        => $"UpMySleeve{(Upgrades > 0 ? $"+{Upgrades}" : "")}/r{_costReduction}";
}

// ===========================================================================
// Batch 8 — the formerly-deferred cards, now ported to the project's
// "real with a driver / degrade in pure search" bar. Card SELECTION uses a
// deterministic default (matching Armaments / Burning Pact / Headbutt); draws
// are no-ops without an ambient Rng; the mid-turn-draw triggers are inert in
// pure search. Faithful with a concrete driver / the trace validator.
// ===========================================================================

/// <summary>Play every Shiv in your exhaust pile at the target (deal each Shiv's damage). Cost 2. Upgrade:
/// the exhaust Shivs are upgraded first. (MegaCrit Knife Trap) Fully deterministic — no draw, no selection.
/// Shiv-damage modifiers (Accuracy, Phantom Blades, Strength, Vulnerable) apply via the Shiv card source.</summary>
public sealed class KnifeTrap : CardModel
{
    public override string Name => "KnifeTrap";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        var exhaust = combat.Player.ExhaustPile;
        for (int i = 0; i < exhaust.Count; i++)
        {
            if (exhaust[i] is not Shiv s) continue;
            // Upgrade by REPLACING with a cloned copy — never mutate the shared instance in place.
            if (Upgrades > 0) { s = (Shiv)s.Clone().Upgraded(1); exhaust[i] = s; }
            if (!play.Target!.IsAlive) break;
            Cmd.Attack(combat, combat.Player, play.Target!, s.Damage, ValueProp.Move, s);
        }
    }
}

/// <summary>Choose a card in your hand; at the start of your next turn, add 3 copies of it to your hand.
/// Cost 3. Exhaust. Upgrade: cost 2. (MegaCrit Nightmare) The chosen card is a default (player choice not
/// modelled); the 3 copies are real card generation via <see cref="NightmarePower"/>.</summary>
public sealed class Nightmare : CardModel
{
    public override string Name => "Nightmare";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public int Copies => 3;
    // Which hand card to copy is a real decision node (copy a Strike vs a power vs a Shiv changes next turn).
    public override IEnumerable<string> Choices(CombatState combat) => HandChoices(combat);
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        var chosen = ChosenHandCard(combat, play.ChoiceKey);
        if (chosen == null) return;
        Cmd.ApplyPower(combat, combat.Player, new NightmarePower { Selected = chosen.Clone() }, Copies, combat.Player);
    }
}

/// <summary>Draw 3 cards, then discard 1 of your choice. Cost 1. Upgrade: draw 4. (MegaCrit Acrobatics) In
/// search the draw is a chance node and the discard a post-draw player MAX over the drawn hand
/// (<see cref="PostDrawDiscardCount"/>); with an ambient Rng it draws then discards a heuristic default.</summary>
public sealed class Acrobatics : CardModel
{
    public override string Name => "Acrobatics";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Cards => 3 + Upgrades;
    public override bool HasPostDraw => true;
    public override int PostDrawDiscardCount => 1;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Rng == null) { Cmd.DeferDrawThenResolve(combat, Cards, this); return; }   // search: defer + discard-choice
        int drew = Cmd.Draw(combat, Cards);
        if (drew > 0) SilentCardHelpers.DiscardDefault(combat, 1);
    }
}

/// <summary>Draw 1 card, then discard 1 of your choice. Cost 0. Upgrade: draw 2 / discard 2. (MegaCrit Prepared)
/// In search the draw is a chance node and the discard a post-draw player MAX; with an ambient Rng it draws
/// then discards a heuristic default.</summary>
public sealed class Prepared : CardModel
{
    public override string Name => "Prepared";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Cards => 1 + Upgrades;
    public override bool HasPostDraw => true;
    public override int PostDrawDiscardCount => Cards;
    public override bool LoopRiskDraw => true;   // cost 0 + draws + returns to discard ⇒ replayable
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Rng == null) { Cmd.DeferDrawThenResolve(combat, Cards, this); return; }   // search: defer + discard-choice
        int drew = Cmd.Draw(combat, Cards);
        if (drew > 0) SilentCardHelpers.DiscardDefault(combat, Cards);
    }
}

/// <summary>Discard 2 cards of your choice, then add 2 Shivs to your hand. Cost 0. Upgrade: the Shivs are
/// upgraded. (MegaCrit Hidden Daggers) The discard is a REAL player MAX in search (a discard-of-choice resolved
/// before the Shivs are created, matching the game's order — so the Shivs are never discardable); the Shivs are
/// added as the discard's continuation (<see cref="OnPostDiscard"/>). A concrete Rng discards a default eagerly.</summary>
public sealed class HiddenDaggers : CardModel
{
    public override string Name => "HiddenDaggers";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Shivs => 2;
    // Game order (decompiled): discard 2 of choice FIRST, then create the Shivs — so the Shivs are never in the
    // discard pool. In SEARCH that discard is a real player MAX over the hand (DeferDiscardThenResolve), and the
    // Shiv creation runs as the discard's CONTINUATION (OnPostDiscard) once both discards resolve — sound because
    // the choice set is the pre-Shiv hand exactly as the game offers. With a concrete Rng both resolve eagerly
    // (heuristic default discard), matching the engine's other selection cards.
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Rng == null) { Cmd.DeferDiscardThenResolve(combat, 2, this); return; }
        SilentCardHelpers.DiscardDefault(combat, 2);
        OnPostDiscard(combat);
    }
    public override void OnPostDiscard(CombatState combat)
        => SilentCardHelpers.AddShivsToHand(combat, Shivs, Upgrades > 0 ? 1 : 0);
}

/// <summary>Power: at the start of each of your turns, draw 1 extra card and discard 1 (a default). Cost 1.
/// Upgrade: cost 0. (MegaCrit Tools of the Trade) HP-neutral card filtering via
/// <see cref="ToolsOfTheTradePower"/>.</summary>
public sealed class ToolsOfTheTrade : CardModel
{
    public override string Name => "ToolsOfTheTrade";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new ToolsOfTheTradePower(), 1, combat.Player);
}

/// <summary>Draw 1 card; if it is a Skill, gain 3 Block. Cost 0. Upgrade: +2 Block. (MegaCrit Escape Plan)
/// The drawn card is a chance node: in search the draw is deferred and the conditional Block is applied
/// per-outcome as a post-draw step (<see cref="OnPostDraw"/>); with an ambient Rng it draws+resolves eagerly.</summary>
public sealed class EscapePlan : CardModel
{
    public override string Name => "EscapePlan";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 3 + 2 * Upgrades;
    public override bool HasPostDraw => true;
    public override bool LoopRiskDraw => true;   // cost 0 + draws + returns to discard ⇒ replayable
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Rng == null) { Cmd.DeferDrawThenResolve(combat, 1, this); return; }   // search: defer + post-draw
        int drew = Cmd.Draw(combat, 1);
        OnPostDraw(combat, drew);
    }
    // Block iff the card actually DRAWN is a Skill. The drawn card is the hand's last when drawn > 0; when both
    // piles are empty the draw produces nothing (drawn == 0) — the game's drawn card is null ⇒ no block. Guarding
    // on drawn > 0 (vs reading a pre-existing Hand[^1]) keeps this from optimistically granting block in search.
    public override void OnPostDraw(CombatState combat, int drawn)
    {
        if (drawn > 0 && combat.Player.Hand[^1].Type == CardType.Skill)
            Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
    }
}

/// <summary>Power: whenever you draw a card this turn, apply 2 Poison to ALL enemies (removed at end of turn).
/// Cost 1. Upgrade: +1 Poison. (MegaCrit Corrosive Wave) Real only on mid-turn draws with an ambient Rng;
/// inert in pure search.</summary>
public sealed class CorrosiveWave : CardModel
{
    public override string Name => "CorrosiveWave";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Poison => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new CorrosiveWavePower(), Poison, combat.Player);
}

/// <summary>Power: whenever you draw a card mid-turn, deal 2 damage to ALL enemies. Cost 2. Upgrade: Innate
/// (not modelled). (MegaCrit Speedster) Real only with an ambient Rng; inert in pure search.</summary>
public sealed class Speedster : CardModel
{
    public override string Name => "Speedster";
    public override bool Innate => Upgrades > 0;   // Upgrade: Innate (guaranteed in the opening hand)
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Damage => 2;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new SpeedsterPower(), Damage, combat.Player);
}

/// <summary>Deal (1 + the total number of cards you've drawn this combat) damage. Cost 3. Upgrade: cost 2.
/// (MegaCrit Murder) The draw count is tracked only for decks containing Murder (gated on
/// <see cref="CombatState.TracksCardsDrawn"/>), so it scales in exact search too.</summary>
public sealed class Murder : CardModel
{
    public override string Name => "Murder";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, 1 + combat.CardsDrawnThisCombat, ValueProp.Move, this);
}
