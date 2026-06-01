using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>At the start of the owner's turn, grant the owner Amount Strength. Permanent and stacking.
/// Applied mid-turn, so the first grant lands at the NEXT turn start (no same-turn skip needed, unlike
/// Ritual which hooks turn-end). (MegaCrit DemonFormPower — Demon Form.)</summary>
public sealed class DemonFormPower : PowerModel
{
    public override string Id => "DemonForm";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side)
            Cmd.ApplyPower(combat, Owner, new StrengthPower(), Amount, Owner);
    }
}
/// <summary>Whenever the owner plays an Attack this turn, gain Amount Block (unpowered — no Dexterity).
/// Lasts a single turn: removed at the owner's turn end. AfterCardPlayed runs after a card's OnPlay, so
/// playing Rage itself (a Skill) never triggers it. (MegaCrit RagePower — Rage.)</summary>
public sealed class RagePower : PowerModel
{
    public override string Id => "Rage";
    public override PowerType Type => PowerType.Buff;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (card.Type == CardType.Attack)
            Cmd.GainBlock(combat, Owner, Amount, ValueProp.Unpowered, null);
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) Owner.RemovePower(Id);   // Rage only lasts the turn it's played
    }
}
/// <summary>Thorns: whenever the owner is hit by a powered attack, deal Amount damage back to the dealer
/// (unpowered — no Strength/Vulnerable — but blockable). Removed at the OTHER side's turn end, so it
/// survives the enemy turn it was set up to punish, then expires. (MegaCrit FlameBarrierPower — Flame
/// Barrier.)</summary>
public sealed class FlameBarrierPower : PowerModel
{
    public override string Id => "FlameBarrier";
    public override PowerType Type => PowerType.Buff;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (target == Owner && dealer != null && dealer.IsAlive && props.IsPoweredAttack())
            Cmd.Attack(combat, Owner, dealer, Amount, ValueProp.Unpowered, null);
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) Owner.RemovePower(Id);   // removed at the enemy's turn end (after retaliating)
    }
}
/// <summary>StoneArmor's Plating: a decaying turn-end block engine. At the owner's (player's) turn end
/// the owner gains Amount block (unpowered — no Dexterity); at the owner's turn start the counter
/// decrements by 1. The first decrement lands on turn 2, since the power is applied mid-turn-1 (after
/// that turn's start hook has already fired), so no turn-1 guard is needed. The block gained at turn end
/// survives the enemy turn (cleared only at the player's next turn start), mirroring Metallicize-style
/// block. (MegaCrit PlatingPower — Stone Armor, single-player / player-owned case.)</summary>
public sealed class PlatingPower : PowerModel
{
    public override string Id => "Plating";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side)
            Cmd.GainBlock(combat, Owner, Amount, ValueProp.Unpowered, null);
    }

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) { Amount--; this.NormalizeOrRemove(Owner); }   // decays one per owner turn
    }
}
/// <summary>Colossus's guard: while owned, any powered attack against the owner from a dealer that is
/// itself Vulnerable deals ×0.5 damage. The counter decrements at the enemy's turn end, so Colossus 1
/// protects through exactly the next enemy turn, then expires. (MegaCrit ColossusPower.)</summary>
public sealed class ColossusPower : PowerModel
{
    public override string Id => "Colossus";
    public override PowerType Type => PowerType.Buff;

    public override decimal ModifyDamageMultiplicative(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (target != Owner || !props.IsPoweredAttack()) return 1m;
        if (dealer == null || !dealer.HasPower("Vulnerable")) return 1m;
        return 0.5m;
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == CombatSide.Enemy) { Amount--; this.NormalizeOrRemove(Owner); }   // lasts through the enemy turn
    }
}
/// <summary>Rupture: whenever the owner loses HP from a card it plays on its own turn, gain Amount
/// Strength — applied AFTER the triggering card resolves (so that card's own later hits aren't boosted),
/// mirroring the game's deferred grant. HP lost on the enemy's turn (incoming attacks) does not trigger.
/// NOTE: only card-play self-damage is modelled; end-of-turn status self-damage (Burn/Infection) under
/// Rupture is a known unmodelled edge — kept out of the validated decks. (MegaCrit RupturePower.)</summary>
public sealed class RupturePower : PowerModel
{
    public override string Id => "Rupture";
    public override PowerType Type => PowerType.Buff;

    private int _pending;   // Strength owed from HP the owner lost during the card currently resolving

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (target == Owner && unblockedDamage > 0 && combat.CurrentSide == Owner.Side)
            _pending += Amount;
    }

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (_pending <= 0) return;
        int owed = _pending; _pending = 0;
        Cmd.ApplyPower(combat, Owner, new StrengthPower(), owed, Owner);   // granted after the card, never boosting it
    }

    public override PowerModel Clone()
    {
        var c = (RupturePower)base.Clone();
        c._pending = _pending;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}{(_pending != 0 ? $"+{_pending}" : "")}";
    public override long HashValue() => base.HashValue() ^ ((long)_pending * 0x2545F4914F6CDD1DL);
}
/// <summary>Juggernaut: whenever the owner gains block, deal Amount damage (unpowered) to an enemy. The
/// game picks a random enemy; with a single enemy this is deterministic — the only case we validate.
/// (MegaCrit JuggernautPower.)</summary>
public sealed class JuggernautPower : PowerModel
{
    public override string Id => "Juggernaut";
    public override PowerType Type => PowerType.Buff;

    public override void AfterBlockGained(CombatState combat, Creature creature, int amount, ValueProp props, CardModel? cardSource)
    {
        if (amount <= 0 || creature != Owner) return;
        var target = combat.LivingMonsters.FirstOrDefault();
        if (target != null) Cmd.Attack(combat, Owner, target, Amount, ValueProp.Unpowered, null);
    }
}
/// <summary>Aggression: at the start of each turn, move Amount random Attack cards from the discard pile to
/// hand and upgrade them. The card movement is RNG and HP-neutral, so this is modelled as an inert marker —
/// the moved cards (if played) are reconstructed by the validator. (MegaCrit AggressionPower.)</summary>
public sealed class AggressionPower : PowerModel
{
    public override string Id => "Aggression";
    public override PowerType Type => PowerType.Buff;
}
/// <summary>Stampede: at the post-play phase, auto-play Amount random Attacks from hand. The auto-play is
/// RNG and recorded as auto-plays (which the validator replays from the trace), so this is an inert marker.
/// (MegaCrit StampedePower.)</summary>
public sealed class StampedePower : PowerModel
{
    public override string Id => "Stampede";
    public override PowerType Type => PowerType.Buff;
}
/// <summary>Hellraiser: auto-plays Strike cards as they are drawn (free). The auto-play/draw interaction is
/// RNG and not modelled by the no-op-draw validator; an inert marker. (MegaCrit HellraiserPower.)</summary>
public sealed class HellraiserPower : PowerModel
{
    public override string Id => "Hellraiser";
    public override PowerType Type => PowerType.Buff;
}
/// <summary>Tank (multiplayer-only): grants allies Guarded and reduces their damage taken. Unreachable in
/// single-player and inert here (no allies). Ported for catalog completeness. (MegaCrit TankPower.)</summary>
public sealed class TankPower : PowerModel
{
    public override string Id => "Tank";
    public override PowerType Type => PowerType.Buff;
}
/// <summary>Expect a Fight's marker: while owned, the player cannot gain energy (Cmd.GainEnergy is a
/// no-op). Removed at the owner's turn end. (MegaCrit NoEnergyGainPower.)</summary>
public sealed class NoEnergyGainPower : PowerModel
{
    public override string Id => "NoEnergyGain";
    public override PowerType Type => PowerType.Debuff;
    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) Owner.RemovePower(Id);
    }
}
/// <summary>Vicious: whenever the owner applies Vulnerable to a target, draw Amount cards. (MegaCrit
/// ViciousPower.) The draw is externally supplied during trace replay (no-op without an ambient Rng).</summary>
public sealed class ViciousPower : PowerModel
{
    public override string Id => "Vicious";
    public override PowerType Type => PowerType.Buff;
    public override void AfterPowerApplied(CombatState combat, Creature target, PowerModel power, int amount, Creature? applier)
    {
        if (power.Id == "Vulnerable" && applier == Owner && amount > 0)
            Cmd.Draw(combat, Amount);
    }
}
/// <summary>Inferno: at each of the owner's turn starts, lose SelfDamage HP (scaling 1 per copy played);
/// AND whenever the owner takes unblocked damage on its own turn (including that self-damage), deal Amount
/// to ALL enemies (unpowered). So each turn start the owner bleeds a little and torches the whole enemy
/// side. (MegaCrit InfernoPower.)</summary>
public sealed class InfernoPower : PowerModel
{
    public override string Id => "Inferno";
    public override PowerType Type => PowerType.Buff;

    private int _selfDamage;

    public override void AfterApplied(CombatState combat, Creature? applier) => _selfDamage++;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side && _selfDamage > 0)
            Cmd.LoseHp(combat, Owner, _selfDamage);   // its unblocked loss triggers AfterDamageReceived below
    }

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (target != Owner || unblockedDamage <= 0 || combat.CurrentSide != Owner.Side) return;
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, Owner, m, Amount, ValueProp.Unpowered, null);
    }

    public override PowerModel Clone() { var c = (InfernoPower)base.Clone(); c._selfDamage = _selfDamage; return c; }
    public override string StateKey() => $"{Id}={Amount}/{_selfDamage}";
    public override long HashValue() => base.HashValue() ^ ((long)_selfDamage * 0x85EBCA77L);
}
/// <summary>Unmovable: the owner's first Amount card-sourced Block gains each turn are doubled. Tracks a
/// per-turn count of card-block gains (reset at turn start); while it is below Amount, block ×2.
/// (MegaCrit UnmovablePower.)</summary>
public sealed class UnmovablePower : PowerModel
{
    public override string Id => "Unmovable";
    public override PowerType Type => PowerType.Buff;

    private int _blockGainsThisTurn;

    public override decimal ModifyBlockMultiplicative(Creature target, decimal block, ValueProp props, CardModel? cardSource)
    {
        if (target != Owner || cardSource == null) return 1m;   // only the owner's card-sourced block
        return _blockGainsThisTurn < Amount ? 2m : 1m;
    }

    public override void AfterBlockGained(CombatState combat, Creature creature, int amount, ValueProp props, CardModel? cardSource)
    {
        if (creature == Owner && cardSource != null) _blockGainsThisTurn++;
    }

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) _blockGainsThisTurn = 0;
    }

    public override PowerModel Clone() { var c = (UnmovablePower)base.Clone(); c._blockGainsThisTurn = _blockGainsThisTurn; return c; }
    public override string StateKey() => $"{Id}={Amount}/{_blockGainsThisTurn}";
    public override long HashValue() => base.HashValue() ^ ((long)_blockGainsThisTurn * 0x27D4EB2FL);
}
/// <summary>Juggling: every 3rd Attack the owner plays in a turn, add Amount copies of that Attack to hand.
/// The per-turn attack counter resets at turn start. The generated copies are inert until played (and the
/// validator reconstructs any that get played). (MegaCrit JugglingPower.)</summary>
public sealed class JugglingPower : PowerModel
{
    public override string Id => "Juggling";
    public override PowerType Type => PowerType.Buff;

    private int _attacksThisTurn;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (card.Type != CardType.Attack) return;
        _attacksThisTurn++;
        if (_attacksThisTurn == 3)
            for (int i = 0; i < Amount && combat.Player.Hand.Count < Player.MaxHandSize; i++)
                combat.Player.Hand.Add((CardModel)card.Clone());
    }

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) _attacksThisTurn = 0;
    }

    public override PowerModel Clone() { var c = (JugglingPower)base.Clone(); c._attacksThisTurn = _attacksThisTurn; return c; }
    public override string StateKey() => $"{Id}={Amount}/{_attacksThisTurn}";
    public override long HashValue() => base.HashValue() ^ ((long)_attacksThisTurn * 0x9E3779B1L);
}
/// <summary>Crimson Mantle: at the start of each of the owner's turns, lose SelfDamage HP (unblockable)
/// then gain Amount Block. SelfDamage starts at 0 and rises by 1 per copy played (IncrementSelfDamage runs
/// on each application), so two Mantles → lose 2 HP / gain 2×Block each turn. (MegaCrit CrimsonMantlePower.)</summary>
public sealed class CrimsonMantlePower : PowerModel
{
    public override string Id => "CrimsonMantle";
    public override PowerType Type => PowerType.Buff;

    private int _selfDamage;   // grows by 1 each time the card is played (the game's IncrementSelfDamage)

    public override void AfterApplied(CombatState combat, Creature? applier) => _selfDamage++;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        if (_selfDamage > 0) Cmd.LoseHp(combat, Owner, _selfDamage);
        Cmd.GainBlock(combat, Owner, Amount, ValueProp.Unpowered, null);
    }

    public override PowerModel Clone()
    {
        var c = (CrimsonMantlePower)base.Clone();
        c._selfDamage = _selfDamage;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}/{_selfDamage}";
    public override long HashValue() => base.HashValue() ^ ((long)_selfDamage * 0x100000001B3L);
}
/// <summary>Cruelty: while owned, the owner's powered attacks against Vulnerable enemies deal extra — the
/// Vulnerable multiplier rises by Amount/100 (so Cruelty 25 makes ×1.5 → ×1.75). (MegaCrit CrueltyPower.)</summary>
public sealed class CrueltyPower : PowerModel
{
    public override string Id => "Cruelty";
    public override PowerType Type => PowerType.Buff;
    public override decimal VulnerableMultiplierBonus() => Amount / 100m;
}
/// <summary>Free Attack: the owner's next Amount Attacks cost 0 energy. Each consumes a charge when an
/// Attack whose cost it actually zeroed is played. (MegaCrit FreeAttackPower — Unrelenting.)</summary>
public sealed class FreeAttackPower : PowerModel
{
    public override string Id => "FreeAttack";
    public override PowerType Type => PowerType.Buff;

    public override int ModifyCardCost(CardModel card, int cost)
        => (card.Type == CardType.Attack && Amount > 0 && cost > 0) ? 0 : cost;

    public override void AfterModifyingCardCost(CombatState combat, CardModel card)
    {
        Amount--;
        this.NormalizeOrRemove(Owner);
    }
}
/// <summary>Corruption: while owned, the player's Skills cost 0 energy and are Exhausted when played.
/// (MegaCrit CorruptionPower.)</summary>
public sealed class CorruptionPower : PowerModel
{
    public override string Id => "Corruption";
    public override PowerType Type => PowerType.Buff;

    public override int ModifyCardCost(CardModel card, int cost)
        => card.Type == CardType.Skill ? 0 : cost;

    public override bool OverrideResultPileToExhaust(CardModel card)
        => card.Type == CardType.Skill;
}
/// <summary>One-Two Punch: the owner's next Amount Attacks each resolve one extra time. Each affected
/// Attack consumes a charge; any remaining are removed at the owner's turn end. (MegaCrit OneTwoPunchPower.)</summary>
public sealed class OneTwoPunchPower : PowerModel
{
    public override string Id => "OneTwoPunch";
    public override PowerType Type => PowerType.Buff;

    public override int ModifyCardPlayCount(CardModel card)
        => (card.Type == CardType.Attack && Amount > 0) ? 1 : 0;

    public override void AfterModifyingCardPlayCount(CombatState combat, CardModel card)
    {
        Amount--;
        this.NormalizeOrRemove(Owner);
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) Owner.RemovePower(Id);   // any unused charges expire at turn end
    }
}
/// <summary>Pyre: while owned, the player's max energy is increased by Amount, so each turn resets to a
/// higher value (the benefit lands from the next turn — the turn it's played is already past its reset).
/// (MegaCrit PyrePower.)</summary>
public sealed class PyrePower : PowerModel
{
    public override string Id => "Pyre";
    public override PowerType Type => PowerType.Buff;
    public override int ModifyMaxEnergy(Creature player) => player == Owner ? Amount : 0;
}
/// <summary>Feel No Pain: whenever one of the owner's cards is exhausted, gain Amount block (unpowered).
/// (MegaCrit FeelNoPainPower.)</summary>
public sealed class FeelNoPainPower : PowerModel
{
    public override string Id => "FeelNoPain";
    public override PowerType Type => PowerType.Buff;

    public override void AfterCardExhausted(CombatState combat, CardModel card, bool causedByEthereal)
        => Cmd.GainBlock(combat, Owner, Amount, ValueProp.Unpowered, null);
}
/// <summary>Dark Embrace: whenever one of the owner's cards is exhausted directly (by play/effect), draw
/// Amount cards; Ethereal-expiry exhausts are batched and drawn at the owner's turn end. Draws are
/// externally supplied during trace replay (no-op without an ambient Rng). (MegaCrit DarkEmbracePower.)</summary>
public sealed class DarkEmbracePower : PowerModel
{
    public override string Id => "DarkEmbrace";
    public override PowerType Type => PowerType.Buff;

    private int _etherealPending;

    public override void AfterCardExhausted(CombatState combat, CardModel card, bool causedByEthereal)
    {
        if (causedByEthereal) _etherealPending++;
        else Cmd.Draw(combat, Amount);
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side || _etherealPending == 0) return;
        Cmd.Draw(combat, Amount * _etherealPending);
        _etherealPending = 0;
    }

    public override PowerModel Clone()
    {
        var c = (DarkEmbracePower)base.Clone();
        c._etherealPending = _etherealPending;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}{(_etherealPending > 0 ? $"~{_etherealPending}" : "")}";
    public override long HashValue() => base.HashValue() ^ ((long)_etherealPending * unchecked((long)0x9E3779B97F4A7C15UL));
}
/// <summary>Battle Trance's marker: the owner cannot draw additional cards for the rest of the turn
/// (honoured by Cmd.Draw). Removed at the owner's turn end. Inert during trace replay (mid-turn draws are
/// externally supplied), so it never shows at a checkpoint. (MegaCrit NoDrawPower.)</summary>
public sealed class NoDrawPower : PowerModel
{
    public override string Id => "NoDraw";
    public override PowerType Type => PowerType.Debuff;

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) Owner.RemovePower(Id);
    }
}
/// <summary>Barricade: the owner's block is no longer cleared at the start of its turn — it persists and
/// accumulates across turns. (MegaCrit BarricadePower.)</summary>
public sealed class BarricadePower : PowerModel
{
    public override string Id => "Barricade";
    public override PowerType Type => PowerType.Buff;
    public override bool PreventsBlockClear => true;
}
/// <summary>Mangle: temporarily reduces the target enemy's Strength by Amount until its turn ends, so its
/// upcoming attack hits softer. (MegaCrit ManglePower.)</summary>
public sealed class ManglePower : TemporaryStrengthPower
{
    public override string Id => "Mangle";
    protected override int Sign => -1;
}
/// <summary>Setup Strike: grants the player Amount Strength for the rest of this turn (undone at the
/// player's turn end), boosting subsequent attacks. (MegaCrit SetupStrikePower.)</summary>
public sealed class SetupStrikePower : TemporaryStrengthPower
{
    public override string Id => "SetupStrike";
    protected override int Sign => 1;
}
