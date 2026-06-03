namespace Sts2Solver.Engine;

// ===========================================================================
// The Defect's Orb subsystem (game: MegaCrit.Sts2.Core.Models.Orbs + OrbQueue + OrbCmd).
//
// An orb is CHANNELED into an ordered queue of bounded capacity (the Defect's
// BaseOrbSlotCount = 3). Channeling into a full queue first EVOKES the oldest
// (front) orb, then appends the new orb at the back. Most orbs fire a PASSIVE
// at the player's TURN END (Plasma fires at TURN START); EVOKE triggers when an
// orb leaves the queue. Focus (FocusPower) adds to every focus-affected orb's
// passive/evoke value (clamped ≥0); Plasma is NOT focus-affected.
//
// Gating: a non-Defect player never channels, so OrbSlots stays 0 and the queue
// stays empty — Player hashing/clone/StateKey contribute orb state only then.
// ===========================================================================

/// <summary>A channeled orb. Subclasses define the passive/evoke effect and base values; Dark and Glass
/// carry mutable per-combat accumulators and so are <see cref="Stateful"/> (deep-cloned + value-hashed).</summary>
public abstract class OrbModel
{
    public abstract string Name { get; }

    /// <summary>True for orbs with mutable per-combat state (Dark's growing evoke, Glass's decaying passive)
    /// — these must be deep-cloned per search branch and hashed by value.</summary>
    public virtual bool Stateful => false;

    /// <summary>Plasma fires its passive at the player's TURN START; every other orb at TURN END.</summary>
    public virtual bool PassiveAtTurnStart => false;

    /// <summary>Plasma's energy is fixed; all other orbs add the owner's Focus to their value (clamped ≥0).</summary>
    protected virtual bool FocusAffected => true;

    protected abstract int BasePassive { get; }
    protected abstract int BaseEvoke { get; }

    /// <summary>Game FocusPower.ModifyOrbValue: max(0, value + Focus) for focus-affected orbs.</summary>
    protected int Focused(CombatState c, int v)
        => FocusAffected ? System.Math.Max(0, v + c.Player.GetPowerAmount("Focus")) : v;

    public virtual int PassiveVal(CombatState c) => Focused(c, BasePassive);
    public virtual int EvokeVal(CombatState c) => Focused(c, BaseEvoke);

    /// <summary>Fire the per-turn passive effect.</summary>
    public abstract void Passive(CombatState c);
    /// <summary>Fire the evoke effect (when the orb leaves the queue).</summary>
    public abstract void Evoke(CombatState c);

    public virtual OrbModel Clone() => (OrbModel)MemberwiseClone();
    public virtual void Hash(ref StateHasher h) => h.Add(Name.GetHashCode());
    public virtual string StateKey() => Name;

    /// <summary>Single random/​default enemy for Lightning: random under a concrete Rng (rollout / replay),
    /// the first living enemy in search (Rng null) — the deterministic, pessimistic-sound default used by every
    /// random-target effect (Ricochet/RipAndTear). Exact for single-enemy; an approximation otherwise.</summary>
    protected static Creature? PickEnemy(CombatState c)
    {
        var living = c.LivingMonsters.ToList();
        if (living.Count == 0) return null;
        return c.Rng != null ? living[c.Rng.NextInt(living.Count)] : living[0];
    }
}

/// <summary>Lightning (passive 3 / evoke 8, +Focus). Deals unpowered damage to a random enemy on each
/// trigger. (Game LightningOrb.)</summary>
public sealed class LightningOrb : OrbModel
{
    public override string Name => "Lightning";
    protected override int BasePassive => 3;
    protected override int BaseEvoke => 8;
    public override void Passive(CombatState c)
    {
        var t = PickEnemy(c);
        if (t != null) Cmd.Attack(c, c.Player, t, PassiveVal(c), ValueProp.Unpowered, null);
    }
    public override void Evoke(CombatState c)
    {
        var t = PickEnemy(c);
        if (t != null) Cmd.Attack(c, c.Player, t, EvokeVal(c), ValueProp.Unpowered, null);
    }
}

/// <summary>Frost (passive 2 / evoke 5, +Focus). Gains unpowered block on each trigger. (Game FrostOrb.)</summary>
public sealed class FrostOrb : OrbModel
{
    public override string Name => "Frost";
    protected override int BasePassive => 2;
    protected override int BaseEvoke => 5;
    public override void Passive(CombatState c) => Cmd.GainBlock(c, c.Player, PassiveVal(c), ValueProp.Unpowered, null);
    public override void Evoke(CombatState c) => Cmd.GainBlock(c, c.Player, EvokeVal(c), ValueProp.Unpowered, null);
}

/// <summary>Dark (passive 6, +Focus). Its passive ACCUMULATES into the evoke value (starting at 6); evoking
/// deals the accumulated total to the WEAKEST (lowest-HP) enemy. Stateful. (Game DarkOrb.)</summary>
public sealed class DarkOrb : OrbModel
{
    public override string Name => "Dark";
    public override bool Stateful => true;
    protected override int BasePassive => 6;
    protected override int BaseEvoke => 6;
    private int _evokeVal = 6;                       // accumulator; not focus-modified at evoke time
    public override int EvokeVal(CombatState c) => _evokeVal;
    public override void Passive(CombatState c) => _evokeVal += PassiveVal(c);
    public override void Evoke(CombatState c)
    {
        var living = c.LivingMonsters.ToList();
        if (living.Count == 0) return;
        var weakest = living.OrderBy(m => m.CurrentHp).First();   // game: HittableEnemies.MinBy(CurrentHp)
        Cmd.Attack(c, c.Player, weakest, _evokeVal, ValueProp.Unpowered, null);
    }
    public override void Hash(ref StateHasher h) { h.Add(Name.GetHashCode()); h.Add(_evokeVal); }
    public override string StateKey() => $"Dark#{_evokeVal}";
}

/// <summary>Plasma (passive 1 / evoke 2). Gains energy — passive at TURN START, evoke any time. NOT
/// focus-affected. (Game PlasmaOrb.)</summary>
public sealed class PlasmaOrb : OrbModel
{
    public override string Name => "Plasma";
    public override bool PassiveAtTurnStart => true;
    protected override bool FocusAffected => false;
    protected override int BasePassive => 1;
    protected override int BaseEvoke => 2;
    public override void Passive(CombatState c) => Cmd.GainEnergy(c, PassiveVal(c));
    public override void Evoke(CombatState c) => Cmd.GainEnergy(c, EvokeVal(c));
}

/// <summary>Glass (passive starts 4, +Focus; evoke = passive × 2). Passive deals its value to ALL enemies
/// then DECAYS by 1 (min 0); evoke deals double the current passive to all enemies. Stateful. (Game GlassOrb.)</summary>
public sealed class GlassOrb : OrbModel
{
    public override string Name => "Glass";
    public override bool Stateful => true;
    protected override int BasePassive => 4;
    protected override int BaseEvoke => 0;          // unused: EvokeVal is overridden
    private int _passiveVal = 4;
    public override int PassiveVal(CombatState c) => Focused(c, _passiveVal);
    public override int EvokeVal(CombatState c) => PassiveVal(c) * 2;
    public override void Passive(CombatState c)
    {
        int v = PassiveVal(c);                        // game deals the PRE-decrement value, then decays
        if (v <= 0) return;
        _passiveVal = System.Math.Max(0, _passiveVal - 1);
        foreach (var m in c.LivingMonsters.ToList()) Cmd.Attack(c, c.Player, m, v, ValueProp.Unpowered, null);
    }
    public override void Evoke(CombatState c)
    {
        int v = EvokeVal(c);
        if (v <= 0) return;
        foreach (var m in c.LivingMonsters.ToList()) Cmd.Attack(c, c.Player, m, v, ValueProp.Unpowered, null);
    }
    public override void Hash(ref StateHasher h) { h.Add(Name.GetHashCode()); h.Add(_passiveVal); }
    public override string StateKey() => $"Glass#{_passiveVal}";
}

/// <summary>Channel / evoke / slot operations on the player's orb queue (game OrbCmd). The queue is FIFO:
/// the front is the oldest orb (evoked first on overflow / by Dualcast), the back the newest.</summary>
public static class OrbOps
{
    public const int MaxSlots = 10;

    /// <summary>Channel an orb. If the queue is full, the oldest (front) orb is evoked first to make room,
    /// then the new orb is appended at the back. (Game OrbCmd.Channel.)</summary>
    public static void Channel(CombatState combat, OrbModel orb)
    {
        var p = combat.Player;
        if (p.OrbSlots <= 0) return;                  // no slots → channel is a no-op (Defect has 3)
        // Cumulative Lightning-channel tally (Voltaic) — counted on every channel attempt, gated so only a
        // Voltaic deck tracks/hashes it. Counted even if the orb later overflows (the game logs the channel).
        if (combat.TracksLightningChanneled && orb is LightningOrb) combat.LightningsChanneledThisCombat++;
        if (p.Orbs.Count >= p.OrbSlots) EvokeFront(combat);
        if (p.Orbs.Count < p.OrbSlots) p.Orbs.Add(orb);
    }

    /// <summary>Evoke the oldest (front) orb. <paramref name="dequeue"/> false fires the effect without
    /// removing it (Dualcast's first evoke). (Game OrbCmd.EvokeNext.)</summary>
    public static void EvokeFront(CombatState combat, bool dequeue = true)
    {
        var p = combat.Player;
        if (p.Orbs.Count == 0) return;
        var orb = p.Orbs[0];
        if (dequeue) p.Orbs.RemoveAt(0);
        orb.Evoke(combat);
        FireEvoked(combat, orb);
    }

    /// <summary>Evoke the newest (back) orb. (Game OrbCmd.EvokeLast.)</summary>
    public static void EvokeBack(CombatState combat, bool dequeue = true)
    {
        var p = combat.Player;
        if (p.Orbs.Count == 0) return;
        int last = p.Orbs.Count - 1;
        var orb = p.Orbs[last];
        if (dequeue) p.Orbs.RemoveAt(last);
        orb.Evoke(combat);
        FireEvoked(combat, orb);
    }

    /// <summary>Broadcast an orb evoke to every power (ThunderPower reacts to Lightning evokes). Inert when
    /// no power overrides <see cref="OrbModel.AfterOrbEvoked"/> — i.e. for every non-Thunder deck.</summary>
    private static void FireEvoked(CombatState combat, OrbModel orb)
    {
        foreach (var pw in combat.AllPowers.ToList()) pw.AfterOrbEvoked(combat, orb);
    }

    /// <summary>Evoke every orb currently in the queue, front to back (Reboot / Multi-evoke effects).</summary>
    public static void EvokeAll(CombatState combat)
    {
        while (combat.Player.Orbs.Count > 0) EvokeFront(combat);
    }

    public static void AddSlots(CombatState combat, int n)
        => combat.Player.OrbSlots = System.Math.Min(MaxSlots, combat.Player.OrbSlots + n);

    /// <summary>Remove orb slots, evoking the newest orbs that no longer fit. (Game OrbQueue.RemoveCapacity.)</summary>
    public static void RemoveSlots(CombatState combat, int n)
    {
        var p = combat.Player;
        p.OrbSlots = System.Math.Max(0, p.OrbSlots - n);
        while (p.Orbs.Count > p.OrbSlots) EvokeBack(combat);
    }

    /// <summary>Fire turn-boundary orb passives: at the player's TURN START only Plasma triggers; at TURN END
    /// every other orb. Orbs fire in queue order. (Game OrbQueue.AfterTurnStart / BeforeTurnEnd.)</summary>
    public static void TriggerPassives(CombatState combat, bool turnStart)
    {
        var p = combat.Player;
        if (p.Orbs.Count == 0) return;
        foreach (var orb in p.Orbs.ToList())
        {
            if (orb.PassiveAtTurnStart != turnStart) continue;
            if (combat.IsCombatOver) break;
            orb.Passive(combat);
        }
    }
}
