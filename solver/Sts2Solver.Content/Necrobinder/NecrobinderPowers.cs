using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>Summon helper for Osty (mirrors OstyCmd.Summon). An alive Osty grows MaxHp and heals by the
/// same amount; a missing/dead one is (re)created at full HP with DieForYou attached. Summons of 0 (or
/// less) are no-ops, matching the game's ModifySummonAmount==0 early-out.</summary>
public static class NecroOsty
{
    public static void Summon(CombatState combat, int amount)
    {
        if (amount <= 0) return;
        var p = combat.Player;
        if (p.IsOstyAlive)
        {
            p.Osty!.MaxHp += amount;       // GainMaxHp: raise the cap and heal by the same
            p.Osty.CurrentHp += amount;
        }
        else
        {
            var o = p.Osty ?? new Osty();
            o.MaxHp = amount;
            o.CurrentHp = amount;
            o.Block = 0;
            if (!o.HasPower("DieForYou")) o.AddPower(new DieForYouPower(), 1);
            p.Osty = o;
        }
    }

    /// <summary>An Osty-powered attack: dealt by Osty (so Calcify applies, the player's Strength does not),
    /// fizzling while Osty is missing. <paramref name="hits"/> resolves the hit count. Bumps the
    /// per-turn Osty-attack counter once. Returns total HP dealt to the (single) target.</summary>
    public static int OstyHit(CombatState combat, Creature target, int dmg, CardModel card, int hits = 1)
    {
        if (combat.Player.IsOstyMissing) return 0;
        int lost = Cmd.AttackMulti(combat, combat.Player.Osty!, target, dmg, hits, ValueProp.Move, card);
        combat.OstyAttacksThisTurn++;
        return lost;
    }

    /// <summary>An Osty-powered attack against every living enemy (Bone Shards, High Five, Sweeping Gaze).</summary>
    public static void OstyHitAll(CombatState combat, int dmg, CardModel card)
    {
        if (combat.Player.IsOstyMissing) return;
        foreach (var m in combat.LivingMonsters.ToList())
            Cmd.Attack(combat, combat.Player.Osty!, m, dmg, ValueProp.Move, card);
        combat.OstyAttacksThisTurn++;
    }
}

// ─────────────────────────── Osty core ───────────────────────────

/// <summary>On Osty. Redirects powered attacks aimed at the player onto Osty while Osty is alive — Osty
/// "dies for you". Non-powered loss (poison, self-damage) is not redirected. (Game: DieForYouPower.)</summary>
public sealed class DieForYouPower : PowerModel
{
    public override string Id => "DieForYou";
    public override PowerType Type => PowerType.Buff;

    public override Creature ModifyUnblockedDamageTarget(Creature target, int unblocked, ValueProp props, Creature? dealer)
    {
        if (Owner.CurrentHp <= 0) return target;          // a dead Osty cannot tank
        if (!props.IsPoweredAttack()) return target;
        return target is Player ? Owner : target;         // pull the player's post-block hit onto Osty
    }
}

/// <summary>On the player. Whenever Osty loses HP — to a redirected enemy hit, or to being sacrificed —
/// deal that much × Amount to every enemy, unblockable. (Game: NecroMasteryPower.AfterCurrentHpChanged.)</summary>
public sealed class NecroMasteryPower : PowerModel
{
    public override string Id => "NecroMastery";
    public override PowerType Type => PowerType.Buff;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (unblockedDamage <= 0 || target is not Osty) return;
        foreach (var m in combat.HittableEnemies.ToList())
            Cmd.ApplyDamage(combat, m, unblockedDamage * Amount, ValueProp.Unblockable | ValueProp.Unpowered, combat.Player);
    }
}

/// <summary>Doom (counter debuff). A creature whose current HP is at or below its Doom stacks dies at the
/// end of its own side's turn. End of Days kills doomed enemies immediately via <see cref="KillDoomed"/>.
/// Doom does not tick down. Self-Doom (Neurosurge) can execute the player. (Game: DoomPower.)</summary>
public sealed class DoomPower : PowerModel
{
    public override string Id => "Doom";
    public override PowerType Type => PowerType.Debuff;

    public bool IsDoomed => Owner.CurrentHp <= Amount;

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side && Owner.IsAlive && IsDoomed) Cmd.Kill(combat, Owner);
    }

    /// <summary>Immediately kill every doomed creature among <paramref name="creatures"/> (End of Days).</summary>
    public static void KillDoomed(CombatState combat, IEnumerable<Creature> creatures)
    {
        foreach (var c in creatures.ToList())
            if (c.IsAlive && c.GetPower("Doom") is DoomPower d && d.IsDoomed) Cmd.Kill(combat, c);
    }
}

// IntangiblePower is cross-character (Silent/Special/Necrobinder), so it lives in Core/CommonPowers.cs — the
// canonical version caps HP loss directly (covers poison/unblockable, not just the engine's ApplyDamage clamp).

// ─────────────────────────── Osty-attack / Doom synergy ───────────────────────────

/// <summary>On the player. Osty's powered attacks deal +Amount damage (Strength for Osty). (Game: CalcifyPower.)</summary>
public sealed class CalcifyPower : PowerModel
{
    public override string Id => "Calcify";
    public override PowerType Type => PowerType.Buff;

    public override decimal ModifyDamageAdditive(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
        => (dealer is Osty && props.IsPoweredAttack()) ? Amount : 0m;
}

/// <summary>On the player. At the start of each player turn, apply Amount Doom to an enemy. (Game: CountdownPower.)</summary>
public sealed class CountdownPower : PowerModel
{
    public override string Id => "Countdown";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != CombatSide.Player) return;
        var enemy = combat.HittableEnemies.FirstOrDefault();   // game picks a random enemy; first is HP-equivalent here
        if (enemy != null) Cmd.ApplyPower(combat, enemy, new DoomPower(), Amount, Owner);
    }
}

/// <summary>On the player. At the start of each player turn, gain Amount Doom yourself — a self-execute
/// risk traded for Neurosurge's energy/draw. (Game: NeurosurgePower.)</summary>
public sealed class NeurosurgePower : PowerModel
{
    public override string Id => "Neurosurge";
    public override PowerType Type => PowerType.Debuff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == CombatSide.Player) Cmd.ApplyPower(combat, Owner, new DoomPower(), Amount, Owner);
    }
}

/// <summary>On the player. After a powered attack from the player or Osty deals damage, apply Doom equal
/// to that damage × Amount to the target. (Game: ReaperFormPower.)</summary>
public sealed class ReaperFormPower : PowerModel
{
    public override string Id => "ReaperForm";
    public override PowerType Type => PowerType.Buff;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (unblockedDamage <= 0 || !props.IsPoweredAttack() || target is not Monster) return;
        if (dealer != combat.Player && dealer is not Osty) return;
        Cmd.ApplyPower(combat, target, new DoomPower(), unblockedDamage * Amount, combat.Player);
    }
}

/// <summary>On the player. The first Attack you play each turn deals (1 + Amount/100)× damage. (Game: LethalityPower.)</summary>
public sealed class LethalityPower : PowerModel
{
    public override string Id => "Lethality";
    public override PowerType Type => PowerType.Buff;

    private bool _used;   // an Attack has already been played this turn

    public override decimal ModifyDamageMultiplicative(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (_used || cardSource is null || cardSource.Type != CardType.Attack || !props.IsPoweredAttack()) return 1m;
        return 1m + (decimal)Amount / 100m;
    }

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (card.Type == CardType.Attack) _used = true;
    }

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == CombatSide.Player) _used = false;
    }

    public override PowerModel Clone() { var c = (LethalityPower)base.Clone(); c._used = _used; return c; }
    public override string StateKey() => $"{Id}={Amount}{(_used ? "u" : "")}";
    public override long HashValue() => base.HashValue() ^ (_used ? 0x4C45 : 0);
}

/// <summary>On an enemy. Hang cards deal ×Amount damage to it (and each Hang doubles the stack). (Game: HangPower.)</summary>
public sealed class HangPower : PowerModel
{
    public override string Id => "Hang";
    public override PowerType Type => PowerType.Debuff;

    public override decimal ModifyDamageMultiplicative(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
        => (target == Owner && cardSource?.Name == "Hang") ? Amount : 1m;
}

/// <summary>On an enemy. Doubles the Vulnerable damage it takes and the Weak penalty it deals, on powered
/// attacks. Ticks down at the enemy's turn end. (Game: DebilitatePower.)</summary>
public sealed class DebilitatePower : PowerModel
{
    public override string Id => "Debilitate";
    public override PowerType Type => PowerType.Debuff;

    public override decimal TransformVulnerableMultiplier(decimal mult) => mult + (mult - 1m);   // 1.5 -> 2.0
    public override decimal TransformWeakMultiplier(decimal mult) => mult - (1m - mult);         // 0.75 -> 0.5

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) { Amount--; this.NormalizeOrRemove(Owner); }
    }
}

/// <summary>On an enemy. While present, Osty hitting this enemy summons Amount. Removed at the player's
/// turn end. (Game: SicEmPower.)</summary>
public sealed class SicEmPower : PowerModel
{
    public override string Id => "SicEm";
    public override PowerType Type => PowerType.Debuff;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (target == Owner && dealer is Osty) NecroOsty.Summon(combat, Amount);
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == CombatSide.Player) Owner.RemovePower(Id);
    }
}

/// <summary>On the player. Applying a (non-temporary) debuff to an enemy deals Amount to it. (Game: SleightOfFleshPower.)</summary>
public sealed class SleightOfFleshPower : PowerModel
{
    public override string Id => "SleightOfFlesh";
    public override PowerType Type => PowerType.Buff;

    public override void AfterPowerApplied(CombatState combat, Creature target, PowerModel power, int amount, Creature? applier)
    {
        if (amount == 0 || applier != Owner || !target.IsEnemy || power.Type != PowerType.Debuff) return;
        if (power is TemporaryStrengthPower) return;       // the game excludes temporary (one-turn) debuffs
        Cmd.ApplyDamage(combat, target, Amount, ValueProp.Unpowered, Owner);
    }
}

/// <summary>On the player. Applying Doom gains Amount block. (Game: ShroudPower.)</summary>
public sealed class ShroudPower : PowerModel
{
    public override string Id => "Shroud";
    public override PowerType Type => PowerType.Buff;

    public override void AfterPowerApplied(CombatState combat, Creature target, PowerModel power, int amount, Creature? applier)
    {
        if (applier == Owner && power is DoomPower) Cmd.GainBlock(combat, Owner, Amount, ValueProp.Unpowered, null);
    }
}

/// <summary>On the player. Playing a Soul deals Amount unblockable to an enemy. (Game: HauntPower.)</summary>
public sealed class HauntPower : PowerModel
{
    public override string Id => "Haunt";
    public override PowerType Type => PowerType.Buff;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (card.Name != "Soul") return;
        var enemy = combat.HittableEnemies.FirstOrDefault();
        if (enemy != null) Cmd.ApplyDamage(combat, enemy, Amount, ValueProp.Unblockable | ValueProp.Unpowered, Owner);
    }
}

/// <summary>On the player. Playing a Soul summons Amount. (Game: DevourLifePower.)</summary>
public sealed class DevourLifePower : PowerModel
{
    public override string Id => "DevourLife";
    public override PowerType Type => PowerType.Buff;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (card.Name == "Soul") NecroOsty.Summon(combat, Amount);
    }
}

/// <summary>On an enemy. Loses Amount Strength for its upcoming turn (undone at its turn end). A
/// temporary debuff, so it does NOT trigger Sleight of Flesh. (Game: EnfeeblingTouchPower.)</summary>
public sealed class EnfeeblingTouchPower : TemporaryStrengthPower
{
    public override string Id => "EnfeeblingTouch";
    protected override int Sign => -1;
}

// ─────────────────────────── Block / energy / cost ───────────────────────────

/// <summary>On the player. Before playing a card costing ≥2, gain Amount block. (Game: DanseMacabrePower.)</summary>
public sealed class DanseMacabrePower : PowerModel
{
    public override string Id => "DanseMacabre";
    public override PowerType Type => PowerType.Buff;

    public override void BeforeCardPlayed(CombatState combat, CardModel card)
    {
        if (card.Cost >= 2) Cmd.GainBlock(combat, Owner, Amount, ValueProp.Unpowered, null);
    }
}

/// <summary>On the player. Before playing an Ethereal card, gain Amount block. (Game: SpiritOfAshPower.)</summary>
public sealed class SpiritOfAshPower : PowerModel
{
    public override string Id => "SpiritOfAsh";
    public override PowerType Type => PowerType.Buff;

    public override void BeforeCardPlayed(CombatState combat, CardModel card)
    {
        if (card.Ethereal) Cmd.GainBlock(combat, Owner, Amount, ValueProp.Unpowered, null);
    }
}

/// <summary>On an enemy. The next card you play this turn applies Amount Doom to it; removed at the
/// player's turn end. (Game: OblivionPower — applies on every card played while active.)</summary>
public sealed class OblivionPower : PowerModel
{
    public override string Id => "Oblivion";
    public override PowerType Type => PowerType.Debuff;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        Cmd.ApplyPower(combat, Owner, new DoomPower(), Amount, Owner);
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == CombatSide.Player) Owner.RemovePower(Id);
    }
}

/// <summary>On the player. +Amount max energy each turn. (Game: FriendshipPower / DemesnePower energy half.)</summary>
public sealed class FriendshipPower : PowerModel
{
    public override string Id => "Friendship";
    public override PowerType Type => PowerType.Buff;
    public override int ModifyMaxEnergy(Creature player) => Amount;
}

/// <summary>On the player. +Amount max energy and +Amount cards drawn each turn (the draw bonus is inert
/// here — mid-combat draw is RNG/validator-driven, HP-neutral). (Game: DemesnePower.)</summary>
public sealed class DemesnePower : PowerModel
{
    public override string Id => "Demesne";
    public override PowerType Type => PowerType.Buff;
    public override int ModifyMaxEnergy(Creature player) => Amount;
}

/// <summary>On the player. Cards cost Amount more this turn; removed at the player's turn end.
/// (Game: BorrowedTimePower — the downside paired with Borrowed Time's energy.)</summary>
public sealed class BorrowedTimePower : PowerModel
{
    public override string Id => "BorrowedTime";
    public override PowerType Type => PowerType.Debuff;

    public override int ModifyCardCost(CardModel card, int cost) => cost + Amount;

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == CombatSide.Player) Owner.RemovePower(Id);
    }
}

/// <summary>On the player. The next Amount Ethereal cards you play cost 0. (Game: VeilpiercerPower.)</summary>
public sealed class VeilpiercerPower : PowerModel
{
    public override string Id => "Veilpiercer";
    public override PowerType Type => PowerType.Buff;

    public override int ModifyCardCost(CardModel card, int cost) => (card.Ethereal && Amount > 0) ? 0 : cost;

    public override void BeforeCardPlayed(CombatState combat, CardModel card)
    {
        if (card.Ethereal && Amount > 0) { Amount--; this.NormalizeOrRemove(Owner); }
    }
}

// EnergyNextTurnPower is cross-character, so it lives in Core/CommonPowers.cs.

/// <summary>On the player. Summon Amount at the start of next turn, then expire. (Game: SummonNextTurnPower.)</summary>
public sealed class SummonNextTurnPower : PowerModel
{
    public override string Id => "SummonNextTurn";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != CombatSide.Player) return;
        NecroOsty.Summon(combat, Amount);
        Owner.RemovePower(Id);
    }
}

// ─────────────────────────── Documented-inert powers ───────────────────────────
// These hook subsystems the HP-faithful engine does not model (card generation, mid-combat draw count,
// post-combat rewards). They attach as inert markers so the cards that grant them still resolve.

/// <summary>Game: when you draw an Ethereal card, draw Amount. Inert — mid-combat draw is RNG/validator
/// driven and HP-neutral. (PagestormPower.)</summary>
public sealed class PagestormPower : PowerModel
{
    public override string Id => "Pagestorm";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Game: each turn, add Amount SweepingGaze (Osty attacks) to hand. Inert — card generation is
/// not modelled. (SentryModePower.)</summary>
public sealed class SentryModePower : PowerModel
{
    public override string Id => "SentryMode";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Game: each turn, add Amount random Ethereal cards to hand. Inert — card generation is not
/// modelled. (CallOfTheVoidPower.)</summary>
public sealed class CallOfTheVoidPower : PowerModel
{
    public override string Id => "CallOfTheVoid";
    public override PowerType Type => PowerType.Buff;
}

// ForbiddenGrimoire (Ancient) + its inert ForbiddenGrimoirePower are owned by the Event/Ancient pool in
// Content/Special/ (identical card), not duplicated here.
