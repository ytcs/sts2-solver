using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Colorless cards (the cross-character pool: Apotheosis, Mind Blast, Flash of
// Steel, …). Shared status cards live in Core/StatusCards.cs; generic keyword
// powers (Strength, Weak, …) in Core/CommonPowers.cs; Colorless-only powers in
// ColorlessPowers.cs; registration in ColorlessCatalog.cs.
//
// Faithfulness: cards whose only unported half is RNG card-generation, an
// in-game selection prompt, or a meta (gold/relic) reward are either ported as
// their HP-faithful subset or skipped, with the reason documented inline.
// ===========================================================================

// --------------------------------------------------------------------------
// Attacks
// --------------------------------------------------------------------------

/// <summary>Deal 5 damage. Draw 1 card. Cost 0. Upgrade: +2 damage. (MegaCrit FlashOfSteel)</summary>
public sealed class FlashOfSteel : CardModel
{
    public override string Name => "FlashOfSteel";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 5 + 2 * Upgrades;
    public int Cards => 1;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.Draw(combat, Cards);
    }
}

/// <summary>Deal 11 damage to ALL enemies. Exhaust. Innate. Upgrade: +4. (Innate is not modelled — it only
/// affects the opening hand.) (MegaCrit DramaticEntrance)</summary>
public sealed class DramaticEntrance : CardModel
{
    public override string Name => "DramaticEntrance";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AllEnemies;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Damage => 11 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, combat.Player, m, Damage, ValueProp.Move, this);
    }
}

/// <summary>Deal damage equal to the number of cards in your draw pile. Innate. Upgrade: costs 0. (Innate is
/// not modelled — it only affects the opening hand.) (MegaCrit MindBlast)</summary>
public sealed class MindBlast : CardModel
{
    public override string Name => "MindBlast";
    public override int BaseCost => Upgrades > 0 ? 0 : 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, combat.Player.DrawPile.Count, ValueProp.Move, this);
}

/// <summary>Deal 20 damage. If this kills the target, gain 20 Gold. Cost 2. Upgrade: +5 damage. (Gold is a
/// meta reward — ignored in combat; only the damage is modelled.) (MegaCrit HandOfGreed)</summary>
public sealed class HandOfGreed : CardModel
{
    public override string Name => "HandOfGreed";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 20 + 5 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 14 damage. Can only be played if every other card in your hand is an Attack. Cost 0.
/// Upgrade: +4 damage. (The play-restriction is HP-neutral — it never changes the effect, only whether the
/// card is legal to play; the autopilot/recorder only ever feeds legal plays, so only the damage is
/// modelled here.) (MegaCrit Clash)</summary>
public sealed class Clash : CardModel
{
    public override string Name => "Clash";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 14 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

// --------------------------------------------------------------------------
// Skills — damage-faithful (block / draw / debuff)
// --------------------------------------------------------------------------

/// <summary>Gain 4 Block. Draw 1 card. Cost 0. Upgrade: +2 Block. (MegaCrit Finesse)</summary>
public sealed class Finesse : CardModel
{
    public override string Name => "Finesse";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Block => 4 + 2 * Upgrades;
    public int Cards => 1;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.Draw(combat, Cards);
    }
}

/// <summary>Reduce the target enemy's Strength by 9 until its turn ends, weakening its upcoming attack.
/// Exhaust. Cost 0. Upgrade: +6. (Same temporary-Strength mechanic as Piercing Wail / Mangle.) (MegaCrit
/// DarkShackles)</summary>
public sealed class DarkShackles : CardModel
{
    public override string Name => "DarkShackles";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int StrengthLoss => 9 + 6 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (play.Target!.IsAlive)
            Cmd.ApplyPower(combat, play.Target!, new DarkShacklesPower(), StrengthLoss, combat.Player);
    }
}

/// <summary>Draw 3 cards. Exhaust. Cost 0. Upgrade: costs nothing extra but draws +1 (4). (MegaCrit
/// MasterOfStrategy — Draw is only real with an ambient Rng, otherwise replayed from the trace.)</summary>
public sealed class MasterOfStrategy : CardModel
{
    public override string Name => "MasterOfStrategy";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Cards => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Draw(combat, Cards);
}

/// <summary>Draw 2 cards, then put a card from your hand on top of your draw pile. Exhaust. Cost 0. Upgrade:
/// +1 card. (The "put back" half is an HP-neutral selection prompt — for replay it returns an arbitrary hand
/// card to the draw pile top; validation checks only HP-affecting state. The draw is real only under an
/// ambient Rng.) (MegaCrit ThinkingAhead)</summary>
public sealed class ThinkingAhead : CardModel
{
    public override string Name => "ThinkingAhead";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Cards => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Draw(combat, Cards);
        var hand = combat.Player.Hand;
        if (hand.Count > 0)
        {
            var card = hand[^1];
            hand.RemoveAt(hand.Count - 1);
            combat.Player.DrawPile.Insert(0, card);   // put a card back on top (HP-neutral choice)
        }
    }
}

/// <summary>If you have no Attacks in your hand, draw 2 cards. Cost 0. Upgrade: +1 card. (Draw is real only
/// under an ambient Rng; the hand check uses the live hand at play time.) (MegaCrit Impatience)</summary>
public sealed class Impatience : CardModel
{
    public override string Name => "Impatience";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Common;
    public override TargetType Target => TargetType.Self;
    public int Cards => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (!combat.Player.Hand.Any(c => c.Type == CardType.Attack))
            Cmd.Draw(combat, Cards);
    }
}

// (Expertise is a Silent card — defined in Silent/SilentCards.cs, not here.)

/// <summary>Gain 30 Block. You cannot gain Block for the next 2 turns. Exhaust. Cost 0. Upgrade: +10 Block.
/// (The "cannot gain Block" aftermath is modelled by NoBlockPower.) (MegaCrit PanicButton)</summary>
public sealed class PanicButton : CardModel
{
    public override string Name => "PanicButton";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Block => 30 + 10 * Upgrades;
    public int NoBlockTurns => 2;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new NoBlockPower(), NoBlockTurns, combat.Player);
    }
}

// --------------------------------------------------------------------------
// Powers
// --------------------------------------------------------------------------

/// <summary>Power: every 5 cards you play, deal 10 damage to ALL enemies. Cost 0. Upgrade: +4 damage.
/// (MegaCrit Panache — the play-counter lives in PanachePower, which is Stateful per combat.)</summary>
public sealed class Panache : CardModel
{
    public override string Name => "Panache";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Damage => 10 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new PanachePower(), Damage, combat.Player);
}

/// <summary>At the end of your turn 3 turns from now, deal 40 damage to ALL enemies. Cost 2. Upgrade:
/// +10 damage. (The delayed explosion is modelled by TheBombPower's per-turn countdown.) (MegaCrit TheBomb)
/// </summary>
public sealed class TheBomb : CardModel
{
    public override string Name => "TheBomb";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Countdown => 3;
    public int Damage => 40 + 10 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, combat.Player, new TheBombPower(), Damage, combat.Player);
        (combat.Player.GetPower("TheBomb") as TheBombPower)?.SetCountdown(Countdown);
    }
}

/// <summary>Power: at the start of each of your turns, play the top card of your draw pile. Cost 2. (MegaCrit
/// Mayhem) — the auto-played card depends on draw order (RNG) and is recorded as an auto-play in the trace,
/// so this is ported as an inert marker (auto-plays are replayed from the trace), mirroring Aggression /
/// Stampede / Hellraiser in the Ironclad set. Unit-tested as applying the power.</summary>
public sealed class Mayhem : CardModel
{
    public override string Name => "Mayhem";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new MayhemPower(), 1, combat.Player);
}

// --------------------------------------------------------------------------
// HP-neutral / RNG cards ported as their faithful subset (documented inert)
// --------------------------------------------------------------------------

/// <summary>Upgrade ALL of your cards for the rest of combat. Exhaust. Innate. Cost 2. (MegaCrit Apotheosis)
/// — upgrading a card is HP-neutral until that card is later played, and the resulting plays are recorded
/// individually with their upgraded values, so this is modelled as upgrading every card currently in the
/// player's piles. It exhausts itself; Innate only affects the opening hand and is not modelled.
///
/// SOUNDNESS: this MUST NOT mutate a card instance in place. The immutable majority of cards are SHARED
/// across cloned search states (Player.Clone only deep-clones <see cref="CardModel.Stateful"/> cards), so
/// bumping a shared card's <see cref="CardModel.Upgrades"/> would change its <see cref="CardModel.StateKey"/>
/// in every sibling branch — corrupting the draw enumerator's pile bookkeeping (it crashed with
/// "Pile missing card …"). Instead we REPLACE each upgradable card with a freshly cloned, upgraded instance
/// that this state alone owns, leaving the shared original untouched. (Same isolation guarantee Rampage gets
/// from Stateful, achieved here by not aliasing the mutation onto a shared instance.)</summary>
public sealed class Apotheosis : CardModel
{
    public override string Name => "Apotheosis";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        // Upgrade every not-yet-upgraded card in every pile by REPLACING it with a private, freshly-cloned
        // upgraded copy — never mutate the (shared, non-Stateful) instances in place, or sibling search
        // branches that share them get corrupted (the draw enumerator then throws "Pile missing card …").
        var p = combat.Player;
        UpgradePile(p.Hand);
        UpgradePile(p.DrawPile);
        UpgradePile(p.DiscardPile);
    }

    private static void UpgradePile(List<CardModel> pile)
    {
        for (int i = 0; i < pile.Count; i++)
            if (pile[i].Upgrades == 0)
                pile[i] = pile[i].Clone().Upgraded(1);   // own a private upgraded copy; never mutate a shared instance
    }
}

/// <summary>Add 3 random Attacks to your draw pile (they cost 0 this combat). Exhaust. Cost 2. (MegaCrit
/// Metamorphosis) — the generated Attacks are random/unrecorded and HP-neutral until played (any the
/// autopilot plays are reconstructed by the validator), so this is modelled as exhaust-self only. Unit-tested
/// as inert.</summary>
public sealed class Metamorphosis : CardModel
{
    public override string Name => "Metamorphosis";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // generated cards are RNG/HP-neutral
}

/// <summary>Reduce the cost of all cards in your hand to 0 this turn. Exhaust. Cost 0. (MegaCrit
/// Enlightenment — STS2 zeroes hand-card costs for the turn) — the cost reduction is HP-neutral (it never
/// changes a card's effect, only whether it is affordable; the recorder only ever feeds affordable plays),
/// so this is modelled as exhaust-self only. Unit-tested as inert.</summary>
public sealed class Enlightenment : CardModel
{
    public override string Name => "Enlightenment";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // cost reduction is HP-neutral
}

/// <summary>Exhaust up to 3 chosen cards in your hand. Retain. Exhaust. Cost 0. (MegaCrit Purity) — the
/// choice is an HP-neutral selection prompt; exhausting cards from the hand only matters via on-exhaust
/// triggers (Feel No Pain / Dark Embrace), which fire correctly here. For replay the choice is unrecorded,
/// so this exhausts up to 3 arbitrary hand cards. Retain only affects card flow and is not modelled.</summary>
public sealed class Purity : CardModel
{
    public override string Name => "Purity";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int MaxExhaust => 3 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var c in combat.Player.Hand.Take(MaxExhaust).ToList())
            Cmd.ExhaustFromHand(combat, c);
    }
}
