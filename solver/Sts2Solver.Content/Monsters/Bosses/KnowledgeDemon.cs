using Sts2Solver.Engine;

namespace Sts2Solver.Content;

public static partial class Monsters
{
    /// <summary>
    /// KnowledgeDemon (Act-3 Hive BOSS, MegaCrit). Decompile: MegaCrit.Sts2.Core.Models.Monsters.KnowledgeDemon.
    ///
    /// HP: MinInitialHp == MaxInitialHp == 379 (399 on ToughEnemies) — a fixed value, not a roll.
    ///
    /// MOVE STATE MACHINE (decompile GenerateMoveStateMachine). The chain is a fixed FollowUp sequence
    ///   CURSE_OF_KNOWLEDGE → SLAP → KNOWLEDGE_OVERWHELMING → PONDER → [CurseOfKnowledgeBranch]
    /// and opens on CURSE_OF_KNOWLEDGE. The branch (game ConditionalBranchState) reads the boss's curse
    /// counter: counter &lt; 3 ⇒ CURSE_OF_KNOWLEDGE again; counter ≥ 3 ⇒ SLAP. CurseOfKnowledge increments
    /// the counter each time it RESOLVES (there are exactly 3 curse sets, indices 0/1/2). So the realised
    /// sequence is:
    ///   Curse(0) → Slap → KnowOver → Ponder → Curse(1) → Slap → KnowOver → Ponder → Curse(2) → Slap →
    ///   KnowOver → Ponder → [counter now 3] → Slap → KnowOver → Ponder → Slap → KnowOver → Ponder → …
    /// i.e. three full Curse cycles, then an endless Slap → KnowledgeOverwhelming → Ponder loop. The
    /// ConditionalBranchState is reproduced as a 0/1-weighted RandomBranchState keyed on the boss's curse
    /// counter (the engine has no ConditionalBranchState primitive; a deterministic 0/1 split is exact).
    /// The counter is carried on a per-creature counter power (KdCurseCountPower) so it clones + hashes with
    /// the monster across every search branch (a shared-closure int would leak between unrelated nodes).
    ///
    /// MOVES (decompile, DeadlyEnemies on the left where it applies):
    ///  - SLAP                  : single attack SlapDamage = 18 / 17. SingleAttackIntent.
    ///  - KNOWLEDGE_OVERWHELMING: 3-hit multi-attack KnowledgeOverwhelmingDamage = 9 / 8
    ///                            (_knowledgeOverwhelmingRepeat = 3). MultiAttackIntent(_, 3). Also flips an
    ///                            IsBurnt flag (cosmetic: swaps hurt/die anims to the "burnt" variants — NO
    ///                            combat effect, so not modelled). PONDER flips it back.
    ///  - PONDER                : single attack PonderDamage = 13 / 11, THEN heals ITSELF 30 × players.Count
    ///                            (= 30 in single-player; _ponderHeal = 30) AND gains PonderStrength = 3 / 2
    ///                            Strength on itself. SingleAttackIntent + HealIntent + BuffIntent. The +Str
    ///                            ramps every later powered attack (Slap, KnowledgeOverwhelming per hit, the
    ///                            next Ponder), so it is reproduced via StrengthPower.
    ///  - CURSE_OF_KNOWLEDGE    : NO direct boss-attack. Forces the player to take a CURSE card (see below).
    ///                            DebuffIntent. Increments the curse counter.
    ///
    /// CURSE OF KNOWLEDGE — soundness-critical, and PARTIALLY a missing-engine-mechanic (flagged here, not
    /// silently omitted). The move presents the player a choice of ONE of two status cards from the set at the
    /// current counter index; whichever is chosen immediately applies a player-side debuff power:
    ///   set 0: { Disintegration(6), MindRot(1)   }
    ///   set 1: { Disintegration(7), Sloth(3)     }
    ///   set 2: { Disintegration(8), WasteAway(1) }
    /// Disintegration appears in EVERY set; its power deals Amount UNBLOCKABLE HP loss to the player at the END
    /// of EACH of the player's turns, for the rest of the fight (decompile DisintegrationPower.AfterSideTurnEnd-
    /// Late ⇒ CreatureCmd.Damage(Amount)). The alternatives are non-damage player debuffs the engine cannot
    /// express: MindRot = draw 1 fewer card/turn; Sloth = may only play 3 cards/turn; WasteAway = −1 max energy.
    ///
    /// SOUND MODELLING (pessimistic, never under-credit the boss): the player picks ONE card per set, and the
    /// engine has neither the monster-offers-a-card-choice mechanism nor the four player-debuff powers. We model
    /// the WORST case for the player — they take Disintegration every set — because Disintegration is the only
    /// quantifiable, reproducible HP threat and is always available; assuming it each time OVER-estimates the
    /// recurring damage vs a player who dodges some onto the (also-harmful but non-damaging) alternative. The
    /// three Disintegrations STACK as recurring end-of-player-turn HP loss (a self-contained KdDisintegration-
    /// Power applied to the player), so after all three curses the player bleeds 6+7+8 = 21 unblockable HP at
    /// the end of every one of their turns for the rest of the fight. The non-damage alternatives (draw/energy/
    /// play-restriction debuffs) are NOT modelled — they are MISSING engine mechanics; omitting them is the
    /// sound direction (they only weaken the PLAYER further, so leaving them out under-states the difficulty,
    /// never over-states it). See the report for the explicit flag.
    ///
    /// No death-phase / survive-at-0 / summon / transform: KnowledgeDemon dies normally at 0 HP (decompile
    /// BeforeRemovedFromRoom only twiddles a music parameter; no DeathPhaseEntryMove analogue).
    /// </summary>
    public static Monster KnowledgeDemon(int hp = -1, int ascension = 0)
    {
        if (hp < 0) hp = Asc.Tough(ascension, 399, 379);   // MinInitialHp == MaxInitialHp (fixed, no roll)
        int slapDamage = Asc.Deadly(ascension, 18, 17);
        int knowledgeOverwhelmingDamage = Asc.Deadly(ascension, 9, 8);
        const int knowledgeOverwhelmingHits = 3;           // _knowledgeOverwhelmingRepeat
        int ponderDamage = Asc.Deadly(ascension, 13, 11);
        int ponderStrength = Asc.Deadly(ascension, 3, 2);
        const int ponderHeal = 30;                         // _ponderHeal × players.Count (=1 single-player)
        // _disintegrationDamageValues[counter]: the worst-case curse damage taken on each of the 3 curses.
        int[] disintegrationByCurse = { 6, 7, 8 };
        const int curseSets = 3;                           // _curseOfKnowledgeSets.Count
        var monster = new Monster { Name = "KnowledgeDemon", MaxHp = hp, CurrentHp = hp };

        // SLAP: straight single attack.
        var slap = new MoveState("SLAP_MOVE",
            (combat, self) => Cmd.Attack(combat, self, combat.Player, slapDamage, ValueProp.Move, null),
            intentDamage: slapDamage);

        // KNOWLEDGE_OVERWHELMING: 3-hit multi-attack (block soaks per hit). IsBurnt flip is cosmetic.
        var knowledgeOverwhelming = new MoveState("KNOWLEDGE_OVERWHELMING_MOVE",
            (combat, self) => Cmd.AttackMulti(combat, self, combat.Player, knowledgeOverwhelmingDamage,
                knowledgeOverwhelmingHits, ValueProp.Move, null),
            intentDamage: knowledgeOverwhelmingDamage, intentHits: knowledgeOverwhelmingHits);

        // PONDER: single attack, then self-heal 30, then +Strength to itself (ramps every later powered hit).
        var ponder = new MoveState("PONDER_MOVE",
            (combat, self) =>
            {
                Cmd.Attack(combat, self, combat.Player, ponderDamage, ValueProp.Move, null);
                self.Heal(ponderHeal);                     // PonderHeal × players.Count == ×1 single-player
                Cmd.ApplyPower(combat, self, new StrengthPower(), ponderStrength, self);
            },
            intentDamage: ponderDamage);

        // CURSE_OF_KNOWLEDGE: no boss attack. Worst-case curse = Disintegration at this curse index ⇒ recurring
        // end-of-player-turn HP loss applied to the player (see KdDisintegrationPower). Increments the counter.
        var curse = new MoveState("CURSE_OF_KNOWLEDGE_MOVE",
            (combat, self) =>
            {
                int idx = self.GetPowerAmount(KdCurseCountPower.PowerId);   // 0,1,2 across the three curses
                int dmg = idx < disintegrationByCurse.Length ? disintegrationByCurse[idx] : disintegrationByCurse[^1];
                Cmd.ApplyPower(combat, combat.Player, new KdDisintegrationPower(), dmg, self);
                Cmd.ApplyPower(combat, self, new KdCurseCountPower(), 1, self);   // counter++
            },
            intentDamage: null);

        // CurseOfKnowledgeBranch (game ConditionalBranchState: counter < 3 ⇒ Curse, else Slap). Reproduced as a
        // deterministic 0/1-weighted RandomBranch keyed on the boss's curse counter — exact for a 0/1 split.
        var curseBranch = new RandomBranchState("CURSE_OF_KNOWLEDGE_BRANCH")
            .Add(curse.Id, m => m.GetPowerAmount(KdCurseCountPower.PowerId) < curseSets ? 1f : 0f)
            .Add(slap.Id,  m => m.GetPowerAmount(KdCurseCountPower.PowerId) >= curseSets ? 1f : 0f);

        curse.FollowUp = slap;
        slap.FollowUp = knowledgeOverwhelming;
        knowledgeOverwhelming.FollowUp = ponder;
        ponder.FollowUp = curseBranch;

        monster.Ai = new MonsterMoveStateMachine(
            new MonsterState[] { curse, slap, knowledgeOverwhelming, ponder, curseBranch }, curse.Id);
        return monster;
    }
}

/// <summary>
/// KnowledgeDemon's curse counter (the game's _curseOfKnowledgeCounter, carried as a per-creature counter power
/// so it clones + hashes with the monster across search branches). CurseOfKnowledge bumps it by 1 each time it
/// resolves; the CURSE_OF_KNOWLEDGE_BRANCH compares it against 3 to decide Curse-again vs fall through to Slap.
/// A pure-state buff: no hooks, no display, never decrements.
/// </summary>
public sealed class KdCurseCountPower : PowerModel
{
    public const string PowerId = "KdCurseCount";
    public override string Id => PowerId;
    public override PowerType Type => PowerType.Buff;
}

/// <summary>
/// KnowledgeDemon's Disintegration curse, modelled as the player-side debuff it applies (decompile
/// DisintegrationPower). The player is forced to take a Disintegration status card (worst-case of the curse
/// choice — see the boss summary); its power deals <c>Amount</c> UNBLOCKABLE, unpowered HP loss to the player
/// at the END of EACH of the player's turns, for the rest of the fight (game AfterSideTurnEndLate ⇒
/// CreatureCmd.Damage(Amount)). The three curses STACK (counter-stacked: 6, then +7, then +8 ⇒ 21/turn) — the
/// pessimistic over-estimate that never under-credits the boss. The tick is unblockable HP loss, so block does
/// not absorb it (matching the game, which damages the player directly with no block interaction here).
/// </summary>
public sealed class KdDisintegrationPower : PowerModel
{
    public const string PowerId = "KdDisintegration";
    public override string Id => PowerId;
    public override PowerType Type => PowerType.Debuff;

    public override void AfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        // The game's AfterSideTurnEndLate fires at the END of the side's turn for participants that include the
        // owner. The owner is the player, so this ticks at the player-turn end (block already gone / unblockable).
        if (side != Owner.Side || Amount <= 0 || !Owner.IsAlive) return;
        Cmd.LoseHp(combat, Owner, Amount);   // unblockable + unpowered HP loss
    }
}
