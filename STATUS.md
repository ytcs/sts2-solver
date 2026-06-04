# STS2 Solver — Project Status

_Last updated: 2026-06-03._

## Goal

A **standalone solver** that, given a deck and an encounter, computes the **HP lost under optimal play** in Slay
the Spire 2. Optimal play is **lexicographic: maximize win probability first, then minimize expected HP loss**.
Combat is stochastic (monster-move RNG, draw order), so the answer is an expectation over outcomes. The solver
is a faithful re-implementation of the game's combat engine (ported 1:1 from decompiled C#), driven by
expectimax + MCTS search, and **differentially validated against the real game** via a recorder mod + a headless
harness. The decompile is the **spec**; the real game is the **oracle** (see "Why reimplement" below).

## Current state (summary)

- **Content — every in-scope card ported (573/577).** Five characters complete (88/88 each): Ironclad, Silent,
  Regent (Stars + Forge/Sovereign Blade), Necrobinder (Osty pet + Doom), Defect (full orb subsystem + Focus) —
  plus the full Colorless pool (53/53), Status (12) + Token (14) pools, the Event/Ancient "Special" pool, curses
  (18), and all 21 multiplayer-only cards (modelled as their single-player projection). Out of scope: 3 Quest/map
  items (ByrdonisEgg/LanternKey/SpoilsMap) + MadScience (RNG card-gen). Monsters: Act-1 elites 12/12 + the
  normal-monster set, all trace-validated.
- **Zero known OPTIMISTIC (over-crediting) gaps** in the catalog or engine. Every remaining modelling shortfall is
  PESSIMISTIC (under-credits the player) and therefore sound — see "Known pessimistic gaps" below.
- **Search:** the exact lexicographic expectimax `Solver` is the ground-truth oracle; the sampling `MctsSolver`
  (UCT*/DP-UCT, action progressive widening + lexicographic PUCT) is gated to converge to it byte-for-byte. A
  30-card-vs-elite solve runs ~1.5–2.7 s. Sound `HorizonBound` + `LossCertificate` pruning (value-preserving).
  The MCTS leaf is the **faithful greedy rollout** (the only leaf — the static-heuristic and learned-VF leaves
  were removed: both badly mis-estimated big decks, ~18 HP off the rollout, and weren't in the production path).
- **Advisor:** `ranwid` live companion — reads the unmodded save, benchmarks the deck vs the Act's elites,
  recommends card removals + reward take/skip.
- **Tests: 725 passing, 0 skipped/failed. Traces: 80 recorded game traces, all PASS.**

---

## Repo layout

```
solver/                         C#/.NET 9 solution (NO .sln — build/test via Sts2Solver.Tests.csproj)
  Sts2Solver.Engine/            faithful combat engine (no Godot/UI): CombatManager, CombatState, Cmd,
                                CardModel, Hashing (128-bit FNV memo key), powers pipeline, Orbs.cs (Defect
                                orb subsystem — gated, FIFO queue + Focus + turn-boundary passives)
  Sts2Solver.Content/           ported content, flat namespace, folders for nav:
    Core/        Catalog.cs (BuildCard/SetupCombat) · CommonPowers.cs · StatusCards.cs · Curses.cs ·
                 CalibrationFixtures.cs · TrainingFixtures.cs
    Ironclad/ Silent/ Regent/ Necrobinder/ Defect/ Colorless/ Special/   <Char>Cards.cs · <Char>Powers.cs · <Char>Catalog.cs
    Monsters/    Monsters.cs · MonsterPowers.cs · MonsterCatalog.cs · EncounterCatalog.cs
    Validation/  TraceValidator.cs        (adding a character = a folder + one yield in Core CardTables())
  Sts2Solver.Search/            Solver.cs (exact) · MctsSolver.cs · DrawEnumerator.cs · EncounterEvaluator.cs
                                (auto exact-or-MCTS) · PolicyRollout.cs · CombatHeuristic.cs · CalibrationHarness.cs
                                · HorizonBound/DeckProfile/MultiEnemy/LossCertificate
  Sts2Solver.Ranwid/ (ranwid)   live-run advisor (SaveLocator/RunSave/GameIds/Reporting/Advisor/Companion)
  Sts2Solver.Cli/   (sts2solve) solve · --validate · --calibrate · --bridge · --profile/--converge
  Sts2Solver.Tests/             xUnit: pipeline + per-card + solver + trace-replay + MCTS + calibration + horizon
mods/DataDumper/DataDumperCode/ Godot C# mod: MainFile · CombatOracle (records traces) · AutoPilot (`autopilot`
                                console cmd) · HeadlessBatch (STS2_BATCH: headless run+fight+record+quit)
data/  game_data/*.json (dumped metadata) · combat_traces/*.jsonl (80 ground-truth traces) ·
       combat_traces_unresolved/ (documented sound/pessimistic mechanics not modelled, with failing traces)
docs/mcts-solver-design.md      SOTA literature review + chosen sampling/MCTS design
```

Decompile (reference, regenerable): `DOTNET_ROLL_FORWARD=LatestMajor ilspycmd "$DLL" -o /tmp/sts2src -p`.
DLL: `~/.local/share/Steam/steamapps/common/Slay the Spire 2/data_sts2_linuxbsd_x86_64/sts2.dll`.

---

## What works today (engine — validated against the real game)

- **Damage/block pipeline** (additive→multiplicative→floor→block→HP); turn lifecycle (energy reset, draw 5,
  end-of-turn hand discard with Ethereal→exhaust + Retain→keep, block clear, side switch).
- **Power-hook system:** `ModifyDamage/Block{Additive,Mult}`, `AfterSideTurn{Start,End}`, `AfterApplied`,
  `AfterCreatureDeath`, `AfterCardPlayed/Drawn/Exhausted`, `AfterDamageReceived`, `AfterAttackDealt`,
  `TryAbsorbDebuff`, `AfterBlockGained`, `PreventsBlockClear`, `ModifyHpLost`, `ModifyCardCost/PlayCount/MaxEnergy`,
  `ModifyHandDraw`, `OverrideResultPileToExhaust`, the Regent star hooks, the Necrobinder Osty/Doom hooks, the
  Defect orb hooks (`AfterOrbEvoked`, `AfterCardGenerated`).
- **Debuff duration (1:1):** Weak/Frail/Vulnerable all tick at the ENEMY turn end; any debuff applied to a
  player-side creature gets `SkipNextTick` (set centrally in `Cmd.ApplyPower`) so it survives the turn it lands.
- **Monster AI** = weighted Markov chain (telegraphed intents); multi-hit, multi-monster, mid-combat summoning,
  relic post-combat hook, status cards (`Unplayable` + `OnTurnEndInHand`), bounded one-turn stun
  (`Monster.StunnedTurns`, gated), Decimillipede Reattach (2-turn DEAD→REATTACH-to-25 cycle, gated).
- **Mid-turn draw as a chance node** (Shrug It Off, every Silent draw/cycle): real with a concrete `Rng`
  (rollouts/replay); in search deferred onto `PendingDraw` and resolved as an explicit draw chance node. Exact +
  MCTS converge byte-identically. Residual (pessimistic, by design): the solver models each draw as an
  independent hypergeometric over the pile multiset, while the real engine preserves draw-pile ORDER across turns
  ⇒ slight pessimistic lower bound on deck-cycling fights (tracking order would break memoisation).
- **Post-draw / discard-of-choice machinery:** after a deferred draw resolves, `ApplyPostDraw` runs the card's
  post-draw step — a conditional `OnPostDraw` (EscapePlan) or a discard-of-choice (`PendingDiscard`, hashed,
  player picks one at a time). HiddenDaggers discards 2-of-choice then adds Shivs via a discard CONTINUATION
  (`PendingDiscardCard` → `OnPostDiscard`).
- **Card keywords (1:1):** Exhaust, Ethereal, Unplayable, **Innate** (guaranteed opening hand via
  `OpeningDrawAfterInnate`), **Retain** (kept across turns), **Sly** (mid-turn-discard auto-play via
  `TriggerSlyOnDiscard`). **Play-restriction curses 1:1:** Normality (≤3 plays/turn, `EffectivePlayCap`) and
  Enthralled (hand-lockout, `CardPlayAllowed`) — their harm is exact, enforced by every move generator.
- **Affordability is resolved-cost-aware:** all move generators gate on `CombatManager.ResolveCardCost` (runs the
  `ModifyCardCost` pipeline), so VoidForm/Corruption/Borrowed-Time-style discounts are visible to the search.
  Forced-end-of-turn (VoidForm) via gate-hashed `CombatState.PlayerTurnEndForced`, honoured in exact + MCTS +
  rollout.
- **Clone isolation (soundness):** `Player.Clone` shares immutable card instances but **deep-clones any
  `CardModel.Stateful` card** (Rampage, Maul, Sovereign Blade, Panache, …); `KeyHash` isn't cached for those.
  In-pile upgrades (Armaments/Apotheosis) replace rather than mutate shared instances. Per-turn play cap
  (`MaxPlaysPerTurn`) bounds cost-0 cantrip loops; loop-risk decks additionally hash the play counter.
- **Memo key** = 128-bit FNV hash over all state-relevant fields incl. the gated per-turn/per-combat counters
  (Skills/Stars/Attacks/Discarded/Drawn/Ethereal/Osty/Doom/CardsDrawnMidTurn/PlaysThisTurn, per-target
  PoweredHitsThisTurn, PlayerTurnEndForced, PendingDiscard + its continuation card). Gated = only hashed when a
  deck/monster in play actually reads the counter ⇒ byte-identical for fights that don't use it.

### Search / MCTS / advisor / VF — all gated against the oracle
- Exact: lexicographic value `(P_win, E[HP loss])`, exact chance nodes (move-roll + multivariate-hypergeometric
  draws), `BestAction` policy extraction, cooperative `Ct` budget. The small-deck ground-truth oracle.
- MCTS: UCT* (DP-UCT + limited trial length), partial Bellman backups over the transposition DAG, lexicographic
  UCB, hybrid exact-below-threshold, action progressive widening + PUCT, `ObservedWin` floor. Converges
  byte-identically; calibration MAE ~Δsurv 0.8% / Δloss 0.03 at 40k trials.
- `EncounterEvaluator` (auto exact-or-MCTS, with a deck-size tractability gate) + `PolicyRollout` (HP-loss
  distribution). `CombatHeuristic`: survival-first λ-interpolated rollout policy (the MCTS rollout leaf + the
  distribution sampler share this one definition of "reasonable play").
- `ranwid`: live **Spectre.Console dashboard** — always shows the current deck + per-elite survival/HP-loss,
  auto-refreshes on save change (FileSystemWatcher + poll fallback). Removal advice is off by default (slow) and
  on the `[r]` key; `[c]` checks a reward card (auto-correct entry). No engine/algorithm text in the UI; a
  prominent "Ignored" panel surfaces unmodelled relics / unported cards so a number is never silently a
  relic-less lower bound. `ranwid --preview` shows the layout with sample data. Validated on a real save.

---

## How to run

```bash
cd solver/Sts2Solver.Tests && dotnet test -c Release            # build + full suite (~13 min)
dotnet run -c Release --project solver/Sts2Solver.Cli -- --demo --lines
dotnet run -c Release --project solver/Sts2Solver.Cli -- scenario.json [--mcts --trials N --anytime]
dotnet run -c Release --project solver/Sts2Solver.Cli -- --calibrate [--archetype block]
dotnet run -c Release --project solver/Sts2Solver.Cli -- --bridge    # advice-regime accuracy(noise/bias)+latency instrument
dotnet run -c Release --project solver/Sts2Solver.Cli -- --validate [trace.jsonl]   # one file or all
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
first within a tier)** — so to make a specific high-cost / discard-triggered card fire, shape the deck
accordingly (e.g. an all-attack deck so a cost-3 Murder isn't starved by cheap Skills).

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
  `DOTNET_ROLL_FORWARD=LatestMajor`. Quote globs in fish.
- **Tool display quirk:** `rg`/`grep` tool output mangles many C# identifiers (collapsing distinct symbols to
  `n`/`ln`); only the `Read` tool shows true source. `Edit` requires a prior `Read` (Bash/sed reads don't count).

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
  to save ~1 HP), is lexicographic only in the death-penalty→∞ limit, and buys no search speedup.
- **Exact solve is the small-deck ORACLE, not the engine.** Real 40-card decks are intractable exactly → MCTS +
  learned-VF carry late-game. Validation leans on trace outcomes + deck-variety + self-consistency, not only
  exact-equality.
- **SOUNDNESS DOCTRINE (the spine of every modelling choice):**
  - Modelling RANDOM as player CHOICE is optimistically UNSOUND (search cherry-picks favourable outcomes) — NEVER
    do it. Modelling a CHOICE as a fixed default is pessimistic-but-sound.
  - Under-crediting an effect is SAFE (pessimistic); over-crediting is DANGEROUS (optimistic).
  - Random-target attacks use a deterministic first-enemy default in search (Ricochet/RipAndTear/SerpentForm/
    FlakCannon) — exact for single-enemy, an approximation otherwise.
  - Every ported card/power/monster is confirmed by diffing a real game trace; every MCTS/heuristic/horizon/VF
    change is gated to converge byte-identically to the exact oracle. New `Stateful`/counter state MUST be cloned
    + hashed (gated).
- Monsters are **stochastic-but-known, not adversarial** ⇒ MDP/expectimax, not minimax. Determinization/PIMC was
  considered and rejected (needs hidden info we don't have) — see `docs/mcts-solver-design.md`.
- **Performance is met (1–2 s target).** The lever was algorithmic (action-widening + PUCT, lazy chance-node
  enumeration: ~38 s → ~1.5 s). A clean 10× at fixed accuracy is empirically unavailable (root parallelization
  caps ~2.7×, GC-bound). The remaining real lever is a cheaper rollout policy (≤3–4 HP accuracy budget) — deferred
  behind correctness.

---

## Known pessimistic gaps (sound — under-credit the player, left as-is)

All verified to never over-credit. Fixing is low-value; documented so they aren't re-discovered as "bugs":

- **Inert RNG card/orb generation** (Discovery, JackOfAllTrades, CreativeAi, WhiteNoise, Chaos, Calamity, Entropy,
  Metamorphosis, Distraction, DualWield, Begone, Quasar, Supermassive, Largesse, Catastrophe/BeatDown auto-play,
  Uproar, TrashToTreasure's random orb, MadScience): letting the search SEE generated cards as a benefit is the
  forbidden optimistic direction. Tractable selection cards (SecretWeapon/SecretTechnique/SeekerStrike/Wish/
  Hologram/Scavenge) use a sound fixed-default move-to-hand.
- **Single-turn Sly grant** (HandTrick, MasterPlannerPower) — inert; a faithful fix needs a mutable per-card Sly
  flag (⇒ Stateful) + a CHOICE node and risks the optimistic direction. Low frequency.
- **Cross-combat / free-replay scaling** kept at the pessimistic floor: Feral, GeneticAlgorithm, Scrape
  (modelled only under a concrete Rng; skipped in pure search), Purity's variable exhaust-of-choice (fixed
  default), Fetch draw, MakeItSo recursion, Squeeze's PlayPile-omission (≤1 under).
- **Cost-only reductions** (Pinpoint, Stomp per-card, RocketPunch) — HP-neutral, so leaving them is exact for the
  objective.
- **Gold** is HP-neutral (RoyaltiesPower only fires post-combat; no gold relic has in-combat HP/block/damage) ⇒
  intentionally inert, HP-exact, NOT a missing subsystem.
- **Decimillipede middle-segment auto-bury** (documented in `data/combat_traces_unresolved/`): the game buries
  then reattaches-at-25 a middle segment unprompted, which HELPS the player; our engine keeps it
  alive-and-attacking ⇒ strictly pessimistic. The exact trigger isn't recoverable from the obfuscated decompile,
  so modelling it would risk the optimistic direction. Left documented for future work.
- **Multiplayer-only ally payloads** (BeaconOfHope/Flanking/Knockdown/Sneaky/HammerTime/TagTeam) are inert markers
  in single-player (no allies exist); the self-downside **Tank** (×2 attack damage taken) IS modelled so it can't
  be played for free.

---

## Roadmap & next to-dos

Correctness work (the spine) is complete — the catalog + engine have **zero known optimistic gaps**, the search's
value-preserving approximations are audited, and the freshest models are live-validated. What remains is
feature/quality expansion, all behind the standing correctness bar.

### Focus & scope (decided this session)

The user-facing product is **`ranwid`**, and its two pain points are **output accuracy** and **speed** — so the
priority is the advice engine's accuracy + per-evaluation latency (old to-dos 1 + 3). **Descoped (not worth the
complexity):** non-starter relics and potions. Combat-start starter relics already work; the 300+ relic catalog,
Act-1 bosses, and potions are shelved.

**Shippable executables** (for testers): `ranwid` is published self-contained single-file —
`solver/Sts2Solver.Ranwid/bin/Release/net9.0/{win-x64,linux-x64}/publish/ranwid[.exe]`, and the Linux build is
copied to the repo top as `./ranwid`. Windows save-dir auto-detection (`SaveLocator`) probes the **confirmed**
path `%APPDATA%\SlayTheSpire2\steam\<id>\…` (mirror of the Linux layout) first, with profile/Steam-registry
fallbacks and a validated manual folder prompt that persists the choice. See `Sts2Solver.Ranwid/SHIPPING-WINDOWS.md`.

### Recently completed (this session — paused here)

- **Removed the cheap-leaf family** (static-heuristic leaf `CombatHeuristic.Evaluate` + Phase-C `LearnedValue`
  + `VfTrainer` + `--train-vf` + the `mcts-heur`/`mcts-learn` plumbing): the profile showed the static leaf read
  82%/38 vs the faithful rollout's 100%/19.5 (~18 HP off) on a big deck, and neither cheap leaf was in the
  production path. The **faithful greedy rollout is now the only MCTS leaf.** `TrainingFixtures` + its clone/upgrade
  soundness regression tests are kept (standalone deck generator). `--profile` reworked to attribute cost over
  the rollout leaf (confirmed: **rollout playout ≈ 72%** of the solve — the real speed lever, not allocation).
- **Tightened `Advisor.SurvivalBand` 0.05 → 0.03** on bridge evidence: survival has ~0% seed noise, so the band
  only needs to cover the directional convergence BIAS — and because it guards RELATIVE comparisons of
  near-identical decks (deck vs deck-minus-a-card), whose biases largely cancel, the differential bias is far
  below the ~7% absolute bias at the 800-trial ranking budget. 0.03 recovers survival-first ranking precision
  0.05 discarded, with margin. (Going lower needs a higher ranking budget = smaller absolute bias.)
- **Deck strength index** (`Advisor.DeckStrength`): a 0–100 headline scalar, HP-independent — evaluate each Act
  elite from a FIXED 100 HP, average the expected HP loss (death = 100, capped), report `100 − avg`. 100 = takes
  no damage from any elite, 0 = certain death. Shown as a color-coded bar atop the dashboard; uses the full
  trial budget (un-pessimistic). Parallel over elites.
- **Advice speedup ≈ 30× (Tier 1):** a full removal-advice run on a 30-card deck vs 3 Act-1 elites went from
  ~576 s (sequential, exact-attempt + 2k trials + 2k rollouts) to **~19 s**. Levers: (a) ranwid is MCTS-only
  (no exact attempt); (b) advice evals skip the rollout-distribution pass (`ScoreDeck` reads only survival +
  MCTS mean); (c) the candidate × elite grid is evaluated across cores (`RemovalAdvice`/`PickAdvice` PLINQ;
  `Companion.EvaluateElites` `AsOrdered`) — deterministic (per-eval seeded, sorted after); (d) **Server GC**
  (csproj) ~doubles parallel throughput vs the workstation-GC single-heap wall (the documented ~2.7× cap —
  measured 2.0× → 4.3× here); (e) a separate ranking trial budget `Advisor.AdviceTrials = 800` (top cut +
  bottleneck identical at 500/800/2000, ~3× faster) while DISPLAYED numbers keep the full budget to stay
  un-pessimistic. New `ranwid --advice-bench` measures it end-to-end. Note: parallelism alone caps ~4× (GC /
  memory-bandwidth bound) — the durable further lever is cutting allocation (clone-free rollout/make-undo).
- **Ranwid robustness:** save reads fail SILENTLY when a run ends (the game clears `current_run.save`); the last
  good dashboard persists for post-game analysis (`Load(quiet)` + watcher keeps last deck).
- **Speed: exact-attempt tractability gate** (`EncounterEvaluator.ExactMaxDrawPile`, default 14 + `EncounterEvaluatorTests`):
  exact is skipped outright for decks bigger than the gate (it can't finish them anyway), so a real advice eval
  no longer burns the ~8 s exact budget before falling to MCTS. Measured: a 26-card eval dropped from ~13 s to
  ~3.5 s (3–4× on the 100s-of-evals advice path); small fixtures (≤14) still take the exact path. Zero accuracy
  change (big decks fell to MCTS regardless).
- **`ranwid` TUI rework** (Spectre.Console): the live companion is now an always-on dashboard (deck + per-elite
  survival/HP-loss) that auto-refreshes on save change via a FileSystemWatcher (+ poll fallback). Removal advice
  moved off the default path (too slow — it re-evals per card) onto the `[r]` key; `[c]` checks a reward card.
  **No engine/algorithm detail in the UI** (per request); a prominent "Ignored" panel lists unmodelled relics /
  unported cards. `--once` and `--preview` render the same dashboard.
- **Pessimism diagnosis** (user saw ~21% vs TerrorEel on a strong deck): NOT an engine bug — survival is
  dominated by **current HP** (a strong 30-card deck wins 100% at 70 HP but **20%** at 30 HP vs TerrorEel's
  150 HP / 99-Vulnerable; reproduced the ~21% by HP alone). The real residual pessimism is **unmodelled relics**
  (ranwid counts only Burning Blood) — the strongest argument for narrowly modelling a run's *actual* relics
  (revisiting the relic descope for combat-start relics only).
- **Bridge-regime instrument** (`--bridge` + `BridgeInstrumentTests`, 4 tests) — measures advice accuracy AND
  per-eval latency in the regime `ranwid` actually runs (the old `--calibrate` suite only covers the tiny
  exact-tractable decks). A deck-size ladder straddling the exact boundary (`CalibrationFixtures.BridgeRung`,
  exact-anchored at the S06 rung), plus a large CONTESTED probe (`BridgeContestedLarge`) and the existing
  contested fixtures. New harness: `CalibrationHarness.RunMctsSeeds`/`SeedStats` (per-seed survival/loss spread).
  **Two findings that set the next two to-dos:**
  - **Accuracy:** the 2k-trial advice survival estimate has **~0% seed variance** (DP-UCT backs up TRUE
    probabilities) even on a large 80%-survival fight; the only spread is a small **pessimistic convergence
    bias** (~1.5% vs an 8k proxy). ⇒ `Advisor.SurvivalBand=0.05` is ~2–3× larger than needed.
  - **Speed:** **~4–9 s of every `EncounterEvaluator.Evaluate` is a doomed exact attempt** (it can't finish a
    30-card deck, then falls to MCTS anyway). Advice runs 100s of these. `EncounterEvaluator` already supports
    `BudgetSeconds=0` to skip exact — a tractability gate reclaims that time at zero accuracy cost.
- **Live-validated the two freshest 1:1 models** against the oracle (replay validates damage/HP CONSEQUENCES,
  not card-FLOW): **VoidForm** (trace #79, forced-end + first-2-free) and **Sly auto-play** (trace #80,
  `TriggerSlyOnDiscard`).
- **Calibration + soundness** (earlier): elite-AI sweep + seeded random-deck convergence gate; `DiverseAiSoundnessTests`
  horizon/loss oracle-equality across diverse AI (no unsoundness found).

### Next to-dos (forward, ordered — Ranwid accuracy + speed)

1. **Accuracy — model a run's ACTUAL relics** (narrowly): the diagnosed pessimism cause. ranwid counts only
   Burning Blood, so a relic-leaning deck reads weaker than it plays. Model the combat-start relics a run holds
   (revisiting the relic descope for combat-start relics ONLY — not the 300-relic catalog).
2. **Perf lever (deeper — the durable 2nd speedup):** advice parallelism caps ~4× (GC/memory-bandwidth bound),
   so the next big multiple needs CUTTING PER-SOLVE ALLOCATION — a clone-free rollout / APW prior (score a play
   without cloning the state) or make/undo instead of `CombatState.Clone`. ≤3–4 HP accuracy budget vs `--bridge`.
3. **VF / advisor quality:** VF distillation + survival recalibration (Platt/isotonic) + deck-composition features.
4. **Small leftover (low value):** a PreciseCut (hand-size) single-enemy live run.

---

## History (condensed)

Milestones (detail in git log + `~/.claude/plans/`): engine + exact solver + CLI + oracle/autopilot + headless
autonomy; MCTS (UCT*/DP-UCT, validated to converge); Act-1 elites 12/12 + Ascension/A10; content modularization;
the `ranwid` advisor + survival-first rollout + sound horizon bound (Weak / multi-enemy / in-search loss prune);
Phase-C learned value function; the lexicographic-objective question settled; **all five characters 88/88** +
Colorless/Special/Status/Token/curses + 21 multiplayer cards; the performance milestone (action-widening + PUCT,
lazy chance nodes → 1–2 s); mid-turn draws as chance nodes + post-draw resolution; Sly/Innate/Retain keyword
mechanics 1:1; the Defect orb subsystem; move-legality (Normality/Enthralled) + Whistle stun + Decimillipede
Reattach; a 37-card scaling/conditional audit; **a comprehensive optimistic-gap sweep** (every content + engine
module diffed vs the decompile — debuff tick-timing, The Bomb, Sealed Throne/Oblivion, Danse Macabre, FlakCannon,
DyingStar/SevenStars star-gates closed; VoidForm modelled 1:1; Decimillipede auto-bury characterized);
**VoidForm + Sly live-validated** (traces #79–80); **calibration expansion** (elite-AI sweep + seeded
random-deck convergence gate); **a search-soundness audit** (diverse-AI horizon/loss oracle-equality, no
unsoundness found).
