using Sts2Solver.Engine;

namespace Sts2Solver.Content;

// ===========================================================================
// The Defect's powers. Focus is the signature orb modifier; other orb-reactive
// powers (Electrodynamics, Loop, Storm, Echo Form, …) are added as their cards
// are ported.
// ===========================================================================

/// <summary>Focus: adds its amount to every focus-affected orb's passive/evoke value (Lightning / Frost /
/// Dark / Glass — NOT Plasma), clamped ≥0 per value. A signed counter (BiasedCognition can drive it negative,
/// hence <see cref="AllowNegative"/>). Read directly by <see cref="OrbModel"/>.Focused. (Game FocusPower.)</summary>
public sealed class FocusPower : PowerModel
{
    public override string Id => "Focus";
    public override PowerType Type => PowerType.Buff;
    public override bool AllowNegative => true;
}

/// <summary>One for All: the owner's 0-cost Attacks deal +Amount extra. (MegaCrit OneForAllPower.)</summary>
public sealed class OneForAllPower : PowerModel
{
    public override string Id => "OneForAll";
    public override PowerType Type => PowerType.Buff;
    public override decimal ModifyDamageAdditive(Creature? target, decimal amount, ValueProp props, Creature? dealer, CardModel? cardSource)
    {
        if (dealer != Owner || cardSource == null || !props.IsPoweredAttack()) return 0m;
        return cardSource.Cost == 0 ? Amount : 0m;
    }
}

// ===========================================================================
// Batch 3 — orb-reactive / turn-boundary powers. Each is verified 1:1 vs the
// decompile (hook + numbers). Gated naturally: a power only acts when present.
// ===========================================================================

/// <summary>Helper: deal <paramref name="amount"/> Unpowered damage to every living enemy (orb/power AoE).</summary>
internal static class PowerFx
{
    public static void DamageAllEnemies(CombatState combat, int amount)
    {
        if (amount <= 0) return;
        foreach (var m in combat.LivingMonsters.ToList())
        {
            if (combat.IsCombatOver) break;
            Cmd.Attack(combat, combat.Player, m, amount, ValueProp.Unpowered, null);
        }
    }
}

/// <summary>Thunder: whenever the player EVOKES a Lightning orb, deal Amount damage to all enemies. Base 6,
/// +2/upg. (Game ThunderPower, hook AfterOrbEvoked.)</summary>
public sealed class ThunderPower : PowerModel
{
    public override string Id => "Thunder";
    public override PowerType Type => PowerType.Buff;
    public override void AfterOrbEvoked(CombatState combat, OrbModel orb)
    {
        if (orb is LightningOrb) PowerFx.DamageAllEnemies(combat, Amount);
    }
}

/// <summary>Hailstorm: at the player's turn end, if at least 1 Frost orb is in the queue, deal Amount to all
/// enemies. Base 6, +2/upg. (Game HailstormPower, hook BeforeSideTurnEnd / FrostOrbs≥1.)</summary>
public sealed class HailstormPower : PowerModel
{
    public override string Id => "Hailstorm";
    public override PowerType Type => PowerType.Buff;
    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        if (combat.Player.Orbs.Count(o => o is FrostOrb) < 1) return;
        PowerFx.DamageAllEnemies(combat, Amount);
    }
}

/// <summary>Storm: whenever the player plays a Power card, channel Amount Lightning orbs. Base 1, +1/upg.
/// (Game StormPower, Before/AfterCardPlayed dictionary — replicated with a transient armed counter so Storm
/// never triggers the play that applies it; the counter is always 0 at state boundaries, so it needs no
/// hashing.) (Game StormPower.)</summary>
public sealed class StormPower : PowerModel
{
    public override string Id => "Storm";
    public override PowerType Type => PowerType.Buff;
    private int _armed;
    public override void BeforeCardPlayed(CombatState combat, CardModel card)
        => _armed = card.Type == CardType.Power ? Amount : 0;
    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (card.Type == CardType.Power)
            for (int i = 0; i < _armed && !combat.IsCombatOver; i++) OrbOps.Channel(combat, new LightningOrb());
        _armed = 0;
    }
}

/// <summary>Subroutine: whenever the player plays a Power card, gain Amount energy. Fixed 1 (cost −1/upg).
/// (Game SubroutinePower; same transient-armed pattern as Storm.)</summary>
public sealed class SubroutinePower : PowerModel
{
    public override string Id => "Subroutine";
    public override PowerType Type => PowerType.Buff;
    private int _armed;
    public override void BeforeCardPlayed(CombatState combat, CardModel card)
        => _armed = card.Type == CardType.Power ? Amount : 0;
    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (card.Type == CardType.Power && _armed > 0) Cmd.GainEnergy(combat, _armed);
        _armed = 0;
    }
}

/// <summary>Coolant: at the player's turn start, gain block = (distinct orb types) × Amount. Base 2, +1/upg.
/// (Game CoolantPower, hook AfterSideTurnStart.)</summary>
public sealed class CoolantPower : PowerModel
{
    public override string Id => "Coolant";
    public override PowerType Type => PowerType.Buff;
    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        int distinct = combat.Player.Orbs.Select(o => o.Name).Distinct().Count();
        if (distinct > 0) Cmd.GainBlock(combat, combat.Player, distinct * Amount, ValueProp.Unpowered, null);
    }
}

/// <summary>Smokestack: whenever the player generates a Status card, deal Amount to all enemies. Base 5,
/// +2/upg. (Game SmokestackPower, hook AfterCardGeneratedForCombat / Status.)</summary>
public sealed class SmokestackPower : PowerModel
{
    public override string Id => "Smokestack";
    public override PowerType Type => PowerType.Buff;
    public override void AfterCardGenerated(CombatState combat, CardModel card)
    {
        if (card.Type == CardType.Status) PowerFx.DamageAllEnemies(combat, Amount);
    }
}

/// <summary>Loop: at the player's turn start, trigger the FRONT orb's passive Amount extra times. Base 1,
/// +1/upg. (Game LoopPower, hook AfterPlayerTurnStart.)</summary>
public sealed class LoopPower : PowerModel
{
    public override string Id => "Loop";
    public override PowerType Type => PowerType.Buff;
    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side || combat.Player.Orbs.Count == 0) return;
        var front = combat.Player.Orbs[0];
        for (int i = 0; i < Amount && !combat.IsCombatOver; i++) front.Passive(combat);
    }
}

/// <summary>Spinner: at the player's turn start, channel Amount Glass orbs. Base 1. (Game SpinnerPower,
/// hook AfterEnergyReset ≈ AfterSideTurnStart.)</summary>
public sealed class SpinnerPower : PowerModel
{
    public override string Id => "Spinner";
    public override PowerType Type => PowerType.Buff;
    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        for (int i = 0; i < Amount; i++) OrbOps.Channel(combat, new GlassOrb());
    }
}

/// <summary>LightningRod: at the player's turn start, channel 1 Lightning orb, then lose a stack. Base 2.
/// (Game LightningRodPower, hook AfterEnergyReset + Decrement.)</summary>
public sealed class LightningRodPower : PowerModel
{
    public override string Id => "LightningRod";
    public override PowerType Type => PowerType.Buff;
    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        OrbOps.Channel(combat, new LightningOrb());
        Amount--;
        if (Amount <= 0) Owner.RemovePower(Id);
    }
}

/// <summary>BiasedCognition: at the player's turn start, lose Amount Focus. Base 1 (a debuff). (Game
/// BiasedCognitionPower, hook AfterSideTurnStart → applies −Amount FocusPower.)</summary>
public sealed class BiasedCognitionPower : PowerModel
{
    public override string Id => "BiasedCognition";
    public override PowerType Type => PowerType.Debuff;
    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        Cmd.ApplyPower(combat, Owner, new FocusPower(), -Amount, Owner);
    }
}

/// <summary>ConsumingShadow: at the player's turn end, evoke the NEWEST (back) orb Amount times. Base 1,
/// +1/upg comes from the card (more Dark orbs), the power stays 1. (Game ConsumingShadowPower, hook
/// AfterSideTurnEnd → EvokeLast.)</summary>
public sealed class ConsumingShadowPower : PowerModel
{
    public override string Id => "ConsumingShadow";
    public override PowerType Type => PowerType.Buff;
    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        for (int i = 0; i < Amount && combat.Player.Orbs.Count > 0 && !combat.IsCombatOver; i++)
            OrbOps.EvokeBack(combat);
    }
}

/// <summary>Buffer: the next instance of HP loss to the owner is prevented entirely, consuming a stack. Base
/// 1, +1/upg. (Game BufferPower, hook ModifyHpLostAfterOstyLate→0 + Decrement.)</summary>
public sealed class BufferPower : PowerModel
{
    public override string Id => "Buffer";
    public override PowerType Type => PowerType.Buff;
    public override int ModifyHpLost(Creature target, int hpLost, ValueProp props, Creature? dealer)
    {
        // Decrement only — the HP-loss pipeline iterates the live Powers list, so removal happens in the
        // post-pipeline AfterDamageReceived snapshot below.
        if (target != Owner || Amount <= 0 || hpLost <= 0) return hpLost;
        Amount--;
        return 0;
    }
    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (target == Owner && Amount <= 0) Owner.RemovePower(Id);
    }
}

/// <summary>Iteration: the FIRST time the player draws a Status card each turn, draw Amount cards. Base 2,
/// +1/upg. (Game IterationPower, hook AfterCardDrawn.) The per-turn gate lives on a transient flag reset at
/// the player's turn start; in pure search mid-turn draws defer (so this is inert there — sound under-credit),
/// real in rollout/replay with a concrete Rng. The flag is always reset at turn boundaries ⇒ not hashed.</summary>
public sealed class IterationPower : PowerModel
{
    public override string Id => "Iteration";
    public override PowerType Type => PowerType.Buff;
    private bool _firedThisTurn;
    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) _firedThisTurn = false;
    }
    public override void AfterCardDrawn(CombatState combat, CardModel card, bool fromHandDraw)
    {
        if (_firedThisTurn || card.Type != CardType.Status) return;
        _firedThisTurn = true;
        Cmd.Draw(combat, Amount);
    }
}

/// <summary>Temporary Focus marker: schedules removal, at the player's turn end, of the temporary Focus
/// granted this turn (Hotfix / FocusedStrike / Synchronize). The card applies real FocusPower +N immediately
/// AND this marker +N; at turn end the marker applies FocusPower −Amount and removes itself — net: +N Focus
/// for the rest of the current turn only. (Game TemporaryFocusPower: BeforeApplied adds FocusPower,
/// AfterSideTurnEnd removes it.)</summary>
public sealed class TemporaryFocusPower : PowerModel
{
    public override string Id => "TempFocus";
    public override PowerType Type => Amount >= 0 ? PowerType.Buff : PowerType.Debuff;
    public override bool AllowNegative => true;   // Hyperbeam's "lose 3 Focus this turn" is a negative stack
    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != Owner.Side) return;
        Cmd.ApplyPower(combat, Owner, new FocusPower(), -Amount, Owner);
        Owner.RemovePower(Id);
    }

    /// <summary>Grant N Focus for the rest of this turn: real Focus now + a marker stack to undo it at turn end.</summary>
    public static void Grant(CombatState combat, int n)
    {
        if (n == 0) return;
        Cmd.ApplyPower(combat, combat.Player, new FocusPower(), n, combat.Player);
        Cmd.ApplyPower(combat, combat.Player, new TemporaryFocusPower(), n, combat.Player);
    }
}

/// <summary>FreePower: the next Power card you play (from hand or play pile) costs 0, then loses a stack.
/// Base 1 (from Synthesis). (Game FreePowerPower, hook TryModifyEnergyCostInCombatLate + BeforeCardPlayed.)</summary>
public sealed class FreePowerPower : PowerModel
{
    public override string Id => "FreePower";
    public override PowerType Type => PowerType.Buff;
    public override int ModifyCardCost(CardModel card, int cost)
        => card.Type == CardType.Power && Amount > 0 ? 0 : cost;
    public override void BeforeCardPlayed(CombatState combat, CardModel card)
    {
        if (card.Type != CardType.Power || Amount <= 0) return;
        Amount--;
        if (Amount <= 0) Owner.RemovePower(Id);
    }
}

/// <summary>SignalBoost: the next Power card you play is played twice, then loses a stack. Base 1. (Game
/// SignalBoostPower, hook ModifyCardPlayCount + AfterModifyingCardPlayCount.)</summary>
public sealed class SignalBoostPower : PowerModel
{
    public override string Id => "SignalBoost";
    public override PowerType Type => PowerType.Buff;
    public override int ModifyCardPlayCount(CardModel card) => card.Type == CardType.Power && Amount > 0 ? 1 : 0;
    public override void AfterModifyingCardPlayCount(CombatState combat, CardModel card)
    {
        Amount--;
        if (Amount <= 0) Owner.RemovePower(Id);
    }
}

/// <summary>EchoForm: the first Amount cards played each turn are played twice. Base 1, +1/stack. (Game
/// EchoFormPower, hook ModifyCardPlayCount with a first-in-turn gate.) Self-contained per-turn counter
/// (reset at the player's turn start) — hashed so memoisation stays sound.</summary>
public sealed class EchoFormPower : PowerModel
{
    public override string Id => "EchoForm";
    public override PowerType Type => PowerType.Buff;
    private int _used;   // doubles granted so far this turn
    public override int ModifyCardPlayCount(CardModel card) => _used < Amount ? 1 : 0;
    public override void AfterModifyingCardPlayCount(CombatState combat, CardModel card) => _used++;
    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) _used = 0;
    }
    public override long HashValue() => base.HashValue() ^ ((long)_used << 40);
    public override string StateKey() => _used == 0 ? $"EchoForm={Amount}" : $"EchoForm={Amount}/{_used}";
}

// ---- Inert-but-sound markers (port the power so it is represented + hashed; the active effect is a
// documented PESSIMISTIC gap — under-credits the player, never optimistic). ----

/// <summary>MachineLearning: +Amount cards drawn each turn-start. MODELLED via <see cref="ModifyHandDraw"/>,
/// chained over the player's powers in <see cref="CombatManager.TurnStartDrawCount"/> and applied at every
/// turn-start draw site (exact opening + per-turn, MCTS chance-node fan, rollout, replay). The power is already
/// in the state key, so the larger hand is just a deterministic function of it — no new gating needed.
/// (Game MachineLearningPower, hook ModifyHandDraw.)</summary>
public sealed class MachineLearningPower : PowerModel
{
    public override string Id => "MachineLearning";
    public override PowerType Type => PowerType.Buff;
    public override int ModifyHandDraw(Creature player, int count) => count + Amount;
}

/// <summary>TrashToTreasure: channel Amount RANDOM orbs whenever the player generates a Status card. INERT
/// (sound): random orb-type selection is never a search decision (would be optimistically unsound to pick
/// the best); leaving it inert under-credits. (Game TrashToTreasurePower.)</summary>
public sealed class TrashToTreasurePower : PowerModel
{
    public override string Id => "TrashToTreasure";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>CreativeAi: before each turn's hand draw, add Amount random Power cards to hand. INERT (sound):
/// full-pool RNG card-generation is documented out-of-scope (Metamorphosis/WhiteNoise class); under-credits.
/// (Game CreativeAiPower.)</summary>
public sealed class CreativeAiPower : PowerModel
{
    public override string Id => "CreativeAi";
    public override PowerType Type => PowerType.Buff;
}

/// <summary>Feral: a 0-cost Attack you play returns to your hand, up to Amount times per turn. INERT (sound):
/// return-to-hand free-replay is beneficial and loop-prone; leaving it inert under-credits the player.
/// (Game FeralPower.)</summary>
public sealed class FeralPower : PowerModel
{
    public override string Id => "Feral";
    public override PowerType Type => PowerType.Buff;
}
