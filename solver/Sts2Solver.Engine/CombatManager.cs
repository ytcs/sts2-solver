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
        combat.PlaysThisTurn = 0;                         // per-turn play counter (cap loop-risk cantrip decks)
        combat.CardExhaustedThisTurn = false;            // per-turn flag (Evil Eye / Forgotten Ritual)
        combat.PlayerLostHpThisTurn = false;             // per-turn flag (Spite)
        combat.SkillsPlayedThisTurn = 0;                 // per-turn counter (Regent Lunar Blast)
        combat.StarsGainedThisTurn = 0;                  // per-turn counter (Regent Radiate)
        combat.AttacksPlayedThisTurn = 0;                // per-turn counter (Finisher / Necrobinder Lethality)
        combat.CardsDiscardedThisTurn = 0;               // per-turn counter (Memento Mori)
        combat.OstyAttacksThisTurn = 0;                  // per-turn counter (Necrobinder Flatten/Rattle)
        combat.EnergySpentThisTurn = 0;                  // per-turn counter (Defect HelixDrill)
        combat.CardsDrawnMidTurn = 0;                    // per-turn counter (Necrobinder DeathMarch)
        combat.DoomAppliedThisTurn = false;              // per-turn flag (Necrobinder Death's Door)
        if (combat.TracksPoweredHits)                    // per-target counter (Regent BeatIntoShape)
        {
            foreach (var m in combat.Monsters) m.PlayerPoweredHitsThisTurn = 0;
            combat.Player.PlayerPoweredHitsThisTurn = 0;
        }
        // Block is NOT cleared on turn 1, nor while a Barricade-style power keeps it (PreventsBlockClear).
        if (!firstTurn && !PreventsBlockClear(combat.Player)) combat.Player.ClearBlock();

        // Relic turn-start effects (Bound Phylactery re-summons Osty after turn 1). Fired after energy
        // reset, before start-of-turn powers, mirroring the game's AfterEnergyResetLate ordering.
        foreach (var r in combat.Player.Relics) r.OnPlayerTurnStart(combat);

        FireAfterSideTurnStart(combat, CombatSide.Player);

        // Plasma orbs add energy at the player's turn start (after the energy reset above). Every other orb's
        // passive fires at turn END (see EndPlayerTurn). Gated on the player having orbs → inert for non-Defect.
        OrbOps.TriggerPassives(combat, turnStart: true);
    }

    /// <summary>Play a card from hand at an optional target. Validates cost/target, spends energy,
    /// runs the effect, and moves the card to its result pile. <paramref name="choiceKey"/> carries an in-play
    /// choice (an option's <see cref="CardModel.StateKey"/>) for cards that require one; null = no/ default choice.</summary>
    /// <summary>The card's fully-resolved energy cost: <see cref="CardModel.EffectiveCost"/> plus every active
    /// power's <see cref="PowerModel.ModifyCardCost"/> (Free Attack / Corruption reducers, Borrowed Time
    /// increases), clamped to ≥0 — the value <see cref="PlayCard"/> spends and the game's
    /// <c>EnergyCost.GetResolved()</c>. Side-effect free (the reducer-consume side effects fire later via
    /// AfterModifyingCardCost), so powers that gate on resolved cost (Danse Macabre) can call it from a hook.</summary>
    public static int ResolveCardCost(CombatState combat, CardModel card)
    {
        int c = card.EffectiveCost(combat);
        foreach (var pw in combat.AllPowers.ToList()) c = pw.ModifyCardCost(card, c);
        return System.Math.Max(0, c);
    }

    public static void PlayCard(CombatState combat, CardModel card, Creature? target, string? choiceKey = null)
    {
        var player = combat.Player;
        if (!player.Hand.Contains(card)) throw new InvalidOperationException("Card not in hand.");
        if (card.Unplayable) throw new InvalidOperationException($"Card '{card.Name}' is unplayable.");
        if (card.NeedsTarget && (target == null || !target.IsAlive))
            throw new InvalidOperationException("Card requires a living target.");

        // Cost modifiers (Free Attack zeroes the next Attack; Corruption zeroes Skills). A power that
        // lowers the cost is a "contributor" and gets AfterModifyingCardCost (Free Attack consumes there).
        int effCost = card.EffectiveCost(combat);
        List<PowerModel>? costContributors = null;
        foreach (var pw in combat.AllPowers.ToList())
        {
            int nc = pw.ModifyCardCost(card, effCost);
            if (nc < effCost) (costContributors ??= new()).Add(pw);   // only reducers get AfterModifyingCardCost
            effCost = nc;                                             // but increases (Borrowed Time) still apply
        }
        effCost = Math.Max(0, effCost);
        if (!card.IsXCost && effCost > player.Energy) throw new InvalidOperationException("Not enough energy.");

        int spend = card.IsXCost ? player.Energy : effCost;   // X-cost cards consume all remaining energy
        player.LoseEnergy(spend);
        if (spend > 0)
        {
            if (combat.TracksEnergySpent) combat.EnergySpentThisTurn += spend;   // Defect HelixDrill scaling
            foreach (var pw in combat.AllPowers.ToList()) pw.AfterEnergySpent(combat, spend);
        }

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
        // Before-play hook (Danse Macabre / Spirit of Ash block, Veilpiercer charge). Runs before OnPlay so
        // a Power card never triggers the power it is in the middle of applying.
        foreach (var pw in combat.AllPowers.ToList()) pw.BeforeCardPlayed(combat, card);

        for (int i = 0; i <= bonusPlays; i++)
        {
            card.OnPlay(combat, new CardPlay { Card = card, Target = target, XValue = spend, StarsSpent = starCost, ChoiceKey = choiceKey });
            if (combat.IsCombatOver) break;   // don't keep swinging at a cleared board / after death
        }
        // Per-turn/combat play counters, incremented after the effect resolves so a card never counts itself.
        // AttacksPlayedThisTurn is read by Finisher (Silent) and Lethality (Necrobinder); a card played
        // multiple times (bonus plays) still counts as one finished play. EtherealPlayedThisCombat feeds the
        // Necrobinder's Pull from Below / Banshee's Cry.
        if (card.Type == CardType.Attack) combat.AttacksPlayedThisTurn++;
        combat.PlaysThisTurn++;   // per-turn play count (bounds cost-0 cantrip loops on BoundsPlays decks)
        if (card.Ethereal) combat.EtherealPlayedThisCombat++;
        if (combat.TracksCardsPlayed) combat.CardsPlayedThisCombat++;   // GoldAxe scaling (every finished play)
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

        // Orb turn-END passives (Lightning damage / Frost block / Dark accumulate / Glass damage) fire before
        // the hand is discarded and the side switches. Plasma fires at turn START, not here. Inert for non-Defect.
        // (These touch enemies / player block / energy only — they never kill the player.)
        OrbOps.TriggerPassives(combat, turnStart: false);

        // Cards with an end-of-turn-in-hand effect (e.g. Infection's 3 self-damage) trigger before the
        // hand is discarded. Snapshot the hand: the effect doesn't add/remove hand cards in scope.
        foreach (var card in player.Hand.ToList())
        {
            if (card.HasTurnEndInHandEffect) card.OnTurnEndInHand(combat);
            if (combat.PlayerDead) break;
        }

        // Standard STS: the hand is discarded at end of turn — except Ethereal cards, which exhaust (and
        // fire the on-exhaust hook with causedByEthereal=true, e.g. DarkEmbrace's deferred draw), and
        // Retain cards (e.g. Sovereign Blade), which stay in hand into the next turn.
        // Hex (SpectralKnight): while held it makes EVERY player card Ethereal (decompile HexPower/Hexed adds the
        // Ethereal keyword to all cards), so under Hex the whole hand exhausts. Modelling it is both faithful and
        // the SOUND direction — it thins the deck, never inflates it, so forward search can't over-credit by
        // retaining a card the real game would have exhausted. Gated on the (rare) power → inert for every other fight.
        bool hexed = player.HasPower("Hex");
        var retained = new List<CardModel>();
        foreach (var card in player.Hand)
        {
            if (card.Ethereal || hexed)
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
            if (m.StunnedTurns > 0) { m.StunnedTurns--; continue; }   // Whistle stun: skip this move (delayed to next turn)
            m.PerformCurrentMove(combat);
            if (combat.PlayerDead) break;
        }

        FireAfterSideTurnEnd(combat, CombatSide.Enemy);

        // Decimillipede reattach: a downed segment skips this enemy turn (DEAD_MOVE; it's at 0 HP so the loop
        // above already passed it over), counting down; on the second enemy turn it reattaches (REATTACH_MOVE →
        // heal to its Reattach amount) — but only if ANOTHER segment is still alive (else it stays downed and the
        // all-segments-at-0 board is a clear). Deterministic ⇒ modelled identically in exact/MCTS/rollout. Inert
        // (ReattachIn==0) for every non-Decimillipede fight.
        if (!combat.PlayerDead)
            foreach (var m in combat.Monsters)
            {
                if (m.ReattachIn <= 0) continue;
                m.ReattachIn--;
                if (m.ReattachIn == 0 && combat.Monsters.Any(o => o != m && o.HasPower("Reattach") && o.IsAlive))
                    m.Heal(m.GetPowerAmount("Reattach"));
            }

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

    /// <summary>Draw n cards, reshuffling the discard pile into the draw pile when it empties. Stops at max hand
    /// size. <paramref name="fromHandDraw"/> marks the turn-start hand draw (vs a mid-turn effect draw) — it feeds
    /// the gated draw counters and the per-card on-draw hooks via <see cref="OnCardsDrawn"/> exactly as the search
    /// chance-node draw paths do, so every path stays consistent.</summary>
    public static void DrawCards(CombatState combat, int n, Rng rng, bool fromHandDraw = false)
    {
        var p = combat.Player;
        int before = p.Hand.Count;
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
        OnCardsDrawn(combat, before, fromHandDraw);
    }

    /// <summary>Apply the per-draw bookkeeping that EVERY draw path shares — concrete <see cref="DrawCards"/> and
    /// the search/MCTS chance-node enumerators — to the cards now occupying hand slots <c>[handBefore, Count)</c>.
    /// <list type="bullet">
    /// <item>Murder's <see cref="CombatState.CardsDrawnThisCombat"/> counts EVERY draw (incl. the turn-start hand
    /// draw — the game logs a CardDrawnEntry for those too and Murder doesn't filter on FromHandDraw).</item>
    /// <item>DeathMarch's <see cref="CombatState.CardsDrawnMidTurn"/> counts only MID-TURN (non-hand) draws.</item>
    /// <item>Per-card <see cref="CardModel.OnDraw"/> on-draw effects (Void: −1 energy) fire on ANY draw.</item>
    /// </list>
    /// Each is gated (a flag or <see cref="CardModel.HasOnDraw"/>) so the common draw pays nothing. Note the
    /// power-level <see cref="PowerModel.AfterCardDrawn"/> hook is NOT fired here — it stays in <see cref="Cmd.Draw"/>
    /// (mid-turn only), preserving its existing semantics.</summary>
    public static void OnCardsDrawn(CombatState combat, int handBefore, bool fromHandDraw)
    {
        var p = combat.Player;
        int drawnNow = p.Hand.Count - handBefore;
        if (drawnNow <= 0) return;
        if (combat.TracksCardsDrawn) combat.CardsDrawnThisCombat += drawnNow;
        if (!fromHandDraw && combat.TracksMidTurnDraws) combat.CardsDrawnMidTurn += drawnNow;
        for (int i = handBefore; i < p.Hand.Count; i++)
            if (p.Hand[i].HasOnDraw) p.Hand[i].OnDraw(combat);
    }

    /// <summary>The player's turn-start hand-draw count: <paramref name="baseCount"/> (default 5) plus any
    /// power-granted bonus (MachineLearning's <see cref="PowerModel.ModifyHandDraw"/>, chained over the player's
    /// powers). Used at every turn-start draw site so the bonus is modelled identically in exact search, MCTS and
    /// rollout. Never negative.</summary>
    public static int TurnStartDrawCount(CombatState combat, int baseCount = Player.CardsDrawnPerTurn)
    {
        int count = baseCount;
        foreach (var pw in combat.Player.Powers) count = pw.ModifyHandDraw(combat.Player, count);
        return Math.Max(0, count);
    }

    /// <summary>Apply the Innate keyword to the opening (turn-1) hand draw and return the number of cards the
    /// random draw should still produce. The game, on turn 1 only, moves Innate cards to the top of the draw pile
    /// and draws <c>max(5, innateCount)</c> (capped at the hand size) — guaranteeing every Innate card in the
    /// opening hand. Here we pull the Innate cards straight into the hand (they count as drawn — a CardDrawnEntry,
    /// so they feed the Murder counter) and return the residual random-draw count for the caller's draw routine
    /// (exact <c>DrawEnumerator</c> or concrete <c>DrawCards</c>) to deal from the remaining pile.
    ///
    /// A no-op on every turn but the first (<c>TurnNumber != 1</c>) and for innate-free piles, so it can be wired
    /// at every turn-start draw site uniformly — the turn-1 guard confines it to the opening. Must be called AFTER
    /// <see cref="BeginPlayerTurn"/> (which sets TurnNumber to 1) and BEFORE the random draw.</summary>
    public static int OpeningDrawAfterInnate(CombatState combat, int baseCount)
    {
        if (combat.TurnNumber != 1) return baseCount;   // Innate seeds ONLY the opening hand
        var p = combat.Player;
        var innate = p.DrawPile.Where(c => c.Innate).Take(Player.MaxHandSize).ToList();
        if (innate.Count == 0) return baseCount;
        int handBefore = p.Hand.Count;
        foreach (var c in innate) { p.DrawPile.Remove(c); p.Hand.Add(c); }
        OnCardsDrawn(combat, handBefore, fromHandDraw: true);   // innate cards are a turn-start hand draw (counters + on-draw)
        int handDraw = Math.Min(Math.Max(baseCount, innate.Count), Player.MaxHandSize);
        return Math.Max(0, handDraw - innate.Count);
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

    /// <summary>Run a deferred draw's POST-draw step on the just-drawn hand. Called by the solver on each draw
    /// outcome (after <see cref="Cmd.DeferDrawThenResolve"/> registered it). A discard-of-choice card sets
    /// <see cref="CombatState.PendingDiscard"/> (resolved later as a player MAX); a conditional card
    /// (EscapePlan) applies <see cref="CardModel.OnPostDraw"/> immediately. Clears the pending-card marker.</summary>
    public static void ApplyPostDraw(CombatState combat)
    {
        var card = combat.PendingDrawCard;
        combat.PendingDrawCard = null;
        if (card == null) return;
        if (card.PostDrawDiscardCount > 0) combat.PendingDiscard += card.PostDrawDiscardCount;
        else card.OnPostDraw(combat, combat.Player.Hand.Count - combat.PendingDrawHandBefore);   // actual cards drawn
    }

    /// <summary>Run a discard-of-choice card's CONTINUATION once its discards have fully resolved (HiddenDaggers
    /// adds its Shivs here). Called by the solver/MCTS the moment <see cref="CombatState.PendingDiscard"/> drains
    /// to 0 (or the hand empties early), and by the concrete-Rng default-discard path. No-op when no continuation
    /// card is registered (the common Acrobatics/Prepared discard). Clears the marker.</summary>
    public static void ApplyPostDiscard(CombatState combat)
    {
        var card = combat.PendingDiscardCard;
        combat.PendingDiscardCard = null;
        card?.OnPostDiscard(combat);
    }

    /// <summary>Auto-play every Sly card among <paramref name="discarded"/>, in discard order — the game does
    /// this right after a mid-turn discard (CardCmd.Discard collects IsSlyThisTurn cards, discards them, then
    /// AutoPlays each for free). Wired into the explicit mid-turn discard primitives (<see cref="Cmd.DiscardFromHand"/>
    /// and the Silent discard helpers); the end-of-turn hand flush deliberately does NOT call it (the game flushes
    /// via CardPileCmd.Add, which bypasses the Sly trigger). The Silent Sly cards are all Self / AllEnemies /
    /// RandomEnemy, so a null-target <see cref="CardModel.OnPlay"/> is faithful — RandomEnemy / draw effects
    /// degrade exactly as a normal play does (random target with a concrete Rng, first-enemy default + deferred
    /// draw in pure search). The card is already in the discard pile; a Power auto-played from there is removed and
    /// an Exhaust card exhausts (result-pile rules). Residual (documented, sound): the auto-play does not re-fire
    /// the AfterCardPlayed power hooks or bump the per-turn play counters, so Afterimage-style "on card played"
    /// reactions and Finisher's attack count under-credit a Sly auto-play — the pessimistic direction.</summary>
    public static void TriggerSlyOnDiscard(CombatState combat, IReadOnlyList<CardModel> discarded)
    {
        for (int i = 0; i < discarded.Count; i++)
        {
            var card = discarded[i];
            if (!card.IsSly || combat.IsCombatOver) continue;
            card.OnPlay(combat, new CardPlay { Card = card });   // free auto-play, no target (Self/AllEnemies/RandomEnemy)
            if (card.ResultPile == CardResultPile.Removed) combat.Player.DiscardPile.Remove(card);
            else if (card.ResultPile == CardResultPile.Exhaust && combat.Player.DiscardPile.Remove(card))
            { combat.Player.ExhaustPile.Add(card); combat.CardExhaustedThisTurn = true; }
        }
    }

    /// <summary>Award post-combat relic effects (e.g. Burning Blood heals 6) on victory.</summary>
    public static void OnVictory(CombatState combat)
    {
        foreach (var r in combat.Player.Relics) r.AfterCombatVictory(combat);
    }
}
