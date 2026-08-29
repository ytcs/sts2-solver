using System.Collections.Generic;
using Sts2Solver.Engine;

namespace Sts2Solver.Content;

public static partial class Monsters
{
    /// <summary>
    /// The Queen (Act-3 BOSS, MegaCrit) — the Queen + a TorchHeadAmalgam ally (decompile
    /// MegaCrit.Sts2.Core.Models.Monsters.QueenBoss.GenerateMonsters: amalgam + queen).
    /// </summary>
    public static IEnumerable<Monster> QueenEncounter(int ascension = 0)
    {
        yield return TorchHeadAmalgam(ascension: ascension);   // "amalgam" slot
        yield return Queen(ascension: ascension);             // "queen" slot
    }

    /// <summary>
    /// Torch Head Amalgam (the Queen's ally). HP 199 (211 Tough), fixed. Combat start: MinionPower (no combat-stat
    /// effect — modelled inert). Move chain (decompile FollowUpState): STRONG_TACKLE → TACKLE → BEAM → WEAK_TACKLE →
    /// WEAK_TACKLE → BEAM (loops on the BEAM → WeakTackle → WeakTackle cycle after the opening Strong+Tackle).
    ///  - STRONG_TACKLE : single attack 26 / 32 (v0.109.0 turn-1 opener).
    ///  - TACKLE        : single attack 18 / 22. SingleAttackIntent.
    ///  - BEAM          : 3-hit attack 8 × 3 (_soulBeamRepeat = 3). MultiAttackIntent.
    ///  - WEAK_TACKLE   : single attack 14 / 16. SingleAttackIntent.
    /// </summary>
    public static Monster TorchHeadAmalgam(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 211, 199);   // MinInitialHp == MaxInitialHp (fixed)
        int strongTackleDamage = Asc.Deadly(ascension, 32, 26);
        int tackleDamage = Asc.Deadly(ascension, 22, 18);
        int weakTackleDamage = Asc.Deadly(ascension, 16, 14);
        int beamDamage = 8;                                 // SoulBeamDamage (not Deadly-scaled)
        const int beamHits = 3;                             // _soulBeamRepeat
        var monster = new Monster { Name = "TorchHeadAmalgam", MaxHp = hp, CurrentHp = hp };

        MoveState Tackle(string id, int dmg) => new MoveState(id,
            (combat, self) => Cmd.Attack(combat, self, combat.Player, dmg, ValueProp.Move, null),
            intentDamage: dmg);

        var tackle1 = Tackle("STRONG_TACKLE_MOVE", strongTackleDamage);
        var tackle2 = Tackle("TACKLE_2_MOVE", tackleDamage);
        var beam = new MoveState("BEAM_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, beamDamage, beamHits, ValueProp.Move, null),
            intentDamage: beamDamage, intentHits: beamHits);
        var wTackle1 = Tackle("TACKLE_3_MOVE", weakTackleDamage);
        var wTackle2 = Tackle("TACKLE_4_MOVE", weakTackleDamage);

        tackle1.FollowUp = tackle2;
        tackle2.FollowUp = beam;
        beam.FollowUp = wTackle1;
        wTackle1.FollowUp = wTackle2;
        wTackle2.FollowUp = beam;   // loop back to BEAM (decompile moveState5.FollowUp = moveState3)

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { tackle1, tackle2, beam, wTackle1, wTackle2 }, tackle1.Id);
        return monster;
    }

    /// <summary>
    /// The Queen. HP 400 (419 Tough), fixed. Combat start: <see cref="QueenAmalgamWatchPower"/> (flips her branch
    /// when the Amalgam dies). Move machine (decompile GenerateMoveStateMachine):
    ///
    ///   PUPPET_STRINGS → YOU_ARE_MINE → [Amalgam alive: BURN_BRIGHT (loops) | Amalgam dead: OFF_WITH_YOUR_HEAD]
    ///   OFF_WITH_YOUR_HEAD → EXECUTION → ENRAGE → OFF_WITH_YOUR_HEAD (loop)
    ///
    /// Moves (DeadlyEnemies on the left where applicable):
    ///  - PUPPET_STRINGS     : apply ChainsOfBinding 3 to the player (modelled as −3 cards drawn/turn — see
    ///                         <see cref="QueenChainsPower"/>). No damage.
    ///  - YOU_ARE_MINE       : apply 99 Frail + 99 Weak + 99 Vulnerable to the player (a fight-long crippling). No damage.
    ///  - BURN_BRIGHT_FOR_ME : give the Amalgam +1 Strength and gain 20 Block (only while the Amalgam lives). Buff + Defend.
    ///  - OFF_WITH_YOUR_HEAD : 5-hit attack 3 / 4 each (_offWithYourHeadRepeat = 5). MultiAttackIntent.
    ///  - EXECUTION          : single attack 15 / 18. SingleAttackIntent.
    ///  - ENRAGE             : +2 Strength to herself. BuffIntent.
    /// </summary>
    public static Monster Queen(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 419, 400);   // MinInitialHp == MaxInitialHp (fixed)
        const int chains = 3;                               // ChainsOfBinding amount
        int offWithHeadDamage = Asc.Deadly(ascension, 4, 3);
        const int offWithHeadHits = 5;                      // _offWithYourHeadRepeat
        int executionDamage = Asc.Deadly(ascension, 18, 15);
        const int burnBrightStrength = 1;                   // each teammate (not Deadly-scaled)
        const int burnBrightBlock = 20;
        const int enrageStrength = 2;
        var monster = new Monster { Name = "Queen", MaxHp = hp, CurrentHp = hp };

        var puppet = new MoveState("PUPPET_STRINGS_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, combat.Player, new QueenChainsPower(), chains, self),
            intentDamage: null);
        var youAreMine = new MoveState("YOU_ARE_MINE_MOVE",
            (combat, self) =>
            {
                Cmd.ApplyPower(combat, combat.Player, new FrailPower(), 99, self);
                Cmd.ApplyPower(combat, combat.Player, new WeakPower(), 99, self);
                Cmd.ApplyPower(combat, combat.Player, new VulnerablePower(), 99, self);
            },
            intentDamage: null);
        var burnBright = new MoveState("BURN_BRIGHT_FOR_ME_MOVE",
            (combat, self) =>
            {
                foreach (var t in combat.Monsters)
                    if (t != self && t.IsAlive)
                        Cmd.ApplyPower(combat, t, new StrengthPower(), burnBrightStrength, self);
                Cmd.GainBlock(combat, self, burnBrightBlock, ValueProp.Move, null);
            },
            intentDamage: null);
        var offWithHead = new MoveState("OFF_WITH_YOUR_HEAD_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, offWithHeadDamage, offWithHeadHits, ValueProp.Move, null),
            intentDamage: offWithHeadDamage, intentHits: offWithHeadHits);
        var execution = new MoveState("EXECUTION_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, executionDamage, ValueProp.Move, null),
            intentDamage: executionDamage);
        var enrage = new MoveState("ENRAGE_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, self, new StrengthPower(), enrageStrength, self),
            intentDamage: null);

        // Branch on whether the Amalgam has died (deterministic 1/0 weights). Two branch points, mirroring the
        // game: one after YOU_ARE_MINE and one after each BURN_BRIGHT.
        RandomBranchState AmalgamBranch(string id) => new RandomBranchState(id)
            .Add(burnBright.Id, (Monster m) => QueenAmalgamWatchPower.Died(m) ? 0f : 1f)
            .Add(offWithHead.Id, (Monster m) => QueenAmalgamWatchPower.Died(m) ? 1f : 0f);
        var branch1 = AmalgamBranch("YOURE_MINE_NOW_BRANCH");
        var branch2 = AmalgamBranch("BURN_BRIGHT_FOR_ME_BRANCH");

        puppet.FollowUp = youAreMine;
        youAreMine.FollowUp = branch1;
        burnBright.FollowUp = branch2;
        offWithHead.FollowUp = execution;
        execution.FollowUp = enrage;
        enrage.FollowUp = offWithHead;   // loop

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { puppet, youAreMine, branch1, burnBright, branch2, offWithHead, execution, enrage },
            puppet.Id);
        monster.AddPower(new QueenAmalgamWatchPower(), 1);
        return monster;
    }
}
