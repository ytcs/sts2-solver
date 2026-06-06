using Sts2Solver.Engine;

namespace Sts2Solver.Content;

public static partial class Monsters
{
    /// <summary>
    /// Test Subject (Act-3 BOSS, MegaCrit) — a 3-FORM revive boss. Decompile:
    /// MegaCrit.Sts2.Core.Models.Monsters.TestSubject.
    ///
    /// HP: First form 100 (111 Tough), Second 200 (212), Third 300 (313) — all fixed (min == max). Combat start:
    /// <see cref="AdaptablePower"/> (the revive) + <see cref="EnragePower"/> (gains Strength when the player plays
    /// a Skill). Adaptable VETOES death at 0 HP twice (<see cref="CombatManager"/> → <see cref="Cmd"/> death veto):
    ///   • 1st death → heal to Second-form HP, gain <see cref="PainfulStabsPower"/> (its hits add Wounds), switch
    ///     to the MULTI_CLAW phase.
    ///   • 2nd death → heal to Third-form HP, gain <see cref="NemesisPower"/> (Intangible every other turn), DROP
    ///     Adaptable + PainfulStabs, switch to the LACERATE/POUNCE/GROWL phase.
    ///   • 3rd death (Adaptable gone) is real → the fight ends.
    ///
    /// MOVE MACHINE (decompile GenerateMoveStateMachine):
    ///   Phase 1: BITE ⇄ SKULL_BASH (initial BITE).
    ///   On revive: RESPAWN (a no-op telegraph turn — the heal already happened in the veto) → branch on Respawns:
    ///     Respawns &lt; 2 → MULTI_CLAW (self-loop; +1 hit each use). Respawns ≥ 2 → LACERATE → BIG_POUNCE →
    ///     BURNING_GROWL → LACERATE (loop).
    ///
    /// Moves (DeadlyEnemies on the left where applicable):
    ///  - BITE          : single attack 20 / 22. SingleAttackIntent.
    ///  - SKULL_BASH    : single attack 14 / 16 then apply 1 Vulnerable to the player. SingleAttack + Debuff.
    ///  - MULTI_CLAW    : multi-attack 10 / 11 × (3 + uses-so-far) — ramps a hit each time. MultiAttackIntent.
    ///  - PHASE3_LACERATE: multi-attack 10 / 11 × 3. MultiAttackIntent.
    ///  - BIG_POUNCE    : single attack 45. SingleAttackIntent.
    ///  - BURNING_GROWL : add 3 / 5 Burn to the player's discard + gain Strength 2 / 3. Status + Buff.
    /// </summary>
    public static Monster TestSubject(int hp = -1, int ascension = 0)
    {
        int firstFormHp = Asc.Tough(ascension, 111, 100);
        int secondFormHp = Asc.Tough(ascension, 212, 200);
        int thirdFormHp = Asc.Tough(ascension, 313, 300);
        if (hp < 0) hp = firstFormHp;
        int enrageAmount = Asc.Deadly(ascension, 3, 2);
        int biteDamage = Asc.Deadly(ascension, 22, 20);
        int skullBashDamage = Asc.Deadly(ascension, 16, 14);
        int multiClawDamage = Asc.Deadly(ascension, 11, 10);
        const int multiClawBaseHits = 3;
        int lacerateDamage = Asc.Deadly(ascension, 11, 10);
        const int lacerateHits = 3;
        const int bigPounceDamage = 45;
        int burnCount = Asc.Deadly(ascension, 5, 3);
        int growlStrength = Asc.Deadly(ascension, 3, 2);
        var monster = new Monster { Name = "TestSubject", MaxHp = hp, CurrentHp = hp };

        // Phase 1.
        var bite = new MoveState("BITE_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, biteDamage, ValueProp.Move, null),
            intentDamage: biteDamage);
        var skullBash = new MoveState("SKULL_BASH_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, skullBashDamage, ValueProp.Move, null);
                Cmd.ApplyPower(combat, combat.Player, new VulnerablePower(), 1, self);
            },
            intentDamage: skullBashDamage);
        bite.FollowUp = skullBash;
        skullBash.FollowUp = bite;

        // Revive telegraph (heal already applied by AdaptablePower's veto) → branch to the phase move set.
        var respawn = new MoveState("RESPAWN_MOVE", (combat, self) => { }, intentDamage: null);

        // Phase 2: ramping multi-claw (loops on itself).
        var multiClaw = new MoveState("MULTI_CLAW_MOVE",
            (combat, self) =>
            {
                Cmd.AttackMulti(combat, self, combat.Player, multiClawDamage, multiClawBaseHits + self.ExtraHits, ValueProp.Move, null);
                self.ExtraHits++;   // +1 hit next time
            },
            intentDamage: multiClawDamage, intentHits: multiClawBaseHits);
        multiClaw.FollowUp = multiClaw;

        // Phase 3.
        var lacerate = new MoveState("PHASE3_LACERATE_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, lacerateDamage, lacerateHits, ValueProp.Move, null),
            intentDamage: lacerateDamage, intentHits: lacerateHits);
        var bigPounce = new MoveState("BIG_POUNCE_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, bigPounceDamage, ValueProp.Move, null),
            intentDamage: bigPounceDamage);
        var burningGrowl = new MoveState("BURNING_GROWL_MOVE",
            (combat, self) =>
            {
                for (int i = 0; i < burnCount; i++)
                    Cmd.GenerateStatusCard(combat, new Burn(), combat.Player.DiscardPile);
                Cmd.ApplyPower(combat, self, new StrengthPower(), growlStrength, self);
            },
            intentDamage: null);
        lacerate.FollowUp = bigPounce;
        bigPounce.FollowUp = burningGrowl;
        burningGrowl.FollowUp = lacerate;   // loop

        // After RESPAWN, branch on how many times we've revived (deterministic 1/0 weights).
        var reviveBranch = new RandomBranchState("REVIVE_BRANCH")
            .Add(multiClaw.Id, (Monster m) => m.Respawns < 2 ? 1f : 0f)
            .Add(lacerate.Id, (Monster m) => m.Respawns >= 2 ? 1f : 0f);
        respawn.FollowUp = reviveBranch;

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { bite, skullBash, respawn, reviveBranch, multiClaw, lacerate, bigPounce, burningGrowl },
            bite.Id);

        monster.AddPower(new AdaptablePower { SecondFormHp = secondFormHp, ThirdFormHp = thirdFormHp }, 1);
        monster.AddPower(new EnragePower(), enrageAmount);
        return monster;
    }
}
