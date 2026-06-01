# STS2 Solver — Project Status

_Last updated: 2026-06-01 (Next-steps (1)+(2) complete. **HorizonBound v2** — sound bound now covers Weak-bearing decks (max-Weak trajectory), multi-enemy fights (kill-order reasoning), and an admissible in-search early-loss prune (`LossCertificate`: provably-lost decision nodes resolve to their exact `(0, CurrentHp)` value without expansion — value-preserving, e.g. 56k→3 states on a pure-loss fight, all oracle-gated). **Phase-C learned value function** — `LearnedValue`: a compact regression (logistic survival head + linear loss head over 18 features incl. the static heuristic's own estimate) fit to 425k exact labels via `--train-vf`; held-out survival MAE **0.023 vs the static baseline's 0.046**, an opt-in MCTS leaf (`UseLearnedLeaf`). Earlier: survival-first λ-rollout + ObservedWin floor (Δsurv 0.8%); `ranwid` advisor; Ironclad 87/87 + Act-1 elites 12/12; Silent 29/88.)_

## Goal

A **standalone solver** that, given a deck and an encounter, computes the **HP lost under optimal play** in Slay the Spire 2. Optimal play is **lexicographic: maximize win probability first, then minimize expected HP loss**. Combat is stochastic (monster-move RNG, draw order), so the answer is an expectation over outcomes. The solver is a faithful re-implementation of the game's combat engine (ported from decompiled C#), driven by expectimax + MCTS search, and **differentially validated against the real game** via a recorder mod + a headless harness.

---

## Repo layout

```
solver/                         C#/.NET 9 solution
  Sts2Solver.Engine/            faithful combat engine (no Godot/UI)
  Sts2Solver.Content/           ported content, by domain (flat namespace, folders for nav):
    Core/        Catalog.cs (BuildCard aggregates CardTables()) · CommonPowers.cs
                 (Vulnerable/Weak/Frail/Strength/Dexterity/Poison) · StatusCards.cs ·
                 CalibrationFixtures.cs (diverse held-out oracle-calibration set) ·
                 TrainingFixtures.cs (broad HP-swept grid for learned-VF label harvesting)
    Ironclad/    IroncladCards.cs · IroncladPowers.cs · IroncladRelics.cs · IroncladCatalog.cs
    Silent/      SilentCards.cs · SilentPowers.cs · SilentCatalog.cs
    Monsters/    Monsters.cs · MonsterPowers.cs · MonsterCatalog.cs · EncounterCatalog.cs
    Validation/  TraceValidator.cs
                 (adding a character = a <Char>/ folder + one yield in Core CardTables())
  Sts2Solver.Search/            exact lexicographic expectimax + draw enumerator + MCTS:
                                Solver.cs · MctsSolver.cs · DrawEnumerator.cs · EncounterEvaluator.cs
                                (auto-engine entry pt) · PolicyRollout.cs (loss distribution) ·
                                CombatHeuristic.cs (shared intent-aware policy/leaf) · CalibrationHarness.cs ·
                                HorizonBound.cs + DeckProfile.cs + MultiEnemy.cs + LossCertificate.cs (sound
                                horizon v2: Weak / multi-enemy / in-search loss prune) ·
                                LearnedValue.cs + VfTrainer.cs (Phase-C learned leaf + offline trainer)
  Sts2Solver.Ranwid/ (ranwid)   live-run advisor: reads the unmodded current_run.save, benchmarks the deck
                                vs the Act's 3 elites (SaveLocator/RunSave/GameIds/Reporting)
  Sts2Solver.Cli/   (sts2solve) CLI: solve scenarios · --validate traces · --calibrate (MCTS vs exact over
                                CalibrationFixtures; --archetype, --exact-budget) · --horizon (bound probe)
  Sts2Solver.Tests/             xUnit: pipeline + card + solver + trace-replay + MCTS + calibration + horizon
mods/DataDumper/                Godot C# mod (loads into the real game)
  DataDumperCode/               MainFile (entry/metadata) · CombatOracle (records traces) ·
                                AutoPilot (`autopilot` console cmd; energy-gated; ICardSelector prompt hook) ·
                                HeadlessBatch (STS2_BATCH: autonomous headless run+fight+record+quit)
data/
  game_data/*.json              dumped metadata (cards/monsters/encounters/powers/...)
  combat_traces/*.jsonl         66 recorded ground-truth traces (all passing): A0 Ironclad + Silent + A10
docs/mcts-solver-design.md      SOTA literature review + chosen sampling/MCTS design
~/.claude/plans/                cozy-splashing-kernighan.md (approved design + recovered specs);
                                mossy-swimming-allen.md (MCTS-quality plan + session logs)
```

Whole-game decompile (reference, regenerable): `ilspycmd "$DLL" -o /tmp/sts2src -p` (needs `DOTNET_ROLL_FORWARD=LatestMajor`).
DLL: `~/.local/share/Steam/steamapps/common/Slay the Spire 2/data_sts2_linuxbsd_x86_64/sts2.dll`

---

## What works today

### Engine (Sts2Solver.Engine) — validated against the real game
- Exact damage/block pipeline (additive→multiplicative→floor→block→HP); turn lifecycle (energy reset,
  draw 5, end-of-turn hand discard with Ethereal→exhaust, block clear, side switch).
- Power-hook system: `ModifyDamage/Block{Additive,Multiplicative}`, `AfterSideTurn{Start,End}`,
  `AfterApplied`, `AfterCreatureDeath`, `AfterCardPlayed`, `AfterDamageReceived`, `AfterAttackDealt`,
  `TryAbsorbDebuff`, `AfterBlockGained`, `PreventsBlockClear`, `AfterCardExhausted`, `ModifyHpLost`,
  `ModifyCardCost`/`ModifyCardPlayCount`/`ModifyMaxEnergy`, `OverrideResultPileToExhaust`.
- Monster AI = weighted Markov chain (`MonsterMoveStateMachine` + `MoveState` + `RandomBranchState`),
  telegraphed intents, tolerant of injected no-op moves ("STUNNED").
- Multi-hit attacks, multi-monster combat, mid-combat **summoning** (two-phase fights), relic post-combat
  hook, **status cards** (`Unplayable` + `OnTurnEndInHand`, e.g. Infection's 3 self-damage).
- **Clone isolation for self-mutating cards (soundness):** `Player.Clone` shares the immutable card-instance
  majority across search clones (cheap) but **deep-clones any `CardModel.Stateful` card** (one whose identity
  changes mid-fight, e.g. `Rampage`'s escalating damage), and `KeyHash` is not cached for those. Without this
  the exact search tree shared one mutable instance, so a play in one branch corrupted siblings' value (and
  crashed the draw enumerator). Surfaced by the randomized training decks; gated by `TrainingFixturesTests`
  (clone-isolation + Rampage-solves). `Rampage` is currently the only stateful card; the flag makes the rule
  explicit so future ports can't reintroduce the bug.

### Exact solver (Sts2Solver.Search/Solver.cs)
- Lexicographic expectimax: value `(P_win, E[HP loss])`, memoized on a 128-bit structural hash.
- Chance nodes: monster move-roll distribution + exact multivariate-hypergeometric card draws.
- Demo (Ironclad starter vs CalcifiedCultist) solves exactly in ~18s / ~550k states. Fine for slice-sized
  problems; large decks/long fights need MCTS. Stays the **ground-truth oracle** the sampler is validated against.
- `BestAction` (policy extraction) + a cooperative `CancellationToken Ct` (wall-clock budget) — additive.

### Sampling/MCTS solver (Sts2Solver.Search/MctsSolver.cs) — validated to converge to the oracle
- **UCT\* = DP-UCT + limited trial length** (THTS family; `docs/mcts-solver-design.md`).
- **Partial Bellman backups:** each explicated chance outcome weighted by its *true* probability,
  normalized by explicated mass `P^k` (→1) — composes correctly over the transposition DAG.
- **Chance nodes:** enemy-move rolls enumerated exactly; card draws exact below a distinct-count threshold,
  else **Double Progressive Widening** with exact sampled-hand probabilities.
- **Lexicographic value + Lexicographic-UCB** (HP loss normalized by maxHP; no scalarization). DAG-aware.
- **Hybrid** (`HybridExactBelow`): provably-small subtrees defer to the memoized exact oracle.

### Live-run advisor (`ranwid`) — interactive companion + advice engine
- **`Sts2Solver.Ranwid`** (assembly `ranwid`): runs as a **persistent live companion** (default), not a
  one-shot. Loads the ongoing run, prints the deck + per-elite survival/HP-loss stats + the best card to
  remove, then an **interactive prompt** (`Companion`): type the cards a reward screen offers to get a
  **take-vs-skip** verdict against the Act's elites; commands `cuts`/`deck`/`help`/`quit`. A background watcher
  refreshes against the save when you change screens (no restart). `--once` = the old non-interactive one-shot.
  - **Ergonomics:** `CardNameMatcher` resolves typed names separator/case-insensitively with typo tolerance
    (`bludgon`→Bludgeon, `iron wave`→IronWave; prefix→Levenshtein), preserving `+N`; `LineEditor` gives Tab
    auto-complete (plain `ReadLine` fallback when piped). Gated by `CardNameMatcherTests`.
  - **Advice engine** (`Advisor`): `RemovalAdvice` (best card to cut) and `PickAdvice` (take-each-offered vs
    **skip**) rank by a lexicographic `DeckScore` over the Act's elites — primary = WORST (bottleneck) survival,
    secondary = total E[HP loss]. Built entirely on `EncounterEvaluator` (auto exact-or-MCTS) so it scales to
    large run decks where exact is infeasible. **HP-loss-leaning metric:** survival diffs within a `SurvivalBand`
    (0.05) are treated as tied → HP loss decides (survival is still noisy; lower the band toward 0 once
    survival is well-calibrated). Gated by `AdvisorTests` (mechanics + guaranteed-direction checks).
  - No mod needed — the game writes plain-JSON `current_run.save`. `SaveLocator` finds the newest unmodded
    `current*run*.save` (excludes `modded/`; pass `--save` for a modded profile), `RunSaveReader` parses deck
    (`+N` upgrades), relics, HP/energy, ascension, `elite_encounter_ids`. `GameIds` maps `CARD/ENCOUNTER/RELIC.*`.
  - **Reward options are NOT in the save** (only RNG counters; the game regenerates the 3 offered cards from
    RNG on screen-open) — so the user types them (auto-corrected). Confirmed by inspecting a real reward-screen
    save. Limitations (flagged): MP≈single-player; only Burning Blood modelled; enchantments/un-ported skipped.
  - **Validated on a real modded save** (Ironclad starter, Act-1 Byrdonis/PhrogParasite/BygoneEffigy): correctly
    advised *take Bludgeon* (bottleneck survival 0%→41.4%). **Open concern — performance:** ~20–30s per elite at
    40k trials; advice over 3 elites + options + cuts is minutes. See Roadmap (algorithmic, not engineering).
- **`EncounterEvaluator`** (the library interface — all combat math lives here): auto-engine = exact under a
  wall-clock budget (default 8s) else MCTS; returns `CombatStats` (survival, mean/net HP loss,
  min/p10/p50/p90/max, engine, work, ms). **`PolicyRollout`** turns the policy into the HP-loss distribution
  via faithful engine playouts (`ExactMemoPolicy` on the exact path, `HeuristicPolicy` on the MCTS path).
- **`CombatHeuristic`** (the rollout/leaf policy — the core-algorithm lever): one shared **survival-first**
  score used by both MCTS rollouts and the distribution sampler. Reads **post-modifier** telegraphed incoming.
  - **λ-interpolated score** between the block↔damage ends: `Score = λ·race + (1−λ)·survival + overblock`.
    λ=1 all-damage, λ=0 all-block — and since `IncomingDamage` counts only *living* monsters, a lethal blow
    removes incoming, so λ=0 means "block **or kill**" (**killing is blocking**, free). `survival` has a
    death-cliff + per-HP-lost penalty; `race` sped by Strength / enemy Vulnerable. Default rollout = balanced
    **λ=0.5**. Replaced the old race-leaning linear score that under-blocked and threw away winnable fights.
    Weights env-overridable (`STS2_*`) for oracle sweeping.
  - **Ruled out** (don't re-try): per-rollout λ-sampling and K-sample-averaged leaf seeds — no accuracy gain,
    7–15× slower (one seed/leaf makes random-λ pure variance; thin wins need coordinated draw+play). The
    static race-model leaf (`Evaluate`, behind `UseHeuristicLeaf`) is the baseline the Phase-C learned VF beats.
  - **Blended leaf** (`MctsOptions.LeafBlend`, option, not yet default): convex mix `(1−α)·rollout + α·learned`
    at a fresh tip. The rollout UNDER-estimates razor-thin survival, the learned VF OVER-estimates it; truth is
    between. Oracle-measured on block/Byrdonis (exact 22.1%): rollout 18%, learned 38%, **α=0.25 → 24%**
    (survival MAE 0.0075→0.0027). Held as an option pending the wider calibration suite (the current suite has
    only ONE non-trivial fixture). α-sweep lives as a skipped manual tool (`CalibrationHarness.RunMctsBlend`).
- **`ObservedWin` floor:** `MctsSolver` tracks whether any winning line was seen; `EncounterEvaluator` floors
  a backed-up **0.0%** to 0.5% when a win was observed — eliminating the one dangerous output (a false 0% that
  would make a player skip a beatable elite). HP-loss is always reported as computed.
- **Calibration vs the exact oracle** (`CalibrationHarness` + `sts2solve --calibrate` over the diverse
  `CalibrationFixtures` suite — starter/power/debuff/block/aggro/engine, so the heuristic can't overfit one
  style; gated by `CalibrationTests`, 40k trials):

  | fixture | exact (win/loss) | mcts-roll |
  |---|---|---|
  | starter/Cultist | 100% / 0.9 | 100% / 0.9 |
  | power/Inflame-vs-DampCultist | 100% / 0.1 | 100% / 0.1 |
  | debuff/Uppercut-vs-Byrdonis | 100% / 17.3 | 100% / 17.3 |
  | block/Defends-vs-Byrdonis | 22.1% / 39.3 | **17.6%** / 39.5 (was 10.3% pre-fix) |
  | aggro/Draw-vs-CorpseSlug | 100% / 0.0 | 100% / 0.0 |
  | engine/DemonForm-vs-Effigy | 100% / 18.1 | 100% / 18.1 |

  Mean abs error (default rollout leaf) is **Δsurv 0.8% / Δloss 0.03** at 40k trials. **Known residual:**
  razor-thin fights are still *under*-estimated by the greedy rollout (block/Byrdonis 17.6% vs exact 22.1%) —
  acceptable (HP-loss tight, no false 0%); the tree itself is correct (→ exact 22.1% at ~500k trials).

### Phase-C learned value function (`LearnedValue` + `VfTrainer`) — built, beats the baseline
- **What it is:** a compact, self-contained regression that predicts the lexicographic leaf value
  `(P_win, E[HP loss])` of a decision state — a **logistic survival head + linear loss head** over **18
  standardised features** (raw combat geometry + race-margin from one simulated heuristic turn, and crucially
  the static `Evaluate`'s own survival/loss as features, so the model learns a *correction* on top of the
  baseline rather than relearning it). Weights are embedded literals (deterministic, allocation-light); it is
  the MCTS tip evaluator under `MctsOptions.UseLearnedLeaf` (opt-in; the faithful rollout stays the default).
- **Trained** offline by `sts2solve --train-vf`: harvests exact-solver-labelled decision states (via a new
  `Solver.OnSolved` hook) over the `TrainingFixtures` corpus, fits both heads by batch GD + L2. The corpus is
  now **the curated archetype grid PLUS a randomized draw** (`TrainingFixtures.Random`): decks of varying size
  (5–11) and composition sampled from the whole deck-buildable `Catalog.CardPool`, each paired with a random
  monster + HP. Built from a small number of distinct card types per deck (keeps each exact solve tractable)
  with a guaranteed attack (keeps the full survival spectrum), deterministic in seed, every fixture rebuilding
  fresh card instances. So the regression sees a far wider variety of deck types than the archetypes alone.
- **Result — beats the static baseline** (the documented Phase-C goal): held-out survival prediction MAE
  **0.023 (learned) vs 0.046 (static `Evaluate`)** — error halved, on HP combos outside the training grid and
  on the out-of-sample TerrorEel. Gated by `LearnedValueTests`. As an MCTS leaf it trades the rollout's
  razor-thin *under*-estimate for a mild *over*-estimate (block/Byrdonis 38% vs exact 22%) — the safe
  direction for play decisions (never dismisses a beatable fight) — and improves loss (Δloss 0.86 vs 0.96).

### Sound horizon bound v2 (`HorizonBound` + `DeckProfile` + `MultiEnemy` + `LossCertificate`)
- **The base bound** (`HorizonBound.Compute`, the block-deficit idea made rigorous): if by turn T the player's
  HP + the most block the deck could possibly produce can't cover the least damage the enemy could possibly
  deal, the player is dead by T under *any* play, so T is a sufficient horizon (wins precede the death). Built
  from a **min-damage trajectory** (enemy forced to its min move via the real engine — exact for deterministic
  AIs, captures ramp/multi-hit, now bails on mid-fight summons) and a **per-cycle deck-block upper bound**
  (`DeckProfile` probes each card's on-play block via the engine + slack). Only ever *reduces* MaxTurns.
- **v2a — Weak-bearing decks** no longer bail: the trajectory keeps each enemy permanently Weak (×0.75 — the
  most mitigation the player could sustain), which pushes the proven death *later* (a longer, still-sound
  horizon). e.g. `debuff/Uppercut-vs-Byrdonis` now bounds to 12 (was bailing to 40), oracle-equal.
- **v2b — multi-enemy** (`MultiEnemy`, kill-order reasoning): a sound LOWER bound on cumulative incoming over
  undecided lines (≥1 enemy alive) — every non-survivor treated as killed as early as the deck's
  upper-bound damage budget alone could manage, survivor chosen to maximise the saving. e.g. two ramping
  cultists vs a block-less deck → bound 10, oracle-equal (was unconditional bail).
- **v2c — in-search early-loss prune** (`LossCertificate`): per-node, proves a decision state unwinnable
  (player can't deal the enemy's HP before guaranteed death) and returns its **exact** value `(0, CurrentHp)`
  without expansion. Value-preserving because every doomed line dies within the horizon and (heal-free)
  forward loss-to-death = current HP — so the exact solver computes the same, just faster (e.g. **56,315 → 3
  states** on a pure-loss fight; 134k → 98k on block/Byrdonis). Wired into both the exact `Solver` and MCTS.
  `DeckProfile` upper-bounds player damage by probing each card across a battery of state perturbations and
  bailing on anything state-dependent (Body Slam), growth (Strength/Vulnerable) or healing.
- **Gated by `HorizonBoundTests` + `LossPruningTests`**: exact@auto == exact@big (incl. Weak + multi-enemy),
  and pruned-exact == unpruned-exact on win AND loss while provably reducing states explored.

### Ported content
- **Ironclad — 87/87 ✅** (full pool from decompiled `IroncladCardPool`): 75 live-trace-validated, 12
  unit-tested only (RNG card-gen / auto-play / MP-only, where the harness can't replay the effect). Covers
  Powers (Demon Form, Rage, Stone Armor, Barricade, Feel No Pain, Dark Embrace, Corruption, Rupture,
  Juggernaut, …), conditional/scaling attacks (Body Slam, Bully, Ashen Strike, Perfected Strike, Rampage),
  temporary-Strength (Mangle, Setup Strike, Fight Me), block/utility (Impervious, Blood Wall, Colossus, Not
  Yet), mid-combat **draw** (Shrug It Off, Pommel Strike, Battle Trance — ambient `CombatState.Rng` + null-safe
  `Cmd.Draw`), **energy / X-cost** (Bloodletting, Offering, Whirlwind), exhaust-cards (Fiend Fire, Second
  Wind, True Grit, Cinder), generation/movement (Anger, Headbutt, Sword Boomerang, Infernal Blade), and
  per-turn-counter / cost-mod / unblocked-hit-counter cards. Prompt-needing cards (Burning Pact, Brand,
  Headbutt, Armaments) validated via the `ICardSelector` hook + upgrade-level recording.
- **Silent — 40/88**: batches 1–2 LIVE-VALIDATED ✅ (poison core, block/dex/debuff, Shiv/attack); batch 3
  unit-tested (Survivor, Backstab, DaggerThrow, Predator, BouncingFlask, Caltrops/Thorns, GrandFinale, Skewer,
  Adrenaline, Backflip, Expertise + `DrawNextTurnPower`). Deferred (engine-level): hand-discard selection
  (Acrobatics/CalculatedGamble), draw-conditional (EscapePlan), next-Skill-double (Burst), cost-set-on-hand
  (BulletTime), Intangible (WraithForm), and the renamed/new STS2 "blade/ink" set (unverifiable LocStrings).
- **Colorless — 18 ported (new module)** `Content/Colorless/`: 13 full-effect (FlashOfSteel, DramaticEntrance,
  MindBlast, HandOfGreed, Clash, Finesse, DarkShackles, MasterOfStrategy, ThinkingAhead, Impatience,
  PanicButton, + Powers Panache/TheBomb) and 5 documented HP-neutral subsets (Mayhem, Apotheosis,
  Metamorphosis, Enlightenment, Purity). New powers DarkShackles/`NoBlock` + the **Stateful** `Panache`/`TheBomb`
  (counter in `StateKey`/`HashValue`/`Clone`). Unit-tested. (`Expertise` is Silent, not Colorless.)
- **Monsters / Act-1 elites — 12/12 ported + validated ✅:** Byrdonis, BygoneEffigy, PhrogParasite (+4
  Wrigglers on death), TerrorEel (Shriek→Terror), SoulNexus (RandomBranch), MechaKnight (Artifact),
  Entomancer (Personal Hive / Dazed flood), SkulkingColony (HardenedShell cap), InfestedPrism (Vital Spark),
  PhantasmalGardeners (×4, Skittish), Knights (×3 distinct AIs), Decimillipede (segments — Reattach revival
  un-modelled, HP-neutral on traces). Plus normal-tier Cultists / Corpse Slug. **Powers:** the keyword set +
  Ritual/Ravenous/Territorial/Slow/Infested/Vigor/Shriek/Artifact/PersonalHive/HardenedShell/VitalSpark/
  Tainted/Skittish, and inert markers (Hex/Dampen/Reattach).
- **Monsters / Act-1 normal encounters — +8 monsters, +9 encounters (unit-tested only, NOT trace-validated):**
  SnappingJaxfruit, Flyconid, CubexConstruct, FuzzyWurmCrawler, ShrinkerBeetle, Mawler, Nibbit, Inklet, with
  encounter builders (NibbitsNormal, InkletsNormal, OvergrowthCrawlers, …). New powers `ShrinkPower` (×0.7
  powered-attack) + `SlipperyPower` (Intangible-like HP-loss-to-1 cap). New **normal-encounter API**
  (`BuildEncounter`/`IsKnownNormalEncounter`) alongside the elite API. Deferred: VineShambler (cost-raise hook),
  Slime monsters (Slimed status card), Fogmog (illusion summons), RubyRaiders (5-type random), Act-1 bosses.
- **Relic:** BurningBlood (heal 6 on victory) — the only modelled relic (the relic hook system has only
  `AfterCombatVictory`; combat-relevant relics need new engine hooks — see Roadmap).
- **Ascension:** combat-relevant levels are `ToughEnemies` (+HP, ≥A8) and `DeadlyEnemies` (+damage, ≥A9);
  all monsters scale via `Asc.Tough/Deadly` with exact decompiled constants. `BuildMonster(name, asc=10)`
  defaults A10. A5 `AscendersBane` curse modelled (Unplayable+Ethereal).

### Validation status
- **xUnit tests all green** (pipeline + per-card Ironclad/Silent/Colorless + monster-port + solver +
  trace-replay + MCTS-convergence + calibration + horizon-bound v2 + loss-pruning oracle-equality +
  learned-VF beats-baseline + clone-isolation/Rampage soundness + randomized-corpus sanity + advisor +
  card-name matcher). Last confirmed full run **254 green**; +14 since (advisor + matcher) → full re-run after
  the latest merge in progress. Run `dotnet test` to confirm.
- **66 recorded game traces — all PASS, 0 skips, 0 fails** (manual + console-autopilot + headless), incl.
  multi-turn elite fights for every Act-1 elite (Byrdonis ramp, Effigy Slow+Wake, PhrogParasite death-burst,
  TerrorEel Shriek→Terror, SoulNexus randoms, MechaKnight Artifact+Burn, Entomancer Hive, SkulkingColony cap,
  InfestedPrism Vital Spark, PhantasmalGardeners ×4, Knights ×3, Decimillipede segments).

### Tooling — fully autonomous data collection ✅
- `DataDumper` mod records every combat to `data/combat_traces/`; `autopilot` console cmd plays hands-free
  (real plays, energy-gated, answers selection prompts via the model-layer `ICardSelector` hook).
- **Headless batch**: one command boots the game with no display, starts an Ironclad run, enters an
  encounter, auto-plays, records, quits — no human in the loop. **Custom-deck harness** (`STS2_DECK`)
  replaces the run deck after `CreateForNewRun` so any ported cards reach a real fight for validation.

---

## How to run

```bash
# Build + test
cd solver/Sts2Solver.Tests && dotnet test -c Release

# Exact solve the demo / a scenario file ({playerHp, deck|deckPreset, relics, monsters, maxTurns, ascension})
dotnet run -c Release --project solver/Sts2Solver.Cli -- --demo --lines
dotnet run -c Release --project solver/Sts2Solver.Cli -- scenario.json

# MCTS (fights beyond exact search): --mcts [--trials N] [--anytime] [--hybrid N]
dotnet run -c Release --project solver/Sts2Solver.Cli -- scenario.json --mcts --trials 200000 --anytime

# Calibrate MCTS vs exact over the diverse suite (now incl. an mcts-learn row); probe the horizon bound
dotnet run -c Release --project solver/Sts2Solver.Cli -- --calibrate [--archetype block] [--exact-budget 75]
dotnet run -c Release --project solver/Sts2Solver.Cli -- --horizon

# (Re)train the Phase-C learned value function: harvest exact labels over TrainingFixtures, fit, emit weights
# to /tmp/vf-weights.txt (paste into LearnedValue.Weights). Flags: --budget-seconds --epochs --maxturns --sample-rate
dotnet run -c Release --project solver/Sts2Solver.Cli -- --train-vf --budget-seconds 6 --epochs 4000 --maxturns 14

# Live-run advisor — interactive companion (default): deck+elite stats, best cut, then a prompt where you
# type a reward screen's offered cards (Tab completes, typos auto-correct) for a take-vs-skip verdict.
dotnet run -c Release --project solver/Sts2Solver.Ranwid                       # watch newest unmodded run
dotnet run -c Release --project solver/Sts2Solver.Ranwid -- --save <current_run.save>   # e.g. a modded profile
dotnet run -c Release --project solver/Sts2Solver.Ranwid -- --once --rewards Bludgeon,Inflame,Whirlwind  # scriptable one-shot

# Validate the engine against all recorded traces
dotnet run -c Release --project solver/Sts2Solver.Cli -- --validate

# Rebuild + deploy the mod (post-build copies into the Steam mods folder)
cd mods/DataDumper && dotnet build -c Debug

# Autonomous headless trace collection (game MUST be closed; Steam running):
GAME="$HOME/.local/share/Steam/steamapps/common/Slay the Spire 2"
cd "$GAME" && STS2_BATCH=BYRDONIS_ELITE STS2_TRACE_DIR="$HOME/Projects/sts2-solver/data/combat_traces" \
  ./SlayTheSpire2 --headless
#   STS2_BATCH: CULTISTS_NORMAL, CORPSE_SLUGS_*, BYRDONIS_ELITE, BYGONE_EFFIGY_ELITE, PHROG_PARASITE_ELITE, …
#   optional: STS2_BATCH_KILL=1 · STS2_SEED=… · STS2_ASCENSION (default 10) · STS2_DECK="StrikeIronclad:3,…"
```

Card-validation loop: port + unit-test → `STS2_DECK` headless run vs a simple encounter → `--validate` green
→ promote the trace into `data/combat_traces/`.

---

## Operational gotchas (IMPORTANT)

- **Mods get disabled in settings sometimes.** `~/.local/share/SlayTheSpire2/steam/<id>/settings.save` →
  `mod_settings.mod_list[].is_enabled` — **BaseLib AND DataDumper must both be true**, plus `mods_enabled:
  true`. A failed/early headless boot can reset these. Re-enable before runs.
- **`steam_appid.txt`** (contents `2868840`) in the game install dir — required to boot `--headless` outside
  Steam. Harmless for normal launches; reversible.
- **Headless requires:** Steam client running, the game NOT already open, and the real `HOME` (Steam
  discovery needs `~/.steam`). The batch uses `shouldSave:false` and backs up `settings.save`
  (`.pre-headless.bak`). Startup logs a harmless "leaked at exit" + `ObjectDisposedException`.
- Shell aliases `grep`→`ugrep` and `ls`→`eza` (different escaping). Use `/usr/bin/grep`, `/usr/bin/ls`.
- `ilspycmd` needs `DOTNET_ROLL_FORWARD=LatestMajor` (targets .NET6, only 9 installed).
- `dotnet` working dir doesn't persist across Bash calls here — use absolute project paths.

---

## Key design decisions

- **Objective:** survival-first, then min expected HP loss (lexicographic). **Under review** — survival is the
  noisy component; we may collapse to a single HP-loss scalar (death = full HP loss already encodes survival).
  See Roadmap "drop survival probability".
- **Exact solve is a small-deck tool, not the engine.** It stays the ground-truth ORACLE for gating, but real
  decks (40+ cards) are intractable exactly — the MCTS+learned-VF path carries late-game, so algorithmic
  efficiency + VF calibration are where accuracy now comes from (not deeper exact search). Validation must
  therefore lean on trace outcomes + deck-variety + self-consistency, not only exact-equality.
- **Engine in C#** to mirror decompiled source 1:1 (lowest fidelity risk).
- **Card clone isolation:** `CardModel.Stateful` cards (mutable per-combat state, e.g. Rampage) are deep-cloned
  per search state; the immutable majority are shared. Forgetting `Stateful` on a future self-mutating card
  re-introduces a silent oracle-unsoundness — gate new such cards.
- **Validate, don't trust:** every ported card/power/monster is confirmed by diffing a real game trace;
  every MCTS / heuristic / horizon change is gated against the exact oracle.
- Monsters are **stochastic-but-known, not adversarial** ⇒ MDP/expectimax, not minimax.
- Determinization/PIMC/ISMCTS was considered and rejected (its pathologies need hidden information, which we
  don't have). Full rationale + citations in `docs/mcts-solver-design.md`.

---

## Next steps

1. ✅ **Learned value function (Phase C)** — DONE. `LearnedValue` (logistic+linear heads, 18 features incl.
   the static baseline's own estimate) fit to 425k exact labels via `--train-vf`; held-out survival MAE
   0.023 vs the static `Evaluate` baseline's 0.046 (gated by `LearnedValueTests`). _Follow-ups:_ as an MCTS
   leaf it now over-estimates razor-thin survival (block/Byrdonis 38% vs exact 22%) where the rollout
   under-estimated — recalibrate (Platt/isotonic on a held-out set) or blend leaf+rollout; widen the training
   grid beyond Ironclad block/aggro/Byrdonis-shaped fights; add deck-composition features.
2. ✅ **Horizon bound v2** — DONE. Weak-bearing decks (max-Weak trajectory), multi-enemy (`MultiEnemy`
   kill-order), and the admissible in-search loss prune (`LossCertificate`, value-preserving) — all
   oracle-gated. _Follow-ups:_ tighten the multi-enemy saving bound (shared-budget kill scheduling rather
   than per-enemy dedicated budget); extend the loss prune past the single-deterministic-enemy / no-growth /
   heal-free deck class; an admissible *win* certificate (prune provably-won subtrees too).
3. ✅ **Advice engine + live companion** — DONE. `ranwid` is an interactive companion (`Companion`): deck +
   per-elite stats, best card to remove (`Advisor.RemovalAdvice`), and reward take-vs-skip (`PickAdvice`) over
   the Act's elites; ergonomic card entry (`CardNameMatcher` auto-correct + `LineEditor` Tab complete); HP-loss-
   leaning metric (`SurvivalBand`). Validated on a real save. _Follow-ups:_ more relics (needs engine hooks);
   Act-1 bosses; potions.

### ⭐ TOP PRIORITY — performance is the gate to late-game usability
The advisor calls `EncounterEvaluator` per deck-variant per elite; at ~20–30s/elite (40k trials) even the
9-card STARTER is too slow, and real decks reach **40+ cards** (hands of 10 with many distinct cards → huge
per-decision branching). **Pursue an ALGORITHMIC fix, not an engineering one** (caching/parallelism are a
band-aid). Candidate directions from stochastic-control / planning literature to evaluate against the oracle:
   - **Action-dimension progressive widening + heuristic priors (PUCT).** Today decision nodes expand EVERY
     legal play (`SelectEdge` gives unvisited edges +∞ → must visit all once); only chance nodes use DPW. Bias
     selection by a policy prior (from `CombatHeuristic`/the VF) and widen actions progressively → focuses
     trials on good plays, the dominant branching cost for big hands. Add First-Play-Urgency too.
   - **Sparse sampling / forward-search sparse sampling (Kearns–Mansour–Ng)** — bounded-width sampling with a
     value-function bootstrap; sample complexity ~independent of state-space size. Pairs with the learned VF.
   - **Fitted value iteration / ADP with the learned VF as the approximator** — let the VF carry more of the
     load (shallow search or near-greedy on the VF) once it's well-calibrated, instead of deep sampling.
   - **Action abstraction** — dedupe symmetric plays (identical cards, target symmetry) more aggressively.

### ⭐ OPEN QUESTION — drop survival probability, optimize HP-loss only?
Survival prob is the noisy, hard-to-calibrate component (MCTS under-/over-shoots razor-thin fights; PhrogParasite
shows 0% with the starter); HP loss tracks the oracle tightly. **Seriously evaluate collapsing to a single
scalar objective: minimize E[HP loss] where death costs your full remaining HP.** Death is already the maximal
loss, so a scalar HP-loss objective *implicitly* encodes survival pressure — likely capturing most of the
decision quality while removing the lexicographic machinery (single-component UCB/backups, one VF head, no
death-cliff tuning). Measure the **complexity + runtime payoff** and the decision-quality delta vs the current
lexicographic objective on the calibration suite. (This revisits Key-design #1 — keep it as an experiment
behind a flag; only flip the default if it's clearly a win.)

### Other near-term
4. **Calibration expansion** — cover all 12 elites + a high variety of decks (random-draw generator), with
   exact ground truth where tractable and MCTS self-consistency / anytime-convergence where (as decks grow)
   exact can't reach. This is the measuring stick for both the perf and objective experiments, and gates making
   the **leaf-blend (α≈0.25) the default**.
5. **VF distillation + recalibration** — after a large training round (now drawing colorless + varied decks),
   distill feature importance to simplify the model without losing accuracy; recalibrate survival (Platt/
   isotonic) IF we keep it; add deck-composition features.
6. **Content** — Silent 40 → 88; trace-validate the 8 new normal monsters; Act-1 bosses; relic engine hooks;
   the deferred solver-side mid-turn **draw chance-node** for forward search.

### History (condensed)
Milestones complete: engine + exact solver + CLI + oracle/autopilot + headless autonomy; MCTS solver
(UCT\*/DP-UCT, validated to converge); Act-1 elites 12/12; Ironclad 87/87; Ascension/A10; content
modularization (per-character folders, flat namespace); the `ranwid` advisor + survival-first rollout +
sound horizon bound; **horizon bound v2** (Weak / multi-enemy / in-search loss prune) + **Phase-C learned
value function**. In progress: Silent pool (29/88). Detailed per-batch/per-milestone history lives in git and
the plan files under `~/.claude/plans/`.
