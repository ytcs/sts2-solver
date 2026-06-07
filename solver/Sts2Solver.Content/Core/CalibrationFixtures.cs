using Sts2Solver.Engine;

namespace Sts2Solver.Content;

/// <summary>
/// A shared, deliberately *diverse* set of (deck, encounter) fixtures for calibrating the sampling solver
/// against the exact oracle. The point is to span deck archetypes beyond the Ironclad starter — block-heavy,
/// Strength/scaling, debuff-setup, card-draw/aggro, power-engine — so the heuristic policy can't be tuned
/// to one style. Every fixture is kept small enough (compact deck, single enemy, modest HP) that the exact
/// expectimax can still solve it in seconds, so each has a ground-truth label.
///
/// Lives in Content (the only project that can both see <see cref="Catalog"/> and is referenced by the CLI
/// and the test project); Search can't reference Content, so the harness passes the built state in.
/// </summary>
public static class CalibrationFixtures
{
    /// <summary>One calibration scenario. <see cref="Build"/> returns a fresh setup state each call
    /// (combat states are mutated during search, so callers clone per engine).</summary>
    public sealed record Fixture(string Name, string Archetype, Func<CombatState> Build, int MaxTurns)
    {
        public CombatState Setup() => Build();
    }

    // A compact deck spec → card list (e.g. "3xStrikeIronclad" or "Inflame"). Upgrades via the usual "+N".
    private static List<CardModel> Deck(params string[] specs)
    {
        var deck = new List<CardModel>();
        foreach (var spec in specs)
        {
            int x = spec.IndexOf('x');
            if (x > 0 && int.TryParse(spec[..x], out int n))
                for (int i = 0; i < n; i++) deck.Add(Catalog.BuildCard(spec[(x + 1)..]));
            else
                deck.Add(Catalog.BuildCard(spec));
        }
        return deck;
    }

    private static Fixture Make(string name, string archetype, int hp, int energy, int maxTurns,
        Func<Monster> monster, params string[] deck) =>
        new(name, archetype,
            () => Catalog.SetupCombat(
                Catalog.BuildPlayer(Deck(deck), hp, hp, energy, new[] { "BurningBlood" }),
                new[] { monster() }),
            maxTurns);

    /// <summary>The fixture set. Ascension 0 (base stats) keeps fights small; HP/energy/horizon are tuned so
    /// each fight is non-trivial (survival or loss meaningfully &gt; 0) yet exact-solvable in seconds. The
    /// horizon (MaxTurns) is set tight — comfortably above the turns optimal play needs, but well below the
    /// default 30 — because the exact tree's size is dominated by how many stalling turns it must explore;
    /// both engines share the same horizon, so the comparison stays fair.</summary>
    public static IReadOnlyList<Fixture> All { get; } = new[]
    {
        // --- Baseline: the starter deck (the style the heuristic was first tuned on). ---
        Make("starter/Cultist", "starter", hp: 50, energy: 3, maxTurns: 12,
            () => Monsters.CalcifiedCultist(),
            "5xStrikeIronclad", "4xDefendIronclad", "Bash"),

        // --- Strength/power: must PLAY a power (Inflame) that doesn't reduce the position score this turn
        //     but wins the race against a fast-ramping cultist. A greedy-on-current-score policy never
        //     plays it — the canonical overfitting probe. ---
        Make("power/Inflame-vs-DampCultist", "power", hp: 50, energy: 3, maxTurns: 12,
            () => Monsters.DampCultist(),
            "Inflame", "4xStrikeIronclad", "2xDefendIronclad"),

        // --- Debuff setup: Uppercut applies Vulnerable; sequencing the debuff before the Strikes wins the
        //     race vs a ramping elite. Tests whether the policy values setting up Vulnerable. ---
        Make("debuff/Uppercut-vs-Byrdonis", "debuff", hp: 50, energy: 3, maxTurns: 12,
            () => Monsters.Byrdonis(hp: 52),
            "Uppercut", "4xStrikeIronclad", "1xDefendIronclad"),

        // --- Block-defensive: a hard hitter (Byrdonis Swoop 17) where surviving needs real block, not race.
        //     Tests block valuation / over-block discipline under pressure. ---
        Make("block/Defends-vs-Byrdonis", "block", hp: 40, energy: 3, maxTurns: 14,
            () => Monsters.Byrdonis(hp: 58),
            "ShrugItOff", "3xDefendIronclad", "2xStrikeIronclad"),

        // --- Aggro/draw: Pommel Strike + Anger churn cards; CorpseSlug applies Frail (cuts our block) and
        //     opens with +Strength. Tests card-draw value and the Frail/block interaction. ---
        Make("aggro/Draw-vs-CorpseSlug", "aggro", hp: 45, energy: 3, maxTurns: 12,
            () => Monsters.CorpseSlug(),
            "PommelStrike", "Anger", "2xStrikeIronclad", "Bash"),

        // --- Power-engine: Demon Form is pure scaling (+Strength every turn) at a steep up-front tempo cost
        //     vs a tanky fight (BygoneEffigy sleeps then hits hard). The hardest case for a myopic policy:
        //     the right line spends tempo early to compound later. ---
        Make("engine/DemonForm-vs-Effigy", "engine", hp: 70, energy: 3, maxTurns: 15,
            () => Monsters.BygoneEffigy(hp: 70),
            "DemonForm", "4xStrikeIronclad", "1xDefendIronclad"),
    };

    /// <summary>Build a fixture for an ARBITRARY character: explicit relic(s), energy and deck, so the
    /// per-character suite can exercise each class's defining mechanic (Osty / poison / orbs / stars) rather
    /// than only the Ironclad starter. Kept draw-free and small so the exact oracle still labels it and the
    /// greedy-turn regret measurement is deterministic.</summary>
    private static Fixture MakeChar(string name, string archetype, int hp, int energy, int maxTurns,
        string[] relics, Func<Monster> monster, params string[] deck) =>
        new(name, archetype,
            () => Catalog.SetupCombat(
                Catalog.BuildPlayer(Deck(deck), hp, hp, energy, relics),
                new[] { monster() }),
            maxTurns);

    // -----------------------------------------------------------------------
    // Per-character fixtures: each exercises a class's DEFINING mechanic in a
    // fight small enough for the exact oracle to label, so the greedy-leaf
    // policy-regret instrument (CalibrationHarness.MeasurePolicyRegret) can see
    // where the heuristic mis-plays that mechanic. The exact solver plays the
    // real engine (mechanic-optimal by construction); any regret here is the
    // LEAF HEURISTIC's, which is exactly what the per-character redesign targets.
    // Draw-free decks only (deterministic greedy turn + exact-tractable).
    // Surfaced via `--calibrate --percharacter` / `--regret`.
    // -----------------------------------------------------------------------
    public static IReadOnlyList<Fixture> PerCharacter { get; } = new[]
    {
        // NECROBINDER — Osty is a per-turn damage sponge (DieForYou) AND the Unleash damage multiplier, but the
        // leaf Score can't see it, so growing Osty (Bodyguard) reads as a dead play. Byrdonis' Swoop (heavy
        // telegraphed hit) makes keeping Osty alive/large the survival lever the oracle uses and greedy misses.
        MakeChar("necro/Osty-vs-Byrdonis", "necrobinder", hp: 44, energy: 3, maxTurns: 10,
            new[] { "BoundPhylactery" }, () => Monsters.Byrdonis(hp: 40),
            "4xStrikeNecrobinder", "3xDefendNecrobinder", "Bodyguard", "Unleash"),

        // NECROBINDER — an Unleash-heavy race: most damage routes through Osty (Unleash = 6 + Osty HP), so the
        // optimal line grows Osty FIRST (Bodyguard) to make every Unleash hit harder and end the fight sooner.
        // A leaf that can't see Osty HP under-values the grow-first line → larger HP-loss regret than the
        // single-Unleash starter. Byrdonis is the tanky hitter that makes the extra Unleash damage pay.
        MakeChar("necro/Unleash-race-vs-Byrdonis", "necrobinder", hp: 48, energy: 3, maxTurns: 10,
            new[] { "BoundPhylactery" }, () => Monsters.Byrdonis(hp: 46),
            "2xUnleash", "Bodyguard", "2xStrikeNecrobinder", "2xDefendNecrobinder"),

        // IRONCLAD SANITY — the heuristic was tuned on Ironclad, so its regret here should already be ~0. Acts as
        // the control: a redesign that helps Necrobinder must not move this off zero. (Mirrors the starter fixture
        // but built through the same MakeChar path so the suites are comparable.)
        MakeChar("iron/Starter-vs-Cultist", "ironclad", hp: 50, energy: 3, maxTurns: 12,
            new[] { "BurningBlood" }, () => Monsters.CalcifiedCultist(),
            "5xStrikeIronclad", "4xDefendIronclad", "Bash"),
    };

    // -----------------------------------------------------------------------
    // LONG / HARD fixtures (NO exact label — deliberately beyond the oracle).
    // The regime where a character's scaling mechanic is supposed to matter:
    // tanky, Strength-ramping enemies fought over many turns, so Osty has time
    // to grow (Bound Phylactery +1/turn + summons) and Osty-HP attacks (Unleash
    // 6+OstyHp, Protector 6+Osty maxHp) scale. Paired decks — the STARTER vs a
    // BUILT deck that can actually leverage Osty (extra summons + Osty attacks) —
    // so a policy benchmark can ask "does growing Osty lower HP loss here?" where
    // the exact solver can't. Benchmarked by HP loss, not a ground-truth label.
    // Surfaced via `sts2solve --osty-bench`.
    // -----------------------------------------------------------------------
    private static readonly string[] NecroBuiltDeck =
        { "3xBodyguard", "Reanimate", "3xUnleash", "Protector",
          "3xStrikeNecrobinder", "3xDefendNecrobinder" };

    public static IReadOnlyList<Fixture> PerCharacterLong { get; } = new[]
    {
        // Byrdonis at FULL HP: Territorial ramps its Strength every turn ⇒ a long grind that punishes a slow clock
        // and rewards sustained damage mitigation — exactly where Osty's per-turn wall should earn its keep.
        MakeChar("necroLong/starter-vs-Byrdonis90", "necrobinder", hp: 100, energy: 3, maxTurns: 40,
            new[] { "BoundPhylactery" }, () => Monsters.Byrdonis(hp: 90),
            "4xStrikeNecrobinder", "4xDefendNecrobinder", "Bodyguard", "Unleash"),
        MakeChar("necroLong/built-vs-Byrdonis90", "necrobinder", hp: 100, energy: 3, maxTurns: 40,
            new[] { "BoundPhylactery" }, () => Monsters.Byrdonis(hp: 90), NecroBuiltDeck),

        // BygoneEffigy: tanky (127 HP), sleeps then ramps Strength — a long fight that tests sustained Osty value.
        MakeChar("necroLong/starter-vs-Effigy127", "necrobinder", hp: 120, energy: 3, maxTurns: 45,
            new[] { "BoundPhylactery" }, () => Monsters.BygoneEffigy(hp: 127),
            "4xStrikeNecrobinder", "4xDefendNecrobinder", "Bodyguard", "Unleash"),
        MakeChar("necroLong/built-vs-Effigy127", "necrobinder", hp: 120, energy: 3, maxTurns: 45,
            new[] { "BoundPhylactery" }, () => Monsters.BygoneEffigy(hp: 127), NecroBuiltDeck),
    };

    // -----------------------------------------------------------------------
    // Elite-monster sweep: NEW ground-truth labels spanning the SINGLE-monster
    // Act-1 elites the base suite doesn't reach, each with a distinct AI shape
    // (stun / life-drain / windup-burst). HP is cut well below the real elite so
    // the exact oracle still solves in seconds — the point is monster-mechanic
    // diversity for the heuristic, not the real HP race. Every fixture here is
    // exact-tractable (so the convergence test can gate them).
    //
    // NOT included: PhrogParasite (its INFECT poison counter is hashed, so any
    // fight long enough to be non-trivial explodes the exact tree past minutes —
    // it has no ground-truth label and is dropped). The MULTI-monster elites
    // (Knights, Decimillipede, SkulkingColony, InfestedPrisms, Entomancer,
    // Gardeners) are likewise exact-intractable and are covered by recorded
    // TRACES instead, not calibration. Surfaced via `--calibrate --elites`.
    // -----------------------------------------------------------------------
    public static IReadOnlyList<Fixture> EliteSweep { get; } = new[]
    {
        // TerrorEel — CRASH/THRASH with a self-stun window; tests timing around a telegraphed big hit.
        Make("elite/TerrorEel", "elite", hp: 48, energy: 3, maxTurns: 10,
            () => Monsters.TerrorEel(hp: 34),
            "Bash", "6xStrikeIronclad", "2xDefendIronclad"),

        // SoulNexus — DRAIN_LIFE heals it, so the policy must out-damage the heal (no stalling). The one
        // already-comfortable exact solve; kept at a meatier HP for a non-trivial label (~92% survival).
        Make("elite/SoulNexus", "elite", hp: 50, energy: 3, maxTurns: 13,
            () => Monsters.SoulNexus(hp: 48),
            "Bash", "6xStrikeIronclad", "2xDefendIronclad"),

        // MechaKnight — WINDUP then a heavy FLAMETHROWER; the block-discipline elite. The rollout leaf badly
        // mismodelled this (read ~1% survival at higher HP) — a short exact-solvable race adjudicates it.
        Make("elite/MechaKnight", "elite", hp: 46, energy: 3, maxTurns: 8,
            () => Monsters.MechaKnight(hp: 28),
            "Bash", "7xStrikeIronclad", "1xDefendIronclad"),
    };

    // -----------------------------------------------------------------------
    // Random-deck generator: the strongest anti-overfit lever. The heuristic
    // was hand-tuned on the six archetype fixtures above, so a deck drawn at
    // random from the curated "calibration-safe" pool is one it has never seen.
    // Deterministic in `seed` (same seed ⇒ same fixtures) so a test over these
    // is reproducible.
    //
    // TRACTABILITY is the binding constraint: exact expectimax must still solve
    // each fight in seconds to give a ground-truth label. The pool is therefore
    // restricted to cards with NO chance node and NO fight-prolonging state —
    // i.e. NO draw (each draw is a hypergeometric chance node that compounds),
    // NO deck-growth (Anger), NO energy ramp (Bloodletting), NO multi-turn power
    // (Demon Form). Combined with low monster HP + a short horizon, a random
    // draw from this pool resolves in ~2–4 turns ⇒ a small, exact-solvable tree.
    // (An earlier draft with draw/power cards at 55 HP / 12 turns blew the exact
    // budget on every deck — the calibration is worthless without a ground-truth
    // label, so the pool buys tractability at the cost of some card variety.)
    // -----------------------------------------------------------------------
    private static readonly string[] SafeAttacks =
        { "StrikeIronclad", "Bash", "Uppercut", "IronWave", "TwinStrike", "Thunderclap",
          "Headbutt", "PerfectedStrike", "BodySlam", "Hemokinesis", "SwordBoomerang" };
    private static readonly string[] SafeSkills = { "DefendIronclad", "FlameBarrier" };

    // CorpseSlug is deliberately ABSENT: its Frail + Strength ramp drags fights out enough that exact blows
    // past tens of seconds even at low HP (it has a tuned home in the aggro/Draw-vs-CorpseSlug base fixture).
    // The four kept here all solve exactly in ≤11s across the generator's HP range ⇒ reliably gradable.
    private static readonly (string Name, Func<int, Monster> Make)[] SafeMonsters =
    {
        ("CalcifiedCultist", hp => Monsters.CalcifiedCultist(hp)),
        ("DampCultist",      hp => Monsters.DampCultist(hp)),
        ("Byrdonis",         hp => Monsters.Byrdonis(hp)),
        ("BygoneEffigy",     hp => Monsters.BygoneEffigy(hp)),
    };

    /// <summary>Generate <paramref name="count"/> deterministic (seeded) random calibration fixtures. Each is a
    /// small deck — guaranteed ≥5 attacks + ≥1 block so it's neither unwinnable nor degenerate — drawn from the
    /// chance-node-free <see cref="SafeAttacks"/>/<see cref="SafeSkills"/> pool, paired with a random low-HP safe
    /// monster on a short horizon so the exact oracle still labels it. Same <paramref name="seed"/> ⇒ identical
    /// fixtures, so a convergence test over these is reproducible. Used by `--calibrate --random N --seed S` and
    /// the random-deck convergence test.</summary>
    public static IReadOnlyList<Fixture> RandomDecks(int count, int seed)
    {
        var rng = new Random(seed);
        var list = new List<Fixture>(count);
        for (int i = 0; i < count; i++)
        {
            var specs = new List<string>();
            int nAttacks = 5 + rng.Next(0, 2);   // 5–6
            int nSkills  = 1 + rng.Next(0, 2);   // 1–2
            for (int a = 0; a < nAttacks; a++) specs.Add(SafeAttacks[rng.Next(SafeAttacks.Length)]);
            for (int s = 0; s < nSkills;  s++) specs.Add(SafeSkills[rng.Next(SafeSkills.Length)]);
            var deck = specs.ToArray();   // captured by value in the closure below

            // Round-robin the monster (offset by seed) so a batch of N≥5 covers all five — random selection
            // clumps for many seeds (one seed put 7/10 on DampCultist), which would starve monster diversity.
            var (mname, make) = SafeMonsters[(i + seed % SafeMonsters.Length) % SafeMonsters.Length];
            int mhpCapt = 24 + rng.Next(0, 9);    // 24–32 (short race ⇒ exact solves in seconds)
            int phpCapt = 40 + rng.Next(0, 11);   // 40–50
            var monster = make;                    // capture

            list.Add(new Fixture($"rand{i:00}/{mname}", "random",
                () => Catalog.SetupCombat(
                    Catalog.BuildPlayer(Deck(deck), phpCapt, phpCapt, 3, new[] { "BurningBlood" }),
                    new[] { monster(mhpCapt) }),
                9));
        }
        return list;
    }

    // -----------------------------------------------------------------------
    // Bridge ladder: one archetype scaled up in DECK SIZE (and monster HP) in
    // rungs that straddle the exact-tractability boundary, so the instrument can
    // see (a) where exact stops giving a ground-truth label and (b) how the
    // 2k-trial advice estimate behaves — bias vs the best available truth and
    // its seed-to-seed noise — as the deck approaches the real Ranwid regime
    // (30–40 cards), where NO exact label exists.
    //
    // The pool is the chance-node-free SafeAttacks/SafeSkills set (no draw / no
    // power / no deck-growth), composed round-robin into a realistic ~70/30
    // attack/block mix. That keeps the SMALL rungs exact-solvable; the LARGE
    // rungs exceed exact and fall back to an MCTS@40k proxy-truth. Same
    // composition at every size ⇒ the only variable across rungs is scale, so a
    // change in the 2k-vs-truth gap is attributable to size, not archetype.
    // Surfaced via `--bridge`.
    // -----------------------------------------------------------------------
    private static readonly string[] BridgePool =
        { "StrikeIronclad", "DefendIronclad", "Bash", "Uppercut", "IronWave", "DefendIronclad",
          "TwinStrike", "Thunderclap", "StrikeIronclad", "Headbutt" };

    /// <summary>The default rung sizes (deck card counts) of the bridge ladder.</summary>
    public static readonly int[] BridgeSizes = { 8, 11, 14, 18, 24, 30 };

    /// <summary>A bridge fixture at one deck size: <paramref name="size"/> cards drawn round-robin from the
    /// chance-node-free <see cref="BridgePool"/>, vs Byrdonis. The RACE LENGTH (monster HP × horizon) — not the
    /// deck size — is what governs exact tractability, so the ladder deliberately scales the race UP with the
    /// deck: the small rungs are short, low-HP races that exact can still label (the ground-truth ANCHOR), the
    /// large rungs reach the real elite HP band + horizon (MCTS-only — the Ranwid regime). An explicit
    /// <paramref name="maxTurns"/> overrides the size-derived horizon.</summary>
    public static Fixture BridgeRung(int size, int? maxTurns = null)
    {
        var specs = new string[size];
        for (int i = 0; i < size; i++) specs[i] = BridgePool[i % BridgePool.Length];
        int monsterHp = (int)(3.0 * size);                 // 6→18 (exact-anchorable) … 30→90 (real elite band)
        int turns = maxTurns ?? Math.Clamp(5 + size / 5, 6, 12);   // 6→6 (short race, exact) … 30→11
        int playerHp = 50 + size;                          // 8→58 … 30→80
        return new Fixture($"bridge/S{size:00}/Byrdonis", "bridge",
            () => Catalog.SetupCombat(
                Catalog.BuildPlayer(Deck(specs), playerHp, playerHp, 3, new[] { "BurningBlood" }),
                new[] { Monsters.Byrdonis(hp: monsterHp) }),
            turns);
    }

    /// <summary>The full bridge ladder at the default rung sizes.</summary>
    public static IReadOnlyList<Fixture> Bridge { get; } =
        BridgeSizes.Select(s => BridgeRung(s)).ToList();

    /// <summary>A LARGE, genuinely CONTESTED fight: a big chance-node-rich deck (so it's the real Ranwid regime,
    /// not exact-tractable) vs a hard-hitting elite at LOW player HP, tuned so survival lands in the uncertain
    /// band rather than a decided 0%/100%. No exact label exists here — the point is to measure the MCTS
    /// estimate's SEED-TO-SEED VARIANCE (which needs no ground truth) where it should be worst, so the advisor's
    /// SurvivalBand can be sized against the real noise floor rather than a winnable-fight one.</summary>
    public static Fixture BridgeContestedLarge(int size = 26, int playerHp = 30, int monsterHp = 74, int maxTurns = 14)
    {
        var specs = new string[size];
        for (int i = 0; i < size; i++) specs[i] = BridgePool[i % BridgePool.Length];
        return new Fixture($"bridge/contested-L{size}/Byrdonis", "bridge",
            () => Catalog.SetupCombat(
                Catalog.BuildPlayer(Deck(specs), playerHp, playerHp, 3, new[] { "BurningBlood" }),
                new[] { Monsters.Byrdonis(hp: monsterHp) }),
            maxTurns);
    }
}
