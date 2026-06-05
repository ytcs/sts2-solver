using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>
/// LagavulinMatriarch (Act-1 BOSS, MegaCrit). Decompile: MegaCrit.Sts2.Core.Models.Monsters.LagavulinMatriarch.
///
/// HP: MinInitialHp == MaxInitialHp == 233 (222 below ToughEnemies) — a fixed value, not a roll.
///
/// SLEEP / WAKE (the boss opens ASLEEP). At combat start (AfterAddedToRoom → Sleep) the boss applies to
/// ITSELF: <c>MatriarchPlatingPower 12</c> (a block-each-turn buff; 14 on ToughEnemies — Slash2Block tracks
/// the same Tough value, see below — actually Plating is a flat 12 regardless of Tough; only Slash2's BLOCK is
/// Tough-scaled) and <c>MatriarchAsleepPower 3</c> (a countdown). While asleep the AI loops the no-op
/// SLEEP_MOVE; it wakes either (a) when Asleep counts down to 0 at an enemy-turn end (the 3rd enemy turn), or
/// (b) IMMEDIATELY the first time it takes UNBLOCKED damage — which strips its Plating, self-stuns the upcoming
/// move (WakeUpMove, a skipped turn) and forces the next move to SLASH. The on-hit wake is the FASTER-attack
/// path and is modelled faithfully so the boss is never UNDER-credited (a player who breaks the asleep boss's
/// block gets attacked sooner than the 3-turn timeout).
///
/// AWAKE LOOP (deterministic 4-move cycle, starting on Slash after waking):
///   Slash (21 / 19 below DeadlyEnemies) → Disembowel (10 / 9 ×2) → Slash2 (14 / 12 + gain 14 / 12 block;
///   the +block is ToughEnemies-scaled) → Soul Siphon (apply -2 Strength AND -2 Dexterity to the player,
///   +2 Strength to itself) → back to Slash.
///
/// Decompile damage/amount sources (DeadlyEnemies on the left where applicable):
///   SlashDamage = 21 / 19; Slash2Damage = 14 / 12; Slash2Block = 14 / 12 (ToughEnemies);
///   DisembowelDamage = 10 / 9, DisembowelRepeat = 2; Soul Siphon: StrengthPower -2 + DexterityPower -2 to the
///   player, StrengthPower +2 to self. Plating 12, Asleep 3 (both fixed).
///
/// The game's ConditionalBranchState (HasAsleep ⇒ Sleep else Slash) is reproduced as a RandomBranchState whose
/// branch weights are 0/1 by HasPower("MatriarchAsleep") — deterministic, exactly the game's condition. The
/// engine has no ConditionalBranchState primitive; the weighted-Markov form is equivalent for a 0/1 split.
/// </summary>
public static partial class Monsters
{
    public static Monster LagavulinMatriarch(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 233, 222);   // MinInitialHp == MaxInitialHp (fixed, no roll)
        int slashDamage = Asc.Deadly(ascension, 21, 19);
        int slash2Damage = Asc.Deadly(ascension, 14, 12), slash2Block = Asc.Tough(ascension, 14, 12);
        int disembowelDamage = Asc.Deadly(ascension, 10, 9), disembowelHits = 2;
        int soulSiphonStrDebuff = 2, soulSiphonDexDebuff = 2, soulSiphonSelfStr = 2;
        const int plating = 12, asleep = 3;
        var monster = new Monster { Name = "LagavulinMatriarch", MaxHp = hp, CurrentHp = hp };

        // ---- Asleep loop: no-op move whose FollowUp is the wake/attack branch ----
        var sleep = new MoveState("SLEEP_MOVE",
            (combat, self) => { /* asleep: no action */ },
            intentDamage: null);

        // ---- Awake 4-move deterministic loop ----
        var slash = new MoveState("SLASH_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, slashDamage, ValueProp.Move, null),
            intentDamage: slashDamage);
        var disembowel = new MoveState("DISEMBOWEL_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, disembowelDamage, disembowelHits, ValueProp.Move, null),
            intentDamage: disembowelDamage, intentHits: disembowelHits);
        var slash2 = new MoveState("SLASH2_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, slash2Damage, ValueProp.Move, null);
                Cmd.GainBlock(combat, self, slash2Block, ValueProp.Move, null);
            },
            intentDamage: slash2Damage);
        var soulSiphon = new MoveState("SOUL_SIPHON_MOVE",
            (combat, self) =>
            {
                // Game: -2 Strength AND -2 Dexterity to the player (debuffs that persist and weaken the
                // player's attacks/blocks), then +2 Strength to itself. StrengthPower/DexterityPower are
                // Buff-typed even at negative amount (so no Artifact-absorb) — matches the decompile.
                Cmd.ApplyPower(combat, combat.Player, new StrengthPower(), -soulSiphonStrDebuff, self);
                Cmd.ApplyPower(combat, combat.Player, new DexterityPower(), -soulSiphonDexDebuff, self);
                Cmd.ApplyPower(combat, self, new StrengthPower(), soulSiphonSelfStr, self);
            },
            intentDamage: null);

        // SLEEP_BRANCH (game's ConditionalBranchState): still asleep ⇒ SLEEP_MOVE; awake ⇒ SLASH_MOVE.
        // Reproduced as a 0/1-weighted RandomBranch keyed on the Asleep power — deterministic, equivalent.
        var sleepBranch = new RandomBranchState("SLEEP_BRANCH")
            .Add(sleep.Id,  m => m.HasPower(MatriarchAsleepPower.PowerId) ? 1f : 0f)
            .Add(slash.Id,  m => m.HasPower(MatriarchAsleepPower.PowerId) ? 0f : 1f);

        sleep.FollowUp = sleepBranch;
        slash.FollowUp = disembowel;
        disembowel.FollowUp = slash2;
        slash2.FollowUp = soulSiphon;
        soulSiphon.FollowUp = slash;   // awake loop

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { sleep, slash, disembowel, slash2, soulSiphon, sleepBranch }, sleep.Id);

        // Applied to ITSELF at combat start (AfterAddedToRoom → Sleep). Plating grants block each turn while
        // asleep; Asleep counts down to wake. Both are stripped the moment the boss wakes (see the powers).
        monster.AddPower(new MatriarchPlatingPower(), plating);
        monster.AddPower(new MatriarchAsleepPower(), asleep);
        return monster;
    }
}

/// <summary>
/// LagavulinMatriarch's Asleep countdown (decompile AsleepPower, specialised to this boss). The boss opens with
/// Asleep 3. TWO wake paths, both modelled:
///   • TIMEOUT: at each enemy-turn end the counter decrements; when it reaches 0 the boss wakes (its Plating is
///     gone too — see MatriarchPlatingPower's matching turn-end-≤1 strip). Removing the Asleep power flips the
///     SLEEP_BRANCH to SLASH, so the boss attacks on the move AFTER the one that drained it (the 3rd sleep turn
///     end ⇒ Slash telegraphed for the 4th enemy turn).
///   • ON-HIT: the first time the boss takes UNBLOCKED damage it wakes instantly — strips its Plating, removes
///     this power (flipping the branch to Slash) and self-stuns the currently telegraphed (SLEEP) move so the
///     upcoming enemy turn is skipped (game: CreatureCmd.Stun(WakeUpMove) then sets next = SLASH_MOVE). Because
///     the skipped move's FollowUp is SLEEP_BRANCH and Asleep is now gone, the next roll yields SLASH — exactly
///     the game's "next move = SLASH_MOVE". This is the FASTER-attack path: breaking the asleep boss's block can
///     get the player attacked SOONER than the 3-turn timeout, so it is modelled (never under-credit the boss).
/// </summary>
public sealed class MatriarchAsleepPower : PowerModel
{
    public const string PowerId = "MatriarchAsleep";
    public override string Id => PowerId;
    public override PowerType Type => PowerType.Buff;

    public override void AfterDamageReceived(CombatState combat, Creature target, int unblockedDamage, Creature? dealer, ValueProp props)
    {
        if (target != Owner || unblockedDamage <= 0) return;   // fully-blocked hits do NOT wake (game guard)
        if (!Owner.IsAlive) return;
        // Wake-on-hit: strip Plating, self-stun the telegraphed sleep move (skip one enemy turn), drop Asleep so
        // the SLEEP_BRANCH FollowUp resolves to SLASH for the next roll.
        Owner.RemovePower(MatriarchPlatingPower.PowerId);
        if (Owner is Monster m) Cmd.Stun(combat, m, 1);
        Owner.RemovePower(Id);
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != CombatSide.Enemy) return;   // Asleep ticks at the enemy turn end
        Amount--;
        if (Amount <= 0)
        {
            // Timeout wake: Plating is also gone now (the game removes Plating as Asleep hits ≤1, then WakeUpMove
            // fires when Asleep ≤0). Removing Asleep flips the branch to SLASH for the following enemy turn.
            Owner.RemovePower(MatriarchPlatingPower.PowerId);
            Owner.RemovePower(Id);
        }
    }
}

/// <summary>
/// LagavulinMatriarch's sleeping armour (decompile PlatingPower, specialised to this boss). Applied 12 at combat
/// start while the boss is asleep, it grants the boss <c>Amount</c> BLOCK and counts down by 1 each enemy turn.
/// In-game the block is granted at the player's turn start (round 1) AND at the boss's own turn end, and the
/// boss's block carries through the player's turn (block clears only on the owner's own turn). We grant the
/// block at the start of EACH player turn the boss still holds Plating — so the block is present while the
/// player attacks (the dominant case). This slightly OVER-states the block vs the game's round-1 player-start
/// gate (the engine has no separate before-turn-start / turn-end-early block hooks), which is the SOUND
/// direction: never under-credit the boss's survivability. Any UNBLOCKED hit wakes the boss and strips this
/// power (see MatriarchAsleepPower), so in practice the player breaks through this block to wake it.
/// </summary>
public sealed class MatriarchPlatingPower : PowerModel
{
    public const string PowerId = "MatriarchPlating";
    public override string Id => PowerId;
    public override PowerType Type => PowerType.Buff;

    public override void AfterSideTurnStart(CombatState combat, CombatSide side)
    {
        // Grant the asleep boss its armour block before the player acts (so the player must break it to wake it).
        if (side == CombatSide.Player && Amount > 0)
            Cmd.GainBlock(combat, Owner, Amount, ValueProp.Unpowered, null);
    }

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        if (side != CombatSide.Enemy) return;   // counts down with the Asleep timer
        Amount--;
        this.NormalizeOrRemove(Owner);
    }
}
