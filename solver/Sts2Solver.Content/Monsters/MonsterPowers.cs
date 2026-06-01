using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>At the owner's turn end, grant the owner Strength equal to Amount. Skips the turn-end it was
/// applied on when applied by an enemy. (MegaCrit RitualPower)</summary>
/// <summary>When an ally dies, the owner devours it and gains Strength equal to Amount. (The in-game
/// one-turn stun is reflected by the observed/injected move during replay.) (MegaCrit RavenousPower)</summary>
public sealed class RavenousPower : PowerModel
{
    public override string Id => "Ravenous";
    public override PowerType Type => PowerType.Buff;

    public override void AfterCreatureDeath(CombatState combat, Creature dead)
    {
        if (dead != Owner && dead.Side == Owner.Side && Owner.IsAlive)
            Cmd.ApplyPower(combat, Owner, new StrengthPower(), Amount, Owner);
    }
}
/// <summary>At the owner's turn end, grant the owner Strength equal to Amount (no skip, unlike Ritual).
/// (MegaCrit TerritorialPower — Byrdonis ramps Strength each turn.)</summary>
public sealed class TerritorialPower : PowerModel
{
    public override string Id => "Territorial";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side)
            Cmd.ApplyPower(combat, Owner, new StrengthPower(), Amount, Owner);
    }
}
/// <summary>Counts cards played by the player this turn; each one makes powered attacks against the
/// owner deal ×(1 + 0.1·count) damage. Resets at the owner's turn start. The card currently being
/// played does NOT count toward its own damage (the counter ticks after the effect resolves).
/// (MegaCrit SlowPower — BygoneEffigy starts with it.) The applied stack count (Amount) is cosmetic;
/// behaviour is driven solely by the per-turn counter.</summary>
public sealed class SlowPower : PowerModel
{
    public override string Id => "Slow";
    public override PowerType Type => PowerType.Debuff;

    private int _cardsPlayed;

    public override void AfterCardPlayed(CombatState combat, CardModel card) => _cardsPlayed++;

    public override decimal ModifyDamageMultiplicative(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (target != Owner) return 1m;
        if (!props.IsPoweredAttack()) return 1m;
        return 1m + 0.1m * _cardsPlayed;
    }

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) _cardsPlayed = 0;
    }

    public override PowerModel Clone()
    {
        var c = (SlowPower)base.Clone();
        c._cardsPlayed = _cardsPlayed;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}/{_cardsPlayed}";
    public override long HashValue() => base.HashValue() ^ ((long)_cardsPlayed * 0x9E3779B1L);
}
/// <summary>When the owner dies, it bursts into 4 Wrigglers (stunned for their first turn), keeping the
/// combat alive — a two-phase elite. Bite-first / wriggle-first alternate by spawn slot (1,3 bite-first;
/// 2,4 wriggle-first), matching the game's per-slot conditional. (MegaCrit InfestedPower — Phrog Parasite.)</summary>
public sealed class InfestedPower : PowerModel
{
    public override string Id => "Infested";
    public override PowerType Type => PowerType.Buff;

    public override void AfterCreatureDeath(CombatState combat, Creature dead)
    {
        if (dead != Owner) return;
        for (int i = 0; i < 4; i++)
            Cmd.Summon(combat, Monsters.Wriggler(stunned: true, biteFirst: i % 2 == 0));
    }
}
/// <summary>Decimillipede segments carry Reattach(25): a segment brought to 0 HP does not die while any
/// other segment lives — it goes dormant, then reattaches (heals to 25) on its next turn; only when all
/// other segments are already dead does a lethal blow end the fight. NOTE: the revival itself is NOT yet
/// modelled (it needs engine support for "downed" creatures + a segment-aware win check); this is a
/// marker so the segment's Reattach amount validates and a fight that never downs a segment passes.
/// (MegaCrit ReattachPower — a known modelling TODO for the solver.)</summary>
public sealed class ReattachPower : PowerModel
{
    public override string Id => "Reattach";
    public override PowerType Type => PowerType.Buff;
}
/// <summary>SpectralKnight's Hex: in-game it makes all the player's cards Ethereal (exhaust when held).
/// That deck-thinning is fully captured by the recorded hands during trace replay, and the effect is HP-
/// neutral, so we model Hex as an inert player-side marker debuff. NOTE: the solver does not yet make the
/// player's cards Ethereal under Hex (a known modelling gap for forward search). (MegaCrit HexPower.)</summary>
public sealed class HexPower : PowerModel
{
    public override string Id => "Hex";
    public override PowerType Type => PowerType.Debuff;
}
/// <summary>MagiKnight's Dampen: in-game it downgrades the player's upgraded cards (restored on the
/// caster's death). HP-neutral, not checked by the validator, and a no-op on an unupgraded starter deck,
/// so we model it as an inert marker. NOTE: card downgrading is not modelled for the solver. (Dampen.)</summary>
public sealed class DampenPower : PowerModel
{
    public override string Id => "Dampen";
    public override PowerType Type => PowerType.Debuff;
}
/// <summary>The first time the owner takes unblocked damage from a player attack each turn, it gains
/// Amount block (reactively, after that hit). The once-per-turn latch resets at the player's turn end.
/// (MegaCrit SkittishPower — Phantasmal Gardeners, 6.)</summary>
public sealed class SkittishPower : PowerModel
{
    public override string Id => "Skittish";
    public override PowerType Type => PowerType.Buff;

    private bool _gainedThisTurn;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (target != Owner || unblockedDamage <= 0 || _gainedThisTurn) return;
        if (dealer == null || !dealer.IsPlayer || !props.IsPoweredAttack()) return;
        _gainedThisTurn = true;
        Cmd.GainBlock(combat, Owner, Amount, ValueProp.Unpowered, null);
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) _gainedThisTurn = false;   // reset at the player's turn end
    }

    public override PowerModel Clone()
    {
        var c = (SkittishPower)base.Clone();
        c._gainedThisTurn = _gainedThisTurn;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}{(_gainedThisTurn ? "*" : "")}";
    public override long HashValue() => base.HashValue() ^ (_gainedThisTurn ? 0x6F4A7C15L : 0L);
}
/// <summary>The player's debuff side of Vital Spark: while held, the player takes +Amount damage from
/// each powered attack. Removed at enemy turn end. Stacks if several Tainted skills are played in a turn.
/// (MegaCrit TaintedPower.)</summary>
public sealed class TaintedPower : PowerModel
{
    public override string Id => "Tainted";
    public override PowerType Type => PowerType.Debuff;

    public override decimal ModifyDamageAdditive(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (target != Owner || !props.IsPoweredAttack()) return 0m;
        return Amount;
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == CombatSide.Enemy) Owner.RemovePower(Id);
    }
}
/// <summary>Taints every player Skill: playing one applies Tainted(Amount) to the player (so it takes
/// +Amount/attack that turn). The taint amount tracks the live Vital Spark, which Pulsate grows. We model
/// this directly (no per-card affliction state): on any Skill played while the living owner has Vital
/// Spark, apply Tainted(Amount) to the player. (MegaCrit VitalSparkPower — InfestedPrism 2.)</summary>
public sealed class VitalSparkPower : PowerModel
{
    public override string Id => "VitalSpark";
    public override PowerType Type => PowerType.Buff;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (card.Type == CardType.Skill && Owner.IsAlive && Amount > 0)
            Cmd.ApplyPower(combat, combat.Player, new TaintedPower(), Amount, Owner);
    }
}
/// <summary>Caps the owner's total HP loss to Amount per turn — damage beyond that is negated. The
/// per-turn counter resets at each side-turn start. Amount (the cap) is constant (what the recorder
/// dumps); the spent amount is tracked internally. (MegaCrit HardenedShellPower — SkulkingColony 20.)</summary>
public sealed class HardenedShellPower : PowerModel
{
    public override string Id => "HardenedShell";
    public override PowerType Type => PowerType.Buff;

    private int _takenThisTurn;

    public override int ModifyHpLost(Creature target, int hpLost, ValueProp props, Creature? dealer)
    {
        if (target != Owner || hpLost <= 0) return hpLost;
        int remaining = Math.Max(0, Amount - _takenThisTurn);
        return Math.Min(hpLost, remaining);
    }

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (target == Owner && unblockedDamage > 0) _takenThisTurn += unblockedDamage;
    }

    public override void AfterSideTurnStart(CombatState combat, CombatSide side) => _takenThisTurn = 0;

    public override PowerModel Clone()
    {
        var c = (HardenedShellPower)base.Clone();
        c._takenThisTurn = _takenThisTurn;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}/{_takenThisTurn}";
    public override long HashValue() => base.HashValue() ^ ((long)_takenThisTurn * 0x27D4EB2FL);
}
/// <summary>Negates the next Amount debuffs applied to the owner, consuming one charge per negated
/// debuff. (MegaCrit ArtifactPower — MechaKnight starts with Artifact 3.)</summary>
public sealed class ArtifactPower : PowerModel
{
    public override string Id => "Artifact";
    public override PowerType Type => PowerType.Buff;

    public override bool TryAbsorbDebuff(CombatState combat, PowerModel incoming)
    {
        if (Amount <= 0) return false;
        Amount--;
        this.NormalizeOrRemove(Owner);
        return true;   // the incoming debuff is negated
    }
}
/// <summary>Each time the owner is hit by a powered attack, it shuffles Amount Dazed cards into the
/// attacker's draw pile. Entomancer starts with Personal Hive 1 and grows it (to 3) via Pheromone Spit.
/// (MegaCrit PersonalHivePower.)</summary>
public sealed class PersonalHivePower : PowerModel
{
    public override string Id => "PersonalHive";
    public override PowerType Type => PowerType.Buff;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (target != Owner || dealer == null || !dealer.IsPlayer || !props.IsPoweredAttack()) return;
        for (int i = 0; i < Amount; i++) combat.Player.DrawPile.Add(new Dazed());
    }
}
/// <summary>The owner's next powered attack deals +Amount, then the power is consumed. (MegaCrit
/// VigorPower — TerrorEel's Thrash grants itself Vigor 6, boosting its following Crash 16→22.)
/// Caveat: a multi-hit attack consumes it after its first hit (the eel only ever boosts single-hit Crash).</summary>
public sealed class VigorPower : PowerModel
{
    public override string Id => "Vigor";
    public override PowerType Type => PowerType.Buff;

    public override decimal ModifyDamageAdditive(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (dealer != Owner) return 0m;
        if (!props.IsPoweredAttack()) return 0m;
        return Amount;
    }

    public override void AfterAttackDealt(CombatState combat, Creature dealer, ValueProp props)
    {
        if (dealer == Owner && props.IsPoweredAttack()) Owner.RemovePower(Id);
    }
}
/// <summary>The owner (TerrorEel) carries this as a counter (70). When the owner takes unblocked damage
/// that brings it to ≤Amount HP, it is stunned and forced into its Terror move (STUN → TERROR → resume),
/// and the power is removed (one-time). The stun is modelled as the eel's STUN_MOVE, whose FollowUp
/// telegraphs TERROR. (MegaCrit ShriekPower.)</summary>
public sealed class ShriekPower : PowerModel
{
    public const string StunStateId = "STUN_MOVE";

    public override string Id => "Shriek";
    public override PowerType Type => PowerType.Debuff;
    public override bool AllowNegative => true;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (target != Owner || unblockedDamage <= 0) return;
        if (!Owner.IsAlive || Owner.CurrentHp > Amount) return;   // not yet at the threshold (dead = moot)
        if (Owner is Monster m) m.Ai.CurrentMoveId = StunStateId;  // interrupt: next turn STUN, then TERROR
        Owner.RemovePower(Id);                                      // one-time trigger
    }
}
/// <summary>ShrinkerBeetle's Shrink: applied to the PLAYER, it reduces the damage of the player's powered
/// attacks by 30% (multiplicative ×0.7) for as long as it is owned. ShrinkerBeetle applies it with
/// Amount -1, which the game treats as "infinite" — it never ticks down and lasts the whole combat (so the
/// owner here is the player and it persists). A positive Amount would be a countdown that decrements at the
/// owner's turn end; ShrinkerBeetle never uses that, so we model the -1 (whole-combat) case and keep the
/// countdown path faithful. The 30% reduction is the game constant ShrinkPower.damageDecrease.
/// (MegaCrit ShrinkPower.) UNIT-TESTED ONLY — not trace-validated.</summary>
public sealed class ShrinkPower : PowerModel
{
    public override string Id => "Shrink";
    public override PowerType Type => PowerType.Debuff;
    public override bool AllowNegative => true;   // ShrinkerBeetle applies -1 = infinite

    private bool IsInfinite => Amount < 0;

    public override decimal ModifyDamageMultiplicative(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (dealer != Owner) return 1m;            // only the shrunk creature's own attacks are weakened
        if (!props.IsPoweredAttack()) return 1m;
        return 0.7m;                                // (100 - 30) / 100
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (IsInfinite) return;                     // -1 = never expires
        if (side != Owner.Side) return;             // count down on the owner's own turn end
        Amount--;
        this.NormalizeOrRemove(Owner);
    }
}
/// <summary>Slippery: an Intangible-like damage cap. While the owner has Slippery, each instance of HP loss
/// it would take is capped to 1, and the counter (Amount) decrements by 1 each time the owner takes ≥1
/// unblocked damage; at 0 the power is gone. Inklets start combat with Slippery 1 (their first incoming hit
/// is reduced to 1 HP, then it wears off). Modelled via the engine's ModifyHpLost cap + AfterDamageReceived
/// decrement; the counter rides on the base power Amount, so base StateKey/HashValue already serialise it
/// (no extra mutable field). (MegaCrit SlipperyPower.) UNIT-TESTED ONLY — not trace-validated.</summary>
public sealed class SlipperyPower : PowerModel
{
    public override string Id => "Slippery";
    public override PowerType Type => PowerType.Buff;

    public override int ModifyHpLost(Creature target, int hpLost, ValueProp props, Creature? dealer)
    {
        if (target != Owner || Amount <= 0 || hpLost < 1) return hpLost;
        return 1;   // game: ModifyHpLostAfterOsty caps to 1
    }

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (target != Owner || Amount <= 0 || unblockedDamage < 1) return;
        Amount--;                       // consumed one "dodge"
        this.NormalizeOrRemove(Owner);
    }
}
public sealed class RitualPower : PowerModel
{
    public override string Id => "Ritual";
    public override PowerType Type => PowerType.Buff;

    private bool _skipNextTrigger;

    public override void AfterApplied(CombatState combat, Creature? applier)
    {
        if (Owner.IsEnemy) _skipNextTrigger = true;
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;       // only the owner's own turn end
        if (_skipNextTrigger) { _skipNextTrigger = false; return; }
        Cmd.ApplyPower(combat, Owner, new StrengthPower(), Amount, Owner);
    }

    public override PowerModel Clone()
    {
        var c = (RitualPower)base.Clone();
        c._skipNextTrigger = _skipNextTrigger;
        return c;
    }

    public override string StateKey() => $"{Id}={Amount}{(_skipNextTrigger ? "*" : "")}";
    public override long HashValue() => base.HashValue() ^ (_skipNextTrigger ? 0x5BD1E995L : 0L);
}
