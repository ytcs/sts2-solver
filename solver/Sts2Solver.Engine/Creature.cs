namespace Sts2Solver.Engine;

/// <summary>
/// A combat participant (player or monster). Holds HP, block and powers, plus the helpers the
/// pipelines and hooks use. Mirrors MegaCrit.Sts2.Core.Entities.Creatures.Creature (the bits that
/// affect combat resolution; animation/VFX state is dropped).
/// </summary>
public abstract class Creature
{
    public string Name = "";
    public int CurrentHp;
    public int MaxHp;
    public int Block;
    public CombatSide Side;

    /// <summary>Powered (Move, non-Unpowered) attack hits this creature has received from the player during
    /// the current player turn (reset at player-turn start). Regent's BeatIntoShape forges based on the
    /// target's prior such hits. Only tracked + hashed when a deck reads it
    /// (<see cref="CombatState.TracksPoweredHits"/>), so the common case isn't fragmented.</summary>
    public int PlayerPoweredHitsThisTurn;

    public readonly List<PowerModel> Powers = new();

    public virtual bool IsAlive => CurrentHp > 0;
    public bool IsPlayer => Side == CombatSide.Player;
    public bool IsEnemy => Side == CombatSide.Enemy;

    public PowerModel? GetPower(string id) => Powers.FirstOrDefault(p => p.Id == id);
    public int GetPowerAmount(string id) => GetPower(id)?.Amount ?? 0;
    public bool HasPower(string id) => GetPower(id) != null;

    /// <summary>
    /// Apply (stack) a power. Counter-stacked powers add their amounts; a new power is attached
    /// fresh. Drops to ≤0 remove the power unless it allows negative stacks.
    /// </summary>
    public void AddPower(PowerModel power, int amount)
    {
        var existing = GetPower(power.Id);
        if (existing != null)
        {
            existing.Amount += amount;
            existing.NormalizeOrRemove(this);
            return;
        }
        power.Owner = this;
        power.Amount = amount;
        if (power.NormalizeOrRemove(this)) return; // removed immediately if ≤0 and not allowed
        Powers.Add(power);
    }

    public void RemovePower(string id) => Powers.RemoveAll(p => p.Id == id);

    public void GainBlockDirect(int amount)
    {
        if (amount > 0) Block += amount;
    }

    public void ClearBlock() => Block = 0;

    /// <summary>Reduce HP by an already-finalised amount (post-block, post-modifiers). Floors at 0.</summary>
    public void LoseHpInternal(int amount)
    {
        if (amount <= 0) return;
        CurrentHp = Math.Max(0, CurrentHp - amount);
    }

    public void Heal(int amount)
    {
        if (amount <= 0) return;
        CurrentHp = Math.Min(MaxHp, CurrentHp + amount);
    }

    /// <summary>Raise max HP and heal by the same amount (Feed's permanent gain on a kill).</summary>
    public void GainMaxHp(int amount)
    {
        if (amount <= 0) return;
        MaxHp += amount;
        CurrentHp += amount;
    }

    /// <summary>Lose max HP (Brightest Flame). Lowers MaxHp and clamps CurrentHp down to the new cap
    /// (the game's LoseMaxHp). Floors MaxHp at 1.</summary>
    public void LoseMaxHp(int amount)
    {
        if (amount <= 0) return;
        MaxHp = Math.Max(1, MaxHp - amount);
        CurrentHp = Math.Min(CurrentHp, MaxHp);
    }

    protected void CopyCreatureBaseTo(Creature dst)
    {
        dst.Name = Name;
        dst.CurrentHp = CurrentHp;
        dst.MaxHp = MaxHp;
        dst.Block = Block;
        dst.Side = Side;
        dst.PlayerPoweredHitsThisTurn = PlayerPoweredHitsThisTurn;
        dst.Powers.Clear();
        foreach (var p in Powers)
        {
            var pc = p.Clone();
            pc.Owner = dst;
            dst.Powers.Add(pc);
        }
    }

    public abstract Creature Clone();

    /// <summary>Contribute this creature's state to the structural hash. Overrides add their extras.</summary>
    public virtual void Hash(ref StateHasher h)
    {
        h.Add(CurrentHp);
        h.Add(Block);
        Span<long> buf = stackalloc long[Powers.Count];
        for (int i = 0; i < Powers.Count; i++) buf[i] = Powers[i].HashValue();
        h.AddSorted(buf);
    }

    public virtual string StateKey()
    {
        var powers = string.Join(",", Powers.OrderBy(p => p.Id).Select(p => p.StateKey()));
        return $"{Name}|hp{CurrentHp}|b{Block}|[{powers}]";
    }
}

public static class PowerNormalize
{
    /// <summary>Returns true if the power was removed.</summary>
    public static bool NormalizeOrRemove(this PowerModel power, Creature owner)
    {
        if (power.AllowNegative) return false;
        if (power.Amount <= 0)
        {
            owner.Powers.Remove(power);
            return true;
        }
        return false;
    }
}
