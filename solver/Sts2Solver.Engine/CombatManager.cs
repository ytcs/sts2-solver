namespace Sts2Solver.Engine;

/// <summary>
/// Drives the combat turn lifecycle (Appendix C). Stochastic steps (shuffle, draw, monster-move roll)
/// are split out so the solver can expand them as chance nodes while a concrete driver resolves them
/// with an <see cref="Rng"/>.
/// </summary>
public static class CombatManager
{
    // ---- Deterministic lifecycle ----

    /// <summary>Begin a player turn: reset energy, clear block (except turn 1), fire start-of-turn powers.
    /// Does NOT draw — drawing is a separate stochastic step.</summary>
    public static void BeginPlayerTurn(CombatState combat)
    {
        combat.CurrentSide = CombatSide.Player;
        bool firstTurn = combat.TurnNumber == 0;
        combat.TurnNumber++;

        combat.Player.ResetEnergy();
        combat.CardExhaustedThisTurn = false;            // per-turn flag (Evil Eye / Forgotten Ritual)
        combat.PlayerLostHpThisTurn = false;             // per-turn flag (Spite)
        combat.SkillsPlayedThisTurn = 0;                 // per-turn counter (Regent Lunar Blast)
        combat.StarsGainedThisTurn = 0;                  // per-turn counter (Regent Radiate)
        // Block is NOT cleared on turn 1, nor while a Barricade-style power keeps it (PreventsBlockClear).
        if (!firstTurn && !PreventsBlockClear(combat.Player)) combat.Player.ClearBlock();

        FireAfterSideTurnStart(combat, CombatSide.Player);
    }

    /// <summary>Play a card from hand at an optional target. Validates cost/target, spends energy,
    /// runs the effect, and moves the card to its result pile.</summary>
    public static void PlayCard(CombatState combat, CardModel card, Creature? target)
    {
        var player = combat.Player;
        if (!player.Hand.Contains(card)) throw new InvalidOperationException("Card not in hand.");
        if (card.Unplayable) throw new InvalidOperationException($"Card '{card.Name}' is unplayable.");
        if (card.NeedsTarget && (target == null || !target.IsAlive))
            throw new InvalidOperationException("Card requires a living target.");

        // Cost modifiers (Free Attack zeroes the next Attack; Corruption zeroes Skills). A power that
        // lowers the cost is a "contributor" and gets AfterModifyingCardCost (Free Attack consumes there).
        int effCost = card.Cost;
        List<PowerModel>? costContributors = null;
        foreach (var pw in combat.AllPowers.ToList())
        {
            int nc = pw.ModifyCardCost(card, effCost);
            if (nc < effCost) { (costContributors ??= new()).Add(pw); effCost = nc; }
        }
        effCost = Math.Max(0, effCost);
        if (!card.IsXCost && effCost > player.Energy) throw new InvalidOperationException("Not enough energy.");

        int spend = card.IsXCost ? player.Energy : effCost;   // X-cost cards consume all remaining energy
        player.LoseEnergy(spend);
        if (spend > 0)
            foreach (var pw in combat.AllPowers.ToList()) pw.AfterEnergySpent(combat, spend);

        // Star cost (Regent). VoidForm can zero it via ModifyStarCost; X-star cards (Stardust) spend all.
        int starCost = card.IsXStarCost ? player.Stars : card.StarCost;
        foreach (var pw in combat.AllPowers.ToList()) starCost = pw.ModifyStarCost(card, starCost);
        starCost = Math.Max(0, starCost);
        if (starCost > player.Stars) throw new InvalidOperationException("Not enough stars.");
        player.SpendStars(starCost);

        player.Hand.Remove(card);

        // Card-play-count modifiers (One-Two Punch resolves an Attack an extra time). The card is "played"
        // once (one AfterCardPlayed below), but its effect runs 1 + bonus times.
        int bonusPlays = 0;
        List<PowerModel>? playCountContributors = null;
        foreach (var pw in combat.AllPowers.ToList())
        {
            int b = pw.ModifyCardPlayCount(card);
            if (b > 0) { bonusPlays += b; (playCountContributors ??= new()).Add(pw); }
        }
        for (int i = 0; i <= bonusPlays; i++)
        {
            card.OnPlay(combat, new CardPlay { Card = card, Target = target, XValue = spend, StarsSpent = starCost });
            if (combat.IsCombatOver) break;   // don't keep swinging at a cleared board / after death
        }
        if (playCountContributors != null)
            foreach (var pw in playCountContributors) pw.AfterModifyingCardPlayCount(combat, card);
        if (costContributors != null)
            foreach (var pw in costContributors) pw.AfterModifyingCardCost(combat, card);

        // Mirror Hook.AfterCardPlayed: fires after the card's effect resolves (so the card never
        // boosts its own damage), before the card moves to its result pile.
        foreach (var p in combat.AllPowers.ToList()) p.AfterCardPlayed(combat, card);

        // Stars-spent hooks fire after the play resolves (ChildOfTheStars gains block, BlackHole damages).
        if (starCost > 0)
            foreach (var p in combat.AllPowers.ToList()) p.AfterStarsSpent(combat, starCost);

        if (card.Type == CardType.Skill) combat.SkillsPlayedThisTurn++;   // per-turn count (Lunar Blast)

        // Result pile, with Corruption-style overrides (a Skill is exhausted instead of discarded).
        var resultPile = card.ResultPile;
        if (resultPile == CardResultPile.Discard)
            foreach (var pw in combat.AllPowers.ToList())
                if (pw.OverrideResultPileToExhaust(card)) { resultPile = CardResultPile.Exhaust; break; }
        switch (resultPile)
        {
            case CardResultPile.Discard: player.DiscardPile.Add(card); break;
            case CardResultPile.Exhaust:
                player.ExhaustPile.Add(card);
                combat.CardExhaustedThisTurn = true;
                foreach (var p in combat.AllPowers.ToList()) p.AfterCardExhausted(combat, card, false);
                break;
            case CardResultPile.Removed: break; // vanishes (e.g. Power cards)
        }
    }

    /// <summary>End the player turn: discard the hand and hand control to the enemy.</summary>
    public static void EndPlayerTurn(CombatState combat)
    {
        var player = combat.Player;

        // Cards with an end-of-turn-in-hand effect (e.g. Infection's 3 self-damage) trigger before the
        // hand is discarded. Snapshot the hand: the effect doesn't add/remove hand cards in scope.
        foreach (var card in player.Hand.ToList())
        {
            if (card.HasTurnEndInHandEffect) card.OnTurnEndInHand(combat);
            if (combat.PlayerDead) break;
        }

        // Standard STS: the hand is discarded at end of turn — except Ethereal cards, which exhaust (and
        // fire the on-exhaust hook with causedByEthereal=true, e.g. DarkEmbrace's deferred draw), and
        // Retain cards (Sovereign Blade), which stay in hand into the next turn.
        var retained = new List<CardModel>();
        foreach (var card in player.Hand)
        {
            if (card.Ethereal)
            {
                player.ExhaustPile.Add(card);
                combat.CardExhaustedThisTurn = true;
                foreach (var p in combat.AllPowers.ToList()) p.AfterCardExhausted(combat, card, true);
            }
            else if (card.Retain) retained.Add(card);
            else player.DiscardPile.Add(card);
        }
        player.Hand.Clear();
        player.Hand.AddRange(retained);

        FireAfterSideTurnEnd(combat, CombatSide.Player);
        combat.CurrentSide = CombatSide.Enemy;
    }

    /// <summary>Run the enemy turn: each living monster clears block then performs its telegraphed move,
    /// then end-of-enemy-turn powers tick. Leaves the next-move roll to the caller (stochastic).</summary>
    public static void RunEnemyTurn(CombatState combat)
    {
        FireAfterSideTurnStart(combat, CombatSide.Enemy);

        foreach (var m in combat.Monsters)
        {
            if (!m.IsAlive) continue;
            if (!PreventsBlockClear(m)) m.ClearBlock();
        }
        foreach (var m in combat.Monsters)
        {
            if (!m.IsAlive) continue;
            m.PerformCurrentMove(combat);
            if (combat.PlayerDead) break;
        }

        FireAfterSideTurnEnd(combat, CombatSide.Enemy);
        combat.CurrentSide = CombatSide.Player;
    }

    /// <summary>True if any power on the creature keeps its block from being cleared at turn start (Barricade).</summary>
    private static bool PreventsBlockClear(Creature c)
    {
        foreach (var p in c.Powers) if (p.PreventsBlockClear) return true;
        return false;
    }

    private static void FireAfterSideTurnStart(CombatState combat, CombatSide side)
    {
        foreach (var p in combat.AllPowers.ToList()) p.AfterSideTurnStart(combat, side);
    }

    private static void FireAfterSideTurnEnd(CombatState combat, CombatSide side)
    {
        foreach (var p in combat.AllPowers.ToList()) p.AfterSideTurnEnd(combat, side);
    }

    // ---- Stochastic steps (concrete / RNG-driven; the solver enumerates these instead) ----

    public static void ShuffleDrawPile(CombatState combat, Rng rng) => rng.Shuffle(combat.Player.DrawPile);

    /// <summary>Draw n cards, reshuffling the discard pile into the draw pile when it empties.
    /// Stops at max hand size.</summary>
    public static void DrawCards(CombatState combat, int n, Rng rng)
    {
        var p = combat.Player;
        for (int i = 0; i < n; i++)
        {
            if (p.Hand.Count >= Player.MaxHandSize) break;
            if (p.DrawPile.Count == 0)
            {
                if (p.DiscardPile.Count == 0) break;
                p.DrawPile.AddRange(p.DiscardPile);
                p.DiscardPile.Clear();
                rng.Shuffle(p.DrawPile);
            }
            var card = p.DrawPile[0];
            p.DrawPile.RemoveAt(0);
            p.Hand.Add(card);
        }
    }

    public static void RollInitialMoves(CombatState combat, Rng rng)
    {
        foreach (var m in combat.Monsters)
            m.Ai.CurrentMoveId = PickMove(m.Ai.EnumerateInitial(m), rng);
    }

    public static void RollNextMoves(CombatState combat, Rng rng)
    {
        foreach (var m in combat.Monsters)
        {
            if (!m.IsAlive) continue;
            m.Ai.CurrentMoveId = PickMove(m.Ai.EnumerateNext(m), rng);
        }
    }

    private static string PickMove(List<(double prob, string moveId)> dist, Rng rng)
        => rng.PickWeighted(dist.Select(d => (d.prob, d.moveId)).ToList());

    /// <summary>Award post-combat relic effects (e.g. Burning Blood heals 6) on victory.</summary>
    public static void OnVictory(CombatState combat)
    {
        foreach (var r in combat.Player.Relics) r.AfterCombatVictory(combat);
    }
}
