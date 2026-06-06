using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>At the owner's turn end, grant the owner Strength equal to Amount. Skips the turn-end it was
/// applied on when applied by an enemy. (MegaCrit RitualPower)</summary>
/// <summary>When an ally dies, the owner devours it: gains Strength equal to Amount and is stunned for one
/// turn (skips its next move). The stun must be applied actively — when the player kills the ally mid-turn,
/// the devour fires AFTER the turn-start telegraph snapshot, so the owner's already-committed attack would
/// otherwise still resolve. Overriding the committed move to a no-op "STUNNED" matches the game (the
/// validator re-syncs the real follow-up move from the next turn-start snapshot). (MegaCrit RavenousPower)</summary>
public sealed class RavenousPower : PowerModel
{
    public override string Id => "Ravenous";
    public override PowerType Type => PowerType.Buff;

    public override void AfterCreatureDeath(CombatState combat, Creature dead)
    {
        if (dead != Owner && dead.Side == Owner.Side && Owner.IsAlive)
        {
            Cmd.ApplyPower(combat, Owner, new StrengthPower(), Amount, Owner);
            if (Owner is Monster m) m.Ai.CurrentMoveId = "STUNNED";   // devouring stuns the owner this turn
        }
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
/// <summary>Decimillipede segments carry Reattach(25): a segment brought to 0 HP does NOT die while another
/// segment lives — it goes DOWNED, then reattaches; only a blow that downs the LAST segment ends the fight.
/// MODELLED 1:1 from the decompile's move machine (DEAD_MOVE → REATTACH_MOVE): on downing, the engine strips the
/// segment's non-Reattach powers (standard death cleanup; ReattachPower survives owner death — matches the trace
/// showing a 0-HP segment keeping only ReattachPower) and sets <see cref="Monster.ReattachIn"/>=2; the segment
/// sits at 0 HP (untargetable, doesn't act) for one enemy turn (DEAD_MOVE), then on the next reattaches — healing
/// to 25 if another segment is still alive (<see cref="CombatManager.RunEnemyTurn"/>). This closes an OPTIMISTIC
/// gap: an inert Reattach let the search clear the board one segment at a time across turns. (An earlier cut
/// revived at end-of-enemy-turn, one turn too early, and the oracle trace rejected it — the 2-turn DEAD→REATTACH
/// delay is what the trace and decompile require.) (MegaCrit ReattachPower.)</summary>
public sealed class ReattachPower : PowerModel
{
    public override string Id => "Reattach";
    public override PowerType Type => PowerType.Buff;
}
/// <summary>SpectralKnight's Hex: makes all the player's cards Ethereal (decompile HexPower applies the Hexed
/// affliction, which adds the Ethereal keyword to every player card). MODELLED in forward search:
/// <see cref="CombatManager.EndPlayerTurn"/> exhausts the WHOLE hand while the player has Hex — faithful, and the
/// sound direction (thins the deck, never inflates it, so search can't over-credit by retaining a card the game
/// would exhaust). During trace replay the recorded hands already reflect the thinning. (MegaCrit HexPower.)</summary>
public sealed class HexPower : PowerModel
{
    public override string Id => "Hex";
    public override PowerType Type => PowerType.Debuff;
}
/// <summary>MagiKnight's Dampen: in-game it fully downgrades every upgraded player card while the caster lives
/// (restored on its death; decompile DampenPower.AfterApplied → CardCmd.Downgrade). MODELLED (was an OPTIMISTIC
/// gap — an inert Dampen let an UPGRADED deck keep damage/block the real game strips). When applied we downgrade
/// every upgraded player card across all piles to its base (replacing the instance — never mutating a shared one,
/// matching Armaments). We deliberately do NOT restore on the caster's death: that over-states the harm (cards
/// stay base for the rest of the fight) — the SOUND/pessimistic direction, never optimistic. Idempotent (a second
/// application finds nothing upgraded left). (MegaCrit DampenPower.)</summary>
public sealed class DampenPower : PowerModel
{
    public override string Id => "Dampen";
    public override PowerType Type => PowerType.Debuff;

    public override void AfterApplied(CombatState combat, Creature? applier)
    {
        if (Owner != combat.Player) return;   // Dampen is a player debuff; nothing to do otherwise
        DowngradePile(combat.Player.Hand);
        DowngradePile(combat.Player.DrawPile);
        DowngradePile(combat.Player.DiscardPile);
        DowngradePile(combat.Player.ExhaustPile);
    }

    private static void DowngradePile(System.Collections.Generic.List<CardModel> pile)
    {
        for (int i = 0; i < pile.Count; i++)
        {
            if (pile[i].Upgrades <= 0) continue;
            // Rebuild a fresh base (Upgrades 0) instance by name — a clean downgrade with no stale cached key.
            try { pile[i] = Catalog.BuildCard(pile[i].Name); }
            catch (System.ArgumentException) { /* name not buildable (shouldn't happen) — leave as-is */ }
        }
    }
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
/// <summary>WaterfallGiant's Pressure Gun ramp counter: a per-creature tally of how many times Pressure Gun has
/// fired. Each Pressure Gun resolves for <c>BasePressureGunDamage + 5×Amount</c> (the game's
/// CurrentPressureGunDamage, which permanently grows +5 per use), then increments this. Inert as a hook — it has no
/// passive effect; the move reads <c>GetPowerAmount("PressureGun")</c> directly. Stored as a power so the ramp clones
/// and hashes with the monster (base StateKey/HashValue serialise the Amount), keeping every search branch sound.
/// (MegaCrit WaterfallGiant.CurrentPressureGunDamage / PressureGunIncrease.)</summary>
public sealed class PressureGunPower : PowerModel
{
    public override string Id => "PressureGun";
    public override PowerType Type => PowerType.Buff;
}
/// <summary>WaterfallGiant's Steam Eruption accumulator: a per-creature counter that grows as the boss takes its
/// turns (Pressurize +PressurizeAmount = 15, or 20 on DeadlyEnemies; every other move +3), capturing the size of
/// its guaranteed death-phase explosion. When the boss is brought to 0 HP it does NOT die (death-phase, see
/// <see cref="Monster.DeathPhaseEntryMove"/>): it telegraphs ABOUT_TO_BLOW for one turn, then EXPLODES for damage
/// equal to this counter before truly dying. Inert as a hook — it has no passive effect; the EXPLODE move reads
/// <c>GetPowerAmount("SteamEruption")</c> directly. Stored as a power so the counter clones and hashes with the
/// monster (base StateKey/HashValue serialise the Amount), keeping every search branch sound. (MegaCrit
/// WaterfallGiant.SteamEruptionPower / SteamEruptionDamage.)</summary>
public sealed class SteamEruptionPower : PowerModel
{
    public override string Id => "SteamEruption";
    public override PowerType Type => PowerType.Buff;
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

/// <summary>When a teammate (the other KaiserCrab arm) dies, the owner RAGES: gains 6 Strength and 99 Block,
/// then removes itself (fires once). Killing one arm enrages the survivor — HARM to the player, so modelled for
/// soundness. The amounts are the game's fixed CanonicalVars (Strength 6, Block 99). (MegaCrit CrabRagePower.)</summary>
public sealed class CrabRagePower : PowerModel
{
    public override string Id => "CrabRage";
    public override PowerType Type => PowerType.Buff;

    public override void AfterCreatureDeath(CombatState combat, Creature dead)
    {
        if (dead == Owner || dead.Side != Owner.Side || !Owner.IsAlive) return;
        Cmd.ApplyPower(combat, Owner, new StrengthPower(), 6, Owner);
        Cmd.GainBlock(combat, Owner, 99, ValueProp.Unpowered, null);
        Owner.RemovePower(Id);
    }
}

/// <summary>KaiserCrab "surrounded" back attack. Each arm carries this (Crusher = left, Rocket = right). The arm
/// the player is NOT facing deals ×1.5 with its attacks (game: SurroundedPower + BackAttackLeft/RightPower). The
/// player faces whichever arm they last targeted — tracked as <see cref="CombatState.KaiserFrontId"/>, set in
/// <see cref="CombatManager.PlayCard"/>; the initial facing (id 0) leaves the LEFT arm behind. "Behind" is
/// recomputed at the player's turn end (a hook that has the combat state) and cached for the per-hit
/// <see cref="ModifyDamageMultiplicative"/>, which has none. Once only ONE arm remains the player faces it, so no
/// back attack (and CrabRage has already fired). The cache is derived purely from the hashed
/// <see cref="CombatState.KaiserFrontId"/> + which arms are alive, so it need not be hashed itself.</summary>
public sealed class KaiserBackAttackPower : PowerModel
{
    public bool IsLeft;          // Crusher = left, Rocket = right
    private bool _behind;

    public override string Id => "KaiserBackAttack";
    public override PowerType Type => PowerType.Buff;

    public override decimal ModifyDamageMultiplicative(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
        => (dealer == Owner && target != null && target.IsPlayer && _behind) ? 1.5m : 1m;

    public override void AfterApplied(CombatState combat, Creature? applier) => Recompute(combat);

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == CombatSide.Player) Recompute(combat);   // finalise facing before the arms attack
    }

    private void Recompute(CombatState combat)
    {
        int living = 0;
        foreach (var m in combat.Monsters) if (m.IsAlive && m.HasPower(Id)) living++;
        if (living < 2) { _behind = false; return; }   // lone survivor is faced — no back attack
        _behind = combat.KaiserFrontId == 0 ? IsLeft : (Owner is Monster mo && mo.Id != combat.KaiserFrontId);
    }

    public override PowerModel Clone()
    {
        var c = (KaiserBackAttackPower)base.Clone();
        c.IsLeft = IsLeft;
        c._behind = _behind;
        return c;
    }

    public override string StateKey() => $"{Id}{(IsLeft ? "L" : "R")}";
    public override long HashValue() => base.HashValue() ^ (IsLeft ? 0x1F83D9ABL : 0x428A2F98L);
}

/// <summary>TestSubject's revive (the boss's 3 forms). While owned, the boss does NOT die at 0 HP — it heals to
/// its next form and switches its AI to the RESPAWN move (which branches to that phase's move set). 1st revive →
/// Second-form HP + PainfulStabs; 2nd revive → Third-form HP + Nemesis, and it drops Adaptable + PainfulStabs so
/// the 3rd death is real. (MegaCrit AdaptablePower.)</summary>
public sealed class AdaptablePower : PowerModel
{
    public int SecondFormHp, ThirdFormHp;
    public override string Id => "Adaptable";
    public override PowerType Type => PowerType.Buff;

    public override bool VetoLethalDamage(CombatState combat, Monster owner)
    {
        owner.Respawns++;
        if (owner.Respawns == 1)
        {
            owner.MaxHp = SecondFormHp; owner.CurrentHp = SecondFormHp;
            Cmd.ApplyPower(combat, owner, new PainfulStabsPower(), 1, owner);
        }
        else
        {
            owner.MaxHp = ThirdFormHp; owner.CurrentHp = ThirdFormHp;
            Cmd.ApplyPower(combat, owner, new NemesisPower(), 1, owner);
            owner.RemovePower("PainfulStabs");
            owner.RemovePower(Id);   // no more revives — the next death is fatal
        }
        owner.Ai.CurrentMoveId = "RESPAWN_MOVE";   // run the revive move, then branch to the phase move set
        return true;
    }

    public override PowerModel Clone()
    {
        var c = (AdaptablePower)base.Clone();
        c.SecondFormHp = SecondFormHp; c.ThirdFormHp = ThirdFormHp;
        return c;
    }
}

/// <summary>When the player plays a Skill, the owner gains Amount Strength. (MegaCrit EnragePower — TestSubject.)</summary>
public sealed class EnragePower : PowerModel
{
    public override string Id => "Enrage";
    public override PowerType Type => PowerType.Buff;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (card.Type == CardType.Skill) Cmd.ApplyPower(combat, Owner, new StrengthPower(), Amount, Owner);
    }
}

/// <summary>When the owner lands a powered attack dealing unblocked damage to the player, add 1 Wound to the
/// player's discard — at most once per enemy turn (the per-hit hook fires for every hit of a multi-hit attack, so
/// a per-turn flag keeps it to the game's once-per-attack while TestSubject attacks once a turn in this phase).
/// (MegaCrit PainfulStabsPower.)</summary>
public sealed class PainfulStabsPower : PowerModel
{
    private bool _woundedThisTurn;
    public override string Id => "PainfulStabs";
    public override PowerType Type => PowerType.Buff;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (_woundedThisTurn || dealer != Owner || !target.IsPlayer || unblockedDamage <= 0 || !props.IsPoweredAttack()) return;
        _woundedThisTurn = true;
        Cmd.GenerateStatusCard(combat, new Wound(), combat.Player.DiscardPile);
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) _woundedThisTurn = false;   // reset for the next enemy turn
    }

    public override PowerModel Clone()
    {
        var c = (PainfulStabsPower)base.Clone();
        c._woundedThisTurn = _woundedThisTurn;
        return c;
    }
    public override string StateKey() => $"{Id}={Amount}{(_woundedThisTurn ? "*" : "")}";
    public override long HashValue() => base.HashValue() ^ (_woundedThisTurn ? 0x2545F491L : 0L);
}

/// <summary>At the owner's turn end, toggles "intangible": on alternating turns every hit the owner takes is
/// capped to 1 (caps HP loss like IntangiblePower, but self-toggled so it never collides with Intangible's own
/// per-turn decrement). (MegaCrit NemesisPower — TestSubject's third form.)</summary>
public sealed class NemesisPower : PowerModel
{
    private bool _on;
    public override string Id => "Nemesis";
    public override PowerType Type => PowerType.Buff;

    public override int ModifyHpLost(Creature target, int hpLost, ValueProp props, Creature? dealer)
        => (_on && target == Owner && hpLost >= 1) ? 1 : hpLost;

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) _on = !_on;   // toggle each of the owner's turn ends (intangible every other turn)
    }

    public override PowerModel Clone()
    {
        var c = (NemesisPower)base.Clone();
        c._on = _on;
        return c;
    }
    public override string StateKey() => $"{Id}={Amount}{(_on ? "I" : "")}";
    public override long HashValue() => base.HashValue() ^ (_on ? 0x61C88647L : 0L);
}

/// <summary>Queen's Puppet Strings (game ChainsOfBindingPower, applied to the player at amount 3). In the game it
/// Binds N of the cards the player DRAWS each turn (Bound ⇒ unplayable that turn). Faithful binding hooks every
/// draw — including the turn-start hand, which the search models as chance nodes — so it cannot be reproduced
/// exactly without reworking the draw system (the same reason the affliction subsystem is descoped). Modelled
/// SOUNDLY (never optimistic) as drawing N FEWER cards each turn via the existing <see cref="ModifyHandDraw"/>
/// hook (which every draw path honours): a Bound card is unplayable that turn, and drawing one fewer is at least
/// as harmful, since the game's bound cards still recycle. A documented pessimistic approximation.</summary>
public sealed class QueenChainsPower : PowerModel
{
    public override string Id => "QueenChains";
    public override PowerType Type => PowerType.Debuff;
    public override int ModifyHandDraw(Creature player, int count) => System.Math.Max(0, count - Amount);
}

/// <summary>Rides on the Queen to notice when her TorchHeadAmalgam ally dies, flipping her move machine from the
/// Burn Bright (buff-the-Amalgam) branch to the Off-With-Your-Head attack branch (game: Queen.AfterDeath sets
/// HasAmalgamDied). The flag is read by the Queen's conditional move branch.</summary>
public sealed class QueenAmalgamWatchPower : PowerModel
{
    public bool AmalgamDead;
    public override string Id => "QueenAmalgamWatch";
    public override PowerType Type => PowerType.Buff;

    public override void AfterCreatureDeath(CombatState combat, Creature dead)
    {
        if (dead != Owner && dead.Side == Owner.Side && dead is Monster m && m.Name == "TorchHeadAmalgam")
            AmalgamDead = true;
    }

    public static bool Died(Monster queen) => (queen.GetPower("QueenAmalgamWatch") as QueenAmalgamWatchPower)?.AmalgamDead == true;

    public override PowerModel Clone() { var c = (QueenAmalgamWatchPower)base.Clone(); c.AmalgamDead = AmalgamDead; return c; }
    public override string StateKey() => AmalgamDead ? "QAW!" : "QAW";
    public override long HashValue() => base.HashValue() ^ (AmalgamDead ? 0x71374491L : 0L);
}
