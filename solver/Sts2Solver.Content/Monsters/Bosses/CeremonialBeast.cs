using Sts2Solver.Engine;

namespace Sts2Solver.Content;

public static partial class Monsters
{
    /// <summary>
    /// CeremonialBeast (Act-2 Overgrowth BOSS, MegaCrit). Decompile: MegaCrit.Sts2.Core.Models.Monsters.CeremonialBeast.
    ///
    /// HP: MinInitialHp == MaxInitialHp == 252 (262 on Ascension ToughEnemies) — a fixed value, not a roll.
    ///
    /// TWO-PHASE boss. It opens in a STAMPEDE phase, then — once the player chunks it down past a HP threshold —
    /// it STUNS itself for a turn and drops into a permanent SHRILL phase. The phase break is an ON-HIT event
    /// (PlowPower below), not a scripted move transition, so it can fire at any point in the player's turn.
    ///
    /// PHASE 1 — STAMPEDE (decompile GenerateMoveStateMachine, opens on STAMP):
    ///   STAMP → PLOW → PLOW → PLOW → …  (StampMove.FollowUp = Plow; PlowMove.FollowUp = Plow — self-loops).
    ///   • STAMP (no damage): applies PlowPower = 150 (160 on Ascension DeadlyEnemies) to ITSELF. PlowPower is a
    ///     HP-THRESHOLD marker (game StackType Counter): it does NOT block/reduce damage — it just records the HP
    ///     line below which the next unblocked hit triggers the phase break (see PlowPower).
    ///   • PLOW: a charge attack — PlowDamage = 18 (20 on DeadlyEnemies), then +2 Strength to itself
    ///     (PlowStrength = 2, NOT Deadly-scaled). The Strength compounds every PLOW, so the charge ramps:
    ///     1st PLOW 18/20, 2nd 20/22, 3rd 22/24, … (Strength is a powered-attack additive). Modelled exactly via
    ///     StrengthPower; IntentDamage stays the BASE telegraph (informational), the resolved hit reads live Strength.
    ///
    /// PHASE BREAK (PlowPower.AfterDamageReceived — the SOUNDNESS-critical mechanic). When the beast takes an
    /// UNBLOCKED hit that leaves it at or below the Plow threshold (CurrentHp <= PlowAmount) it:
    ///   (1) sheds ALL its accumulated Strength (game removes every TemporaryStrengthPower then StrengthPower —
    ///       so the ramp it built in phase 1 is LOST),
    ///   (2) self-stuns: the upcoming enemy turn becomes a no-op STUN turn (game CreatureCmd.Stun(StunnedMove)),
    ///   (3) routes the AI so the move AFTER the stun is BEAST_CRY — i.e. it enters phase 2,
    ///   (4) removes PlowPower (one-shot — the break happens exactly once).
    /// The stun COSTS THE PLAYER NOTHING and BUYS THE PLAYER A FREE TURN (the beast skips an attack), and it
    /// permanently swaps the boss off its ramping charge onto the (lower, fixed) phase-2 attacks. Reproduced
    /// faithfully; this is a real defensive break the deck advisor must see (omitting it would OVER-credit the
    /// boss's phase-1 ramp continuing forever — but modelling the FREE skip turn is the player-pessimistic-safe
    /// thing only if we ALSO keep the phase-2 attacks, which we do).
    ///
    /// PHASE 2 — SHRILL (decompile: STUN → BEAST_CRY → STOMP → CRUSH → BEAST_CRY → STOMP → CRUSH → … forever):
    ///   • STUN (no damage): the forced skip turn the phase break injects (game StunnedMove, an Unstun animation).
    ///   • BEAST_CRY (no damage): applies RingingPower(1) to the player — afflicts EVERY player card with Ringing
    ///     for one turn, so each card may be played AT MOST ONCE that turn (the affliction clears at the player's
    ///     turn end). See the RINGING SOUNDNESS NOTE below.
    ///   • STOMP: StompDamage = 15 (17 on DeadlyEnemies).
    ///   • CRUSH: CrushDamage = 17 (19 on DeadlyEnemies), then +3 Strength to itself (4 on DeadlyEnemies). This
    ///     Strength ramps the phase-2 attacks each CRUSH (CrushStrength compounds — STOMP/CRUSH read it), modelled
    ///     exactly via StrengthPower.
    ///
    /// Damage/amount sources (DeadlyEnemies on the left where applicable):
    ///   PlowAmount = 160/150 (threshold), PlowDamage = 20/18, PlowStrength = 2 (fixed);
    ///   StompDamage = 17/15; CrushDamage = 19/17; CrushStrength = 4/3. HP 262/252 (ToughEnemies).
    ///
    /// RINGING (now MODELLED). BEAST_CRY's only combat effect is to apply Ringing to the player's whole deck: the
    /// game's RingingPower.ShouldPlay blocks any further afflicted card once the player has started a card play this
    /// turn — i.e. the player may play AT MOST ONE card that turn. This is exactly a per-turn play cap of 1, so it
    /// is modelled through the engine's existing play-cap machinery: CeremonialBeastRingingPower.PlayCapThisTurn()
    /// returns 1, which CombatState.EffectivePlayCap() mins into every move generator (exact + MCTS + rollout);
    /// applying it sets BoundsPlays so the per-turn play counter is hashed. Ringing makes the fight HARDER (fewer
    /// plays), so modelling it CLOSES the previously-flagged optimistic gap — the boss now reads as strong as it is.
    /// (The decompile's per-CARD affliction is finer-grained than a single cap, but since EVERY player card gets the
    /// affliction and the restriction triggers off "any card already played this turn", the net effect is exactly a
    /// 1-card cap — equivalent for the objective.) The DAMAGE side of the boss is fully and exactly modelled.
    ///
    /// No death-phase / survive-at-0 mechanic: the beast dies normally at 0 HP (decompile has no DeathPhaseEntry
    /// analogue — the only death hook is a die-animation/SFX branch). The phase break is the on-hit PlowPower,
    /// distinct from the death-phase primitive.
    /// </summary>
    public static Monster CeremonialBeast(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 262, 252);     // MinInitialHp == MaxInitialHp (fixed, no roll)
        int plowAmount = Asc.Deadly(ascension, 160, 150);    // HP threshold for the phase break (NOT block)
        int plowDamage = Asc.Deadly(ascension, 20, 18), plowStrength = 2;
        int stompDamage = Asc.Deadly(ascension, 17, 15);
        int crushDamage = Asc.Deadly(ascension, 19, 17), crushStrength = Asc.Deadly(ascension, 4, 3);
        int ringing = 1;
        var monster = new Monster { Name = "CeremonialBeast", MaxHp = hp, CurrentHp = hp };

        // ---- PHASE 1: STAMPEDE ----
        // STAMP: no damage — apply the Plow HP-threshold marker to itself (arms the phase break).
        var stamp = new MoveState("STAMP_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, self, new CeremonialBeastPlowPower(), plowAmount, self),
            intentDamage: null);
        // PLOW: charge attack + permanent +2 Strength (the charge ramps each PLOW). IntentDamage is the BASE
        // telegraph; the resolved hit picks up live Strength (Cmd.Attack adds StrengthPower additively).
        var plow = new MoveState("PLOW_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, plowDamage, ValueProp.Move, null);
                Cmd.ApplyPower(combat, self, new StrengthPower(), plowStrength, self);
            },
            intentDamage: plowDamage);

        // ---- PHASE 2: SHRILL ----
        // STUN: the forced no-op skip turn injected by the phase break (the beast loses this turn — a free
        // turn for the player). Its FollowUp is BEAST_CRY, so phase 2 begins on the move after the stun.
        var stun = new MoveState("STUN_MOVE",
            (combat, self) => { /* stunned: no action this turn */ },
            intentDamage: null);
        // BEAST_CRY: no damage — apply Ringing(1) to the player. The play-restriction harm is INERT (no engine
        // support — see the RINGING SOUNDNESS NOTE), but the marker + the spent turn are reproduced exactly.
        var beastCry = new MoveState("BEAST_CRY_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, combat.Player, new CeremonialBeastRingingPower(), ringing, self),
            intentDamage: null);
        var stomp = new MoveState("STOMP_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, stompDamage, ValueProp.Move, null),
            intentDamage: stompDamage);
        // CRUSH: attack + permanent +3/4 Strength (phase-2 attacks ramp each CRUSH).
        var crush = new MoveState("CRUSH_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, crushDamage, ValueProp.Move, null);
                Cmd.ApplyPower(combat, self, new StrengthPower(), crushStrength, self);
            },
            intentDamage: crushDamage);

        // FollowUp wiring (decompile). Phase 1 is a self-looping PLOW chain; the phase break (PlowPower) is what
        // redirects the telegraph to STUN → BEAST_CRY, after which phase 2 loops forever.
        stamp.FollowUp = plow;
        plow.FollowUp = plow;          // PLOW self-loops in phase 1 (until the on-hit break interrupts it)
        stun.FollowUp = beastCry;      // the break sets CurrentMove = STUN; STUN then enters phase 2
        beastCry.FollowUp = stomp;
        stomp.FollowUp = crush;
        crush.FollowUp = beastCry;     // phase-2 loop: BEAST_CRY → STOMP → CRUSH → BEAST_CRY → …

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { stamp, plow, stun, beastCry, stomp, crush }, stamp.Id);
        return monster;
    }
}

/// <summary>
/// CeremonialBeast's PLOW marker (decompile PlowPower, specialised to this boss). Applied to the beast by STAMP
/// at amount 150 (160 on DeadlyEnemies). It is a HP-THRESHOLD trigger, NOT block/armour — it does not modify any
/// damage. The FIRST unblocked hit that leaves the beast at or below the threshold (CurrentHp &lt;= Amount) breaks
/// phase 1: it sheds the beast's accumulated Strength (game removes TemporaryStrength then Strength — the phase-1
/// charge ramp is lost), self-stuns the beast for the upcoming enemy turn (a FREE turn for the player), routes the
/// AI into phase 2 (the stun's FollowUp is BEAST_CRY), and removes itself (one-shot). Fully-blocked hits do NOT
/// trigger it (game guards on result.UnblockedDamage &gt; 0) — matching CombatState semantics.
/// </summary>
public sealed class CeremonialBeastPlowPower : PowerModel
{
    public const string PowerId = "CeremonialBeastPlow";
    public override string Id => PowerId;
    // Debuff-typed in the game (PowerType.Debuff). It is applied by the monster to ITSELF, so the player can't
    // Artifact it away — and TryAbsorbDebuff only fires on the TARGET's powers, none of which absorb here.
    public override PowerType Type => PowerType.Debuff;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (target != Owner || unblockedDamage <= 0) return;   // fully-blocked hits do NOT break the phase
        if (Owner is not Monster beast || !beast.IsAlive) return;
        if (beast.CurrentHp > Amount) return;                  // not yet past the threshold

        // (1) Shed all accumulated Strength (phase-1 charge ramp is lost). The game removes every
        // TemporaryStrengthPower then StrengthPower; we strip both ids (any negative Strength would have been
        // game-removed too — the beast only ever self-buffs positive Strength here).
        foreach (var p in beast.Powers.Where(p => p is TemporaryStrengthPower).ToList())
            beast.RemovePower(p.Id);
        beast.RemovePower("Strength");

        // (2) + (3) Self-stun and route into phase 2: set the telegraphed move to STUN_MOVE (a no-op turn — the
        // beast skips its upcoming attack). STUN_MOVE.FollowUp is BEAST_CRY, so the move AFTER the stun begins
        // the phase-2 loop. This mirrors the death-phase pattern (WaterfallGiant sets Ai.CurrentMoveId from
        // inside the damage pipeline). Setting CurrentMoveId (vs StunnedTurns) reproduces the game exactly: the
        // CURRENT telegraph becomes the stun and the NEXT roll comes off STUN's FollowUp = BEAST_CRY.
        beast.Ai.CurrentMoveId = "STUN_MOVE";

        // (4) One-shot: remove the marker so the break can never fire twice.
        beast.RemovePower(Id);
    }
}

/// <summary>
/// CeremonialBeast's RINGING (decompile RingingPower, applied to the PLAYER by BEAST_CRY). In the game it afflicts
/// EVERY player card with Ringing for one turn; RingingPower.ShouldPlay then blocks any further afflicted card
/// once the player has STARTED a card play this turn — so the player may play AT MOST ONE card that turn — and the
/// power self-removes at the player's own turn end. MODELLED via the engine's per-turn play cap: while held the
/// power returns <see cref="PlayCapThisTurn"/> = 1, which <see cref="CombatState.EffectivePlayCap"/> mins into the
/// move generators (exact + MCTS + rollout). SOUNDNESS: the cap is HARM (fewer plays for the player), so modelling
/// it CLOSES the previously-flagged optimistic gap — the boss now reads as strong as it is. Applying it sets
/// <see cref="CombatState.BoundsPlays"/> so the per-turn play counter is hashed from then on (the cap depends on it).
/// </summary>
public sealed class CeremonialBeastRingingPower : PowerModel
{
    public const string PowerId = "CeremonialBeastRinging";
    public override string Id => PowerId;
    public override PowerType Type => PowerType.Debuff;

    public override int PlayCapThisTurn() => 1;   // ≤1 card the turn this is held (Game: RingingPower.ShouldPlay)

    // The cap reads PlaysThisTurn, which must then be hashed; gate it on (so a "played my 1 card" state never
    // memo-collides with a "haven't played yet" state). Set once on apply; persists for the rest of combat
    // (harmless slight over-hashing after the power is gone).
    public override void AfterApplied(CombatState combat, Creature? applier) => combat.BoundsPlays = true;

    // Game: RingingPower.AfterSideTurnEnd removes it when the owner's (player's) side turn ends.
    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side == Owner.Side) Owner.RemovePower(Id);
    }
}
