using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Gates the mid-turn-draw chance node (a draw card's drawn hand becomes REAL and playable in search).
/// Before this, <see cref="Cmd.Draw"/> was inert in search (<c>combat.Rng == null</c> → drew 0), so the
/// drawn card never existed in the tree. Now a deferred draw opens an explicit chance node over the exact
/// draw distribution, continuing the same turn.
/// </summary>
public class MidTurnDrawTests
{
    private readonly ITestOutputHelper _out;
    public MidTurnDrawTests(ITestOutputHelper o) => _out = o;

    /// <summary>Decisive, hand-checkable: the ONLY lethal this turn is a Strike sitting in the draw pile,
    /// reachable solely by playing ShrugItOff's draw. Energy 3 affords ShrugItOff (1) + the drawn Strike (1).
    /// With the mid-turn draw modelled the player kills CorpseSlug (6 HP) on turn 1 — win, zero HP lost. Under
    /// the old inert-draw behaviour ShrugItOff drew nothing, the Strike stayed unreachable, the slug survived
    /// to Whip Slap, and the player took damage — so this exact (1.0, 0) is a direct proof the draw fires.</summary>
    [Fact]
    public void ShrugItOff_Draws_The_Lethal_Strike_And_Wins_The_Turn()
    {
        var player = new Player { MaxHp = 20, CurrentHp = 20, Energy = 3, MaxEnergy = 3 };
        player.Hand.Add(Catalog.BuildCard("ShrugItOff"));    // cost 1: gain 8 block, draw 1
        player.DrawPile.Add(Catalog.BuildCard("StrikeIronclad"));  // cost 1: 6 damage — exactly lethal
        var slug = Monsters.CorpseSlug(hp: 6);               // initial move WHIP_SLAP would hit if it survived
        slug.Ai.CurrentMoveId = slug.Ai.InitialStateId;      // telegraph its opening move (normally rolled at setup)
        var combat = new CombatState { Player = player, Monsters = { slug }, TurnNumber = 1 };

        var v = new Solver { MaxTurns = 10 }.SolvePlayerTurn(combat);
        _out.WriteLine($"value = {v}");
        Assert.Equal(1.0, v.Win, 6);   // the drawn Strike kills this turn
        Assert.Equal(0.0, v.Loss, 6);  // the slug dies before it can act ⇒ no HP lost
    }

    /// <summary>Control: replace the draw card with a plain Defend (no draw). The lethal Strike is now
    /// unreachable this turn, so the slug survives to Whip Slap and the player necessarily loses HP — the same
    /// position that the inert-draw bug produced for the ShrugItOff case. Confirms the win above is caused by
    /// the draw, not the board.</summary>
    [Fact]
    public void Without_A_Draw_Card_The_Lethal_Strike_Stays_Unreachable()
    {
        var player = new Player { MaxHp = 20, CurrentHp = 20, Energy = 3, MaxEnergy = 3 };
        player.Hand.Add(Catalog.BuildCard("DefendIronclad"));      // cost 1: 5 block, NO draw
        player.DrawPile.Add(Catalog.BuildCard("StrikeIronclad"));
        var slug = Monsters.CorpseSlug(hp: 6);
        slug.Ai.CurrentMoveId = slug.Ai.InitialStateId;
        var combat = new CombatState { Player = player, Monsters = { slug }, TurnNumber = 1 };

        var v = new Solver { MaxTurns = 10 }.SolvePlayerTurn(combat);
        _out.WriteLine($"value = {v}");
        Assert.True(v.Loss > 0.0, "no draw card ⇒ cannot kill turn 1 ⇒ the slug acts and HP is lost");
    }

    /// <summary>Post-draw CONDITIONAL (EscapePlan: draw 1, +Block iff it is a Skill). Same board both times —
    /// the only difference is the drawn card — proving the conditional resolves on the post-draw hand in search
    /// (it was inert before). We assert the Block itself rather than E[HP loss]: a 1-turn horizon miss now
    /// charges remaining HP, so a doomed timeout costs full HP either way.</summary>
    [Fact]
    public void EscapePlan_Conditional_Block_Fires_On_A_Skill_Draw_In_Search()
    {
        CombatState AfterDraw(string drawCard)
        {
            var p = new Player { MaxHp = 40, CurrentHp = 40, Energy = 3, MaxEnergy = 3 };
            var plan = Catalog.BuildCard("EscapePlan");     // cost 0: draw 1, +3 Block if a Skill
            p.Hand.Add(plan);
            p.DrawPile.Add(Catalog.BuildCard(drawCard));
            var slug = Monsters.CorpseSlug(hp: 60);
            slug.Ai.CurrentMoveId = slug.Ai.InitialStateId;
            var combat = new CombatState { Player = p, Monsters = { slug }, TurnNumber = 1 };
            CombatManager.PlayCard(combat, plan, null);     // search mode: draw deferred onto PendingDraw
            int n = combat.PendingDraw;
            combat.PendingDraw = 0;
            var drawn = DrawEnumerator.EnumerateDraw(combat, n, fromHandDraw: false).Single().state;
            CombatManager.ApplyPostDraw(drawn);
            return drawn;
        }

        var skill = AfterDraw("DefendIronclad");
        var attack = AfterDraw("StrikeIronclad");
        _out.WriteLine($"skill-draw block={skill.Player.Block}  |  attack-draw block={attack.Player.Block}");
        Assert.Equal(3, skill.Player.Block);    // Skill draw ⇒ EscapePlan's +3 Block
        Assert.Equal(0, attack.Player.Block);   // Attack draw ⇒ no Block
    }

    /// <summary>Post-draw DISCARD-of-choice (Acrobatics: draw 3, discard 1 of choice) is a real player MAX, not
    /// a fixed default. The drawn hand holds the lethal Bludgeon plus two filler Defends; keeping Bludgeon (by
    /// discarding a Defend) wins on the spot, while a wrong discard of Bludgeon would forfeit the kill. The
    /// exact (1.0, 0) proves the search discards optimally — and that the net draw is sound (draw 3 − discard 1
    /// = +2, no over-draw).</summary>
    [Fact]
    public void Acrobatics_Discards_Optimally_Keeping_The_Lethal_Card()
    {
        var p = new Player { MaxHp = 30, CurrentHp = 30, Energy = 3, MaxEnergy = 3 };
        p.Hand.Add(Catalog.BuildCard("Acrobatics"));        // cost 1: draw 3, discard 1 of choice
        p.DrawPile.Add(Catalog.BuildCard("Bludgeon"));      // cost 1: 32 damage — exactly lethal
        p.DrawPile.Add(Catalog.BuildCard("DefendIronclad"));
        p.DrawPile.Add(Catalog.BuildCard("DefendIronclad"));
        var slug = Monsters.CorpseSlug(hp: 32);
        slug.Ai.CurrentMoveId = slug.Ai.InitialStateId;
        var combat = new CombatState { Player = p, Monsters = { slug }, TurnNumber = 1 };

        var v = new Solver { MaxTurns = 10 }.SolvePlayerTurn(combat);
        _out.WriteLine($"value = {v}");
        Assert.Equal(1.0, v.Win, 6);   // kept + played Bludgeon this turn
        Assert.Equal(0.0, v.Loss, 6);  // slug dies before acting
    }

    /// <summary>The MCTS post-draw machinery (deterministic EscapePlan block applied to draw outcomes;
    /// discard-of-choice as a player-MAX decision layer) must converge to the exact oracle. Uses an Acrobatics
    /// (draw 3 / discard 1 of choice) deck — Acrobatics is cost-1 (energy-bounded, exact-tractable) so the
    /// discard-choice MAX is exercised in both engines; MCTS within tight tolerance of exact on survival AND
    /// HP loss.</summary>
    [Fact]
    public void Mcts_Converges_On_Discard_Choice_Deck()
    {
        CombatState Build() => Catalog.SetupCombat(
            Catalog.BuildPlayer(new List<CardModel> {
                Catalog.BuildCard("Acrobatics"), Catalog.BuildCard("StrikeIronclad"),
                Catalog.BuildCard("StrikeIronclad"), Catalog.BuildCard("DefendIronclad"),
                Catalog.BuildCard("DefendIronclad") }, 22, 22, 3, new[] { "BurningBlood" }),
            new[] { Monsters.CorpseSlug(hp: 24) });
        const int mt = 8;
        var exact = new Solver { MaxTurns = mt }.Solve(Build());
        var mcts = new MctsSolver(new MctsOptions { Trials = 30_000, Seed = 1, MaxTurns = mt, ActionWidening = true }).Solve(Build());
        _out.WriteLine($"exact {exact}  |  mcts {mcts}");
        Assert.True(Math.Abs(mcts.Win - exact.Win) <= 0.05, $"survival exact {exact.Win:P2} vs mcts {mcts.Win:P2}");
        Assert.True(Math.Abs(mcts.Loss - exact.Loss) <= 2.5, $"loss exact {exact.Loss:F1} vs mcts {mcts.Loss:F1}");
    }

    /// <summary>Termination guard: a deck with a cost-0 replayable draw cantrip (EscapePlan — free play that
    /// draws and recirculates via reshuffle) could otherwise build an unbounded per-turn play chain and blow the
    /// search stack. The deck is flagged <c>BoundsPlays</c> at setup, capping plays per turn so both engines stay
    /// finite. MCTS (sampling) must return a sane value without overflowing or hanging.</summary>
    [Fact]
    public void Cost0_Draw_Cantrip_Deck_Stays_Bounded()
    {
        CombatState Build() => Catalog.SetupCombat(
            Catalog.BuildPlayer(new List<CardModel> {
                Catalog.BuildCard("EscapePlan"), Catalog.BuildCard("EscapePlan"),
                Catalog.BuildCard("StrikeIronclad"), Catalog.BuildCard("StrikeIronclad"),
                Catalog.BuildCard("DefendIronclad"), Catalog.BuildCard("DefendIronclad") }, 30, 30, 3, new[] { "BurningBlood" }),
            new[] { Monsters.CorpseSlug(hp: 22) });
        Assert.True(Build().BoundsPlays, "a cost-0 replayable draw cantrip should flag the deck loop-risk");
        var mcts = new MctsSolver(new MctsOptions { Trials = 20_000, Seed = 1, MaxTurns = 8 }).Solve(Build());
        _out.WriteLine($"mcts {mcts}");
        Assert.InRange(mcts.Win, 0.0, 1.0);   // terminates with a sane value (the cap prevents the stack blow-up)
    }

    /// <summary>Audit gate: every cost-0 replayable (returns-to-discard) draw card is flagged
    /// <c>LoopRiskDraw</c> so its deck hashes the per-turn play counter and the cap memoises soundly. A Power
    /// that draws (Neurosurge — Removed pile, can't be replayed) and a starter deck are NOT loop-risk.</summary>
    [Theory]
    [InlineData("EscapePlan", true)]
    [InlineData("Prepared", true)]
    [InlineData("FlashOfSteel", true)]
    [InlineData("Finesse", true)]
    [InlineData("Impatience", true)]
    [InlineData("BrightestFlame", true)]
    [InlineData("Neurosurge", false)]   // Power ⇒ Removed pile ⇒ not replayable
    [InlineData("StrikeIronclad", false)]
    public void Loop_Risk_Draw_Cards_Are_Flagged(string cardName, bool expectFlagged)
    {
        Assert.Equal(expectFlagged, Catalog.BuildCard(cardName).LoopRiskDraw);
    }

    /// <summary>Forced discard-of-choice with NO draw (Survivor: gain Block, then discard 1 of choice) is
    /// modelled in search and feeds <c>CardsDiscardedThisTurn</c>. Decisive + within-turn: MementoMori deals
    /// 9 + 4×(cards discarded this turn), the slug has 13 HP — unkillable at 0 discards (9), lethal at 1 (13).
    /// The ONLY discard source is Survivor's forced discard, so the exact (1.0, 0) proves (a) the discard fires
    /// in search and increments the counter, and (b) the player MAXes the choice — discards the filler Defend,
    /// keeping MementoMori, and orders Survivor before MementoMori. Under the old omit-the-discard behaviour the
    /// counter stays 0, MementoMori deals 9, the slug survives, and HP is lost.</summary>
    [Fact]
    public void Survivor_Forced_Discard_Feeds_The_Discard_Counter_And_Is_A_Player_Max()
    {
        var p = new Player { MaxHp = 40, CurrentHp = 40, Energy = 3, MaxEnergy = 3 };
        p.Hand.Add(Catalog.BuildCard("Survivor"));        // cost 1: gain Block, discard 1 of choice
        p.Hand.Add(Catalog.BuildCard("MementoMori"));     // cost 1: 9 + 4×discards this turn
        p.Hand.Add(Catalog.BuildCard("DefendIronclad"));  // the filler the player should discard
        var slug = Monsters.CorpseSlug(hp: 13);           // 9 < 13 ≤ 13 ⇒ lethal only with the +1 discard
        slug.Ai.CurrentMoveId = slug.Ai.InitialStateId;
        var combat = new CombatState { Player = p, Monsters = { slug }, TurnNumber = 1 };

        var v = new Solver { MaxTurns = 10 }.SolvePlayerTurn(combat);
        _out.WriteLine($"value = {v}");
        Assert.Equal(1.0, v.Win, 6);   // Survivor's forced discard bumps MementoMori to lethal
        Assert.Equal(0.0, v.Loss, 6);  // slug dies before acting
    }

    /// <summary>Forced discard-of-choice AFTER a draw (DaggerThrow: 9 damage, draw 1, then discard 1 of choice)
    /// resolves on the post-draw hand and feeds the discard counter. Decisive: slug 22 HP; DaggerThrow (9) +
    /// MementoMori (9 + 4×discards). With the discard modelled the counter is 1 ⇒ MementoMori 13 ⇒ 9+13 = 22 =
    /// lethal; with the discard omitted the counter is 0 ⇒ MementoMori 9 ⇒ 18 ⇒ slug survives. The lone draw-pile
    /// card is a filler the player discards (keeping MementoMori); the exact (1.0, 0) proves the post-draw
    /// discard fires and is chosen soundly.</summary>
    [Fact]
    public void DaggerThrow_Post_Draw_Discard_Feeds_The_Discard_Counter()
    {
        var p = new Player { MaxHp = 40, CurrentHp = 40, Energy = 3, MaxEnergy = 3 };
        p.Hand.Add(Catalog.BuildCard("DaggerThrow"));     // cost 1: 9 dmg, draw 1, discard 1 of choice
        p.Hand.Add(Catalog.BuildCard("MementoMori"));     // cost 1: 9 + 4×discards this turn
        p.DrawPile.Add(Catalog.BuildCard("DefendIronclad")); // the only draw ⇒ deterministic; discarded by choice
        var slug = Monsters.CorpseSlug(hp: 22);
        slug.Ai.CurrentMoveId = slug.Ai.InitialStateId;
        var combat = new CombatState { Player = p, Monsters = { slug }, TurnNumber = 1 };

        var v = new Solver { MaxTurns = 10 }.SolvePlayerTurn(combat);
        _out.WriteLine($"value = {v}");
        Assert.Equal(1.0, v.Win, 6);   // DaggerThrow's post-draw discard makes MementoMori lethal
        Assert.Equal(0.0, v.Loss, 6);
    }

    /// <summary>The no-draw forced-discard MAX (Survivor) must converge MCTS→exact, like the post-draw discard
    /// (Acrobatics) already does. A Survivor deck routes a <c>PendingDiscard</c> decision layer at PLAY time (no
    /// draw chance node); MCTS must track exact on survival AND HP loss.</summary>
    [Fact]
    public void Mcts_Converges_On_No_Draw_Discard_Deck()
    {
        CombatState Build() => Catalog.SetupCombat(
            Catalog.BuildPlayer(new List<CardModel> {
                Catalog.BuildCard("Survivor"), Catalog.BuildCard("StrikeIronclad"),
                Catalog.BuildCard("StrikeIronclad"), Catalog.BuildCard("DefendIronclad"),
                Catalog.BuildCard("DefendIronclad") }, 22, 22, 3, new[] { "BurningBlood" }),
            new[] { Monsters.CorpseSlug(hp: 24) });
        const int mt = 8;
        var exact = new Solver { MaxTurns = mt }.Solve(Build());
        var mcts = new MctsSolver(new MctsOptions { Trials = 30_000, Seed = 1, MaxTurns = mt, ActionWidening = true }).Solve(Build());
        _out.WriteLine($"exact {exact}  |  mcts {mcts}");
        Assert.True(Math.Abs(mcts.Win - exact.Win) <= 0.05, $"survival exact {exact.Win:P2} vs mcts {mcts.Win:P2}");
        Assert.True(Math.Abs(mcts.Loss - exact.Loss) <= 2.5, $"loss exact {exact.Loss:F1} vs mcts {mcts.Loss:F1}");
    }

    /// <summary>Cross-validation that the mid-turn draw node makes a fight WINNABLE that is provably unwinnable
    /// without the draw, and that the exact value is sound (no card-duplication / probability blow-up). The
    /// Defends fixture's 2 Strikes (12 dmg) can never chew through Byrdonis' 58 HP on their own — winning needs
    /// the deck to CYCLE, which only ShrugItOff's draw accelerates. So: without modelling the draw the fight is
    /// 0% (the 4-Defend control), and with it the exact survival is solidly positive; faithful Monte-Carlo of
    /// the same optimal policy (real RNG draws) confirms the value is real and bounds the gap.
    ///
    /// The residual exact↔MC gap is NOT a node bug: the exact solver models every draw as an independent
    /// hypergeometric over the pile multiset (the documented <see cref="DrawEnumerator"/> design), whereas the
    /// real engine preserves draw-pile ORDER across turns — so a real shuffle cycles the 2 Strikes more reliably
    /// than the memoiseable independent model, making the exact value a (slight) pessimistic LOWER bound on a
    /// cycle-dependent fight. The control + the deterministic-draw tests above pin the node itself.</summary>
    [Fact]
    public void Mid_Turn_Draw_Makes_The_Fight_Winnable_And_Value_Is_Sound()
    {
        // No-draw control with the same board/deck shape (ShrugItOff → a 4th Defend): provably 0% — the draw is
        // the ONLY route to a win here.
        CombatState NoDraw() => Catalog.SetupCombat(
            Catalog.BuildPlayer(new List<CardModel> {
                Catalog.BuildCard("DefendIronclad"), Catalog.BuildCard("DefendIronclad"),
                Catalog.BuildCard("DefendIronclad"), Catalog.BuildCard("DefendIronclad"),
                Catalog.BuildCard("StrikeIronclad"), Catalog.BuildCard("StrikeIronclad") }, 40, 40, 3, new[] { "BurningBlood" }),
            new[] { Monsters.Byrdonis(hp: 58) });
        var control = new Solver { MaxTurns = 14 }.Solve(NoDraw());
        Assert.Equal(0.0, control.Win, 3);   // unwinnable without the draw acceleration

        var f = CalibrationFixtures.All.First(x => x.Name == "block/Defends-vs-Byrdonis");   // same fight + ShrugItOff
        var solver = new Solver { MaxTurns = f.MaxTurns };
        var exact = solver.Solve(f.Setup());
        solver.Ct = CancellationToken.None;
        var mc = PolicyRollout.Sample(f.Setup(), new ExactMemoPolicy(solver), samples: 6000, maxTurns: f.MaxTurns, baseSeed: 1);
        _out.WriteLine($"control(no draw) {control.Win:P2}  |  exact(draw) {exact.Win:P2}  |  faithful-MC {mc.Survival:P2}");

        Assert.True(exact.Win > 0.5, $"modelling ShrugItOff's draw should make this fight clearly winnable, got {exact.Win:P2}");
        // Exact is a pessimistic lower bound vs the real (order-preserving) shuffle; the gap is the documented
        // independent-hypergeometric approximation, not a node defect — bound it to catch a real blow-up.
        Assert.InRange(mc.Survival - exact.Win, -0.01, 0.08);
    }

    /// <summary>In-hand multi-select promotion (HiddenDaggers: discard 2 of choice, THEN add 2 Shivs). The
    /// game discards FIRST then creates the Shivs, so the Shivs are never in the discard pool — modelling the
    /// discard as a real player MAX over the PRE-Shiv hand (with the Shiv creation as the discard's continuation)
    /// is sound, not optimistic. Decisive: after playing HiddenDaggers (cost 0) the hand is Strike + 2 Defends;
    /// the slug has 10 HP. The only lethal line is discard BOTH Defends (keeping Strike), let the 2 Shivs spawn,
    /// then Strike (6) + one Shiv (4) = 10 — so the exact (1.0, 0) proves BOTH that the discard is MAXed (a wrong
    /// discard of Strike caps damage at 2×Shiv = 8 &lt; 10) AND that the Shiv continuation fires (without it Strike
    /// alone is 6 &lt; 10). Energy 1 affords Strike(1) + Shivs(0).</summary>
    [Fact]
    public void HiddenDaggers_Discards_Optimally_Then_Adds_Shivs_In_Search()
    {
        var p = new Player { MaxHp = 30, CurrentHp = 30, Energy = 1, MaxEnergy = 1 };
        p.Hand.Add(Catalog.BuildCard("HiddenDaggers"));   // cost 0: discard 2 of choice, then add 2 Shivs (4 dmg each)
        p.Hand.Add(Catalog.BuildCard("StrikeSilent"));    // 6 dmg — the card that MUST be kept
        p.Hand.Add(Catalog.BuildCard("DefendSilent"));
        p.Hand.Add(Catalog.BuildCard("DefendSilent"));
        var slug = Monsters.CorpseSlug(hp: 10);
        slug.Ai.CurrentMoveId = slug.Ai.InitialStateId;
        var combat = new CombatState { Player = p, Monsters = { slug }, TurnNumber = 1 };

        var v = new Solver { MaxTurns = 10 }.SolvePlayerTurn(combat);
        _out.WriteLine($"value = {v}");
        Assert.Equal(1.0, v.Win, 6);   // discarded the 2 Defends, kept Strike, Shivs spawned ⇒ Strike+Shiv = 10 lethal
        Assert.Equal(0.0, v.Loss, 6);
    }

    /// <summary>The HiddenDaggers discard-of-choice continuation (a play-time discard-then-act, no preceding
    /// draw) must converge MCTS→exact like the post-draw/no-draw discards already do. A HiddenDaggers deck routes
    /// a <c>PendingDiscard</c> decision layer whose drain fires the Shiv-creation continuation; MCTS must track
    /// exact on survival AND HP loss.</summary>
    [Fact]
    public void Mcts_Converges_On_HiddenDaggers_Deck()
    {
        CombatState Build() => Catalog.SetupCombat(
            Catalog.BuildPlayer(new List<CardModel> {
                Catalog.BuildCard("HiddenDaggers"), Catalog.BuildCard("StrikeSilent"),
                Catalog.BuildCard("StrikeSilent"), Catalog.BuildCard("DefendSilent"),
                Catalog.BuildCard("DefendSilent") }, 22, 22, 3),
            new[] { Monsters.CorpseSlug(hp: 24) });
        const int mt = 8;
        var exact = new Solver { MaxTurns = mt }.Solve(Build());
        var mcts = new MctsSolver(new MctsOptions { Trials = 30_000, Seed = 1, MaxTurns = mt, ActionWidening = true }).Solve(Build());
        _out.WriteLine($"exact {exact}  |  mcts {mcts}");
        Assert.True(Math.Abs(mcts.Win - exact.Win) <= 0.05, $"survival exact {exact.Win:P2} vs mcts {mcts.Win:P2}");
        Assert.True(Math.Abs(mcts.Loss - exact.Loss) <= 2.5, $"loss exact {exact.Loss:F1} vs mcts {mcts.Loss:F1}");
    }
}
