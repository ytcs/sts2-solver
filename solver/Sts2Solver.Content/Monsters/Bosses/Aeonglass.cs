using System.Collections.Generic;
using Sts2Solver.Engine;

namespace Sts2Solver.Content;

public static partial class Monsters
{
    /// <summary>
    /// Aeonglass (Act-4 GLORY BOSS, MegaCrit). Decompile: MegaCrit.Sts2.Core.Models.Monsters.Aeonglass.
    ///
    /// HP: MinInitialHp == MaxInitialHp == 535 (512 below ToughEnemies) — a fixed value, not a roll.
    ///
    /// COMBAT START (decompile AfterAddedToRoom). To ITSELF: <c>ArtifactPower 3</c> (negates the next 3 debuffs
    /// applied to it — modelled via the shared ArtifactPower) and a Withering-Presence counter. The game applies
    /// <c>WitheringPresencePower 6</c> to the PLAYER (a "CardsLeft" counter starting at 6: every 6th card the
    /// PLAYER plays adds 1 Wither to the player's HAND, then resets to 6 — decompile
    /// WitheringPresencePower.AfterCardPlayed). The solver factory only returns the Monster (there is no
    /// combat-start hook to apply a power to the player), so the counter rides on the BOSS instead — its
    /// AfterCardPlayed hook fires on the player's card plays exactly the same (powers see every card played via
    /// combat.AllPowers), and it spawns the Wither into the player's hand. Behaviourally identical; see
    /// <see cref="AeonglassWitheringPresencePower"/>. The boss also bumps a cosmetic music parameter — not
    /// modelled (no combat effect).
    ///
    /// DETERMINISTIC 3-move cycle (decompile GenerateMoveStateMachine: a fixed FollowUpState chain, NO random
    /// branching), opening on EBB:
    ///
    ///   EBB → EYE_LASERS → INCREASING_INTENSITY → (back to) EBB → …
    ///
    /// PATCH NOTE (v0.107.0, 2026-06-04 — decompile-confirmed): EBB and INCREASING_INTENSITY were reworked. EBB is
    /// now <c>SingleAttackIntent(EbbDamage) + DefendIntent</c>: <c>EbbMove</c> attacks then <c>GainBlock(EbbBlock)</c>
    /// — the old −3 Str/−3 Dex EbbPower drain was REMOVED entirely (no EbbPower class remains in the game). The
    /// Block moved off INCREASING_INTENSITY (now <c>StatusIntent(WitherAmount) + BuffIntent</c>, no Defend — its
    /// move no longer gains Block) onto EBB at <c>EbbBlock => 33</c>. Verified against
    /// <c>sts2.dll v0.107.0 (commit 23d60b98)</c> via ilspycmd.
    ///
    /// Moves (DeadlyEnemies on the left where applicable):
    ///  - EBB              : single attack EbbDamage = 26 / 22, then gain Block (EbbBlock = 33, relocated here by
    ///                       the 2026-06 patch — see PATCH NOTE). SingleAttackIntent + DefendIntent.
    ///  - EYE_LASERS       : multi-attack EyeLasersDamage = 12 / 11, 2 hits (EyeLasersRepeat = 2; block soaks per
    ///                       hit). MultiAttackIntent.
    ///  - INCREASING_INTENSITY (decompile IncreasingIntensityMove — the soundness-critical ramp). In one move it:
    ///      (1) FAKE-UPGRADES every Wither already in the player's deck by +3 damage (decompile loops AllCards and
    ///          calls Wither.FakeUpgrade, which adds 3 to the card's damage var);
    ///      (2) increments WitherUpgradeCount (so Withers GENERATED later open pre-upgraded to the same level —
    ///          decompile AfterCardGeneratedForCombat FakeUpgrades each new Wither WitherUpgradeCount times);
    ///      (3) adds WitherAmount = 2 / 1 fresh Wither status cards to the player's DISCARD pile (at the new level);
    ///      (4) gains StrengthPower IncreasingIntensityTotalStrength = IncreasingIntensityBaseStrength (4 / 3) +
    ///          AdditionalStrength to ITSELF, then increments AdditionalStrength.
    ///      (Pre-patch it also gained 33 Block here; the 2026-06 patch moved that Block onto EBB — see PATCH NOTE.)
    ///      Because AdditionalStrength and WitherUpgradeCount both start at 0 and both increment exactly once per
    ///      Increasing Intensity, they are always equal to the number of COMPLETED Increasing Intensities. So the
    ///      k-th Increasing Intensity grants Strength = base + (k−1) (a TRIANGULAR ramp: total after N uses =
    ///      N·base + N(N−1)/2), and every Wither in play sits at level k (each dealing 3 + 3·k). Modelled with a
    ///      single inert counter power (AeonglassIntensityPower, Amount = completed-II count) plus a stateful
    ///      Wither card (AeonglassWither) carrying its own upgrade level. StatusIntent + BuffIntent + DefendIntent.
    ///
    /// WITHER (decompile Wither): a cost-(-1) UNPLAYABLE Status card. At the END of the player's turn, every copy
    /// still held in HAND deals its damage to the player as a BLOCKABLE, unpowered hit (decompile OnTurnEndInHand →
    /// CreatureCmd.Damage with the card's Unpowered|Move damage var). Base 3, +3 per fake-upgrade level. Withers
    /// reach the player two ways — Increasing Intensity (to discard) and Withering Presence (to hand, every 6th
    /// card played) — and the in-hand turn-end chip damage RAMPS as Increasing Intensity fake-upgrades them, so it
    /// is reproduced exactly (the engine's per-card OnTurnEndInHand hook fires every copy held at turn end). The
    /// game's "+N" title is cosmetic and not modelled; the DAMAGE level is.
    ///
    /// No death-phase / survive-at-0 / summon / transform mechanic: Aeonglass dies normally at 0 HP (decompile
    /// AfterDeath only twiddles a music parameter — no DeathPhaseEntryMove analogue).
    /// </summary>
    public static Monster Aeonglass(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 535, 512);          // MinInitialHp == MaxInitialHp (fixed, no roll)
        int ebbDamage = Asc.Deadly(ascension, 26, 22);
        const int ebbBlock = 33;                                  // EbbBlock => 33 gained on EBB (v0.107.0 decompile; relocated here from Increasing Intensity by the 2026-06 patch)
        int eyeLasersDamage = Asc.Deadly(ascension, 12, 11);
        const int eyeLasersHits = 2;                             // EyeLasersRepeat (not Deadly-scaled)
        int intensityBaseStrength = Asc.Deadly(ascension, 4, 3); // IncreasingIntensityBaseStrength
        int witherAmount = Asc.Deadly(ascension, 2, 1);          // Withers added per Increasing Intensity
        const int witheringPresence = 6;                         // CardsLeft start (every 6 player cards → 1 Wither)
        const int artifact = 3;                                   // self Artifact at combat start

        var monster = new Monster { Name = "Aeonglass", MaxHp = hp, CurrentHp = hp };

        // EBB: single attack (picks up the boss's live Strength) + gain Block. The 2026-06 patch reworked this
        // move from SingleAttack+Debuff (the old −3 Str/−3 Dex drain) to SingleAttack+Defend — the dump now shows
        // EBB_MOVE = SingleAttackIntent + DefendIntent (monsters.json), with the Block relocated here from
        // Increasing Intensity. The block AMOUNT is null in the dump (all numeric move fields are nulled), so the
        // relocated 33 is carried over from where it used to live — total boss block per cycle is unchanged, which
        // keeps the estimate sound; confirm the exact value once a numeric move re-dump is available.
        var ebb = new MoveState("EBB_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, ebbDamage, ValueProp.Move, null);
                Cmd.GainBlock(combat, self, ebbBlock, ValueProp.Move, null);
            },
            intentDamage: ebbDamage);

        // EYE_LASERS: 2-hit multi-attack (block soaks per hit; live Strength applies to each hit).
        var eyeLasers = new MoveState("EYE_LASERS_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, eyeLasersDamage, eyeLasersHits, ValueProp.Move, null),
            intentDamage: eyeLasersDamage, intentHits: eyeLasersHits);

        // INCREASING_INTENSITY: the ramp (fake-upgrade + spawn Withers + self-Strength + Block).
        var increasingIntensity = new MoveState("INCREASING_INTENSITY_MOVE",
            (combat, self) =>
            {
                var counter = self.GetPower(AeonglassIntensityPower.PowerId);
                int prior = counter?.Amount ?? 0;        // completed IIs == AdditionalStrength to use now
                int newLevel = prior + 1;                // WitherUpgradeCount after this use; new Wither level

                // (1) Fake-upgrade every Wither already in the player's deck (+3 damage each = +1 level). Since
                // all Withers in this fight share the level, we bump each to the new level.
                BumpWithers(combat.Player.Hand, newLevel);
                BumpWithers(combat.Player.DrawPile, newLevel);
                BumpWithers(combat.Player.DiscardPile, newLevel);
                BumpWithers(combat.Player.ExhaustPile, newLevel);

                // (2) Increment WitherUpgradeCount / AdditionalStrength (both tracked by this one counter).
                if (counter != null) counter.Amount = newLevel;
                else self.AddPower(new AeonglassIntensityPower(), newLevel);

                // (3) Add fresh Withers (at the new level) to the player's DISCARD pile.
                for (int i = 0; i < witherAmount; i++)
                    Cmd.GenerateStatusCard(combat, new AeonglassWither { Level = newLevel }, combat.Player.DiscardPile);

                // (4) Self-Strength = base + AdditionalStrength(prior). After this, AdditionalStrength == newLevel.
                Cmd.ApplyPower(combat, self, new StrengthPower(), intensityBaseStrength + prior, self);

                // (NB) The 2026-06 patch moved this move's Block onto EBB: the dump now shows
                // INCREASING_INTENSITY_MOVE = StatusIntent + BuffIntent only (no DefendIntent), so no block here.
            },
            intentDamage: null);

        // Deterministic fixed cycle (decompile FollowUpState chain), opens on EBB.
        ebb.FollowUp = eyeLasers;
        eyeLasers.FollowUp = increasingIntensity;
        increasingIntensity.FollowUp = ebb;   // loop back to EBB

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { ebb, eyeLasers, increasingIntensity }, ebb.Id);

        // Combat start (AfterAddedToRoom): self Artifact 3; Withering-Presence counter (counts the player's card
        // plays, see the power). Both ride on the boss (the factory has no player handle), and the counter's hook
        // observes player plays via combat.AllPowers exactly as a player-owned power would.
        monster.AddPower(new ArtifactPower(), artifact);
        monster.AddPower(new AeonglassWitheringPresencePower(), witheringPresence);
        return monster;
    }

    /// <summary>Bump every <see cref="AeonglassWither"/> in <paramref name="pile"/> to <paramref name="level"/>
    /// (the fake-upgrade — all Withers in this fight share one level == the boss's completed-II count). Replaces
    /// the instance (never mutates a possibly-shared one) so sibling search branches stay isolated.</summary>
    private static void BumpWithers(List<CardModel> pile, int level)
    {
        for (int i = 0; i < pile.Count; i++)
            if (pile[i] is AeonglassWither w && w.Level < level)
                pile[i] = new AeonglassWither { Level = level };
    }
}

/// <summary>
/// Aeonglass's Wither status card (decompile Wither, specialised so the per-card fake-upgrade ramp is modelled).
/// Cost −1, UNPLAYABLE. At the END of the player's turn, every copy held in HAND deals its damage to the player as
/// a BLOCKABLE, unpowered hit (game OnTurnEndInHand → CreatureCmd.Damage of the Unpowered|Move damage var). Base 3,
/// +3 per fake-upgrade <see cref="Level"/> (Increasing Intensity raises every Wither's level, and Withers spawned
/// afterward open at the boss's current level — see <see cref="Monsters"/>). Stateful so the level clones per
/// search branch and is folded into the state key/hash (sibling branches must not share/desync the ramp).
/// </summary>
public sealed class AeonglassWither : CardModel
{
    public int Level;

    public override string Name => "Wither";
    public override int BaseCost => -1;
    public override CardType Type => CardType.Status;
    public override CardRarity Rarity => CardRarity.Status;
    public override TargetType Target => TargetType.None;
    public override bool Unplayable => true;
    public override bool HasTurnEndInHandEffect => true;
    public override bool Stateful => true;        // mutable Level ⇒ deep-clone per branch, no cached key/hash

    public int Damage => 3 + 3 * Level;

    public override void OnPlay(CombatState combat, CardPlay play) { }   // unplayable

    public override void OnTurnEndInHand(CombatState combat)
        => Cmd.Attack(combat, combat.Player, combat.Player, Damage, ValueProp.Unpowered | ValueProp.Move, this);

    public override CardModel Clone()
    {
        var c = (AeonglassWither)base.Clone();
        c.Level = Level;
        return c;
    }

    public override string StateKey() => Level > 0 ? $"Wither+{Level}" : "Wither";
}

/// <summary>
/// Aeonglass's Increasing-Intensity counter (decompile WitherUpgradeCount / AdditionalStrength, collapsed). An
/// INERT marker on the boss: <see cref="PowerModel.Amount"/> = the number of COMPLETED Increasing Intensities,
/// which equals both the current Wither fake-upgrade level AND the AdditionalStrength applied to the next
/// Increasing Intensity's Strength gain. The Increasing Intensity move reads/advances it directly (it has no
/// passive hook). Stored as a power so the ramp clones and hashes with the monster (base StateKey/HashValue
/// serialise the Amount), keeping every search branch sound.
/// </summary>
public sealed class AeonglassIntensityPower : PowerModel
{
    public const string PowerId = "AeonglassIntensity";
    public override string Id => PowerId;
    public override PowerType Type => PowerType.Buff;
}

// (AeonglassEbbPower removed: the 2026-06 patch dropped EBB's −3 Str/−3 Dex drain — EBB now attacks + blocks.
//  See the PATCH NOTE in the Aeonglass factory docstring. Restore from version control if the drain returns.)

/// <summary>
/// Aeonglass's Withering Presence (decompile WitheringPresencePower — game applies it to the PLAYER at amount 6).
/// A "CardsLeft" counter (<see cref="PowerModel.Amount"/> starts 6): each card the PLAYER plays decrements it, and
/// when it reaches 0 the boss adds 1 Wither to the player's HAND (at the boss's current fake-upgrade level —
/// decompile AfterCardGeneratedForCombat upgrades new Withers WitherUpgradeCount times) and resets the counter to
/// 6. So the player is fed an extra (ramping) Wither every 6 cards played — recurring HARM (more in-hand turn-end
/// chip), reproduced exactly. The Wither is added to HAND, so a copy still held at the player's turn end bites
/// that same turn (the game adds it to hand, not discard) — the pessimistic-sound direction.
///
/// PORT NOTE (behaviourally exact, not a soundness gap): in the game this power is OWNED BY THE PLAYER; here it
/// rides on the BOSS because the solver's monster factory has no combat-start hook to apply a power to the player.
/// The AfterCardPlayed hook fires on every power in combat (player and monster alike), and the player's card plays
/// are the only ones that occur, so counting them on the boss is identical to counting them on the player. Gated
/// on the boss being alive (a dead boss applies nothing — sound).
/// </summary>
public sealed class AeonglassWitheringPresencePower : PowerModel
{
    public const string PowerId = "AeonglassWitheringPresence";
    public const int CardsPerWither = 6;

    public override string Id => PowerId;
    public override PowerType Type => PowerType.Buff;

    public override void AfterCardPlayed(CombatState combat, CardModel card)
    {
        if (!Owner.IsAlive) return;   // dead boss applies nothing
        Amount--;
        if (Amount > 0) return;
        // Spawn a Wither into the player's HAND at the boss's current fake-upgrade level.
        int level = Owner.GetPowerAmount(AeonglassIntensityPower.PowerId);
        Cmd.GenerateStatusCard(combat, new AeonglassWither { Level = level }, combat.Player.Hand);
        Amount = CardsPerWither;   // reset the counter
    }
}
