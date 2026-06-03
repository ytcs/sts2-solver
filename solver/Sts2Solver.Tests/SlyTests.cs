using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Gates the Sly keyword in SEARCH: a Sly card DISCARDED mid-turn auto-plays for free
/// (<see cref="CombatManager.TriggerSlyOnDiscard"/>), so the discard-of-choice MAX must account for the
/// auto-play's effect. Deterministic / random-default / deferred-draw effects all resolve the same way a
/// normal play does, so the exact oracle and MCTS still agree.
/// </summary>
public class SlyTests
{
    private readonly ITestOutputHelper _out;
    public SlyTests(ITestOutputHelper o) => _out = o;

    /// <summary>Decisive: the only damage available is a Sly Ricochet auto-played by Survivor's forced discard.
    /// Ricochet (cost 2, 4×3 = 12 damage) is UNPLAYABLE at 1 energy, so it can never be played directly — but
    /// Survivor (cost 1, gain block, discard 1 of choice) auto-plays it for free when the search discards it,
    /// dealing 12 to the 12-HP slug and killing it on turn 1 before it can act. The exact (1.0, 0) is only
    /// reachable through the Sly auto-play (discarding Ricochet does nothing without it), so it proves the search
    /// resolves the Sly trigger inside the discard MAX.</summary>
    [Fact]
    public void Sly_Ricochet_Auto_Played_By_Survivors_Discard_Wins_The_Turn()
    {
        var player = new Player { MaxHp = 30, CurrentHp = 30, Energy = 1, MaxEnergy = 1 };
        player.Hand.Add(Catalog.BuildCard("Survivor"));     // cost 1: gain block, discard 1 of choice
        player.Hand.Add(Catalog.BuildCard("Ricochet"));     // cost 2 (unplayable at 1 energy), Sly, 12 damage
        player.Hand.Add(Catalog.BuildCard("DefendSilent")); // a filler discard alternative (deals nothing)
        var slug = Monsters.CorpseSlug(hp: 12);
        slug.Ai.CurrentMoveId = slug.Ai.InitialStateId;
        var combat = new CombatState { Player = player, Monsters = { slug }, TurnNumber = 1 };

        var v = new Solver { MaxTurns = 6 }.SolvePlayerTurn(combat);
        _out.WriteLine($"value = {v}");
        Assert.Equal(1.0, v.Win, 6);    // Survivor → discard Ricochet → Sly auto-play 12 → kill turn 1
        Assert.Equal(0.0, v.Loss, 6);   // slug dies before acting
    }

    /// <summary>MCTS must converge to the exact oracle on a Sly deck — the auto-play (deterministic block /
    /// AoE damage / random-default attack / deferred draw) is resolved identically in both engines, including the
    /// Sly draw routed through a draw chance node from the discard layer.</summary>
    [Fact]
    public void Mcts_Converges_On_A_Sly_Deck()
    {
        CombatState Build() => Catalog.SetupCombat(
            Catalog.BuildPlayer(new List<CardModel> {
                Catalog.BuildCard("Survivor"), Catalog.BuildCard("Ricochet"),
                Catalog.BuildCard("Untouchable"), Catalog.BuildCard("StrikeSilent"),
                Catalog.BuildCard("DefendSilent") }, 24, 24, 3),
            new[] { Monsters.CorpseSlug(hp: 22) });
        const int mt = 8;
        var exact = new Solver { MaxTurns = mt }.Solve(Build());
        var mcts = new MctsSolver(new MctsOptions { Trials = 30_000, Seed = 1, MaxTurns = mt, ActionWidening = true }).Solve(Build());
        _out.WriteLine($"exact {exact}  |  mcts {mcts}");
        Assert.True(Math.Abs(mcts.Win - exact.Win) <= 0.05, $"survival exact {exact.Win:P2} vs mcts {mcts.Win:P2}");
        Assert.True(Math.Abs(mcts.Loss - exact.Loss) <= 2.5, $"loss exact {exact.Loss:F1} vs mcts {mcts.Loss:F1}");
    }
}
