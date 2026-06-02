# STS2 Solver — Project Status

_Last updated: 2026-06-02._

## Current state

- **Content — all four characters complete (88/88 each):** Ironclad, Silent, Regent (Stars + Forge/Sovereign
  Blade), Necrobinder (Osty pet + Doom) — plus Colorless (18), the Event/Ancient "Special" pool (24) and
  curses (18). Act-1 elites 12/12 and the normal-monster set, trace-validated against live game recordings.
- **Search:** the exact lexicographic expectimax `Solver` is the ground-truth **oracle** (maximize survival,
  then minimize E[HP loss]); the sampling `MctsSolver` (UCT*/DP-UCT family) is validated to converge to it.
  Production MCTS defaults: action progressive widening + lexicographic **PUCT** (`STS2_APW=0` disables), **lazy
  chance-node enumeration**, the greedy-rollout leaf, and a ~2 000-trial advisor budget. A 30-card-vs-elite
  solve runs in ~1.5–2.7 s; survival is exact at that budget, E[HP loss] ~3 HP pessimistic (accepted tradeoff).
- **Sound horizon + pruning:** `HorizonBound` (Weak / multi-enemy) + `LossCertificate` (admissible early-loss
  prune) — both oracle value-preserving. **Phase-C learned value function** (`LearnedValue` + `--train-vf`) is
  an opt-in MCTS leaf for the razor-thin survival regime.
- **Advisor:** `ranwid` live companion — reads the unmodded save, benchmarks the deck vs the Act's elites, and
  recommends card removals and reward take/skip.
- **Settled questions** (rationale in git history / Roadmap): the lexicographic objective is correct (don't
  collapse survival into a single HP scalar); the late-game perf bottleneck was chance-node enumeration, not
  cloning; the rollout leaf is the most accurate (closed-form / blended leaves are biased).

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
                                CalibrationFixtures) · perf diagnostics (--profile, --converge, --perf-probe) ·
                                --train-vf (fit the learned value function)
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
- **Mid-turn card draw** (Shrug It Off, Pommel Strike, Battle Trance, every Silent draw/cycle effect): real
  with a concrete `Rng` (rollouts / trace replay), and in SEARCH mode deferred onto `CombatState.PendingDraw`
  so the solver resolves it as an explicit draw chance node continuing the same turn — the drawn hand and any
  play on it become real in the exact tree and in MCTS (see Search; Next steps #1).
- **Post-draw resolution** (draw-then-act cards): after a deferred draw resolves, `CombatManager.ApplyPostDraw`
  runs the card's post-draw step per outcome — a deterministic/conditional `CardModel.OnPostDraw` (EscapePlan
  gains Block iff the drawn card is a Skill) or a discard-of-choice (`PostDrawDiscardCount` → `PendingDiscard`,
  a hashed decision resolved as a player MAX one card at a time). Promoted EscapePlan/Acrobatics/Prepared
  (Next steps #2).
- **Forced discard-of-choice (no draw):** a card that forces a discard with no preceding draw (Survivor: "gain
  Block, discard 1") sets `PendingDiscard` directly at play time (`Cmd.DeferDiscardOfChoice`), resolved as the
  same player MAX. Modelling the forced discard (vs omitting it) is the SOUND direction — omitting a *forced*
  discard would leave the player an extra in-turn card the real game removes (optimistic in non-discard-synergy
  decks). Promoted Survivor (no-draw) and DaggerThrow (post-draw) — both surfaced by the selection audit (Next
  steps #3) as the only places a forced discard was dropped rather than defaulted/maxed. The discard feeds
  `CardsDiscardedThisTurn` (Memento Mori), so it has a real within-turn HP effect — gated decisively in
  `MidTurnDrawTests` (a MementoMori-lethal-only-with-the-discard solve) + MCTS-converges + Rng-path mechanics.
- **Per-turn play cap (loop termination):** a cost-0 replayable draw cantrip (EscapePlan/Prepared — free play
  that draws and recirculates via reshuffle) could otherwise build an unbounded per-turn play chain and blow the
  search stack. `PlaysThisTurn` is capped **unconditionally** at `MaxPlaysPerTurn` (40, far above any real
  line ⇒ value-preserving — a hard safety net against any such card, flagged or not). Decks holding a
  `CardModel.LoopRiskDraw` card additionally set `CombatState.BoundsPlays`, which **hashes** the counter so the
  cap memoises soundly; cost-≥1 draws are energy-bounded, so the common deck is never flagged/fragmented. The
  flagged set (audited across all characters — cost-0, returns-to-discard, draws): EscapePlan, Prepared,
  FlashOfSteel, Finesse, Impatience, BrightestFlame. (Power-type draws like Neurosurge use the Removed pile ⇒
  can't be replayed ⇒ not flagged; exhaust / NoDraw-self-limiting draws are likewise safe.) Such cantrip decks
  have a large exact state space (exact may time out → MCTS carries them, as designed).
- **Clone isolation for self-mutating cards (soundness):** `Player.Clone` shares the immutable card-instance
  majority across search clones (cheap) but **deep-clones any `CardModel.Stateful` card** (one whose identity
  changes mid-fight, e.g. `Rampage`'s escalating damage), and `KeyHash` is not cached for those. Without this
  the exact search tree shared one mutable instance, so a play in one branch corrupted siblings' value (and
  crashed the draw enumerator). Surfaced by the randomized training decks; gated by `TrainingFixturesTests`
  (clone-isolation + Rampage-solves). `Rampage` is currently the only stateful card; the flag makes the rule
  explicit so future ports can't reintroduce the bug.
  - **Sibling-mutation via card upgrades (same bug class, second mechanism):** a card whose effect upgrades
    *other* cards in the piles (`Apotheosis` — all piles; `Armaments` — hand) must NOT bump a shared card's
    `Upgrades` in place, since that changes the shared instance's `StateKey` in every sibling branch (the draw
    enumerator then crashes with `Pile missing card …`, e.g. an Impatience or `Armaments+1` deck). Both now
    **replace** each upgradable card with a private, freshly-cloned upgraded copy the playing state alone owns.
    Gated by `TrainingFixturesTests` (`Upgrade_Effects_Do_Not_Mutate_Shared_Card_Instances`,
    `Apotheosis_Impatience_Deck_Solves_Exactly`, `Corpus_Upgrade_Card_Fixtures_Solve_Without_Crashing`).

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
  else **Double Progressive Widening** with exact sampled-hand probabilities. **Mid-turn draws** (Shrug It Off,
  Pommel Strike, every Silent draw/cycle effect) open a dedicated `DrawNode` chance node — a play that defers a
  draw (`PendingDraw`) routes its edge through it, resolving the drawn hand (same exact/DPW split, no enemy turn
  / no HP-loss term) and continuing the same player turn; rollout leaves draw for real (`s.Rng`). Converges
  byte-identically to the exact oracle. **Post-draw steps** (EscapePlan Block / Acrobatics-Prepared
  discard-of-choice) resolve on each draw outcome — the discard-of-choice as a `PendingDiscard` decision layer
  (player MAX, gated to converge to the exact inline MAX).
- **Lexicographic value + Lexicographic-UCB** (HP loss normalized by maxHP; no scalarization). DAG-aware.
- **Hybrid** (`HybridExactBelow`): provably-small subtrees defer to the memoized exact oracle.
- **Action progressive widening + lexicographic PUCT** (opt-in `ActionWidening`, default OFF): ranks plays by a
  softmax heuristic policy prior, opens only ⌈C·N^β⌉ best-first, selects by PUCT. Measured a strict Pareto win
  vs the oracle (faster *and* more accurate; see Roadmap TOP PRIORITY). Gated by `MctsTests.Apw_Converges_*`.

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
  | block/Defends-vs-Byrdonis | 92.6% / 35.8 | 92.6% / 35.8 |
  | aggro/Draw-vs-CorpseSlug | 100% / 0.0 | 100% / 0.0 |
  | engine/DemonForm-vs-Effigy | 100% / 18.1 | 100% / 18.1 |

  Mean abs error (default rollout leaf) is **Δsurv 0.8% / Δloss 0.03** at 40k trials; every row above now
  tracks at Δ 0.0%. (block/Defends jumped 22.1% → **92.6%** once ShrugItOff's mid-turn draw became real in
  search — the extra draw cycles the 2 Strikes fast enough to actually kill Byrdonis; the deck is unwinnable
  without it. Exact and MCTS agree byte-identically there; the only gap left is the documented draw-ORDER
  approximation vs a real shuffle — see Next steps #1.)

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
- **Regent — 88/88 ported (new module)** `Content/Regent/` (unit-tested, NOT yet trace-validated): the full
  decompiled `RegentCardPool`, plus the **Stars** secondary resource and the **Forge → Sovereign Blade** engine.
  New engine support: `Player.Stars` (clone/hash/key), `CardModel.StarCost`/`IsXStarCost`/`Retain`, star-cost
  gating in the 3 solver play-enumeration sites + payment in `PlayCard`, `Cmd.GainStars`, power hooks
  `AfterStarsGained`/`AfterStarsSpent`/`AfterEnergySpent`/`ModifyStarCost`, per-turn counters
  (`SkillsPlayedThisTurn`/`StarsGainedThisTurn` for Lunar Blast / Radiate), a `RelicModel.OnCombatStart` hook,
  and `CardRarity.{Event,Ancient,Token}`. The **Sovereign Blade** is a Stateful, Retained Token whose forged
  damage accumulates over the fight and which reads Parry (block-on-play), Seeking Edge (hits all), Sword Sage
  (extra replays) and Conqueror (×2 vs the marked enemy). Faithful HP-relevant powers: Genesis/Furnace/Orbit,
  Child of the Stars / Black Hole (star payback), Energy/Star-Next-Turn, Monologue, Monarch's Gaze, the
  Crush Under / Dying Star temp-Strength debuffs, NeutronAegis (reuses Plating). Starter relic **DivineRight**
  (3 Stars at combat start). Documented HP-neutral / inert (depend on unported subsystems): RNG card generation
  (Begone, Bundle of Joy, Charge, Guards, Quasar, Collision Course/Crash Landing Debris, Arsenal/Pillar/
  Supermassive generation-scaling), hand-draw-COUNT changes (Tyranny, Pale Blue Dot, Spectrum Shift, Foregone
  Conclusion, next-turn extra draw), gold (Royalties), Reflect's thorns-on-block, Void Form's cost discount,
  on-draw card hooks (Kingly Kick/Punch), auto-play-from-pile (Bombardment, I Am Invincible), and the
  multiplayer-only Largesse/HammerTime. 41 new unit tests.
- **Silent — 88/88 ✅ (full pool)**: batches 1–3 LIVE-VALIDATED/unit-tested as before (poison core,
  block/dex/debuff, Shiv/attack, Survivor/Backstab/DaggerThrow/Predator/BouncingFlask/Caltrops/GrandFinale/
  Skewer/Adrenaline/Backflip/Expertise). **This round (batches 4–8, +50 cards, unit-tested):**
  primitive attacks/skills/powers (Abrasive, Assassinate, Expose, LeadingStrike, Malaise [X-cost], Pounce
  [FreeSkill], Reflex, Ricochet, Tactician, Untouchable, StormOfSteel, CalculatedGamble); the
  Shiv/poison-synergy power set (Accelerant [Poison already reads it], Accuracy, Anticipate [temp Dex],
  Strangle, InfiniteBlades, PhantomBlades, Outbreak, SerpentForm, Tracking, Burst, Shadowmeld, FanOfKnives,
  ShadowStep [+DoubleDamage], BladeOfInk [Inky shivs: +1 dmg/+Weak], and MasterPlanner/WellLaidPlans/Sneaky/
  Flanking — MP-only / Sly / Retain, inert in single-player); counter/conditional attacks (Finisher,
  MementoMori, PreciseCut, Mirage, EchoingSlash) on new per-turn `CombatState` counters (attacks-played /
  cards-discarded, in the key+hash); WraithForm (new `IntangiblePower` caps HP-loss to 1); BulletTime
  (cost-zero-this-turn) + UpMySleeve (a **Stateful** self-cost-reducing card); and the **formerly-deferred
  set, now ported to the project's "real-with-a-driver / degrade-in-pure-search" bar** (batch 8): KnifeTrap
  (deterministic exhaust replay — exact), Nightmare (`NightmarePower` adds 3 copies of a default-chosen card
  next turn), Acrobatics/Prepared/HiddenDaggers/ToolsOfTheTrade (default card SELECTION à la Armaments/Burning
  Pact), EscapePlan (conditional on the drawn card), CorrosiveWave/Speedster (new **AfterCardDrawn** mid-turn-
  draw hook), and Murder (scales on cumulative cards drawn — a gated `TracksCardsDrawn`/`CardsDrawnThisCombat`
  counter so only Murder decks pay the state-key cost). New powers in `SilentPowers.cs`; `SilentCardTests`
  49→110. (Selection/draw effects use deterministic defaults / no-op without an ambient Rng, matching the
  existing Ironclad prompt/draw cards.)
- **Soundness fix (this round): in-pile card upgrades** (`Armaments`, `Apotheosis`) now **replace** the
  upgraded card with a freshly-cloned upgraded copy instead of mutating the shared (non-Stateful) instance in
  place — the latter corrupted sibling search branches (draw enumerator: "Pile missing card …"). A latent
  pre-existing bug (the STATUS-claimed fix was never actually in the code) surfaced by the wider random corpus;
  `CardModel.Upgraded` now also drops its cached `KeyHash`. Gated by `Upgrade_In_Pile_Does_Not_Corrupt_Shared_Instances`.
- **Soundness fix (merge-time): Regent counters missing from the memo key.** The per-turn counters
  `SkillsPlayedThisTurn`/`StarsGainedThisTurn` (added by the Regent port) were in `StateKey()` but NOT in
  `HashKey()` — the sole 128-bit FNV memo key, no string fallback. Two states differing only in those collided
  → a stale memoised exact-oracle value for Lunar Blast / Radiate decks. Added them to `HashKey` (unconditional,
  so field position is fixed). Latent because Lunar Blast/Radiate are tested via direct plays, not memoised search.
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
- **Event/Ancient special pool — 24 ported (new module `Content/Special/`):** 14 Event + 10 Ancient cards
  registered in `SpecialCardFactories` (deck-buildable, in `CardPool`). Full-effect: ByrdSwoop, Exterminate,
  Peck, Squash, RipAndTear (random-target approximated), Entrench (double block), Stack (block=discard count),
  Outmaneuver/Relax (`EnergyNextTurnPower`), FeedingFrenzy (`FeedingFrenzyPower`: one-turn temp Strength),
  Rebound (+inert `ReboundPower`), ToricToughness (`ToricToughnessPower`: re-grants the stored block 2 turns),
  MeteorShower (AoE dmg+Weak+Vuln), Maul (Rampage-style self-escalation, Stateful), NeowsFury, Whistle
  (dmg only — stun deferred), BrightestFlame (`Creature.LoseMaxHp`), Apparition/WraithForm (new
  `IntangiblePower` HP-loss-to-1 cap + `WraithFormPower` dex drain). Inert markers (no combat-HP effect):
  HelloWorld/Distraction/DualWield (RNG/selection card-gen), ForbiddenGrimoire/TheSealedThrone (meta
  reward/Stars). **Deferred — need an unported subsystem:** MadScience (variable TinkerTime card),
  BiasedCognition/Quadcast (Defect orbs/Focus), Protector (Osty pet). Gated by `SpecialAndCurseCardTests`.
- **Curses — 18 ported (`Content/Core/Curses.cs`):** registered in `CommonCardFactories` (buildable by name,
  excluded from the deck-buildable `CardPool`, like status cards). Combat-acting via `OnTurnEndInHand`:
  BadLuck (13 unblockable self-dmg), Decay (2 blockable), Regret (dmg = hand size), Doubt/Shame (self
  Weak/Frail that survive the apply-turn tick via the new base-power `SkipNextTick`, mirroring the game's
  `SkipNextDurationTick`). Pure draw-dilution (Unplayable, no effect): Clumsy/Folly (Ethereal), CurseOfTheBell,
  Greed, Injury, PoorSleep, Writhe, Guilty, Debt, Normality. Playable no-ops: SporeMind (cost-1 exhaust),
  Enthralled (cost-2). **Documented-deferred restrictions:** Enthralled's hand-lockout and Normality's
  3-cards/turn cap need a move-legality hook the search lacks, so they degrade to dilution (harm under-stated).
  (AscendersBane stays in Core/StatusCards.cs.) Engine: added `CardRarity.Event/Ancient` + `Creature.LoseMaxHp`.
- **Relic:** BurningBlood (heal 6 on victory) — the only modelled relic (the relic hook system has only
  `AfterCombatVictory`; combat-relevant relics need new engine hooks — see Roadmap).
- **Ascension:** combat-relevant levels are `ToughEnemies` (+HP, ≥A8) and `DeadlyEnemies` (+damage, ≥A9);
  all monsters scale via `Asc.Tough/Deadly` with exact decompiled constants. `BuildMonster(name, asc=10)`
  defaults A10. A5 `AscendersBane` curse modelled (Unplayable+Ethereal).

### Validation status
- **xUnit tests all green** (pipeline + per-card Ironclad/Silent/Colorless + monster-port + solver +
  trace-replay + MCTS-convergence + calibration + horizon-bound v2 + loss-pruning oracle-equality +
  learned-VF beats-baseline + clone-isolation/Rampage soundness + randomized-corpus sanity + advisor +
  card-name matcher + Event/Ancient/curse ports + per-card Regent (Stars / Forge / Sovereign Blade /
  star-payback) + per-card Silent 88/88 (incl. the batch-4–8 powers, per-turn counters, AfterCardDrawn hook,
  and the in-pile-upgrade soundness gate) + mid-turn-draw chance nodes + post-draw resolution (`MidTurnDrawTests`:
  deterministic lethal-draw solve, no-draw control, draw-makes-fight-winnable + exact↔faithful-MC bound,
  EscapePlan conditional, Acrobatics optimal-discard, discard-choice MCTS convergence, cost-0 cantrip
  cap, loop-risk-card flag audit, Survivor/DaggerThrow forced-discard-feeds-the-counter + no-draw-discard MCTS
  convergence + Rng-path discard mechanics)). **503 passed, 0 skipped, 0 failed.**
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

# Calibrate MCTS vs exact over the diverse suite (incl. an mcts-learn row)
dotnet run -c Release --project solver/Sts2Solver.Cli -- --calibrate [--archetype block] [--exact-budget 75]

# Perf diagnostics: clone/leaf/chance attribution + 30-card self-convergence vs the 1–2 s target
dotnet run -c Release --project solver/Sts2Solver.Cli -- --profile [--trials 1000]
dotnet run -c Release --project solver/Sts2Solver.Cli -- --converge [--trials 8000 --every 500]

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

- **Objective:** survival-first, then min expected HP loss (lexicographic). **Settled — keep it.** The
  single-scalar alternative (minimise E[HP loss], death = full remaining HP) was measured exactly against the
  oracle and rejected: it sacrifices survival for trivial HP savings (up to −55.6 % to save ~1 HP) on
  partial-survival fixtures, is lexicographic only in the death-penalty→∞ limit, and buys no exact-search
  speedup. Survival noise is an *estimation* problem (advisor `SurvivalBand` + VF calibration), not an objective
  one. (Full rationale under "Settled — keep the lexicographic objective" below.)
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

### ▶ Current priority order (set 2026-06-01): CORRECTNESS first, performance later
The goal is to accurately reproduce **every** card's behavior; perf is explicitly deferred until then (a clean
10× was proven unavailable this session — see Performance — and we're already at the 1–2 s target, so it's not
blocking).
1. ✅ **Mid-turn draws as chance nodes** — DONE (2026-06-02). `Cmd.Draw` in search mode (`combat.Rng == null`)
   now **defers** onto `CombatState.PendingDraw` instead of returning 0; the solver resolves it as an explicit
   draw chance node immediately after the play (`Solver.ContinuePlay` + MCTS `DrawNode`), continuing the SAME
   turn so the drawn hand — and any play made on it — is real in search. Both engines reuse the existing
   `DrawEnumerator` (exact hypergeometric below a distinct-count threshold, else DPW), PendingDraw is drained at
   every turn boundary so decision states stay clean (never in the memo key), and the MCTS rollout now sets
   `s.Rng` so its leaf playouts draw for real. Cross-validated three ways: a deterministic hand-checkable solve
   (ShrugItOff draws the lethal Strike → exact (1.0, 0)), **MCTS converges byte-identically to the new exact
   oracle** (block/Defends 92.59% / 35.78 on both), and the calibration suite tracks at Δ 0.0%. _Gated by
   `MidTurnDrawTests` + the existing oracle/convergence/calibration gates._ **Known residual (documented, not a
   bug):** the solver models each draw as an independent hypergeometric over the pile multiset, while the real
   engine preserves draw-pile ORDER across turns — so on a deck-cycling-dependent fight the exact value is a
   slight *pessimistic* lower bound (block/Defends exact 92.6% vs real-shuffle faithful-MC ~97.1%). Tightening
   it would mean tracking draw-pile order in the state key (breaks the multiset memo canonicalisation) — out of
   scope by design. _Follow-ups now UNBLOCKED:_ #2 (multi-select choice) and the draw-then-select half of #3
   (Acrobatics / Prepared / ThinkingAhead) — the post-draw hand the choice ranges over now exists in search.
   Draw-then-act cards are now PROMOTED via the post-draw resolution machinery (2026-06-02, see #2): the draw
   resolves as a chance node and the card's post-draw step runs on each drawn outcome — EscapePlan's conditional
   Block (deterministic per-outcome), Acrobatics/Prepared's discard-of-choice (a player MAX over the post-draw
   hand). ThinkingAhead's put-back stays HP-neutral/arbitrary (the multiset draw model ignores order).
2. ◐ **Post-draw + multi-select choice machinery** — POST-DRAW DONE (2026-06-02), in-hand multi-select still
   open. The post-draw step now resolves on the drawn hand: a deterministic/conditional effect
   (`CardModel.OnPostDraw`, e.g. EscapePlan's Block) applied per draw outcome, or a discard-of-choice
   (`PostDrawDiscardCount`) resolved as a **player MAX** one card at a time (`PendingDiscard`, a hashed decision
   state; exact MAXes inline, MCTS as a discard decision layer — both gated to converge). Promoted EscapePlan,
   Acrobatics, Prepared. _Still open:_ the IN-HAND multi-select cards HiddenDaggers (discard 2 of choice) and
   Purity (exhaust 0..N) — they choose a subset of the CURRENT hand (no draw), so they need the same discard/
   exhaust-of-choice MAX wired at PLAY time (the machinery now exists — `PendingDiscard` + the discard decision
   layer — so this is mostly reuse). STATUS still rates their HP value low (discards return; Purity is HP-neutral
   without on-exhaust powers).
3. ✅ **All-cards selection audit** — DONE (2026-06-02). Cross-referenced every selection / card-gen / discard /
   exhaust / draw-then-act card across ALL modules (Ironclad, Silent, Colorless, Special, Regent, Necrobinder)
   against the **decompiled OnPlay** (`CardSelectCmd.From{Hand,CombatPile}` = player choice; `Rng.CombatCardSelection`
   / `Rng.CombatCardGeneration` = random). **Headline result: ZERO optimistically-unsound findings** — no card
   anywhere models a *random* pick as a player *choice* (the dangerous direction that would let the search
   cherry-pick an RNG outcome). Every promoted `Choices` decision node maps to a real player select; every RNG
   generation (Metamorphosis/Distraction/HelloWorld/Mayhem; BundleOfJoy/Quasar/ManifestAuthority; InfernalBlade/
   Stoke; the auto-play cards) is correctly left inert or collapsed to a deterministic default; the two
   random-vs-choice trap cards are handled (True Grit: random base / choice upgraded; Cinder: random all levels).
   The audit's one actionable finding: **Survivor + DaggerThrow** were *omitting* a forced "discard 1 of choice"
   entirely (a pre-machinery gap) — the only optimistic edge case (an extra in-turn card the game removes). Both
   now promoted to the discard-of-choice MAX (see "Forced discard-of-choice" above). The remaining genuine
   player choices left as fixed defaults (HiddenDaggers, Purity, ThinkingAhead, Tools of the Trade, DualWield,
   NeowsFury, and the Regent/Necrobinder put-back/transform cards) are all ⚠️ *under-modeled-but-SOUND*
   (pessimistic — a fixed default can only under-credit the player) and documented per-card; promoting them is
   optional accuracy work, not a soundness fix. (A faithful random-pick chance node remains a sub-item of #1's
   machinery for the random-gen cards, if/when their generated cards matter.)
4. **THEN performance** — revisit only after correctness. The remaining lever is a cheaper rollout policy
   (≤3–4 HP accuracy budget); parallelism/truncation are refuted (see Performance).

1. ✅ **Learned value function (Phase C)** — DONE. `LearnedValue` (logistic+linear heads, 18 features incl.
   the static baseline's own estimate) fit to 425k exact labels via `--train-vf`; held-out survival MAE
   0.023 vs the static `Evaluate` baseline's 0.046 (gated by `LearnedValueTests`). _Follow-ups:_ as an MCTS
   leaf it now over-estimates razor-thin survival (block/Byrdonis 38% vs exact 22%) where the rollout
   under-estimated — recalibrate (Platt/isotonic on a held-out set); widen the training grid beyond Ironclad
   block/aggro/Byrdonis-shaped fights; add deck-composition features.
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

### ⭐ Performance — the 1–2 s late-game target is met
The advisor solves a deck vs an elite per variant; the lever was **algorithmic**, not engineering
(caching/parallelism would be a band-aid). Two oracle-preserving changes took a 30-card-vs-elite solve from
~38 s to ~1.5 s:
   - ✅ **Action progressive widening + lexicographic PUCT** (default ON; `STS2_APW=0` = classic UCT*). Ranks
     legal plays by a heuristic policy prior and opens only ⌈C·N^β⌉ best-first (always opening EndTurn; every
     candidate opens as N→∞, so it stays consistent), bounding the per-decision branching that grows with card
     variety. A partial-survival sweep showed a clear cost win (≈−25 % nodes / −27 % ms) at survival-neutral
     accuracy for a small (~0.3 HP) E[loss] regression. Oracle-gated by `MctsTests.Apw_Converges_*`.
   - ✅ **Lazy chance-node enumeration**: the chance node yields its (move-roll × draw) outcomes on demand
     rather than eagerly building all of them — the real late-game bottleneck (cloning was only ~12 %). ~25×
     faster, byte-identical oracle values.
   - Advisor trial budget set to ~2 000: survival is exact, E[HP loss] ~3 HP pessimistic (accepted).

**The hunt for a further 10× was run as experiments — and the two obvious levers were empirically REFUTED**
(probes `--parallel`, `--truncate`; 30-card Ironclad vs Byrdonis @ 4 000 trials; survival is 100 % so E[HP loss]
is the quality signal; full-rollout reference = 20.4):
   - ❌ **Root parallelization** (K independent trees, mean-combined). Speed-mode (matched total trials, split K
     ways) caps at **~2.7× (K=7) and *regresses* at K=14** — the workload is memory/GC-bound (~250 k clones/solve),
     so cores contend, not scale — AND quality degrades (loss +1.6 @ K=4 … +4.6 @ K=14: splitting trials pushes
     each tree below the convergence knee; the max-backup bias survives averaging). Quality-mode (K trees @ full
     trials) confirms 4 000 trials is already converged (no improvement at 7–14× the trials). **Not the lever.**
   - ❌ **Truncated rollout + VF bootstrap** (Kearns–Mansour–Ng). At accuracy-preserving depth (d=3) it gives
     only ~1.2× — because the leaf tips are late-game, so **the average rollout is already ~1–2 turns** (nothing
     to truncate); the 72 % rollout cost is *volume of short rollouts*, not length. Worse, the VF bootstrap costs
     ≈ a turn-sim, so learned d=1/d=2 are *slower* than the full rollout. Only pure VF (d=0) is fast and there
     quality collapses (heuristic 80.7 % survival; learned 98.1 %/24.6). **Not the lever.**
   - **Root cause (measured):** *every* play evaluation — the rollout policy, both VFs' `SimulateTurnOutput`, the
     APW prior — pays a full `clone + PlayCard + Score`. That single primitive is the 72 %. A clean 10× at fixed
     accuracy is **not available**; it would require a ~10×-cheaper, necessarily *less faithful* play evaluator.
   - **Remaining real lever (accuracy tradeoff, ≤3–4 HP budget):** a cheaper rollout policy — top-k candidate
     pruning by a clone-free prior, or a learned action-value that scores a play without applying it. Realistic
     2–4×, with the HP cost measured against the oracle. Parallelism could then be a *secondary* multiplier once
     allocation is cut. (The `--parallel`/`--truncate` probes that produced the numbers above were reverted as
     refuted scaffolding — zero fat; reconstruct from this note + git history if the cheaper-policy work resumes.)
   - ✅ **Choice fan-out kept out of the rollout** (this session): the greedy leaf no longer enumerates/scores
     in-card choice variants it then ignored — ~20 % faster on a 1-choice deck, value byte-identical; scales with
     per-card choice count on Silent.

### ✅ Settled — keep the lexicographic objective (don't collapse survival into one HP scalar)
An exact-oracle experiment (apparatus since removed; conclusion recorded here) compared the alternative
"minimise E[HP loss], death = full remaining HP" scalar against lexicographic survival-first. On
partial-survival fixtures the scalar gambles wins for marginal HP (up to **−55.6 % survival** to save ~1 HP);
a finite death penalty recovers lex decisions only at large, problem-dependent magnitudes (lexicographic is the
death-penalty→∞ limit); and there is no runtime payoff (identical exact state counts — the cost the scalar
would remove lives in the *estimator*, not the search). So survival noise is an **estimation** problem —
addressed by the advisor's `SurvivalBand` and VF survival recalibration — not an objective problem. (That
experiment also surfaced the Apotheosis/Armaments clone-aliasing oracle bug, since fixed.)

### Other near-term
4. **Calibration expansion** — cover all 12 elites + a high variety of decks (random-draw generator), with
   exact ground truth where tractable and MCTS self-consistency / anytime-convergence where (as decks grow)
   exact can't reach. This is the measuring stick for the perf work and for any future leaf/VF changes.
5. **VF distillation + recalibration** — after a large training round (now drawing colorless + varied decks),
   distill feature importance to simplify the model without losing accuracy; recalibrate survival (Platt/
   isotonic) IF we keep it; add deck-composition features.
6. **Content** — Silent 88/88 ✅ done; trace-validate the Silent batch-4–8 ports + the 8 new normal monsters
   against the real game (currently unit-tested only); Act-1 bosses; relic engine hooks;
   the deferred solver-side mid-turn **draw chance-node** for forward search.
7. **Intra-card SELECTION as real decision nodes — DONE for single-choice cards; two sound boundaries remain.**
   The machinery is in place and oracle-gated: `CardModel.Choices(state)` declares a card's distinct options
   (each an option's `StateKey`); `PlayerAction`/`CardPlay` carry a `ChoiceKey`; all three `LegalPlays`
   (Solver/MCTS/heuristic) emit one action per `(card, target, choice)`, with symmetric options collapsing for
   free (identical cards share a StateKey) and a null-choice default for trace replay. The rollout leaf is
   deliberately **choice-free** (a noisy estimator shouldn't pay for fan-out it ignores). Shared helpers
   `CardModel.HandChoices` / `ChosenHandCard` (exclude the played card by reference — it leaves hand before
   OnPlay). Each promotion was **verified against the decompiled game OnPlay** (`ilspycmd` vs the Steam
   `sts2.dll`; `CardSelectCmd.FromHand/FromCombatPile` = player choice, `Rng.CombatCardSelection.NextItem` =
   random). **Promoted (player-choice, decompile-confirmed):** Headbutt (topdeck), Armaments (upgrade target,
   base only — upgraded upgrades the whole hand), Burning Pact + Brand (exhaust target, all levels), True Grit
   (exhaust target, **upgraded only** — base is random), Nightmare (copy target). Each changes the oracle value
   *only* for decks that contain it (legitimately more correct); decks without are byte-identical.

   **Random selections are deliberately NOT decision nodes (soundness):** Cinder (random at all levels) and base
   True Grit exhaust a *random* card (`Rng.CombatCardSelection.NextItem`). Modelling a random pick as a player
   choice would let the search cherry-pick it — optimistically unsound. They keep the deterministic default
   (a faithful chance-node model of the random pick is a separate later item). Verify random-vs-choice from the
   decompiled OnPlay before promoting any future selection card — never from `cards.json` (LocString refs only).

   Two categories are deliberately **not** promoted, for sound reasons (not laziness):
   - **Multi-select (combinatorial):** HiddenDaggers (discard 2), Purity (exhaust up to N) choose a *subset* of
     hand — a pair / powerset choice set. Value is low/conditional (discards return; Purity is HP-neutral
     without on-exhaust powers like Feel No Pain / Dark Embrace) and the per-variant APW-prior cost is real, so
     with no perf buffer (see Performance — a clean 10× is proven unavailable) the branching isn't justified.
   - **Draw-then-select — now PROMOTED (2026-06-02):** Acrobatics, Prepared, EscapePlan act on the *post-draw*
     hand. Mid-turn `Cmd.Draw` opens a draw chance node (#1), and the card's POST-draw step then runs per draw
     outcome via the new machinery (#2): `CardModel.OnPostDraw` for a deterministic/conditional effect
     (EscapePlan's +Block iff the drawn card is a Skill), or `PostDrawDiscardCount` for a discard-of-choice
     resolved as a player MAX (`CombatState.PendingDiscard`, hashed; exact MAXes inline, MCTS as a discard
     decision layer). A card registers via `Cmd.DeferDrawThenResolve`; `CombatManager.ApplyPostDraw` dispatches.
     With a concrete Rng (rollouts/replay) the cards draw then discard a heuristic default. ThinkingAhead's
     put-back is left HP-neutral/arbitrary (the multiset draw model ignores draw-pile order anyway).

### History (condensed)
Milestones complete: engine + exact solver + CLI + oracle/autopilot + headless autonomy; MCTS solver
(UCT\*/DP-UCT, validated to converge); Act-1 elites 12/12; Ascension/A10; content modularization (per-character
folders, flat namespace); the `ranwid` advisor + survival-first rollout + sound horizon bound; **horizon bound
v2** (Weak / multi-enemy / in-search loss prune); **Phase-C learned value function**; the objective question
settled (keep lexicographic); **all four characters ported 88/88** (Ironclad, Silent, Regent, Necrobinder) plus
Colorless / Special / curses; the **performance milestone** — action-widening + PUCT default-on and lazy
chance-node enumeration brought a 30-card-vs-elite solve into the 1–2 s target; **mid-turn draws as chance
nodes** (exact `ContinuePlay` + MCTS `DrawNode`, converging byte-identically) — making a core Silent mechanic
real in search; and **post-draw resolution** (conditional `OnPostDraw` + discard-of-choice player MAX) promoting
the draw-then-select cards (EscapePlan / Acrobatics / Prepared). Detailed per-batch/per-milestone narrative
(with measurements) lives in the git commit history and the plan files under `~/.claude/plans/`.
