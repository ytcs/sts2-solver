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
}
