using Sts2Solver.Engine;

namespace Sts2Solver.Content;

public static partial class Monsters
{
    /// <summary>
    /// SoulFysh (Act-1 BOSS, MegaCrit): HP 211 (221 on Ascension ToughEnemies); MinInitialHp == MaxInitialHp, so
    /// the HP is fixed, not a roll. A purely DETERMINISTIC boss — its move state machine is a fixed FollowUp chain
    /// with NO random branching (decompile GenerateMoveStateMachine: each MoveState.FollowUpState points at exactly
    /// the next, looping). Opens on Beckon, then loops a 5-move cycle forever:
    ///
    ///   BECKON → DE_GAS → GAZE → FADE → SCREAM → (back to) BECKON → …
    ///
    /// Moves (decompile):
    ///  - BECKON  : no damage. Generates 2 Beckon status cards into the PLAYER's deck — one into the draw pile and
    ///              one into the discard pile (BeckonMoveAmount = 2). StatusIntent(2).
    ///  - DE_GAS  : attack DeGasDamage = 16 (17 on Ascension DeadlyEnemies). SingleAttackIntent.
    ///  - GAZE    : attack GazeDamage = 7 (8 on DeadlyEnemies) AND generates 1 Beckon into the player's discard pile
    ///              (GazeMoveAmount = 1). SingleAttackIntent + StatusIntent(1).
    ///  - FADE    : pure self-buff — gains 2 Intangible (every instance of HP loss it would take is capped to 1 until
    ///              it decrements off). No player effect. BuffIntent. (The "IsInvisible" flag the decompile toggles is
    ///              cosmetic/animation only — no combat effect — so it is not modelled.)
    ///  - SCREAM  : attack ScreamDamage = 13 (15 on DeadlyEnemies) AND applies 3 Vulnerable to the player
    ///              (ScreamMoveAmount = 3). SingleAttackIntent + DebuffIntent.
    ///
    /// BECKON cards (MegaCrit Beckon — already modelled in Core/StatusCards.cs): a cost-1 Status card; at the END of
    /// the player's turn, every copy still held in hand deals 6 UNBLOCKABLE, unpowered HP loss to the player. They are
    /// added to the draw/discard piles, so they only bite once drawn into hand and not played — modelled faithfully
    /// by the engine's per-card OnTurnEndInHand hook (fired from CombatManager.EndPlayerTurn). This is real, recurring
    /// chip damage on the player, so it is reproduced exactly (never omitted — that would under-credit the boss).
    ///
    /// No death-phase / survive-at-0 / summon / transform mechanic: SoulFysh dies normally at 0 HP (decompile has no
    /// DeathPhaseEntryMove analogue — AfterDeath only twiddles a music parameter).
    /// </summary>
    public static Monster SoulFysh(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 221, 211);   // MinInitialHp == MaxInitialHp (fixed, no roll)
        int deGasDamage = Asc.Deadly(ascension, 17, 16);
        int gazeDamage = Asc.Deadly(ascension, 8, 7);
        int screamDamage = Asc.Deadly(ascension, 15, 13);
        int screamVulnerable = 3;   // ScreamMoveAmount (not Deadly-scaled)
        int fadeIntangible = 2;     // FadeMove applies IntangiblePower(2) to itself
        var monster = new Monster { Name = "SoulFysh", MaxHp = hp, CurrentHp = hp };

        // BECKON: add 2 Beckon status cards to the player — 1 to the draw pile, 1 to the discard pile.
        var beckon = new MoveState("BECKON_MOVE",
            (combat, self) =>
            {
                combat.Player.DrawPile.Add(new Beckon());
                combat.Player.DiscardPile.Add(new Beckon());
            },
            intentDamage: null);
        // DE_GAS: straight attack.
        var deGas = new MoveState("DE_GAS_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, deGasDamage, ValueProp.Move, null),
            intentDamage: deGasDamage);
        // GAZE: attack + add 1 Beckon to the player's discard pile.
        var gaze = new MoveState("GAZE_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, gazeDamage, ValueProp.Move, null);
                combat.Player.DiscardPile.Add(new Beckon());
            },
            intentDamage: gazeDamage);
        // FADE: pure self-buff — gain 2 Intangible. No player-facing effect.
        var fade = new MoveState("FADE_MOVE",
            (combat, self) => Cmd.ApplyPower(combat, self, new IntangiblePower(), fadeIntangible, self),
            intentDamage: null);
        // SCREAM: attack + apply 3 Vulnerable to the player.
        var scream = new MoveState("SCREAM_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, screamDamage, ValueProp.Move, null);
                Cmd.ApplyPower(combat, combat.Player, new VulnerablePower(), screamVulnerable, self);
            },
            intentDamage: screamDamage);

        // Deterministic fixed cycle (decompile FollowUpState chain, opens on Beckon).
        beckon.FollowUp = deGas;
        deGas.FollowUp = gaze;
        gaze.FollowUp = fade;
        fade.FollowUp = scream;
        scream.FollowUp = beckon;   // loop back to Beckon

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { beckon, deGas, gaze, fade, scream }, beckon.Id);
        return monster;
    }
}
