using Sts2Solver.Content;
using Sts2Solver.Engine;
using Sts2Solver.Search;
using Xunit;
using Xunit.Abstractions;

namespace Sts2Solver.Tests;

/// <summary>
/// Audit gate: the two value-preserving search approximations — the sound horizon bound
/// (<see cref="HorizonBound"/>) and the admissible early-loss prune (<see cref="LossCertificate"/>) — must stay
/// EXACT across diverse monster AI, not just the ramping-Byrdonis shape the original HorizonBoundTests /
/// LossPruningTests cover. The soundness arguments are general (incoming lower-bounded by the idle forced-min
/// trajectory, block/damage upper-bounded), but the only thing that proves they survive a DIFFERENT AI is an
/// oracle-equality check against it: a stun cycle (TerrorEel), enemy self-heal / life-drain (SoulNexus), a
/// telegraphed windup burst (MechaKnight), Frail+Strength ramp (CorpseSlug), and poison-on-the-player
/// (PhrogParasite) each stress a distinct way the idle trajectory could mis-estimate incoming.
///
/// The guarantees are CONDITIONAL (a bound only has to be sound WHEN it fires; the prune only WHEN the
/// certificate builds), so each check is skipped when the mechanism legitimately bails — that bail is itself
/// the sound outcome. Fixtures use a no-block deck at low HP so every line terminates within a few turns
/// (a win or a forced death), keeping exact@big a shallow, fast tree regardless of the nominal horizon.
/// </summary>
public class DiverseAiSoundnessTests
{
    private readonly ITestOutputHelper _out;
    public DiverseAiSoundnessTests(ITestOutputHelper o) => _out = o;

    private static CombatState Fixture(IEnumerable<CardModel> deck, int hp, Monster monster) =>
        Catalog.SetupCombat(Catalog.BuildPlayer(deck.ToList(), hp, hp, 3, new[] { "BurningBlood" }), new[] { monster });

    private static List<CardModel> NoBlockDeck() =>
        Enumerable.Range(0, 5).Select(_ => (CardModel)new StrikeIronclad()).ToList();

    private static Monster Make(string name, int hp) => name switch
    {
        "TerrorEel"     => Monsters.TerrorEel(hp),
        "SoulNexus"     => Monsters.SoulNexus(hp),
        "MechaKnight"   => Monsters.MechaKnight(hp),
        "CorpseSlug"    => Monsters.CorpseSlug(hp),
        "PhrogParasite" => Monsters.PhrogParasite(hp),
        _ => throw new ArgumentOutOfRangeException(nameof(name)),
    };

    public static IEnumerable<object[]> Cases => new[]
    {
        new object[] { "TerrorEel", 150 },      // self-stun cycle
        new object[] { "SoulNexus", 120 },      // life-drain (enemy self-heal)
        new object[] { "MechaKnight", 120 },    // windup → heavy burst
        new object[] { "CorpseSlug", 40 },      // Frail (cuts player block) + Strength ramp
        new object[] { "PhrogParasite", 50 },   // poison-on-player (incoming via a debuff tick)
    };

    [Theory]
    [MemberData(nameof(Cases))]
    public void Horizon_And_LossPrune_Stay_Exact_On_Diverse_AI(string monster, int monsterHp)
    {
        const int big = 40;
        const int php = 22;   // low ⇒ tanky elites force a provable death; weak ones die fast — both shallow.

        // ---- Horizon soundness: when the bound fires, exact@bound must equal exact@big. ----
        int bound = HorizonBound.Compute(Fixture(NoBlockDeck(), php, Make(monster, monsterHp)), big);
        bool horizonFired = bound < big;
        if (horizonFired)
        {
            var atBound = new Solver { MaxTurns = bound }.Solve(Fixture(NoBlockDeck(), php, Make(monster, monsterHp)));
            var atBig = new Solver { MaxTurns = big }.Solve(Fixture(NoBlockDeck(), php, Make(monster, monsterHp)));
            _out.WriteLine($"{monster}: horizon bound={bound} (default {big})  exact@bound={atBound}  exact@big={atBig}");
            Assert.Equal(atBig.Win, atBound.Win, 6);
            Assert.Equal(atBig.Loss, atBound.Loss, 4);
        }
        else _out.WriteLine($"{monster}: horizon bound did not fire (bailed to default {big}) — sound bail.");

        // ---- Loss-prune soundness: when the certificate builds, pruned must equal unpruned. ----
        const int h = 14;
        var cert = LossCertificate.TryBuild(Fixture(NoBlockDeck(), php, Make(monster, monsterHp)), h);
        if (cert != null)
        {
            var plain = new Solver { MaxTurns = h };
            var pruned = new Solver { MaxTurns = h, LossProof = cert };
            var vPlain = plain.Solve(Fixture(NoBlockDeck(), php, Make(monster, monsterHp)));
            var vPruned = pruned.Solve(Fixture(NoBlockDeck(), php, Make(monster, monsterHp)));
            _out.WriteLine($"  loss-cert built: plain={vPlain} ({plain.StatesEvaluated:N0} st), pruned={vPruned} ({pruned.StatesEvaluated:N0} st)");
            Assert.Equal(vPlain.Win, vPruned.Win, 9);
            Assert.Equal(vPlain.Loss, vPruned.Loss, 6);
        }
        else _out.WriteLine("  loss-cert: not built (deck/enemy disqualified) — sound bail.");

        // Non-vacuity backstop: the deterministic tanky elites MUST exercise at least one mechanism, else this
        // case would prove nothing. The remaining monsters legitimately bail both and are kept to confirm the
        // SOUND BAIL path on distinct AI:
        //   • SoulNexus picks its move stochastically (branching enumeration) ⇒ the forced-min trajectory is
        //     non-deterministic ⇒ both mechanisms correctly return null/default rather than guess.
        //   • CorpseSlug / PhrogParasite are killed before they can doom a 22-HP player (the player WINS) ⇒
        //     no death to prove, so neither fires.
        bool mustFire = monster is "TerrorEel" or "MechaKnight";
        if (mustFire)
            Assert.True(horizonFired || cert != null,
                $"{monster}: neither soundness mechanism engaged — the case is vacuous and proves nothing");
    }
}
