using Sts2Solver.Engine;

namespace Sts2Solver.Content;

public static partial class Monsters
{
    /// <summary>
    /// Vantom (Act-2 Overgrowth BOSS, MegaCrit). Decompile: MegaCrit.Sts2.Core.Models.Monsters.Vantom.
    ///
    /// HP: MinInitialHp == MaxInitialHp == 183 (173 below ToughEnemies) — a fixed value, not a roll.
    ///
    /// SLIPPERY (the soundness-critical mechanic). At combat start (AfterAddedToRoom) Vantom applies to ITSELF
    /// <c>SlipperyPower</c> with amount 9 (8 below ToughEnemies). Slippery is an Intangible-LIKE counter buff:
    /// every instance of HP loss the owner would take is CAPPED to 1 while the power is owned, BUT — unlike
    /// Intangible, which decrements once per enemy turn — Slippery decrements by 1 on EACH hit that lands ≥1
    /// unblocked damage (decompile SlipperyPower.AfterDamageReceived: UnblockedDamage >= 1 ⇒ Decrement). So the
    /// first 9/8 separate hits that connect are each reduced to 1 damage, after which the boss takes full damage.
    /// This makes Vantom extremely tanky against many small hits and is reproduced EXACTLY (capping each hit to 1
    /// and decrementing per connecting hit) so the boss is never UNDER-credited on survivability. Modelled with
    /// the shared SlipperyPower (Monsters/MonsterPowers.cs), whose per-hit ModifyHpLost cap + per-hit
    /// AfterDamageReceived decrement (both hooks fire once per hit inside Cmd.ApplyDamage) match the game's
    /// per-instance ModifyHpLostAfterOsty / AfterDamageReceived exactly.
    ///
    /// DETERMINISTIC 4-move cycle (decompile GenerateMoveStateMachine: a fixed FollowUpState chain, NO random
    /// branching), opening on Ink Blot:
    ///
    ///   INK_BLOT → INKY_LANCE → DISMEMBER → PREPARE → (back to) INK_BLOT → …
    ///
    /// Moves (decompile, DeadlyEnemies on the left where applicable):
    ///  - INK_BLOT   : single attack InkBlotDamage = 8 / 7. SingleAttackIntent.
    ///  - INKY_LANCE : multi-attack InkyLanceDamage = 7 / 6, 2 hits (MultiAttackIntent(_, 2), _inkyLanceRepeat=2).
    ///  - DISMEMBER  : single attack DismemberDamage = 30 / 26 AND adds 3 Wound status cards to the player's
    ///                 DISCARD pile (_dismemberWounds = 3; decompile CardPileCmd.AddToCombatAndPreview<Wound>
    ///                 (targets, PileType.Discard, 3)). SingleAttackIntent + StatusIntent(3).
    ///  - PREPARE    : pure self-buff — +2 Strength to itself (_prepareStrength = 2). BuffIntent. This buffs
    ///                 every subsequent powered attack (Strength is additive per hit), so after one Prepare the
    ///                 next Ink Blot/Inky Lance/Dismember all hit for +2 (and +2 per Inky-Lance hit) — reproduced
    ///                 via StrengthPower, which the engine applies to the boss's own Move-prop attacks.
    ///
    /// None of the Deadly-scaled numbers are scaled by Tough; only HP and the Slippery amount are Tough-scaled.
    /// The Wound count (3) and Prepare Strength (2) are flat.
    ///
    /// No death-phase / survive-at-0 / summon / transform: Vantom dies normally at 0 HP (decompile AfterDeath only
    /// twiddles a music parameter; no DeathPhaseEntryMove analogue).
    /// </summary>
    public static Monster Vantom(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 183, 173);   // MinInitialHp == MaxInitialHp (fixed, no roll)
        int slipperyAmt = Asc.Tough(ascension, 9, 8);
        int inkBlotDamage = Asc.Deadly(ascension, 8, 7);
        int inkyLanceDamage = Asc.Deadly(ascension, 7, 6);
        const int inkyLanceHits = 2;                        // _inkyLanceRepeat
        int dismemberDamage = Asc.Deadly(ascension, 30, 26);
        const int dismemberWounds = 3;                      // _dismemberWounds (not Deadly-scaled)
        const int prepareStrength = 2;                      // _prepareStrength (not Deadly-scaled)
        var monster = new Monster { Name = "Vantom", MaxHp = hp, CurrentHp = hp };

        // INK_BLOT: straight single attack.
        var inkBlot = new MoveState("INK_BLOT_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, inkBlotDamage, ValueProp.Move, null),
            intentDamage: inkBlotDamage);
        // INKY_LANCE: 2-hit multi-attack (block soaks per hit).
        var inkyLance = new MoveState("INKY_LANCE_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, inkyLanceDamage, inkyLanceHits, ValueProp.Move, null),
            intentDamage: inkyLanceDamage, intentHits: inkyLanceHits);
        // DISMEMBER: heavy single attack + 3 Wounds into the player's discard pile.
        var dismember = new MoveState("DISMEMBER_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, dismemberDamage, ValueProp.Move, null);
                for (int i = 0; i < dismemberWounds; i++)
                    Cmd.GenerateStatusCard(combat, new Wound(), combat.Player.DiscardPile);
            },
            intentDamage: dismemberDamage);
        // PREPARE: pure self-buff — +2 Strength (amplifies every later powered attack). No player-facing effect.
        var prepare = new MoveState("PREPARE_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, self, new StrengthPower(), prepareStrength, self),
            intentDamage: null);

        // Deterministic fixed cycle (decompile FollowUpState chain), opens on Ink Blot.
        inkBlot.FollowUp = inkyLance;
        inkyLance.FollowUp = dismember;
        dismember.FollowUp = prepare;
        prepare.FollowUp = inkBlot;   // loop back to Ink Blot

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { inkBlot, inkyLance, dismember, prepare }, inkBlot.Id);

        // Applied to ITSELF at combat start (AfterAddedToRoom). SlipperyPower (the shared MonsterPowers class)
        // caps each incoming hit to 1 and burns a charge per connecting (>= 1 unblocked) hit — exactly the
        // decompile's ModifyHpLostAfterOsty cap + AfterDamageReceived per-hit decrement.
        monster.AddPower(new SlipperyPower(), slipperyAmt);
        return monster;
    }
}
