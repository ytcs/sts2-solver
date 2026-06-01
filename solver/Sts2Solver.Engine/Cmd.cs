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
        int blocked = 0;
        if (!props.HasFlag(ValueProp.Unblockable))
        {
            blocked = Math.Min(target.Block, modified);
            target.Block -= blocked;
        }
        int unblocked = modified - blocked;

        // HP-loss modifiers (HardenedShell caps the target's total loss per turn).
        foreach (var c in combat.AllCreatures)
            foreach (var p in c.Powers)
                unblocked = p.ModifyHpLost(target, unblocked, props, dealer);
        unblocked = Math.Max(0, unblocked);

        if (unblocked > 0)
        {
            int before = target.CurrentHp;
            target.LoseHpInternal(unblocked);
            int actualLost = before - target.CurrentHp;
            if (target.IsPlayer)
            {
                combat.PlayerHpLost += actualLost;
                combat.PlayerUnblockedHitsCount++;   // Tear Asunder hits 1 + this
                // "Lost HP this turn" (Spite) = unblocked self-damage on the player's own turn.
                if (combat.CurrentSide == CombatSide.Player) combat.PlayerLostHpThisTurn = true;
            }
            if (before > 0 && target.CurrentHp == 0)
                foreach (var p in combat.AllPowers.ToList())
                    p.AfterCreatureDeath(combat, target);
        }
        // Damage-received hook fires even on a fully-blocked attack (PersonalHive reacts to the hit;
        // Shriek guards internally on unblocked>0).
        foreach (var p in combat.AllPowers.ToList())
            p.AfterDamageReceived(combat, target, unblocked, dealer, props);
        return unblocked;
    }

    /// <summary>Direct, unblockable, unpowered HP loss (e.g. Poison ticks).</summary>
    public static int LoseHp(CombatState combat, Creature target, int amount)
        => ApplyDamage(combat, target, amount, ValueProp.Unblockable | ValueProp.Unpowered);

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

    /// <summary>Draw <paramref name="n"/> cards for the player mid-turn (Shrug It Off, Pommel Strike, …).
    /// Real only when an ambient <see cref="CombatState.Rng"/> is set; otherwise a no-op (the trace
    /// validator replays the recorded hand + constructs mid-turn-drawn cards as they are played). A NoDraw
    /// marker (Battle Trance) suppresses the draw. Returns the number actually drawn.</summary>
    public static int Draw(CombatState combat, int n)
    {
        if (n <= 0 || combat.Rng == null) return 0;
        if (combat.Player.HasPower("NoDraw")) return 0;
        int before = combat.Player.Hand.Count;
        CombatManager.DrawCards(combat, n, combat.Rng);
        int drawn = combat.Player.Hand.Count - before;
        if (combat.TracksCardsDrawn) combat.CardsDrawnThisCombat += drawn;   // Murder scales on this
        // Per-card-drawn hooks (CorrosiveWave applies Poison, Speedster deals damage). Mid-turn draws only.
        for (int i = before; i < combat.Player.Hand.Count; i++)
        {
            var card = combat.Player.Hand[i];
            foreach (var pw in combat.AllPowers.ToList()) pw.AfterCardDrawn(combat, card, false);
        }
        return drawn;
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

        target.AddPower(power, amount);
        // AfterApplied fires on the live power instance (the one now attached).
        target.GetPower(power.Id)?.AfterApplied(combat, applier);
        // Broadcast to every power (Vicious draws when the owner applies Vulnerable).
        foreach (var p in combat.AllPowers.ToList()) p.AfterPowerApplied(combat, target, power, amount, applier);
    }
}
