using System.Collections.Generic;
using Sts2Solver.Engine;

namespace Sts2Solver.Content;

public static partial class Monsters
{
    /// <summary>
    /// Kaiser Crab (Act-2 BOSS, MegaCrit) — a 2-arm encounter: <see cref="Crusher"/> (left) + <see cref="Rocket"/>
    /// (right) (decompile MegaCrit.Sts2.Core.Models.Monsters.KaiserCrabBoss.GenerateMonsters).
    ///
    /// SURROUNDED (the soundness-critical mechanic). The player "faces" one arm; the arm they are NOT facing deals
    /// ×1.5 with its attacks (game: SurroundedPower + BackAttackLeft/RightPower). The player faces whichever arm
    /// they last targeted; the initial facing leaves the LEFT arm (Crusher) behind. Modelled by
    /// <see cref="KaiserBackAttackPower"/> on each arm + the gated <see cref="CombatState.KaiserFrontId"/> facing
    /// tracker (set in <see cref="CombatManager.PlayCard"/>). CRAB RAGE: when one arm dies the survivor gains 6
    /// Strength + 99 Block (<see cref="CrabRagePower"/>) — and is then the only arm, so faced (no more back attack).
    /// </summary>
    public static IEnumerable<Monster> KaiserCrab(int ascension = 0)
    {
        yield return Crusher(ascension: ascension);   // "crusher" slot (left)
        yield return Rocket(ascension: ascension);    // "rocket" slot (right)
    }

    /// <summary>
    /// Crusher (Kaiser Crab LEFT arm). HP 209 (219 Tough), fixed. Combat start: <see cref="KaiserBackAttackPower"/>
    /// (left) + <see cref="CrabRagePower"/>. DETERMINISTIC 5-move cycle (decompile FollowUpState chain):
    ///
    ///   THRASH → ENLARGING_STRIKE → BUG_STING → ADAPT → GUARDED_STRIKE → (back to) THRASH
    ///
    /// Moves (DeadlyEnemies on the left where applicable):
    ///  - THRASH          : single attack 12 / 14. SingleAttackIntent.
    ///  - ENLARGING_STRIKE: single attack 4. SingleAttackIntent.
    ///  - BUG_STING       : 2-hit attack 6 / 7 then apply 2 Weak + 2 Frail to the player. MultiAttack + Debuff.
    ///  - ADAPT           : +Strength 2 / 3 to itself. BuffIntent.
    ///  - GUARDED_STRIKE  : single attack 12 / 14 then gain 18 Block. SingleAttackIntent + DefendIntent.
    /// </summary>
    public static Monster Crusher(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 219, 209);   // MinInitialHp == MaxInitialHp (fixed)
        int thrashDamage = Asc.Deadly(ascension, 14, 12);
        int enlargingStrikeDamage = 4;                      // EnlargingStrikeDamage (not Deadly-scaled)
        int bugStingDamage = Asc.Deadly(ascension, 7, 6);
        const int bugStingHits = 2;                         // BugStingTimes
        int adaptStrength = Asc.Deadly(ascension, 3, 2);
        int guardedStrikeDamage = Asc.Deadly(ascension, 14, 12);
        const int guardedStrikeBlock = 18;
        var monster = new Monster { Name = "Crusher", MaxHp = hp, CurrentHp = hp };

        var thrash = new MoveState("THRASH_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, thrashDamage, ValueProp.Move, null),
            intentDamage: thrashDamage);
        var enlarging = new MoveState("ENLARGING_STRIKE_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, enlargingStrikeDamage, ValueProp.Move, null),
            intentDamage: enlargingStrikeDamage);
        var bugSting = new MoveState("BUG_STING_MOVE",
            (combat, self) =>
            {
                Cmd.AttackMulti(combat, self, combat.Player, bugStingDamage, bugStingHits, ValueProp.Move, null);
                Cmd.ApplyPower(combat, combat.Player, new WeakPower(), 2, self);
                Cmd.ApplyPower(combat, combat.Player, new FrailPower(), 2, self);
            },
            intentDamage: bugStingDamage, intentHits: bugStingHits);
        var adapt = new MoveState("ADAPT_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, self, new StrengthPower(), adaptStrength, self),
            intentDamage: null);
        var guarded = new MoveState("GUARDED_STRIKE_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, guardedStrikeDamage, ValueProp.Move, null);
                Cmd.GainBlock(combat, self, guardedStrikeBlock, ValueProp.Move, null);
            },
            intentDamage: guardedStrikeDamage);

        thrash.FollowUp = enlarging;
        enlarging.FollowUp = bugSting;
        bugSting.FollowUp = adapt;
        adapt.FollowUp = guarded;
        guarded.FollowUp = thrash;   // loop

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { thrash, enlarging, bugSting, adapt, guarded }, thrash.Id);
        monster.AddPower(new KaiserBackAttackPower { IsLeft = true }, 1);
        monster.AddPower(new CrabRagePower(), 1);
        return monster;
    }

    /// <summary>
    /// Rocket (Kaiser Crab RIGHT arm). HP 199 (209 Tough), fixed. Combat start: <see cref="KaiserBackAttackPower"/>
    /// (right) + <see cref="CrabRagePower"/> (the game also applies SurroundedPower to the player — folded into the
    /// per-arm back-attack power here). DETERMINISTIC 5-move cycle (decompile FollowUpState chain):
    ///
    ///   TARGETING_RETICLE → PRECISION_BEAM → CHARGE_UP → LASER → RECHARGE → (back to) TARGETING_RETICLE
    ///
    /// Moves (DeadlyEnemies on the left where applicable):
    ///  - TARGETING_RETICLE: single attack 3 / 4. SingleAttackIntent.
    ///  - PRECISION_BEAM    : single attack 18 / 20. SingleAttackIntent.
    ///  - CHARGE_UP         : +Strength 2 / 3 to itself. BuffIntent.
    ///  - LASER             : single attack 31 / 35. SingleAttackIntent.
    ///  - RECHARGE          : does nothing (a rest turn — game's SleepIntent). No combat effect.
    /// </summary>
    public static Monster Rocket(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 209, 199);   // MinInitialHp == MaxInitialHp (fixed)
        int targetingReticleDamage = Asc.Deadly(ascension, 4, 3);
        int precisionBeamDamage = Asc.Deadly(ascension, 20, 18);
        int chargeUpStrength = Asc.Deadly(ascension, 3, 2);
        int laserDamage = Asc.Deadly(ascension, 35, 31);
        var monster = new Monster { Name = "Rocket", MaxHp = hp, CurrentHp = hp };

        var reticle = new MoveState("TARGETING_RETICLE_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, targetingReticleDamage, ValueProp.Move, null),
            intentDamage: targetingReticleDamage);
        var beam = new MoveState("PRECISION_BEAM_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, precisionBeamDamage, ValueProp.Move, null),
            intentDamage: precisionBeamDamage);
        var chargeUp = new MoveState("CHARGE_UP_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, self, new StrengthPower(), chargeUpStrength, self),
            intentDamage: null);
        var laser = new MoveState("LASER_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, laserDamage, ValueProp.Move, null),
            intentDamage: laserDamage);
        var recharge = new MoveState("RECHARGE_MOVE",
            (combat, self) => { /* SleepIntent — recharges, no combat effect */ },
            intentDamage: null);

        reticle.FollowUp = beam;
        beam.FollowUp = chargeUp;
        chargeUp.FollowUp = laser;
        laser.FollowUp = recharge;
        recharge.FollowUp = reticle;   // loop

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { reticle, beam, chargeUp, laser, recharge }, reticle.Id);
        monster.AddPower(new KaiserBackAttackPower { IsLeft = false }, 1);
        monster.AddPower(new CrabRagePower(), 1);
        return monster;
    }
}
