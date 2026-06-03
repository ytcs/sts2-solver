namespace Sts2Solver.Engine;

/// <summary>
/// Combat commands cards and monster moves call into. These replicate the game's damage/block
/// pipelines (Appendix B) and power application. Synchronous (no animation sequencing).
/// </summary>
public static class Cmd
{
    /// <summary>
    /// Resolve an attack from <paramref name="dealer"/> against <paramref name="target"/>.
    /// Returns unblocked HP loss inflicted.
    /// </summary>
    public static int Attack(CombatState combat, Creature dealer, Creature target, int baseDamage,
        ValueProp props, CardModel? cardSource)
    {
        decimal amount = baseDamage;

        // Additive modifiers (Strength: dealer side, powered attacks only).
        foreach (var c in combat.AllCreatures)
            foreach (var p in c.Powers)
                amount += p.ModifyDamageAdditive(target, amount, props, dealer, cardSource);

        // Multiplicative modifiers (Vulnerable ×1.5 on target, Weak ×0.75 on dealer).
        foreach (var c in combat.AllCreatures)
            foreach (var p in c.Powers)
                amount *= p.ModifyDamageMultiplicative(target, amount, props, dealer, cardSource);

        // STS convention: floor once after all modifiers, clamp to ≥0.
        int modified = (int)Math.Floor(Math.Max(0m, amount));

        // Block is absorbed by the original target; any UNBLOCKED remainder may be redirected inside
        // ApplyDamage (Osty's DieForYou pulls the post-block hit off the player). Modifiers above were
        // computed against the original target — matching the game, which redirects only the final hit.
        int lost = ApplyDamage(combat, target, modified, props, dealer);

        // Attack-completion hook (Vigor consumes its bonus here, once spent on a powered attack).
        foreach (var p in combat.AllPowers.ToList()) p.AfterAttackDealt(combat, dealer, props);
        return lost;
    }

    /// <summary>A multi-hit attack. Each hit is resolved independently (block absorbs per hit).</summary>
    public static int AttackMulti(CombatState combat, Creature dealer, Creature target, int baseDamage,
        int hits, ValueProp props, CardModel? cardSource)
    {
        int total = 0;
        for (int i = 0; i < hits && target.IsAlive; i++)
            total += Attack(combat, dealer, target, baseDamage, props, cardSource);
        return total;
    }

    /// <summary>Apply already-modified damage through block to a target's HP. Returns HP lost.</summary>
    public static int ApplyDamage(CombatState combat, Creature target, int modified, ValueProp props, Creature? dealer = null)
    {
        // Intangible clamps any single damage instance to 1 (before block). Guarded on the power so it is
        // inert for characters that never gain it. (Game: IntangiblePower.)
        if (modified > 1 && target.HasPower("Intangible")) modified = 1;

        int blocked = 0;
        if (!props.HasFlag(ValueProp.Unblockable))
        {
            blocked = Math.Min(target.Block, modified);
            target.Block -= blocked;
        }
        int unblocked = modified - blocked;

        // Redirect the UNBLOCKED remainder to a different creature as direct HP loss (Osty's DieForYou pulls
        // the post-block hit off the player — the player's block already soaked its share above). Block was
        // taken from the original target; the redirected creature's own block does NOT apply, matching the
        // game's LoseHpInternal on the unblocked-damage target. (Game: Hook.ModifyUnblockedDamageTarget.)
        Creature hpTarget = target;
        if (unblocked > 0)
            foreach (var c in combat.AllCreatures)
                foreach (var p in c.Powers)
                    hpTarget = p.ModifyUnblockedDamageTarget(hpTarget, unblocked, props, dealer);

        // HP-loss modifiers (HardenedShell caps the target's total loss per turn).
        foreach (var c in combat.AllCreatures)
            foreach (var p in c.Powers)
                unblocked = p.ModifyHpLost(hpTarget, unblocked, props, dealer);
        unblocked = Math.Max(0, unblocked);

        int redirectOverkill = 0;
        if (unblocked > 0)
        {
            int before = hpTarget.CurrentHp;
            hpTarget.LoseHpInternal(unblocked);
            int actualLost = before - hpTarget.CurrentHp;
            if (hpTarget.IsPlayer)
            {
                combat.PlayerHpLost += actualLost;
                combat.PlayerUnblockedHitsCount++;   // Tear Asunder hits 1 + this
                // "Lost HP this turn" (Spite) = unblocked self-damage on the player's own turn.
                if (combat.CurrentSide == CombatSide.Player) combat.PlayerLostHpThisTurn = true;
            }
            if (before > 0 && hpTarget.CurrentHp == 0)
                foreach (var p in combat.AllPowers.ToList())
                    p.AfterCreatureDeath(combat, hpTarget);
            // Damage redirected onto another creature (Osty) that exceeds its HP spills the OVERKILL back
            // onto the original target (the player). Block was already absorbed above. (Game: CreatureCmd
            // applies unblockedDamageResult.OverkillDamage to originalTarget when it != the redirect target.)
            if (hpTarget != target) redirectOverkill = unblocked - before;
        }
        // Damage-received hook fires even on a fully-blocked attack (PersonalHive reacts to the hit;
        // Shriek guards internally on unblocked>0). NecroMastery reflects the HP Osty loses here.
        foreach (var p in combat.AllPowers.ToList())
            p.AfterDamageReceived(combat, hpTarget, unblocked, dealer, props);

        // Apply the redirect overkill to the player as direct HP loss (already past block; the dead Osty
        // can no longer redirect, so this resolves on the player). Fires its own death/received hooks.
        if (redirectOverkill > 0)
            ApplyDamage(combat, target, redirectOverkill, ValueProp.Unblockable | ValueProp.Unpowered, dealer);
        return unblocked;
    }

    /// <summary>Direct, unblockable, unpowered HP loss (e.g. Poison ticks).</summary>
    public static int LoseHp(CombatState combat, Creature target, int amount)
        => ApplyDamage(combat, target, amount, ValueProp.Unblockable | ValueProp.Unpowered);

    /// <summary>Instantly remove a creature's remaining HP (Doom execute; Bone Shards / Sacrifice consuming
    /// Osty). Routed through ApplyDamage so death + damage-received hooks fire — notably NecroMastery, which
    /// reflects the HP Osty loses when it is sacrificed. Returns HP removed.</summary>
    public static int Kill(CombatState combat, Creature target)
    {
        if (!target.IsAlive) return 0;
        return ApplyDamage(combat, target, target.CurrentHp, ValueProp.Unblockable | ValueProp.Unpowered);
    }

    /// <summary>Gain block, applying block modifiers (Dexterity additive, Frail ×0.75).</summary>
    public static void GainBlock(CombatState combat, Creature target, int baseBlock, ValueProp props, CardModel? cardSource)
    {
        decimal block = baseBlock;
        foreach (var c in combat.AllCreatures)
            foreach (var p in c.Powers)
                block += p.ModifyBlockAdditive(target, block, props, cardSource);
        foreach (var c in combat.AllCreatures)
            foreach (var p in c.Powers)
                block *= p.ModifyBlockMultiplicative(target, block, props, cardSource);
        int gained = (int)Math.Floor(Math.Max(0m, block));
        target.GainBlockDirect(gained);

        // Block-gained hook (Juggernaut deals damage to an enemy whenever its owner blocks). Fires even
        // on a 0 gain; the power guards on amount as the game does.
        foreach (var p in combat.AllPowers.ToList()) p.AfterBlockGained(combat, target, gained, props, cardSource);
    }

    /// <summary>Exhaust a specific card currently in the player's hand (Fiend Fire, Second Wind, True Grit,
    /// Cinder). Moves it to the exhaust pile, sets the exhausted-this-turn flag, and fires the on-exhaust
    /// hook (Feel No Pain / Dark Embrace).</summary>
    public static void ExhaustFromHand(CombatState combat, CardModel card)
    {
        if (!combat.Player.Hand.Remove(card)) return;
        combat.Player.ExhaustPile.Add(card);
        combat.CardExhaustedThisTurn = true;
        foreach (var p in combat.AllPowers.ToList()) p.AfterCardExhausted(combat, card, false);
    }

    /// <summary>Grant the player <paramref name="amount"/> energy mid-turn (Bloodletting, Offering). The
    /// validator tracks energy across the turn, so this makes later X-cost / over-3-energy plays resolve.</summary>
    public static void GainEnergy(CombatState combat, int amount)
    {
        if (amount > 0 && !combat.Player.HasPower("NoEnergyGain")) combat.Player.Energy += amount;
    }

    /// <summary>Grant the player <paramref name="amount"/> stars (Regent: Venerate, Glow, DivineRight, …),
    /// then fire the AfterStarsGained hooks (BlackHole damages all enemies on a star gain). Stars persist
    /// across turns within the combat.</summary>
    public static void GainStars(CombatState combat, int amount)
    {
        if (amount <= 0) return;
        combat.Player.Stars += amount;
        if (combat.CurrentSide == CombatSide.Player) combat.StarsGainedThisTurn += amount;   // Radiate
        foreach (var p in combat.AllPowers.ToList()) p.AfterStarsGained(combat, amount);
    }

    /// <summary>Draw <paramref name="n"/> cards for the player mid-turn (Shrug It Off, Pommel Strike, …).
    /// Resolves eagerly when an ambient <see cref="CombatState.Rng"/> is set (rollouts / unit tests). In
    /// SEARCH mode (Rng null) the request is DEFERRED onto <see cref="CombatState.PendingDraw"/>: the solver
    /// resolves it as an explicit draw chance node immediately after the play, so the drawn hand becomes real
    /// in search. (During trace replay — also Rng null — the validator replays the recorded hand and never
    /// reads PendingDraw, so deferral is inert there, exactly as the old no-op was.) A NoDraw marker (Battle
    /// Trance) suppresses the draw. Returns the number drawn EAGERLY (0 when deferred — a deferred draw's cards
    /// are not yet in hand, so any in-effect logic reading the result stays inert, the safe under-estimate).</summary>
    public static int Draw(CombatState combat, int n)
    {
        if (n <= 0) return 0;
        if (combat.Player.HasPower("NoDraw")) return 0;
        if (combat.Rng == null) { combat.PendingDraw += n; return 0; }   // search: defer to a draw chance node
        int before = combat.Player.Hand.Count;
        CombatManager.DrawCards(combat, n, combat.Rng);   // increments CardsDrawnThisCombat (Murder) itself
        int drawn = combat.Player.Hand.Count - before;
        // Per-card-drawn hooks (CorrosiveWave applies Poison, Speedster deals damage). Mid-turn draws only.
        for (int i = before; i < combat.Player.Hand.Count; i++)
        {
            var card = combat.Player.Hand[i];
            foreach (var pw in combat.AllPowers.ToList()) pw.AfterCardDrawn(combat, card, false);
        }
        return drawn;
    }

    /// <summary>Register a deferred draw-THEN-act for SEARCH mode (callers guard <c>combat.Rng == null</c>):
    /// the draw resolves as a chance node and the card's post-draw step (<see cref="CardModel.OnPostDraw"/> or a
    /// discard-of-choice) runs afterward (see <see cref="CombatManager.ApplyPostDraw"/>). Honours NoDraw.</summary>
    public static void DeferDrawThenResolve(CombatState combat, int n, CardModel card)
    {
        if (n <= 0 || combat.Player.HasPower("NoDraw")) return;   // no draw ⇒ no post-draw step
        combat.PendingDraw += n;
        combat.PendingDrawCard = card;
        combat.PendingDrawHandBefore = combat.Player.Hand.Count;   // so ApplyPostDraw knows how many actually drew
    }

    /// <summary>Register a forced discard-of-CHOICE for SEARCH mode (callers guard <c>combat.Rng == null</c>),
    /// with NO preceding draw (e.g. Survivor's "discard 1 card"). It resolves as a player MAX over the hand one
    /// card at a time (<see cref="CombatState.PendingDiscard"/> + the solver's discard decision layer), exactly
    /// like the post-draw discard but without the draw chance node. With a concrete Rng the caller discards a
    /// heuristic default instead. Modelling the forced discard (vs omitting it) is the SOUND direction: omitting
    /// it leaves the player an extra in-turn card the real game removes (optimistic in non-discard-synergy decks).</summary>
    public static void DeferDiscardOfChoice(CombatState combat, int n)
    {
        if (n <= 0) return;
        combat.PendingDiscard += n;
    }

    /// <summary>Register a forced discard-of-CHOICE for SEARCH mode (callers guard <c>combat.Rng == null</c>)
    /// that is FOLLOWED BY a continuation (HiddenDaggers: discard 2 of choice, then add 2 Shivs). The discards
    /// resolve as a player MAX (one card at a time); once the count drains (or the hand empties) the solver runs
    /// <paramref name="card"/>'s <see cref="CardModel.OnPostDiscard"/> (<see cref="CombatManager.ApplyPostDiscard"/>),
    /// matching the game's discard-THEN-act order — so the continuation's products are never in the discard pool.
    /// If <paramref name="n"/> is ≤0 (or no hand) the continuation runs immediately. With a concrete Rng the caller
    /// discards a heuristic default and runs the continuation eagerly instead.</summary>
    public static void DeferDiscardThenResolve(CombatState combat, int n, CardModel card)
    {
        if (n <= 0) { card.OnPostDiscard(combat); return; }
        combat.PendingDiscard += n;
        combat.PendingDiscardCard = card;
    }

    /// <summary>Discard a specific hand card to the discard pile (a mid-turn discard), bumping the discard
    /// counter (Memento Mori). No on-discard hook is modelled (Sly auto-play is unported — omitting it
    /// under-states the player, the safe direction).</summary>
    public static void DiscardFromHand(CombatState combat, CardModel card)
    {
        if (!combat.Player.Hand.Remove(card)) return;
        combat.Player.DiscardPile.Add(card);
        combat.CardsDiscardedThisTurn++;
        if (card.IsSly) CombatManager.TriggerSlyOnDiscard(combat, new[] { card });   // Sly: auto-play on discard
    }

    /// <summary>Summon a monster into combat (e.g. InfestedPower spawning Wrigglers on death). The new
    /// monster is appended to the enemy side, assigned the next encounter id, and telegraphs the move its
    /// AI starts on (a spawned Wriggler starts on its no-op SPAWNED stun). Its rolled HP is whatever the
    /// caller set; the trace validator overrides it with the observed roll via <see cref="Monster.NeedsSpawnHpSync"/>.</summary>
    public static void Summon(CombatState combat, Monster monster)
    {
        monster.Side = CombatSide.Enemy;
        monster.Id = combat.Monsters.Count == 0 ? 1 : combat.Monsters.Max(m => m.Id) + 1;
        monster.Ai.CurrentMoveId = monster.Ai.InitialStateId; // spawned monsters start on their initial (move) state
        monster.NeedsSpawnHpSync = true;
        combat.Monsters.Add(monster);
    }

    /// <summary>Apply (stack) a power on a target, then fire its AfterApplied hook. A debuff offered to a
    /// target carrying Artifact may be negated (consuming an Artifact charge) before it lands.</summary>
    public static void ApplyPower(CombatState combat, Creature target, PowerModel power, int amount, Creature? applier = null)
    {
        if (amount != 0 && power.Type == PowerType.Debuff)
            foreach (var existing in target.Powers.ToList())
                if (existing.TryAbsorbDebuff(combat, power))
                    return;   // negated (e.g. by Artifact)

        // Track player-applied Doom for the turn (Necrobinder Death's Door). String-keyed so the engine
        // need not know the Content power type; inert for every non-Doom power.
        if (amount > 0 && power.Id == "Doom" && applier == combat.Player) combat.DoomAppliedThisTurn = true;

        target.AddPower(power, amount);
        // AfterApplied fires on the live power instance (the one now attached).
        target.GetPower(power.Id)?.AfterApplied(combat, applier);
        // Broadcast to every power (Vicious draws when the owner applies Vulnerable).
        foreach (var p in combat.AllPowers.ToList()) p.AfterPowerApplied(combat, target, power, amount, applier);
    }
}
