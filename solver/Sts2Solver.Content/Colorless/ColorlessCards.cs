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
    public override bool LoopRiskDraw => true;   // cost 0 + draws + returns to discard ⇒ replayable in-turn
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 5 + 3 * Upgrades;   // decompile: Damage.UpgradeValueBy(3)
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
    public override bool Innate => true;   // guaranteed in the opening hand
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
    public override bool Innate => true;   // guaranteed in the opening hand
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
    public override bool LoopRiskDraw => true;   // cost 0 + draws + returns to discard ⇒ replayable in-turn
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
    public override bool LoopRiskDraw => true;   // cost 0 + conditional draw + returns to discard ⇒ replayable
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
    public override bool Innate => true;   // guaranteed in the opening hand
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
    public override bool Retain => true;   // Retain + Exhaust: kept in hand across turns until played, then exhausts
    public int MaxExhaust => 3 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        foreach (var c in combat.Player.Hand.Take(MaxExhaust).ToList())
            Cmd.ExhaustFromHand(combat, c);
    }
}

// ===========================================================================
// Second wave of Colorless cards (the 39 remaining pool entries + the Ancient
// Wish). Shared idioms factored into ColorlessFx below. Soundness follows the
// project doctrine: RNG card-generation / random transforms / potion creation
// are inert (under-credit, never a search decision); selection prompts use a
// deterministic fixed default; return-to-hand replay is left as a documented
// pessimistic gap (mirrors Feral). Numbers are 1:1 from the decompile.
// ===========================================================================

internal static class ColorlessFx
{
    /// <summary>Selection-prompt default: move the first card matching <paramref name="match"/> from the draw
    /// pile into hand (the game lets the player choose; the choice is HP-neutral at move time and any later play
    /// is recorded individually, so a deterministic first-match is sound — same treatment as ThinkingAhead).</summary>
    public static void DrawToHand(CombatState combat, Func<CardModel, bool> match)
    {
        var p = combat.Player;
        if (p.Hand.Count >= Player.MaxHandSize) return;
        var c = p.DrawPile.FirstOrDefault(match);
        if (c == null) return;
        p.DrawPile.Remove(c);
        p.Hand.Add(c);
    }

    /// <summary>Apply a freshly-built power to every living enemy (Shockwave). The factory runs per target so
    /// each enemy gets its own power instance.</summary>
    public static void ApplyToAllEnemies(CombatState combat, Func<PowerModel> factory, int amount)
    {
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.ApplyPower(combat, m, factory(), amount, combat.Player);
    }
}

// --------------------------------------------------------------------------
// Attacks
// --------------------------------------------------------------------------

/// <summary>Deal 3 damage. Returns to your hand next turn if played. Cost 0. Upgrade: +1 damage. (MegaCrit
/// Bolas — the return-to-hand replay is a beneficial card-flow loop left INERT: the card goes to discard, which
/// under-credits its repeat damage and is sound, mirroring Feral.)</summary>
public sealed class Bolas : CardModel
{
    public override string Name => "Bolas";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 3 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 7 damage. Gain Block equal to the damage dealt. Cost 1. Upgrade: +2 damage. (MegaCrit
/// Fisticuffs — block = the HP actually removed; the rare overkill / enemy-block remainder is under-credited
/// because <see cref="Cmd.Attack"/> returns post-block HP loss, which is sound.)</summary>
public sealed class Fisticuffs : CardModel
{
    public override string Name => "Fisticuffs";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 7 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int dealt = Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.GainBlock(combat, combat.Player, dealt, ValueProp.Move, this);
    }
}

/// <summary>Deal damage equal to the number of cards played this combat. Cost 1. Upgrade: gains Retain.
/// (MegaCrit GoldAxe — CalculatedDamage = 0 + 1×CardPlaysFinished; the gated per-combat counter excludes this
/// card itself, which is still in flight when its damage resolves.)</summary>
public sealed class GoldAxe : CardModel
{
    public override string Name => "GoldAxe";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool Retain => Upgrades > 0;
    public override bool TracksCardsPlayedThisCombat => true;
    public int Damage(CombatState combat) => combat.CardsPlayedThisCombat;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage(combat), ValueProp.Move, this);
}

/// <summary>Deal 25 damage. Add 3 random 0-cost cards to your hand. Cost 3. Upgrade: +5 damage. (MegaCrit
/// Jackpot — the generated cards are RNG and HP-neutral until played, so only the damage is modelled.)</summary>
public sealed class Jackpot : CardModel
{
    public override string Name => "Jackpot";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 25 + 5 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);   // 0-cost gen is inert
}

/// <summary>Deal 8 damage to an enemy; deal that much damage to ALL OTHER enemies. Cost 0. Upgrade: +3.
/// (MegaCrit Omnislice — the splash equals the HP removed from the primary target, dealt Unpowered. For a
/// single enemy this is just the 8 damage; multi-enemy splash uses post-block HP loss, a sound under-credit.)
/// </summary>
public sealed class Omnislice : CardModel
{
    public override string Name => "Omnislice";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 8 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int dealt = Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        foreach (var m in combat.LivingMonsters.ToList())
            if (m != play.Target)
                Cmd.Attack(combat, combat.Player, m, dealt, ValueProp.Move | ValueProp.Unpowered, this);
    }
}

/// <summary>Deal 15 damage plus 5 for each permanent debuff on the target. Cost 2. Upgrade: base +3 and
/// per-debuff +3 (18 + 8×debuffs). (MegaCrit Rend — CalculatedDamage = CalcBase + ExtraDamage×count; the count
/// excludes temporary-Strength markers, matching the game's "not ITemporaryPower" filter.)</summary>
public sealed class Rend : CardModel
{
    public override string Name => "Rend";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public int CalcBase => 15 + 3 * Upgrades;
    public int Extra => 5 + 3 * Upgrades;
    public static int PermanentDebuffs(Creature c)
        => c.Powers.Count(p => p.Type == PowerType.Debuff && p is not TemporaryStrengthPower);
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, CalcBase + Extra * PermanentDebuffs(play.Target!), ValueProp.Move, this);
}

/// <summary>Deal 12 damage. Retain a card in your hand next turn. Cost 1. Upgrade: +4 damage. (MegaCrit Salvo —
/// the retain is RetainHandPower, an HP-neutral card-flow marker.)</summary>
public sealed class Salvo : CardModel
{
    public override string Name => "Salvo";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 12 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new RetainHandPower(), 1, combat.Player);
    }
}

/// <summary>Deal 9 damage. Choose 1 of 3 random cards from your draw pile to put into your hand. Cost 1.
/// Upgrade: +3 damage. (MegaCrit SeekerStrike — the choice uses the fixed-default move-to-hand.)</summary>
public sealed class SeekerStrike : CardModel
{
    public override string Name => "SeekerStrike";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Damage => 9 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
        ColorlessFx.DrawToHand(combat, _ => true);
    }
}

/// <summary>Deal 11 damage. Returns to your hand next turn if played. Cost 1. Upgrade: +3 damage. (MegaCrit
/// ThrummingHatchet — return-to-hand replay left INERT, sound under-credit, mirroring Bolas/Feral.)</summary>
public sealed class ThrummingHatchet : CardModel
{
    public override string Name => "ThrummingHatchet";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public int Damage => 11 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 14 damage. Cost 1. Upgrade: +6 damage. (MegaCrit UltimateStrike — Strike-tagged.)</summary>
public sealed class UltimateStrike : CardModel
{
    public override string Name => "UltimateStrike";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AnyEnemy;
    public override bool IsStrike => true;
    public int Damage => 14 + 6 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.Attack(combat, combat.Player, play.Target!, Damage, ValueProp.Move, this);
}

/// <summary>Deal 10 damage to random enemies X times (X = energy spent). Cost X. Upgrade: +4 damage per hit.
/// (MegaCrit Volley — each hit re-targets a living enemy; deterministic for a single enemy, the validated case.)
/// </summary>
public sealed class Volley : CardModel
{
    public override string Name => "Volley";
    public override int BaseCost => 0;
    public override bool IsXCost => true;
    public override CardType Type => CardType.Attack;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.RandomEnemy;
    public int Damage => 10 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        for (int i = 0; i < play.XValue; i++)
        {
            var t = combat.LivingMonsters.FirstOrDefault();
            if (t == null) break;
            Cmd.Attack(combat, combat.Player, t, Damage, ValueProp.Move, this);
        }
    }
}

// --------------------------------------------------------------------------
// Skills
// --------------------------------------------------------------------------

/// <summary>Add a random Potion to your potion belt. Exhaust. Cost 1. Upgrade: costs 0. (MegaCrit Alchemize —
/// potions are a meta subsystem not modelled in combat, so this is inert.)</summary>
public sealed class Alchemize : CardModel
{
    public override string Name => "Alchemize";
    public override int BaseCost => Upgrades > 0 ? 0 : 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // potion generation is out of scope
}

/// <summary>Put all Rare cards from your draw pile into your hand. Exhaust. Cost 1. Upgrade: gains Retain.
/// (MegaCrit Anointed — moving cards draw→hand is HP-neutral card-flow that the solver does not benefit from
/// distinguishing; left inert, a sound under-credit.)</summary>
public sealed class Anointed : CardModel
{
    public override string Name => "Anointed";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override bool Retain => Upgrades > 0;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // card-flow only
}

/// <summary>Play 3 random Attacks from your discard pile. Cost 3. Upgrade: +1 Attack. (MegaCrit BeatDown — the
/// auto-played cards are chosen by RNG and recorded individually in the trace, so this is an inert marker;
/// mirrors Mayhem.)</summary>
public sealed class BeatDown : CardModel
{
    public override string Name => "BeatDown";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.RandomEnemy;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // RNG auto-play replayed from trace
}

/// <summary>Play the top 2 cards of your draw pile. Cost 2. Upgrade: +1 card. (MegaCrit Catastrophe — the
/// auto-played cards depend on draw order (RNG) and are recorded individually, so this is an inert marker;
/// mirrors Mayhem.)</summary>
public sealed class Catastrophe : CardModel
{
    public override string Name => "Catastrophe";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // RNG auto-play replayed from trace
}

/// <summary>Choose 1 of 3 cards to add to your hand (free this turn). Exhaust. Cost 1. Upgrade: loses Exhaust.
/// (MegaCrit Discovery — the offered cards are RNG-generated from the character pool and the pick is a choice;
/// generation is never a search decision, so this is inert.)</summary>
public sealed class Discovery : CardModel
{
    public override string Name => "Discovery";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // RNG card-generation is inert
}

/// <summary>Gain 13 Block. Retain your hand next turn. Cost 2. Upgrade: +3 Block. (MegaCrit Equilibrium — the
/// retain is RetainHandPower, an HP-neutral card-flow marker.)</summary>
public sealed class Equilibrium : CardModel
{
    public override string Name => "Equilibrium";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Block => 13 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new RetainHandPower(), 1, combat.Player);
    }
}

/// <summary>A random Attack/Skill in your draw pile gains Replay 2 (it plays an extra time when next drawn and
/// played). Cost 1. Upgrade: Replay 3. (MegaCrit HiddenGem — which card is boosted is RNG and the replay is a
/// benefit; left inert, a sound under-credit.)</summary>
public sealed class HiddenGem : CardModel
{
    public override string Name => "HiddenGem";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // RNG replay-boost is inert
}

/// <summary>Add 1 random Colorless card to your hand. Exhaust. Cost 0. Upgrade: +1 card. (MegaCrit
/// JackOfAllTrades — RNG card-generation is never a search decision, so this is inert.)</summary>
public sealed class JackOfAllTrades : CardModel
{
    public override string Name => "JackOfAllTrades";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // RNG card-generation is inert
}

/// <summary>Gain 2 energy. Exhaust. Cost 0. Upgrade: +1 energy. (MegaCrit Production.)</summary>
public sealed class Production : CardModel
{
    public override string Name => "Production";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Energy => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play) => Cmd.GainEnergy(combat, Energy);
}

/// <summary>Next turn, gain Block equal to your current Block. Exhaust. Cost 0. Upgrade: loses Exhaust.
/// (MegaCrit Prolong — BlockNextTurnPower carries the current block to the next turn start.)</summary>
public sealed class Prolong : CardModel
{
    public override string Name => "Prolong";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new BlockNextTurnPower(), combat.Player.Block, combat.Player);
}

/// <summary>If this is the only card in your hand, draw 2 cards and gain 2 energy. Retain. Cost 0. Upgrade: +1
/// card and +1 energy. (MegaCrit Restlessness — the hand-only check reads the live hand; this card is already
/// removed from hand when its effect runs, so the condition is "hand is now empty". Draw is real only under an
/// ambient Rng.)</summary>
public sealed class Restlessness : CardModel
{
    public override string Name => "Restlessness";
    public override bool Retain => true;
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Cards => 2 + Upgrades;
    public int Energy => 2 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        if (combat.Player.Hand.Count != 0) return;   // "only card in hand" — it is already removed before OnPlay
        Cmd.Draw(combat, Cards);
        Cmd.GainEnergy(combat, Energy);
    }
}

/// <summary>Draw cards until your hand is full. Exhaust. Cost 1. Upgrade: gains Retain. (MegaCrit Scrawl — draws
/// up to MaxHandSize − current hand; real only under an ambient Rng.)</summary>
public sealed class Scrawl : CardModel
{
    public override string Name => "Scrawl";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override bool Retain => Upgrades > 0;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        int n = Player.MaxHandSize - combat.Player.Hand.Count;
        if (n > 0) Cmd.Draw(combat, n);
    }
}

/// <summary>Put a Skill from your draw pile into your hand. Exhaust. Cost 0. Upgrade: loses Exhaust. (MegaCrit
/// SecretTechnique — selection uses the fixed-default move-to-hand.)</summary>
public sealed class SecretTechnique : CardModel
{
    public override string Name => "SecretTechnique";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play)
        => ColorlessFx.DrawToHand(combat, c => c.Type == CardType.Skill);
}

/// <summary>Put an Attack from your draw pile into your hand. Exhaust. Cost 0. Upgrade: loses Exhaust. (MegaCrit
/// SecretWeapon — selection uses the fixed-default move-to-hand.)</summary>
public sealed class SecretWeapon : CardModel
{
    public override string Name => "SecretWeapon";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => Upgrades > 0 ? CardResultPile.Discard : CardResultPile.Exhaust;
    public override void OnPlay(CombatState combat, CardPlay play)
        => ColorlessFx.DrawToHand(combat, c => c.Type == CardType.Attack);
}

/// <summary>Apply 3 Weak and 3 Vulnerable to ALL enemies. Exhaust. Cost 2. Upgrade: +2 each. (MegaCrit
/// Shockwave.)</summary>
public sealed class Shockwave : CardModel
{
    public override string Name => "Shockwave";
    public override int BaseCost => 2;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.AllEnemies;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public int Amount => 3 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        ColorlessFx.ApplyToAllEnemies(combat, () => new WeakPower(), Amount);
        ColorlessFx.ApplyToAllEnemies(combat, () => new VulnerablePower(), Amount);
    }
}

/// <summary>Choose 1 of 3 random Attacks from other characters to add to your hand (free this turn). Cost 1.
/// (MegaCrit Splash — RNG cross-pool card-generation, never a search decision, so this is inert.)</summary>
public sealed class Splash : CardModel
{
    public override string Name => "Splash";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play) { }   // RNG card-generation is inert
}

/// <summary>Gain 50 Block. If you take unblocked attack damage, you die. Cost 0. Upgrade: +25 Block. (MegaCrit
/// TheGambit — the death clause is TheGambitPower, a persistent debuff faithfully modelled as harm.)</summary>
public sealed class TheGambit : CardModel
{
    public override string Name => "TheGambit";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Block => 50 + 25 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
        Cmd.ApplyPower(combat, combat.Player, new TheGambitPower(), 1, combat.Player);
    }
}

/// <summary>Gain 11 Block. Cost 1. Upgrade: +4 Block. (MegaCrit UltimateDefend — Defend-tagged.)</summary>
public sealed class UltimateDefend : CardModel
{
    public override string Name => "UltimateDefend";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override bool IsDefend => true;
    public int Block => 11 + 4 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.GainBlock(combat, combat.Player, Block, ValueProp.Move, this);
}

/// <summary>Put a card from your draw pile into your hand. Exhaust. Cost 0. Upgrade: gains Retain. (MegaCrit
/// Wish — Ancient rarity; selection uses the fixed-default move-to-hand.)</summary>
public sealed class Wish : CardModel
{
    public override string Name => "Wish";
    public override int BaseCost => 0;
    public override CardType Type => CardType.Skill;
    public override CardRarity Rarity => CardRarity.Ancient;
    public override TargetType Target => TargetType.Self;
    public override CardResultPile ResultPile => CardResultPile.Exhaust;
    public override bool Retain => Upgrades > 0;
    public override void OnPlay(CombatState combat, CardPlay play)
        => ColorlessFx.DrawToHand(combat, _ => true);
}

// --------------------------------------------------------------------------
// Powers
// --------------------------------------------------------------------------

/// <summary>Power: every 10 cards you draw, gain 1 energy. Cost 1. Upgrade: costs 0. (MegaCrit Automation.)</summary>
public sealed class Automation : CardModel
{
    public override string Name => "Automation";
    public override int BaseCost => Upgrades > 0 ? 0 : 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new AutomationPower(), 1, combat.Player);
}

/// <summary>Power: whenever you play an Attack, add 1 random Attack to your hand. Cost 3. Upgrade: costs 2.
/// (MegaCrit Calamity — the generated Attacks are RNG/inert; see CalamityPower.)</summary>
public sealed class Calamity : CardModel
{
    public override string Name => "Calamity";
    public override int BaseCost => 3 - Upgrades;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new CalamityPower(), 1, combat.Player);
}

/// <summary>Power: at the start of each turn, transform 1 card in your hand. Innate when upgraded. Cost 1.
/// (MegaCrit Entropy — the random transform is inert; see EntropyPower.)</summary>
public sealed class Entropy : CardModel
{
    public override string Name => "Entropy";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override bool Innate => Upgrades > 0;   // opening-hand only
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new EntropyPower(), 1, combat.Player);
}

/// <summary>Power: at the end of each turn, gain 9 Block (decays by 1 each turn). Cost 3. Upgrade: +3 Block.
/// (MegaCrit EternalArmor — PlatingPower, the same Metallicize-style block used by Stone Armor.)</summary>
public sealed class EternalArmor : CardModel
{
    public override string Name => "EternalArmor";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Plating => 9 + 3 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new PlatingPower(), Plating, combat.Player);
}

/// <summary>Power: Defend cards give 4 extra Block. Cost 1. Upgrade: +2. (MegaCrit Fasten.)</summary>
public sealed class Fasten : CardModel
{
    public override string Name => "Fasten";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Extra => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new FastenPower(), Extra, combat.Player);
}

/// <summary>Power: the first Attack/Skill you play each turn returns to your draw pile. Cost 1. Upgrade: costs
/// 0. (MegaCrit Nostalgia — card-flow only, inert; see NostalgiaPower.)</summary>
public sealed class Nostalgia : CardModel
{
    public override string Name => "Nostalgia";
    public override int BaseCost => Upgrades > 0 ? 0 : 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new NostalgiaPower(), 1, combat.Player);
}

/// <summary>Power: at the start of each turn, gain 4 Vigor. Cost 1. Upgrade: +2 Vigor. (MegaCrit PrepTime.)</summary>
public sealed class PrepTime : CardModel
{
    public override string Name => "PrepTime";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Vigor => 4 + 2 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new PrepTimePower(), Vigor, combat.Player);
}

/// <summary>Power: gain 1 Strength and 1 Dexterity. Cost 1. Upgrade: +1 each. (MegaCrit Prowess.)</summary>
public sealed class Prowess : CardModel
{
    public override string Name => "Prowess";
    public override int BaseCost => 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public int Amount => 1 + Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
    {
        Cmd.ApplyPower(combat, combat.Player, new StrengthPower(), Amount, combat.Player);
        Cmd.ApplyPower(combat, combat.Player, new DexterityPower(), Amount, combat.Player);
    }
}

/// <summary>Power: at the start of each turn, deal 5 damage to ALL enemies; this increases by 5 each turn.
/// Cost 3. Upgrade: starts at 10. (MegaCrit RollingBoulder — RollingBoulderPower escalates its own amount.)</summary>
public sealed class RollingBoulder : CardModel
{
    public override string Name => "RollingBoulder";
    public override int BaseCost => 3;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Rare;
    public override TargetType Target => TargetType.Self;
    public int Start => 5 + 5 * Upgrades;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new RollingBoulderPower(), Start, combat.Player);
}

/// <summary>Power: after each shuffle, choose a card from your draw pile to add to your hand. Cost 1. Upgrade:
/// costs 0. (MegaCrit Stratagem — card-flow selection, inert; see StratagemPower.)</summary>
public sealed class Stratagem : CardModel
{
    public override string Name => "Stratagem";
    public override int BaseCost => Upgrades > 0 ? 0 : 1;
    public override CardType Type => CardType.Power;
    public override CardRarity Rarity => CardRarity.Uncommon;
    public override TargetType Target => TargetType.Self;
    public override void OnPlay(CombatState combat, CardPlay play)
        => Cmd.ApplyPower(combat, combat.Player, new StratagemPower(), 1, combat.Player);
}
