using System.Collections.Generic;
using Sts2Solver.Engine;

namespace Sts2Solver.Content;

public static partial class Monsters
{
    /// <summary>
    /// The Kin (Act-1 BOSS, MegaCrit) — a 3-creature encounter: two <see cref="KinFollower"/>s + a
    /// <see cref="KinPriest"/> leader (decompile MegaCrit.Sts2.Core.Models.Monsters.TheKinBoss.GenerateMonsters:
    /// slot1 = KinFollower with StartsWithDance, slot2 = KinFollower, leaderSlot = KinPriest).
    ///
    /// WIN CONDITION (pessimistic-sound gap): the followers carry the game's <c>MinionPower</c>
    /// (OwnerIsSecondaryEnemy; a minion's own death is not fatal-triggering). In the solver every monster must die
    /// to win, so if the game actually ends the fight when the PRIEST (the non-minion leader) dies, we require
    /// killing all three instead — over-crediting the boss's survivability (HARDER than reality), which is the
    /// SOUND direction (never optimistic). MinionPower has no damage/block effect, so it isn't modelled otherwise.
    /// </summary>
    public static IEnumerable<Monster> TheKin(int ascension = 0)
    {
        yield return KinFollower(ascension: ascension, startsWithDance: true);   // slot1
        yield return KinFollower(ascension: ascension);                          // slot2
        yield return KinPriest(ascension: ascension);                           // leaderSlot
    }

    /// <summary>
    /// Kin Follower (The Kin minion). HP 58–59 (62–63 Tough). Combat start: <c>MinionPower 1</c> (no combat-stat
    /// effect — see <see cref="TheKin"/>). DETERMINISTIC 3-move cycle (decompile FollowUpState chain):
    ///
    ///   QUICK_SLASH → BOOMERANG → POWER_DANCE → (back to) QUICK_SLASH
    ///
    /// One follower opens on POWER_DANCE (<paramref name="startsWithDance"/> — the game's slot1 KinFollower).
    /// Moves (DeadlyEnemies on the left where applicable):
    ///  - QUICK_SLASH : single attack 5. SingleAttackIntent.
    ///  - BOOMERANG   : 2-hit multi-attack 2×2 (_boomerangRepeat = 2). MultiAttackIntent.
    ///  - POWER_DANCE : pure self-buff +Strength DanceStrength = 3 / 2 (amplifies every later powered hit). BuffIntent.
    /// </summary>
    public static Monster KinFollower(int hp = -1, int ascension = 0, bool startsWithDance = false)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 63, 59);     // max roll of 58–59 base / 62–63 Tough
        int quickSlashDamage = 5;                           // QuickSlashDamage (not Deadly-scaled)
        int boomerangDamage = 2;                            // BoomerangDamage (not Deadly-scaled)
        const int boomerangHits = 2;                        // _boomerangRepeat
        int danceStrength = Asc.Deadly(ascension, 3, 2);    // DanceStrength
        var monster = new Monster { Name = "KinFollower", MaxHp = hp, CurrentHp = hp };

        var quickSlash = new MoveState("QUICK_SLASH_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, quickSlashDamage, ValueProp.Move, null),
            intentDamage: quickSlashDamage);
        var boomerang = new MoveState("BOOMERANG_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, boomerangDamage, boomerangHits, ValueProp.Move, null),
            intentDamage: boomerangDamage, intentHits: boomerangHits);
        var powerDance = new MoveState("POWER_DANCE_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, self, new StrengthPower(), danceStrength, self),
            intentDamage: null);

        quickSlash.FollowUp = boomerang;
        boomerang.FollowUp = powerDance;
        powerDance.FollowUp = quickSlash;   // loop

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { quickSlash, boomerang, powerDance },
            (startsWithDance ? powerDance : quickSlash).Id);
        return monster;
    }

    /// <summary>
    /// Kin Priest (The Kin leader). HP 190 (199 Tough) — fixed (min == max). DETERMINISTIC 4-move cycle
    /// (decompile FollowUpState chain), opening on Orb of Frailty:
    ///
    ///   ORB_OF_FRAILTY → ORB_OF_WEAKNESS → BEAM → RITUAL → (back to) ORB_OF_FRAILTY
    ///
    /// Moves (DeadlyEnemies on the left where applicable):
    ///  - ORB_OF_FRAILTY  : single attack 8 / 9 then apply 1 Frail to the player. SingleAttackIntent + DebuffIntent.
    ///  - ORB_OF_WEAKNESS : single attack 8 / 9 then apply 1 Weak to the player. SingleAttackIntent + DebuffIntent.
    ///  - BEAM            : 3-hit multi-attack 3×3 (_beamRepeat = 3). MultiAttackIntent.
    ///  - RITUAL          : pure self-buff +Strength RitualStrength = 3 / 2. BuffIntent.
    ///
    /// AfterDeath only plays a follower-death speech line / music — no combat effect, so it isn't modelled.
    /// </summary>
    public static Monster KinPriest(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 199, 190);   // MinInitialHp == MaxInitialHp (fixed, no roll)
        int orbOfFrailtyDamage = Asc.Deadly(ascension, 9, 8);
        int orbOfWeaknessDamage = Asc.Deadly(ascension, 9, 8);
        int beamDamage = 3;                                 // BeamDamage (not Deadly-scaled)
        const int beamHits = 3;                             // _beamRepeat
        int ritualStrength = Asc.Deadly(ascension, 3, 2);   // RitualStrength
        var monster = new Monster { Name = "KinPriest", MaxHp = hp, CurrentHp = hp };

        var frailty = new MoveState("ORB_OF_FRAILTY_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, orbOfFrailtyDamage, ValueProp.Move, null);
                Cmd.ApplyPower(combat, combat.Player, new FrailPower(), 1, self);
            },
            intentDamage: orbOfFrailtyDamage);
        var weakness = new MoveState("ORB_OF_WEAKNESS_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, orbOfWeaknessDamage, ValueProp.Move, null);
                Cmd.ApplyPower(combat, combat.Player, new WeakPower(), 1, self);
            },
            intentDamage: orbOfWeaknessDamage);
        var beam = new MoveState("BEAM_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, beamDamage, beamHits, ValueProp.Move, null),
            intentDamage: beamDamage, intentHits: beamHits);
        var ritual = new MoveState("RITUAL_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, self, new StrengthPower(), ritualStrength, self),
            intentDamage: null);

        frailty.FollowUp = weakness;
        weakness.FollowUp = beam;
        beam.FollowUp = ritual;
        ritual.FollowUp = frailty;   // loop

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { frailty, weakness, beam, ritual }, frailty.Id);
        return monster;
    }
}
