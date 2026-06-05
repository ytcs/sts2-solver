using Sts2Solver.Engine;

namespace Sts2Solver.Content;

public static partial class Monsters
{
    /// <summary>
    /// TheInsatiable (Act-3 Hive BOSS, MegaCrit). Decompile: MegaCrit.Sts2.Core.Models.Monsters.TheInsatiable.
    ///
    /// HP: MinInitialHp == MaxInitialHp == 321 (341 on Ascension ToughEnemies) — a fixed value, not a roll.
    ///
    /// AI (decompile GenerateMoveStateMachine). The boss OPENS on LIQUIFY_GROUND (fired exactly once), then loops
    /// a fixed 4-move cycle FOREVER. The decompile uses two identical THRASH states (THRASH_MOVE and THRASH_MOVE_2,
    /// both the same ThrashMove / MultiAttackIntent(ThrashDamage,2)) purely so the FollowUpState chain closes the
    /// loop — there is NO random branching:
    ///
    ///   LIQUIFY_GROUND → THRASH → LUNGING_BITE → SALIVATE → THRASH(2) → LUNGING_BITE → SALIVATE → THRASH(2) → …
    ///
    /// (decompile wiring: liquify.FollowUp = thrash; thrash.FollowUp = bite; bite.FollowUp = salivate;
    ///  salivate.FollowUp = thrash2; thrash2.FollowUp = thrash. Initial state = liquify.) After the opener the cycle
    /// is THRASH → BITE → SALIVATE repeating (the two THRASH ids are behaviourally identical; both are modelled so
    /// the FollowUp graph and move-log are exact).
    ///
    /// Moves (DeadlyEnemies on the left where applicable):
    ///  - LIQUIFY_GROUND : NO direct damage. BuffIntent + StatusIntent(6). It (a) applies the SOUNDNESS-CRITICAL
    ///                     Sandpit death-timer to the player (amount 4 — see SANDPIT below), and (b) generates 6
    ///                     FranticEscape status cards into the player's deck — 3 into the DRAW pile, 3 into the
    ///                     DISCARD pile (decompile: i in [0,6); i<3 ⇒ Draw else Discard). Fires only once (opener).
    ///  - THRASH (×2 ids): MultiAttackIntent(ThrashDamage, 2) — 2 hits of ThrashDamage = 9 / 8 each (block soaks
    ///                     per hit, _thrashRepeat = 2).
    ///  - LUNGING_BITE   : SingleAttackIntent(BiteDamage) — BiteDamage = 31 / 28. The big single hit.
    ///  - SALIVATE       : BuffIntent — pure self-buff: +SalivateStrength = 3 / 2 Strength to ITSELF (StrengthPower).
    ///                     This compounds every cycle, ramping every later Thrash/Bite (Strength is additive per
    ///                     hit), modelled exactly via StrengthPower.
    ///
    /// Ascension scaling: HP is Tough-scaled (341/321); ThrashDamage, BiteDamage and SalivateStrength are
    /// Deadly-scaled (9/8, 31/28, 3/2). The Sandpit amount (4), the FranticEscape counts (6 / 3+3) and the
    /// Thrash hit count (2) are flat.
    ///
    /// =====================================================================================================
    /// SANDPIT death-timer (the soundness-critical mechanic) — MODELLED, with one player-side gap FLAGGED.
    /// =====================================================================================================
    /// LIQUIFY applies <see cref="InsatiableSandpitPower"/> amount 4 to the PLAYER. In the game (SandpitPower):
    ///   • it DECREMENTS by 1 at the start of each ENEMY turn (AfterSideTurnStartLate, Enemy side), and
    ///   • the instant it reaches 0 and is REMOVED (AfterRemoved) it KILLS the player outright — a forced
    ///     CreatureCmd.Kill(force: true) on the player (and pets/Osty). i.e. a hard GAME-OVER countdown: if the
    ///     player has not "escaped" by the time the counter would drain, the run ends.
    /// The ONLY escape is playing FranticEscape status cards: each FranticEscape played calls SandpitPower
    /// .ModifyAmount(+1) (decompile FranticEscape.OnPlay), pushing the counter back UP — and the boss seeds the
    /// player with 6 of them (3 draw + 3 discard) so the player CAN keep the timer alive. (Each FranticEscape also
    /// bumps its OWN energy cost by +1 this combat — EnergyCost.AddThisCombat(1) — so the escape gets more
    /// expensive the more you lean on it.)
    ///
    /// This is reproduced as faithfully as the engine primitives allow:
    ///   (1) The 4-counter is applied to the player and decrements once per enemy turn start (matches the game).
    ///   (2) Reaching 0 KILLS the player via <see cref="Cmd.Kill"/> — the real, fight-defining threat, modelled
    ///       exactly so the boss is NEVER under-credited: a deck that cannot keep the timer fed loses the run.
    ///   (3) The escape IS wired without touching the shared FranticEscape card (which I may not edit): the power's
    ///       <see cref="PowerModel.AfterCardPlayed"/> hook fires on every card the player plays and, when that card
    ///       is a FranticEscape, adds +1 to the Sandpit counter — exactly the game's FranticEscape.OnPlay → Sandpit
    ///       ModifyAmount(+1). So a player that draws and plays its escape cards survives the timer, just like the
    ///       real fight.
    ///
    /// FranticEscape COST RAMP — now MODELLED (gap CLOSED). Each FranticEscape play ramps THIS instance's energy
    ///   cost by 1 for the rest of the combat (game: EnergyCost.AddThisCombat(1)). FranticEscape is generated ONLY
    ///   by this boss (CanBeGeneratedInCombat=false), so it is effectively boss-specific; it is now a Stateful card
    ///   with a per-instance cost counter (see Core/StatusCards.cs), folded into its StateKey/hash. Without the ramp
    ///   the player could replay the escape at cost 1 forever and survive the Sandpit indefinitely — an OPTIMISTIC
    ///   over-credit; with it, escaping grows prohibitively expensive (each of the 6 copies ramps independently), so
    ///   the timer eventually wins — the faithful dynamic. The boss now reads as strong as it is.
    ///
    /// No death-phase / survive-at-0 / summon / transform on the BOSS itself: TheInsatiable dies normally at 0 HP
    /// (decompile AfterDeath only twiddles a music parameter; there is no DeathPhaseEntryMove analogue). The
    /// Sandpit kill is a PLAYER-side timer, distinct from the engine's monster death-phase primitive.
    /// </summary>
    public static Monster TheInsatiable(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 341, 321);     // MinInitialHp == MaxInitialHp (fixed, no roll)
        int thrashDamage = Asc.Deadly(ascension, 9, 8);      // _thrashRepeat = 2 hits
        const int thrashHits = 2;
        int biteDamage = Asc.Deadly(ascension, 31, 28);
        int salivateStrength = Asc.Deadly(ascension, 3, 2);
        const int sandpitAmount = 4;                         // SandpitPower applied to the player by Liquify
        const int franticDraw = 3, franticDiscard = 3;       // 6 FranticEscape: 3 draw + 3 discard
        var monster = new Monster { Name = "TheInsatiable", MaxHp = hp, CurrentHp = hp };

        // LIQUIFY_GROUND (opener, fires once): no direct damage. Apply the Sandpit death-timer (4) to the player
        // and seed 6 FranticEscape status cards (3 into the draw pile, 3 into the discard pile).
        var liquify = new MoveState("LIQUIFY_GROUND_MOVE",
            (combat, self) =>
            {
                Cmd.ApplyPower(combat, combat.Player, new InsatiableSandpitPower(), sandpitAmount, self);
                for (int i = 0; i < franticDraw; i++)
                    Cmd.GenerateStatusCard(combat, new FranticEscape(), combat.Player.DrawPile);
                for (int i = 0; i < franticDiscard; i++)
                    Cmd.GenerateStatusCard(combat, new FranticEscape(), combat.Player.DiscardPile);
            },
            intentDamage: null);

        // THRASH / THRASH_MOVE_2 (behaviourally identical — two ids only to close the FollowUp loop): 2-hit attack.
        Action<CombatState, Monster> thrashPerform =
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, thrashDamage, thrashHits, ValueProp.Move, null);
        var thrash = new MoveState("THRASH_MOVE", thrashPerform, intentDamage: thrashDamage, intentHits: thrashHits);
        var thrash2 = new MoveState("THRASH_MOVE_2", thrashPerform, intentDamage: thrashDamage, intentHits: thrashHits);

        // LUNGING_BITE: heavy single hit.
        var bite = new MoveState("LUNGING_BITE_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, biteDamage, ValueProp.Move, null),
            intentDamage: biteDamage);

        // SALIVATE: pure self-buff — +Strength to itself (compounds, ramping every later attack). No player effect.
        var salivate = new MoveState("SALIVATE_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, self, new StrengthPower(), salivateStrength, self),
            intentDamage: null);

        // FollowUp wiring (decompile): open on LIQUIFY, then the THRASH → BITE → SALIVATE cycle loops forever via
        // the second THRASH id (salivate → thrash2 → thrash → bite → salivate → …).
        liquify.FollowUp = thrash;
        thrash.FollowUp = bite;
        bite.FollowUp = salivate;
        salivate.FollowUp = thrash2;
        thrash2.FollowUp = thrash;

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { liquify, thrash, thrash2, bite, salivate }, liquify.Id);
        return monster;
    }
}

/// <summary>
/// TheInsatiable's SANDPIT death-timer (decompile SandpitPower, specialised to this boss). Applied to the PLAYER
/// by LIQUIFY_GROUND at amount 4. It is a hard GAME-OVER countdown:
///   • DECREMENTS by 1 at the start of each ENEMY turn (game AfterSideTurnStartLate on the Enemy side). The first
///     decrement happens on the enemy turn AFTER the one Liquify resolved on, so the player gets ~4 enemy-turn
///     decrements of grace.
///   • When the counter would hit 0 it KILLS the player (game AfterRemoved → CreatureCmd.Kill(force: true) on the
///     player and any pets/Osty). Modelled with <see cref="Cmd.Kill"/> on the player — a forced, unblockable,
///     full-HP removal — so a deck that lets the timer drain loses the run. This is the fight-defining threat and
///     is reproduced exactly so the boss is never UNDER-credited.
///   • ESCAPE: each FranticEscape the player PLAYS pushes the counter +1 (game FranticEscape.OnPlay → Sandpit
///     ModifyAmount(+1)). The boss seeds 6 FranticEscape cards (3 draw, 3 discard) for exactly this. The +1 is wired
///     here via <see cref="AfterCardPlayed"/> (fires on every card the player plays; we react to FranticEscape).
///     FranticEscape's own +1-per-play COST RAMP is now modelled on the (boss-only) card itself (Core/StatusCards.cs),
///     so escaping grows prohibitively expensive — the prior optimistic gap is CLOSED.
/// Buff-typed in the game (PowerType.Buff) so it is not Artifact-absorbed when applied to the player.
/// </summary>
public sealed class InsatiableSandpitPower : PowerModel
{
    public const string PowerId = "InsatiableSandpit";
    public override string Id => PowerId;
    public override PowerType Type => PowerType.Buff;

    /// <summary>Decrement at the start of each ENEMY turn; on reaching 0, KILL the player (game-over countdown).</summary>
    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        if (side != CombatSide.Enemy) return;       // game: AfterSideTurnStartLate(Enemy) ⇒ Decrement
        if (!Owner.IsAlive) return;
        Amount--;
        if (Amount <= 0)
        {
            // Game: SandpitPower.AfterRemoved force-Kills the player (and pets/Osty). The timer ran out.
            Owner.RemovePower(Id);
            Cmd.Kill(combat, Owner);
        }
    }

    /// <summary>Game: playing a FranticEscape calls Sandpit.ModifyAmount(+1). The shared FranticEscape card is
    /// inert (and outside the editable files), so we react to it here — fires on every card the player plays.</summary>
    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (Owner.IsPlayer && card.Name == "FranticEscape") Amount++;
    }
}
