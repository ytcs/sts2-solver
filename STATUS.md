# STS2 Solver — Project Status

_Last updated: 2026-06-02. Recent: Sly/Innate/Retain keywords modelled 1:1; HiddenDaggers promotion;
Murder live-validation; a full per-module catalog fidelity audit (two number fixes; gaps recorded below)._

## Goal

A **standalone solver** that, given a deck and an encounter, computes the **HP lost under optimal play** in Slay
the Spire 2. Optimal play is **lexicographic: maximize win probability first, then minimize expected HP loss**.
Combat is stochastic (monster-move RNG, draw order), so the answer is an expectation over outcomes. The solver
is a faithful re-implementation of the game's combat engine (ported 1:1 from decompiled C#), driven by
expectimax + MCTS search, and **differentially validated against the real game** via a recorder mod + a headless
harness. The decompile is the **spec**; the real game is the **oracle** (see "Why reimplement" below).

## Current state

- **Content — all four characters complete (88/88 each):** Ironclad, Silent, Regent (Stars + Forge/Sovereign
  Blade), Necrobinder (Osty pet + Doom) — plus Colorless (18), the Event/Ancient "Special" pool (24), curses
  (18). Act-1 elites 12/12 + the normal-monster set, trace-validated against live recordings.
- **Card keywords modelled 1:1:** Exhaust, Ethereal, Unplayable, **Innate** (guaranteed opening hand), **Retain**
  (kept across turns), **Sly** (auto-play on mid-turn discard) — see "Engine" below.
- **Search:** the exact lexicographic expectimax `Solver` is the ground-truth **oracle**; the sampling
  `MctsSolver` (UCT*/DP-UCT) is gated to converge to it byte-for-byte. Production MCTS: action progressive
  widening + lexicographic **PUCT** (`STS2_APW=0` disables), lazy chance-node enumeration, greedy-rollout leaf,
  ~2 000-trial advisor budget. A 30-card-vs-elite solve runs ~1.5–2.7 s; survival exact, E[HP loss] ~3 HP
  pessimistic (accepted).
- **Sound horizon + pruning:** `HorizonBound` (Weak / multi-enemy) + `LossCertificate` (admissible early-loss
  prune), both oracle value-preserving. **Phase-C learned value function** (`LearnedValue` + `--train-vf`) is an
  opt-in MCTS leaf for the razor-thin survival regime.
- **Advisor:** `ranwid` live companion — reads the unmodded save, benchmarks the deck vs the Act's elites,
  recommends card removals + reward take/skip.
- **Tests: 520 passing, 0 skipped/failed. Traces: 73 recorded game traces, all PASS.**

---

## Repo layout

```
solver/                         C#/.NET 9 solution
  Sts2Solver.Engine/            faithful combat engine (no Godot/UI): CombatManager, CombatState, Cmd,
                                CardModel, Hashing (128-bit FNV memo key), powers pipeline
  Sts2Solver.Content/           ported content, flat namespace, folders for nav:
    Core/        Catalog.cs (BuildCard/SetupCombat) · CommonPowers.cs · StatusCards.cs · Curses.cs ·
                 CalibrationFixtures.cs · TrainingFixtures.cs
    Ironclad/ Silent/ Regent/ Necrobinder/ Colorless/ Special/   <Char>Cards.cs · <Char>Powers.cs · <Char>Catalog.cs
    Monsters/    Monsters.cs · MonsterPowers.cs · MonsterCatalog.cs · EncounterCatalog.cs
    Validation/  TraceValidator.cs        (adding a character = a folder + one yield in Core CardTables())
  Sts2Solver.Search/            Solver.cs (exact) · MctsSolver.cs · DrawEnumerator.cs · EncounterEvaluator.cs
                                (auto exact-or-MCTS) · PolicyRollout.cs · CombatHeuristic.cs · CalibrationHarness.cs
                                · HorizonBound/DeckProfile/MultiEnemy/LossCertificate · LearnedValue/VfTrainer
  Sts2Solver.Ranwid/ (ranwid)   live-run advisor (SaveLocator/RunSave/GameIds/Reporting/Advisor/Companion)
  Sts2Solver.Cli/   (sts2solve) solve · --validate · --calibrate · --profile/--converge · --train-vf
  Sts2Solver.Tests/             xUnit: pipeline + per-card + solver + trace-replay + MCTS + calibration + horizon
mods/DataDumper/DataDumperCode/ Godot C# mod: MainFile · CombatOracle (records traces) · AutoPilot (`autopilot`
                                console cmd) · HeadlessBatch (STS2_BATCH: headless run+fight+record+quit)
data/  game_data/*.json (dumped metadata) · combat_traces/*.jsonl (73 ground-truth traces)
docs/mcts-solver-design.md      SOTA literature review + chosen sampling/MCTS design
```

Decompile (reference, regenerable): `DOTNET_ROLL_FORWARD=LatestMajor ilspycmd "$DLL" -o /tmp/sts2src -p`.
DLL: `~/.local/share/Steam/steamapps/common/Slay the Spire 2/data_sts2_linuxbsd_x86_64/sts2.dll`.

---

## What works today (engine — validated against the real game)

- **Damage/block pipeline** (additive→multiplicative→floor→block→HP); turn lifecycle (energy reset, draw 5,
  end-of-turn hand discard with Ethereal→exhaust + **Retain**→keep, block clear, side switch).
- **Power-hook system:** `ModifyDamage/Block{Additive,Mult}`, `AfterSideTurn{Start,End}`, `AfterApplied`,
  `AfterCreatureDeath`, `AfterCardPlayed/Drawn/Exhausted`, `AfterDamageReceived`, `AfterAttackDealt`,
  `TryAbsorbDebuff`, `AfterBlockGained`, `PreventsBlockClear`, `ModifyHpLost`, `ModifyCardCost/PlayCount/MaxEnergy`,
  `OverrideResultPileToExhaust`, the Regent star hooks, the Necrobinder Osty/Doom hooks.
- **Monster AI** = weighted Markov chain (telegraphed intents); multi-hit, multi-monster, mid-combat summoning,
  relic post-combat hook, **status cards** (`Unplayable` + `OnTurnEndInHand`).
- **Mid-turn draw as a chance node** (Shrug It Off, every Silent draw/cycle): real with a concrete `Rng`
  (rollouts/replay); in search deferred onto `PendingDraw` and resolved as an explicit draw chance node
  continuing the same turn. Exact + MCTS converge byte-identically. Residual: the solver models each draw as an
  independent hypergeometric over the pile multiset, while the real engine preserves draw-pile ORDER across turns
  ⇒ slight pessimistic lower bound on deck-cycling fights (by design; tracking order would break memoisation).
- **Post-draw / discard-of-choice machinery:** after a deferred draw resolves, `ApplyPostDraw` runs the card's
  post-draw step — a conditional `OnPostDraw` (EscapePlan) or a discard-of-choice (`PendingDiscard`, a hashed
  player MAX one card at a time). **HiddenDaggers** discards 2-of-choice then adds its Shivs via a discard
  CONTINUATION (`PendingDiscardCard` → `OnPostDiscard`, matching the game's discard-THEN-create order). Promoted:
  EscapePlan / Acrobatics / Prepared / Survivor / DaggerThrow / HiddenDaggers.
- **Card keywords (1:1):**
  - **Innate** (`CardModel.Innate`): guaranteed in the turn-1 opening hand. The solver draws the opening hand as
    a chance node, so `CombatManager.OpeningDrawAfterInnate` pulls Innate cards into hand and returns the residual
    random-draw count (game: move to top, draw `max(5, innateCount)`). Wired at every opening-draw site behind a
    `TurnNumber==1` guard ⇒ byte-identical for innate-free decks. Irrelevant to replay (recorded hands reflect it).
  - **Retain** (`CardModel.Retain`, honoured in `EndPlayerTurn`): kept in hand instead of discarded.
  - **Sly** (`CardModel.IsSly`): a Sly card DISCARDED mid-turn auto-plays for free
    (`CombatManager.TriggerSlyOnDiscard`), matching `CardCmd.Discard` — NOT the end-of-turn flush (which the game
    and our `EndPlayerTurn` route through a direct pile-add). Wired into `Cmd.DiscardFromHand` + the Silent
    discard helpers; the discard-of-choice MAX routes its continuation through `ContinuePlay`/a MCTS `DrawNode` so
    a Sly Reflex's deferred draw resolves as a chance node, not a dangling `PendingDraw`. Residual (sound):
    auto-plays don't re-fire AfterCardPlayed hooks / per-turn counters; the single-turn Sly grant
    (HandTrick/MasterPlanner) is still inert.
- **Clone isolation (soundness):** `Player.Clone` shares immutable card instances but **deep-clones any
  `CardModel.Stateful` card** (Rampage, Maul, Sovereign Blade, Panache, …); `KeyHash` isn't cached for those.
  In-pile upgrades (Armaments/Apotheosis) **replace** rather than mutate shared instances. Per-turn play cap
  (`MaxPlaysPerTurn`) bounds cost-0 cantrip loops; loop-risk decks additionally hash the play counter.
- **Memo key** is a 128-bit FNV hash over all state-relevant fields incl. the per-turn/per-combat counters
  (Skills/Stars/Attacks/Discarded/Drawn[gated]/Ethereal/Osty/Doom, PendingDiscard + its continuation card).

### Search / MCTS / advisor / VF — all gated against the oracle
- Exact: lexicographic value `(P_win, E[HP loss])`, exact chance nodes (move-roll + multivariate-hypergeometric
  draws), `BestAction` policy extraction, cooperative `Ct` budget. The small-deck ground-truth oracle.
- MCTS: UCT* (DP-UCT + limited trial length), partial Bellman backups over the transposition DAG, lexicographic
  UCB, hybrid exact-below-threshold, action progressive widening + PUCT, `ObservedWin` floor. Converges
  byte-identically; calibration MAE ~Δsurv 0.8% / Δloss 0.03 at 40k trials.
- `EncounterEvaluator` (auto exact-or-MCTS) + `PolicyRollout` (HP-loss distribution). `CombatHeuristic`:
  survival-first λ-interpolated rollout/leaf score. `LearnedValue`: logistic+linear heads over 18 features,
  held-out survival MAE 0.023 vs static 0.046.
- `ranwid`: interactive companion (deck + per-elite stats, best cut, reward take/skip) with auto-correct card
  entry + Tab-complete; HP-loss-leaning `SurvivalBand` metric. Validated on a real save.

---

## How to run

```bash
cd solver/Sts2Solver.Tests && dotnet test -c Release            # build + full suite (~13 min)
dotnet run -c Release --project solver/Sts2Solver.Cli -- --demo --lines
dotnet run -c Release --project solver/Sts2Solver.Cli -- scenario.json [--mcts --trials N --anytime]
dotnet run -c Release --project solver/Sts2Solver.Cli -- --calibrate [--archetype block]
dotnet run -c Release --project solver/Sts2Solver.Cli -- --validate [trace.jsonl]   # one file or all
dotnet run -c Release --project solver/Sts2Solver.Cli -- --train-vf --budget-seconds 6 --epochs 4000
dotnet run -c Release --project solver/Sts2Solver.Ranwid                            # live companion
cd mods/DataDumper && dotnet build -c Debug                     # rebuild + deploy the mod

# Headless trace collection (game CLOSED, Steam running, real HOME):
GAME="$HOME/.local/share/Steam/steamapps/common/Slay the Spire 2"
cd "$GAME" && STS2_BATCH=CORPSE_SLUGS_WEAK STS2_CHARACTER=silent STS2_DECK="Murder:3,StrikeSilent:5" \
  STS2_ASCENSION=0 STS2_TRACE_DIR="$HOME/Projects/sts2-solver/data/combat_traces" ./SlayTheSpire2 --headless
#   STS2_BATCH: CULTISTS_NORMAL, CORPSE_SLUGS_WEAK/NORMAL, BYRDONIS_ELITE, … (see HeadlessBatch.ResolveEncounter)
#   STS2_CHARACTER: ironclad|silent|regent|necrobinder|defect ; STS2_DECK="ClassName:Count,…"
```
Card-validation loop: port + unit-test → `STS2_DECK` headless run vs a simple encounter → `--validate` green →
promote the trace into `data/combat_traces/`. **The autopilot plays Powers→Skills→Attacks (most-expensive
first within a tier)** — so to make a specific high-cost / discard-triggered card fire, shape the deck accordingly
(e.g. an all-attack deck so a cost-3 Murder isn't starved by cheap Skills).

## Operational gotchas (IMPORTANT)

- **Mods get disabled in settings sometimes.** `~/.local/share/SlayTheSpire2/steam/<id>/settings.save` →
  `mod_settings.mods_enabled: true` AND `mod_settings.mod_list[]` BaseLib + DataDumper both `is_enabled: true`.
  A failed/early headless boot can reset these. Re-enable before runs.
- **`steam_appid.txt`** (contents `2868840`) in the game install dir — required to boot `--headless` outside Steam.
- **Headless requires:** Steam client running, the game NOT already open (check with `pgrep -x SlayTheSpire2`),
  the real `HOME`. The batch backs up `settings.save` (`.pre-headless.bak`); startup logs a harmless "leaked at
  exit" + `ObjectDisposedException`. The validation harness picks the starter relic from the recorded `player.name`
  (Ironclad→BurningBlood [heal-6-on-victory], Silent→none, Regent→DivineRight, Necrobinder→BoundPhylactery).
- Shell aliases `grep`→`ugrep`, `ls`→`eza`. Use `/usr/bin/grep`, `/usr/bin/ls`. `ilspycmd` needs
  `DOTNET_ROLL_FORWARD=LatestMajor`. `dotnet` working dir DOES persist across this shell's calls, but prefer
  absolute project paths. Quote globs in fish.

---

## Key design decisions (settled — don't relitigate)

- **Why reimplement instead of hooking the decompiled engine?** Search clones `CombatState` ~250k times per
  solve; the game's combat lives in mutable Godot-coupled singletons (`CombatManager.Instance`, `History`, RNG
  tied to `RunState`) that aren't forkable. Every `OnPlay` is `async Task` awaiting hooks/VFX/choice-contexts;
  search needs synchronous, side-effect-free, deterministic transitions. The DLL only runs inside a booted game
  (~1–2 min/fight) — fine for recording ground-truth traces, hopeless as a ~1.5 s search inner loop. So: the
  decompile is the spec (port 1:1), the real game is the oracle (DataDumper traces + differential validation).
- **Objective: survival-first, then min E[HP loss] (lexicographic).** Measured against the single-scalar
  alternative ("death = full remaining HP") and rejected: it gambles wins for trivial HP (up to −55.6% survival
  to save ~1 HP), is lexicographic only in the death-penalty→∞ limit, and buys no search speedup. Survival noise
  is an *estimation* problem (advisor `SurvivalBand`, VF recalibration), not an objective one.
- **Exact solve is the small-deck ORACLE, not the engine.** Real 40-card decks are intractable exactly → MCTS +
  learned-VF carry late-game. Validation leans on trace outcomes + deck-variety + self-consistency, not only
  exact-equality.
- **Soundness directions:** modelling RANDOM as player CHOICE is optimistically UNSOUND (search cherry-picks) —
  never do it. Modelling CHOICE as a fixed default is pessimistic-but-sound. Under-crediting an effect is safe;
  over-crediting is dangerous. Random-target attacks use a deterministic first-enemy default in search (Ricochet,
  RipAndTear, SerpentForm) — exact for single-enemy, an approximation otherwise.
- **Validate, don't trust:** every ported card/power/monster is confirmed by diffing a real game trace; every
  MCTS/heuristic/horizon/VF change is gated against the exact oracle. New `Stateful` cards MUST set the flag.
- Monsters are **stochastic-but-known, not adversarial** ⇒ MDP/expectimax, not minimax. Determinization/PIMC was
  considered and rejected (needs hidden info we don't have) — see `docs/mcts-solver-design.md`.
- **Performance is met (1–2 s target).** The lever was algorithmic (action-widening + PUCT, lazy chance-node
  enumeration: ~38 s → ~1.5 s). A clean 10× at fixed accuracy is empirically unavailable (root parallelization
  caps ~2.7×, GC-bound; truncated rollout gives ~1.2× since leaves are already late-game). The remaining real
  lever is a cheaper rollout policy (≤3–4 HP accuracy budget) — deferred behind correctness.

---

## Current priority & next steps

**Priority: CORRECTNESS — 1:1-model the mechanics of every ported card.** A full per-module fidelity audit
(Silent/Colorless/Ironclad/Special/Curses/Regent/Necrobinder vs the decompile) found the catalog largely
fidelity-correct: **Necrobinder is clean; Ironclad/Special are clean** (sampled damage/upgrade/power numbers all
match). Sly/Innate/Retain are now modelled. The remaining genuine HP-relevant gaps are small and listed first.

1. **Regent `CrescentSpear` — star-count filter.** Game: damage scales on `AllCards.Count(c => c.CanonicalStarCost
   >= 0 || c.HasStarCostX)`; ours counts `StarCost > 0 || IsXStarCost`. **First verify the base
   `CanonicalStarCost` default** (likely −1 for non-star cards ⇒ `>= 0` means "has a star cost", which our
   `> 0` matches unless a 0★ card exists) before changing — don't blind-fix. Pessimistic if wrong (under-damage).
2. **Regent `BeatIntoShape` — history-based Forge.** Game forges `(powered hits on target this turn by player) −
   (this attack's hit count)`; ours forges a fixed `5 + 2·U`. **Optimistic/dangerous** (we over-forge on the
   first hit). Needs a per-creature "powered hits received this turn" counter (the engine has the hooks; add a
   small tracked counter like the existing per-turn counters). Highest-value remaining fix.
3. **Single-turn Sly grant** (HandTrick selects a Skill, MasterPlannerPower grants Sly to played Skills) — still
   inert. Needs a mutable per-card single-turn-Sly flag (makes granted cards Stateful). Moderate; low frequency.
4. **`Purity` variable-count exhaust-of-choice** (0..N) — Retain now set, but the exhaust selection is a fixed
   default; HP-neutral without on-exhaust powers (Feel No Pain / Dark Embrace). Low value.
5. **Live-validate the new mechanics** via headless single-enemy runs: a Sly-discard fight (e.g. CalculatedGamble
   + Untouchable/FlickFlack), an Innate deck, a Retain deck. Murder is already live-validated (trace #73). The
   random-target (SerpentForm/Ricochet/RipAndTear) and hand-size (PreciseCut) cards need single-enemy encounters.

**Documented out-of-scope (need a new subsystem — left inert/approximated, all sound):** gold (HandOfGreed,
Royalties), Defect orbs/Focus (BiasedCognition, Quadcast), full-pool RNG card-generation (Metamorphosis,
Distraction, DualWield, Begone, Quasar, Supermassive's scaling, …), multiplayer-only (Sneaky/Flanking/Largesse/
TankPower), move-legality curses (Normality 3-cards/turn, Enthralled hand-lockout — degrade to dilution,
understate harm), Whistle stun (needs a monster skip-move hook), Rupture end-of-turn self-damage edge, MadScience
TinkerTime, true-RNG card-selection for Cinder / base True Grit (kept deterministic-default — promoting would be
optimistically unsound). Random card-selection is deliberately NEVER a search decision node.

**Longer-horizon roadmap (unchanged, behind correctness):** calibration expansion (all 12 elites + random-deck
generator); VF distillation + survival recalibration (Platt/isotonic) + deck-composition features; more relics
(needs combat-relevant relic hooks) + Act-1 bosses + potions; the cheaper-rollout-policy perf lever
(top-k clone-free prior / learned action-value, ≤3–4 HP budget).

---

## History (condensed)

Milestones (detail in git log + `~/.claude/plans/`): engine + exact solver + CLI + oracle/autopilot + headless
autonomy; MCTS (UCT*/DP-UCT, validated to converge); Act-1 elites 12/12 + Ascension/A10; content modularization;
the `ranwid` advisor + survival-first rollout + sound horizon bound v2 (Weak / multi-enemy / in-search loss
prune); Phase-C learned value function; the lexicographic-objective question settled; **all four characters
88/88** + Colorless/Special/curses; the performance milestone (action-widening + PUCT, lazy chance nodes → 1–2 s);
**mid-turn draws as chance nodes** + **post-draw resolution**; **HiddenDaggers discard-continuation promotion**;
**Murder live-validation** (turn-start-draw reconstruction in the validator, trace #73); **Sly/Innate/Retain
keyword mechanics 1:1** + a full per-module catalog fidelity audit (Predator/FlashOfSteel upgrade-number fixes).
