using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// Hidden relic POWERS carrying per-turn / per-combat counters (batch 4). A relic is shared + immutable and so
// cannot hold a mutable counter soundly; instead its relic installs one of these powers at combat start. A power
// is cloned + hashed by the engine, and only present when the relic is, so the counter is sound for search/
// memoisation and inert for any deck without the relic. The counter is folded into StateKey + HashValue.

/// <summary>Base for a relic that counts qualifying card plays and fires an effect every <see cref="Threshold"/>th.
/// The counter is subtracted (not modulo-grown), so it stays in [0, Threshold) — bounded, so the hash is stable.
/// This fires at the same plays as the game's "counter++ then counter % N == 0".</summary>
public abstract class RelicPlayCounterPower : PowerModel
{
    protected int Count;                              // qualifying plays since the last fire (or since the turn reset)
    protected abstract int Threshold { get; }
    protected abstract bool Counts(CardModel card);
    protected abstract void Fire(CombatState combat);
    protected virtual bool ResetEachTurn => false;    // per-turn counters (Kunai/Shuriken/OrnamentalFan/LetterOpener)
    public override PowerType Type => PowerType.Buff;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (Owner != combat.Player || !Counts(card)) return;
        if (++Count >= Threshold) { Count -= Threshold; Fire(combat); }
    }

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (ResetEachTurn && side == Owner.Side) Count = 0;
    }

    public override PowerModel Clone() { var c = (RelicPlayCounterPower)base.Clone(); c.Count = Count; return c; }
    public override string StateKey() => $"{Id}#{Count}";
    public override long HashValue() => ((long)Id.GetHashCode() << 20) ^ (uint)Count;
}

/// <summary>Kunai: every 3rd Attack played in a turn grants 1 Dexterity. (MegaCrit Kunai.)</summary>
public sealed class KunaiPower : RelicPlayCounterPower
{
    public override string Id => "RelicKunai";
    protected override int Threshold => 3;
    protected override bool ResetEachTurn => true;
    protected override bool Counts(CardModel card) => card.Type == CardType.Attack;
    protected override void Fire(CombatState combat) => Cmd.ApplyPower(combat, Owner, new DexterityPower(), 1, Owner);
}

/// <summary>Shuriken: every 3rd Attack played in a turn grants 1 Strength. (MegaCrit Shuriken.)</summary>
public sealed class ShurikenPower : RelicPlayCounterPower
{
    public override string Id => "RelicShuriken";
    protected override int Threshold => 3;
    protected override bool ResetEachTurn => true;
    protected override bool Counts(CardModel card) => card.Type == CardType.Attack;
    protected override void Fire(CombatState combat) => Cmd.ApplyPower(combat, Owner, new StrengthPower(), 1, Owner);
}

/// <summary>Ornamental Fan: every 3rd Attack played in a turn gains 4 Block. (MegaCrit OrnamentalFan.)</summary>
public sealed class OrnamentalFanPower : RelicPlayCounterPower
{
    public override string Id => "RelicOrnamentalFan";
    protected override int Threshold => 3;
    protected override bool ResetEachTurn => true;
    protected override bool Counts(CardModel card) => card.Type == CardType.Attack;
    protected override void Fire(CombatState combat) => Cmd.GainBlock(combat, Owner, 4, ValueProp.Unpowered, null);
}

/// <summary>Letter Opener: every 3rd Skill played in a turn deals 5 damage to ALL enemies. (MegaCrit LetterOpener.)</summary>
public sealed class LetterOpenerPower : RelicPlayCounterPower
{
    public override string Id => "RelicLetterOpener";
    protected override int Threshold => 3;
    protected override bool ResetEachTurn => true;
    protected override bool Counts(CardModel card) => card.Type == CardType.Skill;
    protected override void Fire(CombatState combat) => RelicAoe.DamageAllEnemies(combat, 5);
}

/// <summary>Nunchaku: every 10th Attack played (across the combat) grants 1 energy. (MegaCrit Nunchaku.)</summary>
public sealed class NunchakuPower : RelicPlayCounterPower
{
    public override string Id => "RelicNunchaku";
    protected override int Threshold => 10;
    protected override bool Counts(CardModel card) => card.Type == CardType.Attack;
    protected override void Fire(CombatState combat) => Cmd.GainEnergy(combat, 1);
}

/// <summary>Tuning Fork: every 10th Skill played (across the combat) gains 7 Block. (MegaCrit TuningFork.)</summary>
public sealed class TuningForkPower : RelicPlayCounterPower
{
    public override string Id => "RelicTuningFork";
    protected override int Threshold => 10;
    protected override bool Counts(CardModel card) => card.Type == CardType.Skill;
    protected override void Fire(CombatState combat) => Cmd.GainBlock(combat, Owner, 7, ValueProp.Unpowered, null);
}

/// <summary>Iron Club: every 4th card played (across the combat) draws 1 card. (MegaCrit IronClub.)</summary>
public sealed class IronClubPower : RelicPlayCounterPower
{
    public override string Id => "RelicIronClub";
    protected override int Threshold => 4;
    protected override bool Counts(CardModel card) => true;
    protected override void Fire(CombatState combat) => Cmd.Draw(combat, 1);
}

/// <summary>Permafrost: the FIRST Power played each combat gains 7 Block (once per combat). The used-flag is
/// hashed so search can't replay it. (MegaCrit Permafrost.)</summary>
public sealed class PermafrostPower : PowerModel
{
    private bool _used;
    public override string Id => "RelicPermafrost";
    public override PowerType Type => PowerType.Buff;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (_used || Owner != combat.Player || card.Type != CardType.Power) return;
        _used = true;
        Cmd.GainBlock(combat, Owner, 7, ValueProp.Unpowered, null);
    }

    public override PowerModel Clone() { var c = (PermafrostPower)base.Clone(); c._used = _used; return c; }
    public override string StateKey() => _used ? "RelicPermafrost!" : "RelicPermafrost";
    public override long HashValue() => base.HashValue() ^ (_used ? 0x5BD1E9955BD1E995L : 0L);
}

/// <summary>Rainbow Ring: the first turn you play an Attack, a Skill AND a Power, gain 1 Strength and 1
/// Dexterity (once per turn). The per-turn type flags are hashed. (MegaCrit RainbowRing.)</summary>
public sealed class RainbowRingPower : PowerModel
{
    private bool _atk, _skl, _pow, _fired;
    public override string Id => "RelicRainbowRing";
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) { _atk = _skl = _pow = _fired = false; }
    }

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (_fired || Owner != combat.Player) return;
        if (card.Type == CardType.Attack) _atk = true;
        else if (card.Type == CardType.Skill) _skl = true;
        else if (card.Type == CardType.Power) _pow = true;
        if (_atk && _skl && _pow)
        {
            _fired = true;
            Cmd.ApplyPower(combat, Owner, new StrengthPower(), 1, Owner);
            Cmd.ApplyPower(combat, Owner, new DexterityPower(), 1, Owner);
        }
    }

    public override PowerModel Clone()
    { var c = (RainbowRingPower)base.Clone(); c._atk = _atk; c._skl = _skl; c._pow = _pow; c._fired = _fired; return c; }
    public override string StateKey() => $"RelicRainbowRing{(_atk ? "a" : "")}{(_skl ? "s" : "")}{(_pow ? "p" : "")}{(_fired ? "!" : "")}";
    public override long HashValue()
        => base.HashValue() ^ (_atk ? 1L : 0) ^ (_skl ? 2L : 0) ^ (_pow ? 4L : 0) ^ (_fired ? 8L : 0);
}
