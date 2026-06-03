using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Gates the Innate keyword: Innate cards are GUARANTEED in the turn-1 opening hand (the game moves them to
/// the top of the draw pile and draws max(5, innateCount)). Modelled in
/// <see cref="CombatManager.OpeningDrawAfterInnate"/>, wired into every opening-draw site (exact / MCTS /
/// rollout). Only affects turn 1, so it is irrelevant to trace replay (recorded hands already reflect it).
/// </summary>
public class InnateTests
{
    private readonly ITestOutputHelper _out;
    public InnateTests(ITestOutputHelper o) => _out = o;

    /// <summary>The opening-draw helper pulls every Innate card straight into the hand and returns the residual
    /// random-draw count (so total opening hand = max(5, innateCount)). A no-op past turn 1.</summary>
    [Fact]
    public void OpeningDrawAfterInnate_Seeds_Innate_Cards_And_Returns_The_Residual()
    {
        var p = new Player { MaxHp = 30, CurrentHp = 30 };
        p.DrawPile.Add(Catalog.BuildCard("Suppress"));         // canonical Innate
        for (int i = 0; i < 9; i++) p.DrawPile.Add(Catalog.BuildCard("DefendSilent"));
        var combat = new CombatState { Player = p, TurnNumber = 1 };

        int residual = CombatManager.OpeningDrawAfterInnate(combat, 5);
        Assert.Equal(4, residual);                              // 5 - 1 innate
        Assert.Contains(p.Hand, c => c.Name == "Suppress");     // guaranteed into hand
        Assert.DoesNotContain(p.DrawPile, c => c.Name == "Suppress");
        Assert.Equal(9, p.DrawPile.Count);                      // only the innate card left the pile

        // Past turn 1 it is a no-op (Innate seeds ONLY the opening hand).
        var p2 = new Player { MaxHp = 30, CurrentHp = 30 };
        p2.DrawPile.Add(Catalog.BuildCard("Suppress"));
        var turn2 = new CombatState { Player = p2, TurnNumber = 2 };
        Assert.Equal(5, CombatManager.OpeningDrawAfterInnate(turn2, 5));
        Assert.Empty(p2.Hand);
    }

    /// <summary>Decisive end-to-end: an Innate lethal attack is ALWAYS in the opening hand, so the exact solver
    /// wins turn 1 with zero HP lost. Suppress (cost 0, 11 damage, Innate) sits in a 10-card deck; the slug has
    /// 11 HP and telegraphs a hit. WITHOUT the Innate guarantee Suppress would be in the opening 5-of-10 only
    /// ~50% of the time, so some openings can't kill turn 1 and the slug lands its hit (loss &gt; 0). The exact
    /// (1.0, 0) therefore proves Suppress is guaranteed into every opening hand.</summary>
    [Fact]
    public void Innate_Lethal_Attack_Is_Always_In_The_Opening_Hand()
    {
        CombatState Build()
        {
            var deck = new List<CardModel> { Catalog.BuildCard("Suppress") };
            for (int i = 0; i < 9; i++) deck.Add(Catalog.BuildCard("DefendSilent"));
            var player = Catalog.BuildPlayer(deck, 30, 30, 3);
            return Catalog.SetupCombat(player, new[] { Monsters.CorpseSlug(hp: 11) });
        }
        var v = new Solver { MaxTurns = 6 }.Solve(Build());
        _out.WriteLine($"value = {v}");
        Assert.Equal(1.0, v.Win, 6);    // Suppress always drawn ⇒ always kill turn 1
        Assert.Equal(0.0, v.Loss, 6);   // slug dies before it can act
    }
}
