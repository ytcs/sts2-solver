using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// Colorless-only powers. Generic keyword powers shared with other characters
// (Poison, Weak, Vulnerable, Strength, Dexterity, …) live in Core/CommonPowers.cs.
// ===========================================================================

/// <summary>Dark Shackles: temporarily reduces the target enemy's Strength by <c>Amount</c> until its turn
/// ends, weakening its upcoming attack (then restored). Same mechanic as Mangle / Piercing Wail. (MegaCrit
/// DarkShacklesPower — a TemporaryStrengthPower with negative sign.)</summary>
public sealed class DarkShacklesPower : TemporaryStrengthPower
{
    public override string Id => "DarkShackles";
    protected override int Sign => -1;
}

/// <summary>Panache: counts cards played; every <c>5th</c> card played this combat deals <c>Amount</c>
/// damage to ALL enemies. The counter is per-combat mutable state, so the card that grants Panache is
/// Stateful via this power's own StateKey (powers are cloned with combat state). (MegaCrit PanachePower.)
/// </summary>
public sealed class PanachePower : PowerModel
{
    public override string Id => "Panache";
    public override PowerType Type => PowerType.Buff;

    public const int Period = 5;
    private int _counter;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        _counter++;
        if (_counter < Period) return;
        _counter = 0;
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, Owner, m, Amount, ValueProp.Move, null);
    }

    public override PowerModel Clone()
    {
        var c = (PanachePower)base.Clone();
        c._counter = _counter;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}/{_counter}";
    public override long HashValue() => base.HashValue() ^ ((long)_counter * 0x100000001B3L);
}

/// <summary>Panic Button aftermath: the owner cannot gain Block for the next <c>Amount</c> turns. Block
/// gains are multiplied by 0; the counter decrements at the owner's turn end. (MegaCrit — modelled via the
/// engine's block-multiplier + a per-turn countdown.)</summary>
public sealed class NoBlockPower : PowerModel
{
    public override string Id => "NoBlock";
    public override PowerType Type => PowerType.Debuff;

    public override decimal ModifyBlockMultiplicative(Creature target, decimal block, ValueProp props, CardModel? cardSource)
        => target == Owner ? 0m : 1m;

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) { Amount--; this.NormalizeOrRemove(Owner); }
    }
}

/// <summary>The Bomb: at the END of the owner's turn the countdown drops by 1; when it reaches 0 it deals
/// <c>BombDamage</c> to ALL enemies (then the power is removed). The countdown is tracked in <c>_turns</c>
/// (mutable per-combat state, carried through Clone / StateKey). (MegaCrit TheBombPower.)</summary>
public sealed class TheBombPower : PowerModel
{
    public override string Id => "TheBomb";
    public override PowerType Type => PowerType.Buff;

    // Amount carries the explosion damage; _turns is the remaining countdown (set on application).
    private int _turns;
    public void SetCountdown(int turns) => _turns = turns;

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        _turns--;
        if (_turns > 0) return;
        foreach (var m in combat.LivingMonsters.ToList())
            // Unpowered (game: DamageVar 40, ValueProp.Unpowered) — the explosion is NOT scaled by the player's
            // Strength, and (since both Strength and Vulnerable gate on IsPoweredAttack) NOT amplified by
            // Vulnerable either. ValueProp.Move here would over-credit a Strength-stacking deck's AoE (optimistic).
            Cmd.Attack(combat, Owner, m, Amount, ValueProp.Unpowered, null);
        Owner.RemovePower(Id);
    }

    public override PowerModel Clone()
    {
        var c = (TheBombPower)base.Clone();
        c._turns = _turns;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}/{_turns}";
    public override long HashValue() => base.HashValue() ^ ((long)_turns * 0x1000193L);
}

/// <summary>Mayhem: at the start of each of the owner's turns, the top card of the draw pile is auto-played
/// <c>Amount</c> times. The auto-played card depends on draw order (RNG) and is recorded as an auto-play in
/// the trace, so this is modelled as an inert marker (auto-plays are replayed from the trace). (MegaCrit
/// MayhemPower — ported as an inert power, mirroring Aggression/Stampede/Hellraiser in IroncladCards.)</summary>
public sealed class MayhemPower : PowerModel
{
    public override string Id => "Mayhem";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Prep Time: at the start of each of the owner's turns, gain <c>Amount</c> Vigor (the next powered
/// attack deals +Amount). (MegaCrit PrepTimePower — AfterSideTurnStart grants VigorPower.)</summary>
public sealed class PrepTimePower : PowerModel
{
    public override string Id => "PrepTime";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side)
            Cmd.ApplyPower(combat, Owner, new VigorPower(), Amount, Owner);
    }
}

/// <summary>Rolling Boulder: at the start of each of the owner's turns, deal <c>Amount</c> damage to ALL
/// enemies, then permanently increase <c>Amount</c> by 5 (the boulder picks up speed). Amount is a base
/// power field, so its growth is already carried through Clone / StateKey / HashValue. (MegaCrit
/// RollingBoulderPower — the +5 step is the power's own DamageVar, which the card upgrade does not change.)
/// </summary>
public sealed class RollingBoulderPower : PowerModel
{
    public override string Id => "RollingBoulder";
    public override PowerType Type => PowerType.Buff;
    public const int Increment = 5;   // DamageVar(5), unchanged by the card's upgrade

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, Owner, m, Amount, ValueProp.Unpowered, null);   // unpowered (no Strength)
        Amount += Increment;
    }
}

/// <summary>Fasten: adds <c>Amount</c> block to powered block gained from a Defend-tagged card played by the
/// owner. (MegaCrit FastenPower — ModifyBlockAdditive gated on CardTag.Defend.)</summary>
public sealed class FastenPower : PowerModel
{
    public override string Id => "Fasten";
    public override PowerType Type => PowerType.Buff;

    public override decimal ModifyBlockAdditive(Creature target, decimal block, ValueProp props, CardModel? cardSource)
    {
        if (target != Owner) return 0m;
        if (!props.IsPoweredBlock()) return 0m;
        if (cardSource == null || !cardSource.IsDefend) return 0m;
        return Amount;
    }
}

/// <summary>Automation: after every 10 cards the owner draws, gain <c>Amount</c> energy. The remaining count
/// (starting at 10) is per-combat mutable state carried through Clone / StateKey / HashValue. The energy gain
/// only fires under a concrete draw (replay / rollout); mid-turn draws are no-ops in pure search, so it is
/// inert there — pessimistic-sound (never grants energy the search can't see). (MegaCrit AutomationPower.)
/// </summary>
public sealed class AutomationPower : PowerModel
{
    public override string Id => "Automation";
    public override PowerType Type => PowerType.Buff;

    public const int Period = 10;
    private int _cardsLeft = Period;

    public override void AfterCardDrawn(CombatState combat, CardModel card, bool fromHandDraw)
    {
        if (--_cardsLeft > 0) return;
        _cardsLeft = Period;
        Cmd.GainEnergy(combat, Amount);
    }

    public override PowerModel Clone()
    {
        var c = (AutomationPower)base.Clone();
        c._cardsLeft = _cardsLeft;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}/{_cardsLeft}";
    public override long HashValue() => base.HashValue() ^ ((long)_cardsLeft * 0x100000001B3L);
}

/// <summary>The Gambit's curse: while owned, the moment the owner takes any unblocked powered-attack damage it
/// DIES (the power is then removed). Persists until triggered — there is no turn-boundary removal in the spec,
/// so once played the owner must block every attack or die. Faithful to the decompile, and pessimistic-sound
/// for survival regardless. (MegaCrit TheGambitPower.)</summary>
public sealed class TheGambitPower : PowerModel
{
    public override string Id => "TheGambit";
    public override PowerType Type => PowerType.Debuff;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (target != Owner || unblockedDamage <= 0 || !props.IsPoweredAttack()) return;
        Owner.RemovePower(Id);            // safe: AfterDamageReceived iterates a snapshot of the power list
        Cmd.Kill(combat, Owner);
    }
}

// --------------------------------------------------------------------------
// Inert markers (sound under-credit): RNG card-generation / card-flow that is
// never a search decision and HP-neutral until the generated/moved card is
// later played (recorded individually in traces). Mirror Mayhem / Metamorphosis.
// --------------------------------------------------------------------------

/// <summary>Calamity: whenever you play an Attack, add <c>Amount</c> random Attacks to your hand (free this
/// turn). The generated Attacks are random/unrecorded and HP-neutral until played, so this is inert. (MegaCrit
/// CalamityPower.)</summary>
public sealed class CalamityPower : PowerModel
{
    public override string Id => "Calamity";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Entropy: at the start of each of your turns, transform <c>Amount</c> cards in your hand into random
/// cards. Random transformation is never a search decision and HP-neutral until played, so this is inert.
/// (MegaCrit EntropyPower.)</summary>
public sealed class EntropyPower : PowerModel
{
    public override string Id => "Entropy";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Nostalgia: the first <c>Amount</c> Attacks/Skills you play each turn return to the top of your draw
/// pile instead of the discard. Card-flow only (a benefit — replaying cards is never required), so leaving the
/// cards in discard under-credits and is sound. Inert. (MegaCrit NostalgiaPower.)</summary>
public sealed class NostalgiaPower : PowerModel
{
    public override string Id => "Nostalgia";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Stratagem: after each shuffle, choose <c>Amount</c> cards from your draw pile to put into your hand.
/// An HP-neutral selection prompt over card flow, never a search decision. Inert. (MegaCrit StratagemPower.)
/// </summary>
public sealed class StratagemPower : PowerModel
{
    public override string Id => "Stratagem";
    public override PowerType Type => PowerType.Buff;
}

// --------------------------------------------------------------------------
// Powers granted by the multiplayer-only Colorless cards. In single player the
// purely ally-facing ones never fire (no other players exist) — they are still
// applied so the 1:1 structure is preserved, but carry no hook. Coordinate's
// power is the exception: its target is the only ally (you), so it is a real
// one-turn Strength buff.
// --------------------------------------------------------------------------

/// <summary>Beacon of Hope: whenever you gain Block, give half of it to your other allies (Unpowered). No
/// other allies exist in single player, so it never fires — inert. (MegaCrit BeaconOfHopePower.)</summary>
public sealed class BeaconOfHopePower : PowerModel
{
    public override string Id => "BeaconOfHope";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Coordinate: grants +Amount temporary Strength until the owner's turn ends. Its target is the only
/// ally — yourself — so it is a real one-turn self Strength buff, exactly the shared TemporaryStrengthPower
/// mechanic. (MegaCrit CoordinatePower : TemporaryStrengthPower.)</summary>
public sealed class CoordinatePower : TemporaryStrengthPower
{
    public override string Id => "Coordinate";
    protected override int Sign => 1;
}

/// <summary>Knockdown: the marked enemy takes double damage from attacks dealt by your allies OTHER than you,
/// until your turn ends. Every attack's dealer is you in single player, so the multiplier never applies —
/// inert. (MegaCrit KnockdownPower, ModifyDamageMultiplicative gated on dealer != applier.)</summary>
public sealed class KnockdownPower : PowerModel
{
    public override string Id => "Knockdown";
    public override PowerType Type => PowerType.Debuff;
}

/// <summary>Tag Team: the marked enemy is struck an extra time by attacks played by your allies OTHER than
/// you. Your own attacks are excluded, so it never fires in single player — inert. (MegaCrit TagTeamPower,
/// ModifyCardPlayCount gated on a foreign card owner.)</summary>
public sealed class TagTeamPower : PowerModel
{
    public override string Id => "TagTeam";
    public override PowerType Type => PowerType.Debuff;
}
