using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Regent character cards (88-card pool). The Regent's mechanics:
//   • Stars  — a secondary resource (Player.Stars). Gained by many cards and
//     spent to play 0-energy star-cost cards; gated + consumed in PlayCard.
//   • Forge → Sovereign Blade — see SovereignBlade.cs.
//   • Parry / Seeking Edge / Sword Sage / Conqueror modify the Sovereign Blade.
//
// Faithful HP-relevant effects are ported directly. Effects that depend on
// subsystems this solver does not model — RNG card generation (colorless / token
// gen, Debris, Minions), card selection/transform, gold, hand-draw-COUNT changes,
// on-draw card hooks, auto-play-from-pile, multiplayer — are ported as the
// HP-faithful subset with the meta-effect documented inline as inert, matching
// the project convention (these are HP-neutral or reconstructed on trace replay).
// Regent-only powers live in RegentPowers.cs; registration in RegentCatalog.cs.
// ===========================================================================

// ---- Starters ----

/// <summary>Deal 6 damage. Upgrade: +3. (MegaCrit StrikeRegent)</summary>
public sealed class StrikeRegent : CardModel
{
    public override string Name => "StrikeRegent";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Damage => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Gain 5 Block. Upgrade: +3. (MegaCrit DefendRegent)</summary>
public sealed class DefendRegent : CardModel
{
    public override string Name => "DefendRegent";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.Self;
    public override bool IsDefend => true;   // Defend tag (Fasten boost)
    public int Block => 5 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Cost 0, 2★. Deal 8 damage, apply 1 Weak + 1 Vulnerable. Upgrade: +4 damage. (MegaCrit FallingStar)</summary>
public sealed class FallingStar : CardModel
{
    public override string Name => "FallingStar";
    public override int BaseCost => 0;
    public override int StarCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 8 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
        {
            Cmd.ApplyPower(combat, play.Target, new WeakPower(), 1, combat.Player);
            Cmd.ApplyPower(combat, play.Target, new VulnerablePower(), 1, combat.Player);
        }
    }
}

/// <summary>Gain 2 stars. Upgrade: +1. (MegaCrit Venerate — Regent starter)</summary>
public sealed class Venerate : CardModel
{
    public override string Name => "Venerate";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Basic;
    public override TargetType Target => TargetType.Self;
    public int Stars => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => Cmd.GainStars(combat, Stars);
}

// ---- Attacks ----

/// <summary>Cost 0, 3★. Deal 6 damage twice to ALL enemies. Upgrade: +2 damage. (MegaCrit AstralPulse)</summary>
public sealed class AstralPulse : CardModel
{
    public override string Name => "AstralPulse";
    public override int BaseCost => 0;
    public override int StarCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 6 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.AttackMulti(combat, combat.Player, m, Damage, 2, ValueProp.Move, this);
    }
}

/// <summary>Deal 5 damage, then Forge 5 × (1 + powered hits the player had already dealt this target this
/// turn). Upgrade: +2 damage, +2 Forge-per-hit. (Game BeatIntoShape: Forge = CalcBase + CalcExtra × hits −
/// thisCard'sHits × CalcExtra = CalcBase + CalcExtra × priorHits, with CalcBase = CalcExtra = 5 (+2/upgrade),
/// where "hits" are powered Move attacks the player dealt this creature this turn. The per-target counter is
/// the engine's <see cref="Creature.PlayerPoweredHitsThisTurn"/>, read BEFORE this card's own hit lands.)</summary>
public sealed class BeatIntoShape : CardModel
{
    public override string Name => "BeatIntoShape";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool TracksTargetPoweredHits => true;
    public int Damage => 5 + 2 * Upgrades;
    public int ForgePerHit => 5 + 2 * Upgrades;   // CalcBase == CalcExtra
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        var target = play.Target!;
        int priorHits = target.PlayerPoweredHitsThisTurn;   // read before this card's hit increments it
        Cmd.Attack(combat, combat.Player, target, Damage, ValueProp.Move, this);
        RegentForge.Forge(combat, ForgePerHit * (1 + priorHits));
    }
}

/// <summary>Exhaust. Deal 18 damage. (Game: once exhausted it auto-plays from the exhaust pile each turn —
/// auto-play-from-pile is not modelled, so this is a one-shot.) Upgrade: +6. (MegaCrit Bombardment)</summary>
public sealed class Bombardment : CardModel
{
    public override string Name => "Bombardment";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Damage => 18 + 6 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 6 damage 3 times. Upgrade: +1 hit. (MegaCrit CelestialMight)</summary>
public sealed class CelestialMight : CardModel
{
    public override string Name => "CelestialMight";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Hits => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.AttackMulti(combat, combat.Player, play.Target!, 6, Hits, ValueProp.Move, this);
}

/// <summary>Deal 11 damage, then add a Debris (a 0-effect status) to hand. (Card generation is inert —
/// Debris does nothing here.) Cost 0. Upgrade: +4 damage. (MegaCrit CollisionCourse)</summary>
public sealed class CollisionCourse : CardModel
{
    public override string Name => "CollisionCourse";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 10 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Cost 0, 5★. Deal 33 damage, apply 3 Weak + 3 Vulnerable. Upgrade: +11 damage. (MegaCrit Comet)</summary>
public sealed class Comet : CardModel
{
    public override string Name => "Comet";
    public override int BaseCost => 0;
    public override int StarCost => 5;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 33 + 11 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
        {
            Cmd.ApplyPower(combat, play.Target, new WeakPower(), 3, combat.Player);
            Cmd.ApplyPower(combat, play.Target, new VulnerablePower(), 3, combat.Player);
        }
    }
}

/// <summary>Deal 21 damage to ALL enemies, then fill hand with Debris (card generation inert). Upgrade:
/// +5 damage. (MegaCrit CrashLanding)</summary>
public sealed class CrashLanding : CardModel
{
    public override string Name => "CrashLanding";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 21 + 5 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
    }
}

/// <summary>Cost 1, 1★. Deal (8 + 2 × number of star-cost cards you own) damage. Upgrade: +1 per-card.
/// (MegaCrit CrescentSpear — "deal damage for each Stars-costing card".)</summary>
public sealed class CrescentSpear : CardModel
{
    public override string Name => "CrescentSpear";
    public override int BaseCost => 1;
    public override int StarCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Per => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        var p = combat.Player;
        // Count star-cost cards the player owns; +1 for this card itself (it is mid-play, in no pile here,
        // but the game counts the card being played).
        int starCards = 1 + p.Hand.Concat(p.DrawPile).Concat(p.DiscardPile).Concat(p.ExhaustPile)
            .Count(c => c.StarCost > 0 || c.IsXStarCost);
        Cmd.Attack(combat, combat.Player, play.Target!, 8 + Per * starCards, ValueProp.Move, this);
    }
}

/// <summary>Deal 7 damage to ALL enemies; they lose 1 Strength until end of their turn. Upgrade: +1 each.
/// (MegaCrit CrushUnder)</summary>
public sealed class CrushUnder : CardModel
{
    public override string Name => "CrushUnder";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 8 + Upgrades;
    public int StrLoss => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        var enemies = combat.LivingMonsters.ToList();
        foreach (var m in enemies) Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
        foreach (var m in enemies) if (m.IsAlive) Cmd.ApplyPower(combat, m, new CrushUnderPower(), StrLoss, combat.Player);
    }
}

/// <summary>Cost 1, 4★. Deal 30 damage. Upgrade: +10. (MegaCrit Devastate)</summary>
public sealed class Devastate : CardModel
{
    public override string Name => "Devastate";
    public override int BaseCost => 1;
    public override int StarCost => 4;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 35 + 10 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Ethereal. Deal 9 damage to ALL enemies; they lose 9 Strength until end of their turn. Upgrade:
/// +2 each. (MegaCrit DyingStar)</summary>
public sealed class DyingStar : CardModel
{
    public override string Name => "DyingStar";
    public override int BaseCost => 1;
    public override int StarCost => 3;   // game: CanonicalStarCost 3 — gates the 9-AoE + 9-Strength-strip
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AllEnemies;
    public override bool Ethereal => true;
    public int Damage => 9 + 2 * Upgrades;
    public int StrLoss => 9 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        var enemies = combat.LivingMonsters.ToList();
        foreach (var m in enemies) Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
        foreach (var m in enemies) if (m.IsAlive) Cmd.ApplyPower(combat, m, new DyingStarPower(), StrLoss, combat.Player);
    }
}

/// <summary>Cost 0, X-energy. Deal 8 damage X times (X = energy spent); if X ≥ 4, hit twice as many times.
/// Upgrade: +2 damage. (MegaCrit HeavenlyDrill)</summary>
public sealed class HeavenlyDrill : CardModel
{
    public override string Name => "HeavenlyDrill";
    public override int BaseCost => 0;
    public override bool IsXCost => true;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 8 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int hits = play.XValue;
        if (hits >= 4) hits *= 2;
        Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, hits, ValueProp.Move, this);
    }
}

/// <summary>Deal 15 damage, gain 2 energy next turn. Upgrade: +3 damage, +1 energy. (MegaCrit Hegemony)</summary>
public sealed class Hegemony : CardModel
{
    public override string Name => "Hegemony";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 15 + 3 * Upgrades;
    public int Energy => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new EnergyNextTurnPower(), Energy, combat.Player);
    }
}

/// <summary>Deal 20 damage, then generate a colorless copy of a card in hand (generation inert). Upgrade:
/// +5 damage. (MegaCrit HeirloomHammer)</summary>
public sealed class HeirloomHammer : CardModel
{
    public override string Name => "HeirloomHammer";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 20 + 5 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 27 damage. (Game: when drawn, costs 1 less this combat — on-draw hooks are not modelled,
/// so the cost is fixed.) Upgrade: +8. (MegaCrit KinglyKick)</summary>
public sealed class KinglyKick : CardModel
{
    public override string Name => "KinglyKick";
    public override int BaseCost => 4;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 27 + 8 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 8 damage. (Game: each time it is drawn it permanently gains +4 damage — on-draw hooks are
/// not modelled, so damage is fixed at base.) Upgrade: +2 damage. (MegaCrit KinglyPunch)</summary>
public sealed class KinglyPunch : CardModel
{
    public override string Name => "KinglyPunch";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 8 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 30 damage. If it kills the target, gain 5 stars. Upgrade: +8 damage. (MegaCrit KnockoutBlow)</summary>
public sealed class KnockoutBlow : CardModel
{
    public override string Name => "KnockoutBlow";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 30 + 8 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        bool wasAlive = play.Target!.IsAlive;
        Cmd.Attack(combat, combat.Player, play.Target, Damage, ValueProp.Move, this);
        if (wasAlive && !play.Target.IsAlive) Cmd.GainStars(combat, 5);
    }
}

/// <summary>Cost 0. Deal 4 damage once per Skill you have played this turn, to ALL enemies. Upgrade: +1
/// damage. (MegaCrit LunarBlast)</summary>
public sealed class LunarBlast : CardModel
{
    public override string Name => "LunarBlast";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 4 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.AttackMulti(combat, combat.Player, play.Target!, Damage, combat.SkillsPlayedThisTurn, ValueProp.Move, this);
}

/// <summary>Cost 0. Deal 6 damage. (Game: returns to hand on every 3rd Skill played — card-return is not
/// modelled, so this is one-shot.) Upgrade: +3. (MegaCrit MakeItSo)</summary>
public sealed class MakeItSo : CardModel
{
    public override string Name => "MakeItSo";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Cost 0, 2★. Deal 14 damage to ALL enemies, apply 2 Weak + 2 Vulnerable to all. Upgrade: +7
/// damage. (MegaCrit MeteorShower — Ancient)</summary>
public sealed class MeteorShower : CardModel
{
    public override string Name => "MeteorShower";
    public override int BaseCost => 0;
    public override int StarCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Ancient;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 14 + 7 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        var enemies = combat.LivingMonsters.ToList();
        foreach (var m in enemies) Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
        foreach (var m in enemies) if (m.IsAlive)
        {
            Cmd.ApplyPower(combat, m, new WeakPower(), 2, combat.Player);
            Cmd.ApplyPower(combat, m, new VulnerablePower(), 2, combat.Player);
        }
    }
}

/// <summary>Cost 0. Deal 5 damage X times (X = stars spent — all your stars), each to a random enemy.
/// Upgrade: +2 damage. (MegaCrit Stardust)</summary>
public sealed class Stardust : CardModel
{
    public override string Name => "Stardust";
    public override int BaseCost => 0;
    public override bool IsXStarCost => true;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.RandomEnemy;
    public int Damage => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        for (int i = 0; i < play.StarsSpent; i++)
        {
            var living = combat.LivingMonsters.ToList();
            if (living.Count == 0) break;
            var t = combat.Rng != null ? living[combat.Rng.NextInt(living.Count)] : living[0];
            Cmd.Attack(combat, combat.Player, t, Damage, ValueProp.Move, this);
        }
    }
}

/// <summary>Cost 0. Deal 3 damage to ALL enemies once per star gained this turn. Upgrade: +1 damage.
/// (MegaCrit Radiate)</summary>
public sealed class Radiate : CardModel
{
    public override string Name => "Radiate";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AllEnemies;
    public int Damage => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int hits = combat.StarsGainedThisTurn;
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.AttackMulti(combat, combat.Player, m, Damage, hits, ValueProp.Move, this);
    }
}

/// <summary>Cost 1, 3★. Gain 1 Strength; ALL enemies lose 1 Strength. Upgrade: +1 self Strength. (MegaCrit
/// Resonance)</summary>
public sealed class Resonance : CardModel
{
    public override string Name => "Resonance";
    public override int BaseCost => 1;
    public override int StarCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AllEnemies;
    public int Str => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, combat.Player, new StrengthPower(), Str, combat.Player);
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.ApplyPower(combat, m, new StrengthPower(), -1, combat.Player);
    }
}

/// <summary>Deal 7 damage 7 times to ALL enemies. Upgrade: cost -1. (MegaCrit SevenStars)</summary>
public sealed class SevenStars : CardModel
{
    public override string Name => "SevenStars";
    public override int BaseCost => 2;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override int StarCost => 7;   // game: CanonicalStarCost 7 — gates the 7x7-to-all AoE
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AllEnemies;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.AttackMulti(combat, combat.Player, m, 7, 7, ValueProp.Move, this);
    }
}

/// <summary>Deal 8 damage, gain 2 stars, then return this card to the draw pile (replayable). Upgrade: +3
/// damage. (MegaCrit ShiningStrike)</summary>
public sealed class ShiningStrike : CardModel
{
    public override string Name => "ShiningStrike";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public override CardResultPile ResultPile => CardResultPile.Removed;   // re-added to draw pile in OnPlay
    public int Damage => 8 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.GainStars(combat, 2);
        combat.Player.DrawPile.Add(this);
    }
}

/// <summary>Deal 9 damage, gain 1 star. Upgrade: +1 damage, +1 star. (MegaCrit SolarStrike)</summary>
public sealed class SolarStrike : CardModel
{
    public override string Name => "SolarStrike";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Damage => 9 + Upgrades;
    public int Stars => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.GainStars(combat, Stars);
    }
}

/// <summary>Deal (5 + 3 × cards generated this combat) damage. Generation is not modelled (count 0), so this
/// deals its base. Upgrade: +1 per-card. (MegaCrit Supermassive)</summary>
public sealed class Supermassive : CardModel
{
    public override string Name => "Supermassive";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, 5, ValueProp.Move, this);
}

/// <summary>Deal 7 damage, Forge 7. Upgrade: +2 each. (MegaCrit WroughtInWar)</summary>
public sealed class WroughtInWar : CardModel
{
    public override string Name => "WroughtInWar";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 7 + 2 * Upgrades;
    public int Forge => 7 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        RegentForge.Forge(combat, Forge);
    }
}

/// <summary>Cost 0, 3★. Deal 13 damage, apply 2 Weak + 2 Vulnerable. Upgrade: +5 damage. (MegaCrit GammaBlast)</summary>
public sealed class GammaBlast : CardModel
{
    public override string Name => "GammaBlast";
    public override int BaseCost => 0;
    public override int StarCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 13 + 5 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        if (play.Target!.IsAlive)
        {
            Cmd.ApplyPower(combat, play.Target, new WeakPower(), 2, combat.Player);
            Cmd.ApplyPower(combat, play.Target, new VulnerablePower(), 2, combat.Player);
        }
    }
}

/// <summary>Cost 1, 2★. Deal 12 damage, draw 2. Upgrade: +1 damage, +1 draw. (MegaCrit GuidingStar)</summary>
public sealed class GuidingStar : CardModel
{
    public override string Name => "GuidingStar";
    public override int BaseCost => 1;
    public override int StarCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 12 + Upgrades;
    public int Cards => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new DrawCardsNextTurnPower(), Cards, combat.Player);
    }
}

// ---- Block / skills ----

/// <summary>Cost 0, 1★. Gain 7 Block. Upgrade: +3. (MegaCrit CloakOfStars)</summary>
public sealed class CloakOfStars : CardModel
{
    public override string Name => "CloakOfStars";
    public override int BaseCost => 0;
    public override int StarCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 7 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Gain 6 Block, then move a card from your discard to the top of the draw pile (HP-neutral —
/// the put-back is not modelled). Upgrade: +3 Block. (MegaCrit CosmicIndifference)</summary>
public sealed class CosmicIndifference : CardModel
{
    public override string Name => "CosmicIndifference";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Gain 12 Block, Forge 10. Upgrade: +3 each. (MegaCrit Bulwark)</summary>
public sealed class Bulwark : CardModel
{
    public override string Name => "Bulwark";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 12 + 3 * Upgrades;
    public int Forge => 10 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        RegentForge.Forge(combat, Forge);
    }
}

/// <summary>Gain 8 Block, gain 1 star. Upgrade: +3 Block. (MegaCrit GatherLight)</summary>
public sealed class GatherLight : CardModel
{
    public override string Name => "GatherLight";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 8 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.GainStars(combat, 1);
    }
}

/// <summary>Gain 11 Block, and 5 Block next turn. Upgrade: +2 each. (MegaCrit Glitterstream)</summary>
public sealed class Glitterstream : CardModel
{
    public override string Name => "Glitterstream";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 11 + 2 * Upgrades;
    public int NextTurn => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new BlockNextTurnPower(), NextTurn, combat.Player);
    }
}

/// <summary>Gain 10 Block. (Game: if on top of the draw pile it auto-plays — not modelled.) Upgrade: +3.
/// (MegaCrit IAmInvincible)</summary>
public sealed class IAmInvincible : CardModel
{
    public override string Name => "IAmInvincible";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Block => 10 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Gain 7 Block, then generate a colorless card to hand (generation inert). Upgrade: +1 Block.
/// (MegaCrit ManifestAuthority)</summary>
public sealed class ManifestAuthority : CardModel
{
    public override string Name => "ManifestAuthority";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 7 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Cost 0, 2★. Gain 9 Block, then return to hand (replayable). Upgrade: +3 Block. (MegaCrit
/// ParticleWall)</summary>
public sealed class ParticleWall : CardModel
{
    public override string Name => "ParticleWall";
    public override int BaseCost => 0;
    public override int StarCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Removed;   // re-added to hand in OnPlay
    public int Block => 9 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        if (!combat.Player.Hand.Contains(this)) combat.Player.Hand.Add(this);
    }
}

/// <summary>Gain 8 Block, gain 2 Vigor (next attack +2 damage). Upgrade: +2 Block, +1 Vigor. (MegaCrit Patter)</summary>
public sealed class Patter : CardModel
{
    public override string Name => "Patter";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 8 + 2 * Upgrades;
    public int Vigor => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new VigorPower(), Vigor, combat.Player);
    }
}

/// <summary>Deal 10 damage, draw 1, put 1 card from hand on top of draw (put-back HP-neutral, not modelled).
/// Upgrade: +3 damage, +1 draw. (MegaCrit PhotonCut)</summary>
public sealed class PhotonCut : CardModel
{
    public override string Name => "PhotonCut";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 10 + 3 * Upgrades;
    public int Cards => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.Draw(combat, Cards);
    }
}

/// <summary>Cost 1, 3★. Gain 16 Block; when you block a powered attack, deal the blocked amount back
/// (thorns-on-block — offensive only, not modelled). Upgrade: +4 Block. (MegaCrit Reflect)</summary>
public sealed class Reflect : CardModel
{
    public override string Name => "Reflect";
    public override int BaseCost => 1;
    public override int StarCost => 3;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 16 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new ReflectPower(), 1, combat.Player);
    }
}

/// <summary>Draw 6 cards. Upgrade: +3. (MegaCrit Prophesize)</summary>
public sealed class Prophesize : CardModel
{
    public override string Name => "Prophesize";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Cards => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => Cmd.Draw(combat, Cards);
}

/// <summary>Draw 3 cards, then put 1 back on top (put-back HP-neutral, not modelled). Upgrade: +1 draw.
/// (MegaCrit Glimmer)</summary>
public sealed class Glimmer : CardModel
{
    public override string Name => "Glimmer";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Cards => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => Cmd.Draw(combat, Cards);
}

/// <summary>Gain 1 star, draw 1, draw 1 more next turn (next-turn draw HP-neutral). Upgrade: +1 star.
/// (MegaCrit Glow)</summary>
public sealed class Glow : CardModel
{
    public override string Name => "Glow";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Stars => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainStars(combat, Stars);
        Cmd.Draw(combat, 1);
        Cmd.ApplyPower(combat, combat.Player, new DrawCardsNextTurnPower(), 1, combat.Player);
    }
}

/// <summary>Gain 2 energy. Cost 0, 3★. Upgrade: +1 energy. (MegaCrit Alignment)</summary>
public sealed class Alignment : CardModel
{
    public override string Name => "Alignment";
    public override int BaseCost => 0;
    public override int StarCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Energy => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => Cmd.GainEnergy(combat, Energy);
}

/// <summary>Forge 3, apply Conqueror to an enemy (your Sovereign Blade deals ×2 to it). Upgrade: +2 Forge.
/// (MegaCrit Conqueror)</summary>
public sealed class Conqueror : CardModel
{
    public override string Name => "Conqueror";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Forge => 3 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        RegentForge.Forge(combat, Forge);
        if (play.Target!.IsAlive) Cmd.ApplyPower(combat, play.Target, new ConquerorPower(), 1, combat.Player);
    }
}

/// <summary>Retain hand next turn (not modelled); gain 1 energy + 1 star next turn. Upgrade: +1 star.
/// (MegaCrit Convergence)</summary>
public sealed class Convergence : CardModel
{
    public override string Name => "Convergence";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Stars => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, combat.Player, new RetainHandPower(), 1, combat.Player);
        Cmd.ApplyPower(combat, combat.Player, new EnergyNextTurnPower(), 1, combat.Player);
        Cmd.ApplyPower(combat, combat.Player, new StarNextTurnPower(), Stars, combat.Player);
    }
}

/// <summary>Cost 0, 6★. Exhaust. Draw 3 cards, then play a chosen Skill 3 times (selection + auto-play not
/// modelled — the draw is faithful). Upgrade: +2 draw. (MegaCrit DecisionsDecisions)</summary>
public sealed class DecisionsDecisions : CardModel
{
    public override string Name => "DecisionsDecisions";
    public override int BaseCost => 0;
    public override int StarCost => 6;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Cards => 3 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => Cmd.Draw(combat, Cards);
}

/// <summary>Apply 1 Weak + 1 Vulnerable to an enemy. Exhaust (removed on upgrade). Cost 0. (MegaCrit
/// KnowThyPlace)</summary>
public sealed class KnowThyPlace : CardModel
{
    public override string Name => "KnowThyPlace";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, play.Target!, new WeakPower(), 1, combat.Player);
        Cmd.ApplyPower(combat, play.Target!, new VulnerablePower(), 1, combat.Player);
    }
}

/// <summary>Apply 6 Vigor (your next attack deals +6). Upgrade: +2. (MegaCrit Terraforming)</summary>
public sealed class Terraforming : CardModel
{
    public override string Name => "Terraforming";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Vigor => 7 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new VigorPower(), Vigor, combat.Player);
}

/// <summary>Gain 1 star, gain 3 stars next turn. Upgrade: +1 next-turn. (MegaCrit HiddenCache)</summary>
public sealed class HiddenCache : CardModel
{
    public override string Name => "HiddenCache";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int NextTurn => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainStars(combat, 1);
        Cmd.ApplyPower(combat, combat.Player, new StarNextTurnPower(), NextTurn, combat.Player);
    }
}

/// <summary>Forge 9, gain 1 energy next turn. Upgrade: +4 Forge. (MegaCrit RefineBlade)</summary>
public sealed class RefineBlade : CardModel
{
    public override string Name => "RefineBlade";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Forge => 8 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        RegentForge.Forge(combat, Forge);
        Cmd.ApplyPower(combat, combat.Player, new EnergyNextTurnPower(), 1, combat.Player);
    }
}

/// <summary>Forge 5, draw 2. Upgrade: +3 Forge. (MegaCrit SpoilsOfBattle)</summary>
public sealed class SpoilsOfBattle : CardModel
{
    public override string Name => "SpoilsOfBattle";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Forge => 6 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        RegentForge.Forge(combat, Forge);
        Cmd.Draw(combat, 2);
    }
}

/// <summary>Apply Seeking Edge (your Sovereign Blade hits all enemies), Forge 7. Upgrade: +4 Forge.
/// (MegaCrit SeekingEdge)</summary>
public sealed class SeekingEdge : CardModel
{
    public override string Name => "SeekingEdge";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Forge => 7 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, combat.Player, new SeekingEdgePower(), 1, combat.Player);
        RegentForge.Forge(combat, Forge);
    }
}

/// <summary>Move all your Sovereign Blades (outside hand) into your hand, then Forge 8. Upgrade: +3 Forge.
/// (MegaCrit SummonForth)</summary>
public sealed class SummonForth : CardModel
{
    public override string Name => "SummonForth";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Forge => 8 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        var p = combat.Player;
        var moved = p.DrawPile.OfType<SovereignBlade>().Concat(p.DiscardPile.OfType<SovereignBlade>())
            .Concat(p.ExhaustPile.OfType<SovereignBlade>()).Cast<CardModel>().ToList();
        foreach (var b in moved)
        {
            p.DrawPile.Remove(b); p.DiscardPile.Remove(b); p.ExhaustPile.Remove(b);
            p.Hand.Add(b);
        }
        RegentForge.Forge(combat, Forge);
    }
}

/// <summary>Cost 1, 4★. Forge 30. Upgrade: +10 Forge. (MegaCrit TheSmith)</summary>
public sealed class TheSmith : CardModel
{
    public override string Name => "TheSmith";
    public override int BaseCost => 1;
    public override int StarCost => 4;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Forge => 30 + 10 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => RegentForge.Forge(combat, Forge);
}

/// <summary>Cost 0, 5★. Exhaust. Gain 9 stars. Upgrade: gains Retain (not separately modelled). (MegaCrit
/// RoyalGamble)</summary>
public sealed class RoyalGamble : CardModel
{
    public override string Name => "RoyalGamble";
    public override int BaseCost => 0;
    public override int StarCost => 5;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override bool Retain => Upgrades > 0;
    public override void OnPlay(CombatState combat, CardPlay play) => Cmd.GainStars(combat, 9);
}

// ---- Power cards ----

/// <summary>Power: gain 1 Strength each time a card is generated into combat (generation inert). Upgrade:
/// Innate (guaranteed in the opening hand). (MegaCrit Arsenal)</summary>
public sealed class Arsenal : CardModel
{
    public override string Name => "Arsenal";
    public override bool Innate => Upgrades > 0;   // Upgrade: Innate (guaranteed in the opening hand)
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new ArsenalPower(), 1, combat.Player);
}

/// <summary>Power: whenever you gain or spend stars, deal 3 damage to ALL enemies. Upgrade: +1. (MegaCrit
/// BlackHole)</summary>
public sealed class BlackHole : CardModel
{
    public override string Name => "BlackHole";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new BlackHolePower(), Amount, combat.Player);
}

/// <summary>Power: whenever you spend stars, gain 2 × (stars spent) Block. Upgrade: +1. (MegaCrit
/// ChildOfTheStars)</summary>
public sealed class ChildOfTheStars : CardModel
{
    public override string Name => "ChildOfTheStars";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new ChildOfTheStarsPower(), Amount, combat.Player);
}

/// <summary>Power: at the start of each turn, Forge 5. Upgrade: +2. (MegaCrit Furnace)</summary>
public sealed class Furnace : CardModel
{
    public override string Name => "Furnace";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 5 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new FurnacePower(), Amount, combat.Player);
}

/// <summary>Power: at the start of each turn, gain 2 stars. Upgrade: +1. (MegaCrit Genesis)</summary>
public sealed class Genesis : CardModel
{
    public override string Name => "Genesis";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Amount => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new GenesisPower(), Amount, combat.Player);
}

/// <summary>Power: whenever you play a card, gain 1 Strength; at end of turn, lose all Strength gained this
/// way. Upgrade: Retain (not separately modelled). (MegaCrit Monologue)</summary>
public sealed class Monologue : CardModel
{
    public override string Name => "Monologue";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override bool Retain => Upgrades > 0;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new MonologuePower(), 1, combat.Player);
}

/// <summary>Power: whenever you deal a powered attack, the enemy permanently loses 1 Strength. Upgrade:
/// cost -1. (MegaCrit MonarchsGaze)</summary>
public sealed class MonarchsGaze : CardModel
{
    public override string Name => "MonarchsGaze";
    public override int BaseCost => 3;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new MonarchsGazePower(), 1, combat.Player);
}

/// <summary>Cost 1, 5★. Power: gain 8 Plating (block at end of each turn, decaying). Upgrade: +3. (MegaCrit
/// NeutronAegis)</summary>
public sealed class NeutronAegis : CardModel
{
    public override string Name => "NeutronAegis";
    public override int BaseCost => 1;
    public override int StarCost => 5;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Amount => 8 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new PlatingPower(), Amount, combat.Player);
}

/// <summary>Power: gain 1 energy for every 4 energy you spend. Upgrade: cost -1. (MegaCrit Orbit)</summary>
public sealed class Orbit : CardModel
{
    public override string Name => "Orbit";
    public override int BaseCost => 2;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new OrbitPower(), 1, combat.Player);
}

/// <summary>Power: if you played ≥5 cards last turn, draw 1 extra (draw-count change not modelled).
/// Upgrade: +1. (MegaCrit PaleBlueDot)</summary>
public sealed class PaleBlueDot : CardModel
{
    public override string Name => "PaleBlueDot";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new PaleBlueDotPower(), Amount, combat.Player);
}

/// <summary>Power: your Sovereign Blade gains Block equal to your Parry when played. Gain 10 Parry. Upgrade:
/// +4. (MegaCrit Parry)</summary>
public sealed class Parry : CardModel
{
    public override string Name => "Parry";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 10 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new ParryPower(), Amount, combat.Player);
}

/// <summary>Power: gain 3 Block each time a card is generated into combat (generation inert). Upgrade: +1.
/// (MegaCrit PillarOfCreation)</summary>
public sealed class PillarOfCreation : CardModel
{
    public override string Name => "PillarOfCreation";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new PillarOfCreationPower(), Amount, combat.Player);
}

/// <summary>Power: gain 30 gold at combat end (gold out of scope, inert). Upgrade: +10. (MegaCrit Royalties)</summary>
public sealed class Royalties : CardModel
{
    public override string Name => "Royalties";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Amount => 30 + 10 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new RoyaltiesPower(), Amount, combat.Player);
}

/// <summary>Power: at the start of each turn, add 1 generated colorless card to hand (generation inert).
/// Upgrade: cost -1. (MegaCrit SpectrumShift)</summary>
public sealed class SpectrumShift : CardModel
{
    public override string Name => "SpectrumShift";
    public override int BaseCost => 2;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 1;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new SpectrumShiftPower(), Amount, combat.Player);
}

/// <summary>Power: your Sovereign Blade replays 1 extra time. Upgrade: cost -1. (MegaCrit SwordSage)</summary>
public sealed class SwordSage : CardModel
{
    public override string Name => "SwordSage";
    public override int BaseCost => 2;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new SwordSagePower(), 1, combat.Player);
}

/// <summary>Cost 1, 3★. Power (Ancient): whenever you play a card, gain 1 star. Upgrade: cost -1. (MegaCrit
/// TheSealedThrone)</summary>
public sealed class TheSealedThrone : CardModel
{
    public override string Name => "TheSealedThrone";
    public override int BaseCost => 1;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override int StarCost => 3;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Ancient;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new TheSealedThronePower(), 1, combat.Player);
}

/// <summary>Power: each turn draw more cards and exhaust cards (draw-count/forced-exhaust not modelled).
/// Upgrade: Innate (guaranteed in the opening hand). (MegaCrit Tyranny)</summary>
public sealed class Tyranny : CardModel
{
    public override string Name => "Tyranny";
    public override bool Innate => Upgrades > 0;   // Upgrade: Innate (guaranteed in the opening hand)
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new TyrannyPower(), 1, combat.Player);
}

/// <summary>Ethereal. Power: the first 2 cards you play each turn cost 0, AND playing it ENDS your turn
/// (PlayerCmd.EndTurn, canBackOut:false). Both modelled 1:1: <see cref="VoidFormPower"/> applies the per-turn
/// cost discount, and we set <see cref="CombatState.PlayerTurnEndForced"/> so the search ends the turn after
/// this play (exact: ContinuePlay routes to the end-turn transition; MCTS: the resulting decision node opens
/// only the EndTurn edge). Modelling the free-cards benefit WITHOUT the end-turn would be optimistic, so the two
/// are inseparable. Upgrade: loses Ethereal. (MegaCrit VoidForm)</summary>
public sealed class VoidForm : CardModel
{
    public override string Name => "VoidForm";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override bool Ethereal => Upgrades == 0;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, combat.Player, new VoidFormPower(), 2, combat.Player);
        combat.PlayerTurnEndForced = true;   // VoidForm ends your turn on play
    }
}

/// <summary>Cost 0. Exhaust. Draw 1, gain 1 star, gain 1 energy, Forge 5. Upgrade: Innate (guaranteed in the opening hand).
/// (MegaCrit BigBang)</summary>
public sealed class BigBang : CardModel
{
    public override string Name => "BigBang";
    public override bool Innate => Upgrades > 0;   // Upgrade: Innate (guaranteed in the opening hand)
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Draw(combat, 1);
        Cmd.GainStars(combat, 1);
        Cmd.GainEnergy(combat, 1);
        RegentForge.Forge(combat, 5);
    }
}

/// <summary>Power: at next hand draw, choose 2 cards from the draw pile to draw (selection HP-neutral, not
/// modelled). Upgrade: +1. (MegaCrit ForegoneConclusion)</summary>
public sealed class ForegoneConclusion : CardModel
{
    public override string Name => "ForegoneConclusion";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Amount => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new ForegoneConclusionPower(), Amount, combat.Player);
}

// ---- Card-generation / selection / transform / multiplayer (HP-neutral; inert) ----

/// <summary>Transform a hand card into a MinionStrike (card transform not modelled — no HP effect).
/// Upgrade: the created card is upgraded. (MegaCrit Begone)</summary>
public sealed class Begone : CardModel
{
    public override string Name => "Begone";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play) { /* card transform — inert */ }
}

/// <summary>Generate 3 colorless cards to hand (generation not modelled). Exhaust. Upgrade: +1. (MegaCrit
/// BundleOfJoy)</summary>
public sealed class BundleOfJoy : CardModel
{
    public override string Name => "BundleOfJoy";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) { /* card generation — inert */ }
}

/// <summary>Transform draw-pile cards into MinionDiveBombs (transform not modelled). (MegaCrit Charge)</summary>
public sealed class Charge : CardModel
{
    public override string Name => "Charge";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play) { /* card transform — inert */ }
}

/// <summary>Transform hand cards into MinionSacrifices (transform not modelled). Exhaust. (MegaCrit Guards)</summary>
public sealed class Guards : CardModel
{
    public override string Name => "Guards";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) { /* card transform — inert */ }
}

/// <summary>Cost 0, 2★. Generate and choose 1 of 3 colorless cards to hand (generation not modelled).
/// (MegaCrit Quasar)</summary>
public sealed class Quasar : CardModel
{
    public override string Name => "Quasar";
    public override int BaseCost => 0;
    public override int StarCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play) { /* card generation — inert */ }
}

/// <summary>Multiplayer-only: give an ally a generated colorless card. Inert in single-player. (MegaCrit
/// Largesse)</summary>
public sealed class Largesse : CardModel
{
    public override string Name => "Largesse";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play) { /* multiplayer-only — inert */ }
}

/// <summary>Multiplayer-only power. Inert in single-player. Upgrade: cost -1. (MegaCrit HammerTime)</summary>
public sealed class HammerTime : CardModel
{
    public override string Name => "HammerTime";
    public override int BaseCost => 2;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play) { /* multiplayer-only — inert */ }
}

/// <summary>Next turn, draw 2. Cost 1. Upgrade: draw 3. (MegaCrit Plot — SP: you are "all players".)</summary>
public sealed class Plot : CardModel
{
    public override string Name => "Plot";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Cards => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new DrawCardsNextTurnPower(), Cards, combat.Player);
}

/// <summary>Draw 1, gain 1 Energy, gain 9 Block. Cost 0. Upgrade: +3 Block. (MegaCrit Constellation —
/// SP: you are the "other player".)</summary>
public sealed class Constellation : CardModel
{
    public override string Name => "Constellation";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 9 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Draw(combat, 1);
        Cmd.GainEnergy(combat, 1);
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
    }
}

/// <summary>Another player chooses a card from their draw pile. Inert in single-player. Cost 1.
/// Upgrade: cost 0. (MegaCrit Tutor.)</summary>
public sealed class Tutor : CardModel
{
    public override string Name => "Tutor";
    public override int BaseCost => 1;
    public override int Cost => Math.Max(0, BaseCost - Upgrades);
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play) { }
}
