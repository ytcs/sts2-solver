using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Soundness gate for the admissible in-search early-loss prune (<see cref="LossCertificate"/>): solving WITH
/// the certificate must produce the exact same lexicographic value as solving without it — the prune may only
/// skip work, never change the answer. We also assert it actually fires (fewer states explored), so the test
/// can't silently pass on a no-op certificate.
/// </summary>
public class LossPruningTests
{
    private readonly ITestOutputHelper _out;
    public LossPruningTests(ITestOutputHelper o) => _out = o;

    private static CombatState Fixture(IEnumerable<CardModel> deck, int hp, Monster monster, int energy = 3) =>
        Catalog.SetupCombat(Catalog.BuildPlayer(deck.ToList(), hp, hp, energy, new[] { "BurningBlood" }), new[] { monster });

    private static List<CardModel> BlockDeck() => new()
    {
        new ShrugItOff(), new DefendIronclad(), new DefendIronclad(),
        new DefendIronclad(), new StrikeIronclad(), new StrikeIronclad(),
    };

    private static List<CardModel> WeakDeck() => new()
    {
        new Neutralize(), new DefendIronclad(), new DefendIronclad(),
        new DefendIronclad(), new StrikeIronclad(), new StrikeIronclad(),
    };

    [Theory]
    // Low-but-nonzero survival (the validated block/Byrdonis calibration shape, ≈22%/39): the prune fires on
    // the doomed lines while the value — including the secondary HP-loss objective — is preserved exactly.
    [InlineData("block", 40, 58, 14)]
    // A Weak-applying deck vs a higher-HP, harder-ramping Byrdonis: mostly/entirely lost.
    [InlineData("weak", 34, 70, 16)]
    // Pure loss (unkillable enemy): every line is doomed ⇒ survival 0, loss bounded by current HP.
    [InlineData("block", 28, 9999, 14)]
    public void Pruning_Preserves_Exact_Value(string deckName, int hp, int monsterHp, int maxTurns)
    {
        List<CardModel> Deck() => deckName == "weak" ? WeakDeck() : BlockDeck();

        var cert = LossCertificate.TryBuild(Fixture(Deck(), hp, Monsters.Byrdonis(hp: monsterHp)), maxTurns);
        Assert.NotNull(cert);   // the fixture must qualify, else the test isn't exercising the prune

        var plain = new Solver { MaxTurns = maxTurns };
        var pruned = new Solver { MaxTurns = maxTurns, LossProof = cert };
        var vPlain = plain.Solve(Fixture(Deck(), hp, Monsters.Byrdonis(hp: monsterHp)));
        var vPruned = pruned.Solve(Fixture(Deck(), hp, Monsters.Byrdonis(hp: monsterHp)));

        _out.WriteLine($"{deckName} hp{hp} vs Byrdonis({monsterHp}), maxTurns {maxTurns}");
        _out.WriteLine($"  plain : {vPlain}  ({plain.StatesEvaluated:N0} states)");
        _out.WriteLine($"  pruned: {vPruned}  ({pruned.StatesEvaluated:N0} states)");

        Assert.Equal(vPlain.Win, vPruned.Win, 9);
        Assert.Equal(vPlain.Loss, vPruned.Loss, 6);
        Assert.True(pruned.StatesEvaluated < plain.StatesEvaluated,
            $"prune did not fire: pruned {pruned.StatesEvaluated} states vs plain {plain.StatesEvaluated}");
    }
}
