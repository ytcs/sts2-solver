# STS2 Solver — Project Status

_Last updated: 2026-06-07._

## Goal

A **standalone solver** that, given a deck and an encounter, computes the **HP lost under optimal play** in Slay
the Spire 2. Optimal play is **lexicographic: maximize win probability first, then minimize expected HP loss**.
Combat is stochastic (monster-move RNG, draw order), so the answer is an expectation over outcomes. The solver
is a faithful re-implementation of the game's combat engine (ported 1:1 from decompiled C#), driven by
expectimax + MCTS search, and **differentially validated against the real game** via a recorder mod + a headless
harness. The decompile is the **spec**; the real game is the **oracle** (see "Why reimplement" below).

## Ranwid TUI migration (`--tui`) — IN PROGRESS, resume here next session

Migrating the Ranwid live companion from the Spectre immediate-mode screen to a **Terminal.Gui v2** front-end
(`ranwid --tui`). All 9 requested UX items + the save dialog are **implemented and committed** (10 checkpoints,
`55c2695`→`0dadc11`); the backend is unit-tested and the **full suite is 1185/1185 green on net10**. The
interactive UI is **compile/init-verified only** — it has NOT been driven in a real terminal yet.

**Done (committed):**
1. Non-blocking spine — persistent panels, per-elite + strength evaluate on background tasks and stream in
   (`Application.Invoke`, generation-guarded); UI stays navigable while solving.
2. Navigable elites with `[x]/[ ]` include/exclude toggles (space) that recompute current-act strength live.
3. Type-grouped, coloured deck panel (skimmable) via `DeckView`.
4. `this act` / `next act` strength bars — next-act = `Advisor.DeckStrengthNextAct` (avg over next act's elite
   pool + ONE averaged-boss term; current act keeps only the known boss).
5. Boss in its own panel above the elites.
6. `r`/`u`/`c` advice overlay (removal/upgrade/reward) showing this-act + next-act strength **and deltas**;
   persistent deck/strength panels stay put (consistent layout).
7. `+/-` stepper for 1–3 cards (`Advisor.RankMoveSets`: exhaustive within a cap, else beam search, flagged
   "heuristic"); reward-check (`c`) takes typed offered cards (CardNameMatcher auto-correct).
8. Save-not-found panel: retry / enter folder / quit.

**NEXT SESSION — resume checklist:**
- [ ] **Verify `ranwid --tui` in a real terminal** (can't be done headless): focus/arrows, space-toggle fires
      ONCE (self-rendered checkbox, but watch for a ListView default Space binding double-firing), `+/-` stepper,
      reward/path-field Enter submit, colour legibility on the user's terminal.
- [ ] **Flip the default** to `--tui` + add a `--classic` Spectre escape hatch (≈5-line change in `Program.cs`) —
      held back until the above is verified (shipping testers an unverified default is hard to reverse).
- [ ] **`--custom` sandbox parity** in the TUI (still Spectre-only).
- [ ] Migrate off the deprecated static `Application.*` API once the instance API exposes a clean `Shutdown`
      (currently suppressed with `#pragma warning disable CS0618` + a note in `RanwidApp`).

**Decisions to revisit:** the TUI's *current-act* strength averages over the run's **curated actual elites +
known boss** (so the toggle is meaningful), a deliberate shift from the Spectre path's representative-elite pool —
confirm this is the intended basis.

**Toolchain note:** `Sts2Solver.Ranwid` + `Sts2Solver.Tests` are **net10** (Terminal.Gui v2 is net10-only);
everything else stays net9. Requires `dotnet-sdk-10.0` installed alongside net9. Code lives in
`solver/Sts2Solver.Ranwid/Tui/` (`RanwidApp`, `TuiState`, `TuiFormat`, `DeckView`); backend additions are in
`Advice.cs` (`DeckStrengthNextAct`, `RankCandidates`, `RankMoveSets`, `CardMove`) and `Companion.cs`
(`EvaluateRow`/`EliteRowSpecs`/`BossRowSpec`/`EncountersForClasses`). New tests in `AdvisorTests.cs`.

**Separately open:** Windows code-signing (metadata + no-compression already landed in the csproj; actual signing
via Azure Artifact Signing / `dotnet sign` not started).

## Current state (summary)

- **Content — every in-scope card ported (573/577).** Five characters complete (88/88 each): Ironclad, Silent,
  Regent (Stars + Forge/Sovereign Blade), Necrobinder (Osty pet + Doom), Defect (full orb subsystem + Focus) —
  plus the full Colorless pool (53/53), Status (12) + Token (14) pools, the Event/Ancient "Special" pool, curses
  (18), and all 21 multiplayer-only cards (modelled as their single-player projection). Out of scope: 3 Quest/map
  items (ByrdonisEgg/LanternKey/SpoilsMap) + MadScience (RNG card-gen). Monsters: Act-1 elites 12/12 + the
  normal-monster set (all trace-validated) + 8 ported bosses (WaterfallGiant/SoulFysh/LagavulinMatriarch/
  CeremonialBeast/Vantom/KnowledgeDemon/TheInsatiable/Aeonglass, unit-tested; the 2 optimistic gaps (Ringing,
  FranticEscape) are closed and the run's act boss is now folded into the deck-strength pool).
- **Zero known OPTIMISTIC (over-crediting) gaps** in the catalog or engine. Every remaining modelling shortfall is
  PESSIMISTIC (under-credits the player) and therefore sound — see "Known pessimistic gaps" below.
- **Search:** the exact lexicographic expectimax `Solver` is the ground-truth oracle; the sampling `MctsSolver`
  (UCT*/DP-UCT, action progressive widening + lexicographic PUCT) is gated to converge to it byte-for-byte. A
  30-card-vs-elite solve runs ~1.5–2.7 s. Sound `HorizonBound` + `LossCertificate` pruning (value-preserving).
  The MCTS leaf is the **faithful greedy rollout** (the only leaf — the static-heuristic and learned-VF leaves
  were removed: both badly mis-estimated big decks, ~18 HP off the rollout, and weren't in the production path).
- **Advisor:** `ranwid` live companion — reads the unmodded save, benchmarks the deck vs the Act's elites,
  recommends card removals + reward take/skip.
- **Combat relics — batches 1–5 (81) ported + live in ranwid.** The combat-affecting relic pool is now modelled
  (was: only the 5 starter relics). 180/298 game relics are combat-affecting; batches 1–5 cover the highest-value
  sound subset (combat-start stat/block/power grants, turn-numbered + every-N-turns energy/block/damage/draw,
  on-play/on-exhaust/end-of-turn triggers, stateful every-Nth-play and damage/stars/play-count reactors, passive
  damage/energy/draw/HP-loss modifiers). ranwid already feeds a run's ACTUAL relics through `BuildPlayer` (the
  wiring was generic) — registering a relic makes the advisor pick it up automatically, so deck strength now
  reflects these relics instead of "ignoring" them.
- **Tests: 1108 passing, 0 skipped/failed. Traces: 81 recorded game traces (incl. the PreciseCut hand-size
  live-validation vs TerrorEel), all PASS.**

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
- `ranwid`: live **Spectre.Console dashboard** for a run of **any of the 5 characters** (not just Ironclad) —
  headlines the **deck-strength index** (0–100), always shows the current deck + per-elite survival/HP-loss,
  auto-refreshes on save change (FileSystemWatcher + poll fallback). Removal advice is off by default (slow) and
  on the `[r]` key; `[c]` checks a reward card (auto-correct entry). **Removal + reward advice both rank by deck
  strength** (consistent with the headline). No engine/algorithm text in the UI; a prominent "Ignored" panel
  surfaces unmodelled relics / unported cards. `ranwid --preview` shows the layout with sample data.
  **`ranwid --custom [character]`** is a save-less deck sandbox (start from a starter deck, add/remove cards by
  hand) — the way a multiplayer GUEST, whose run is never saved locally, still gets deck-strength advice.

---

## How to run

```bash
cd solver/Sts2Solver.Tests && dotnet test -c Release            # build + full suite (~13 min)
dotnet run -c Release --project solver/Sts2Solver.Cli -- --demo --lines
dotnet run -c Release --project solver/Sts2Solver.Cli -- scenario.json [--mcts --trials N --anytime]
dotnet run -c Release --project solver/Sts2Solver.Cli -- --calibrate [--archetype block]
dotnet run -c Release --project solver/Sts2Solver.Cli -- --bridge    # advice-regime accuracy(noise/bias)+latency instrument
dotnet run -c Release --project solver/Sts2Solver.Cli -- --validate [trace.jsonl]   # one file or all
dotnet run -c Release --project solver/Sts2Solver.Ranwid                            # live companion (Spectre)
dotnet run -c Release --project solver/Sts2Solver.Ranwid -- --tui                   # Terminal.Gui dashboard (net10)
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

### Focus & scope

The user-facing product is **`ranwid`**, and its two pain points are **output accuracy** and **speed**. Accuracy
took the big step this round: **81 combat relics ported + live in the advisor**, the **8 bosses' optimistic gaps
closed and folded into deck strength**, and **PreciseCut live-validated** (which also required fixing the headless
harness for the 2026-06 game patch). **Still descoped (not worth the complexity):** potions, the long tail of
cosmetic/economy/map/reward/rest relics, RNG-card/orb-generation relics (optimistic direction — see to-do 1), and
the multi-monster boss *ports* (TheKin/KaiserCrab/Queen). Speed was researched (to-do 3): one safe micro-opt
shipped; the durable lever (make/undo rollout) is documented but deferred behind the correctness bar.

**TUI (`--tui`, Terminal.Gui v2):** an alternate, non-blocking front-end in `Sts2Solver.Ranwid/Tui/`
(`RanwidApp` + `TuiState`/`TuiFormat`/`DeckView`). Persistent panels (deck-strength `this act`/`next act`
bars, type-grouped deck, boss, elites); each elite + the strength index evaluate on background tasks and
stream in (`Application.Invoke`, generation-guarded) so the UI stays navigable. Elites toggle in/out of the
current-act strength (space); `r`/`u`/`c` open a removal/upgrade/reward advice overlay showing this-act + next-act
strength and deltas, with a `+/-` stepper for 1–3 cards (`Advisor.RankMoveSets`, exhaustive within a cap else a
beam search). Save-not-found offers retry / manual-path / quit. Reuses the whole `Companion`/`Advisor` compute
layer; the Spectre companion remains the **default** until the TUI is verified in a real terminal and the default
is flipped (with a `--classic` escape hatch). Backend additions are unit-tested; the interactive UI is not.

**Toolchain (split TFM):** `Sts2Solver.Ranwid` and `Sts2Solver.Tests` target **net10.0** (Ranwid's TUI uses
Terminal.Gui v2, which is net10-only; Tests reference the Ranwid exe so they follow). Engine/Content/Search/Cli
stay **net9.0** — a net10 app references them unchanged (backward compatible), and identical IL means test
behaviour is unaffected. Requires the **net10 SDK** installed alongside net9 (`dotnet-sdk-10.0`). The exe carries
populated Win32 version metadata (`<Version>`/`<Company>`/… in the csproj) and publishes WITHOUT single-file
compression — both reduce antivirus false positives on the self-contained binary.

**Shippable executables** (for testers): `ranwid` is published self-contained single-file (~73 MB) for both
RIDs and copied to the repo top as `./ranwid` (Linux ELF) and `./ranwid.exe` (Windows PE) — both gitignored, both
include `--custom`. Rebuild: `dotnet publish Sts2Solver.Ranwid -c Release -r {linux-x64|win-x64} --self-contained
true -p:PublishSingleFile=true` (run the two RIDs SEQUENTIALLY — parallel publishes race on the shared Content
intermediate and fail `GenerateDepsFile`). Windows save-dir auto-detection (`SaveLocator`) probes the **confirmed**
path `%APPDATA%\SlayTheSpire2\steam\<id>\…` (mirror of the Linux layout) first, with profile/Steam-registry
fallbacks and a validated manual folder prompt that persists the choice; modded-profile saves are excluded on
purpose. Multiplayer **guests** have no local run-save — use `ranwid --custom`. See `Sts2Solver.Ranwid/SHIPPING-WINDOWS.md`.

### Recently completed (2026-06 — bosses, enchantments, MCTS-only output, v0.107 sync)

- **All 12 bosses now modelled — the 4 remaining ported (TheKin, KaiserCrab, TestSubject, Queen).** Verified
  against decompiled `sts2.dll v0.107.0` via `ilspycmd` (run with `DOTNET_ROLL_FORWARD=LatestMajor`). Each came
  with a small gated engine subsystem + literal-value tests:
  - **TheKin** (Act 1, multi-monster): KinPriest + 2 KinFollowers, deterministic cycles. (`TheKinTests` 15.)
  - **KaiserCrab** (Act 2): Crusher + Rocket. The "Surrounded" facing back-attack (×1.5 from the arm you're NOT
    facing) via a gated `CombatState.KaiserFrontId` tracker (set in `PlayCard`; the arm power is
    `KaiserBackAttackPower`) + `CrabRagePower` (survivor +6 Str / +99 Block when one arm dies, via
    `AfterCreatureDeath`). (`KaiserCrabTests` 15.)
  - **TestSubject** (Act 3): a 3-form revive boss via a generic `PowerModel.VetoLethalDamage` death-veto +
    `Monster.Respawns`/`ExtraHits` fields; powers `AdaptablePower` (revive to next form), `EnragePower`,
    `PainfulStabsPower` (Wound on a connecting hit, per-turn), `NemesisPower` (Intangible every other turn via
    self-toggled `ModifyHpLost`). (`TestSubjectTests` 11.)
  - **Queen** (Act 3): Queen + TorchHeadAmalgam; conditional move branch on Amalgam death
    (`QueenAmalgamWatchPower` + `RandomBranchState` 1/0 weights); `YOU_ARE_MINE` 99 Frail/Weak/Vulnerable.
    **ChainsOfBinding is a documented pessimistic gap:** faithful binding hooks every draw (incl. the turn-start
    hand, modelled as chance nodes), so it's modelled SOUNDLY as drawing 3 fewer cards/turn via `ModifyHandDraw`
    (`QueenChainsPower`). (`QueenTests` 11.)
  - Boss-pool placement is fallback-only (real runs resolve by `boss_id`); all 4 registered in `EncounterCatalog`.
- **Card enchantments (15 ported) + a generic `CardEnchantment` system.** New base `CardEnchantment` (Engine)
  with damage add/mult, block, keyword (Retain/Innate/Exhaust-removal), cost, on-play rider, bonus-play, and
  one-shot/ramp state; folded into `CardModel.StateKey`/`Stateful`/`Clone` and consulted by `Cmd.Attack`/
  `GainBlock` + `CombatManager.PlayCard`. Threaded through the build spec (`Name+U@Sharp:3`), `RunSave` (parses
  `{id, amount}`), and `Companion` (warns on unmodelled). Ported from the v0.107.0 decompile: Sharp, Inky,
  Instinct, Corrupted, TezcatarasEmber, Nimble, Adroit, Steady, RoyallyApproved, SoulsPower, Sown, Vigorous,
  Momentum, Spiral, Glam. Warned-as-unmodelled: Slither/Goopy/PerfectFit/Clone + Swift/Imbued/SlumberingEssence
  (need draw / auto-play / per-turn-held-cost machinery). (`EnchantmentTests` 17.)
- **Ranwid output: MCTS-only, Expected HP Loss.** Dropped the rollout HP-loss distribution (Best/Worst columns,
  `HeuristicPolicy`, `EvalOptions.Rollouts`, the `CombatStats` spread fields). The dashboard's "Avg" was the
  search value while Best/Worst came from a separate heuristic-policy rollout — two estimators with no ordering
  guarantee (hence "Best < Avg"). Now one number from MCTS, so the anomaly is structurally impossible.
- **Boss-mapping fixes (two bugs).** (a) `GameIds.ClassName("ENCOUNTER.AEONGLASS_BOSS")` already yields
  `AeonglassBoss`; the code appended `+ "Boss"` → `AeonglassBossBoss`, so the actual-boss path ALWAYS failed and
  fell back. (b) The fallback used `run.ActIndex` against the theme-aligned pool and silently substituted a wrong
  boss. Now: resolve `boss_id` directly; ported boss → shown/scored; known-but-unported → "Boss: X — not
  modelled" (never substituted). (`BossId_*` tests.)
- **Aeonglass synced to v0.107.0 + HP convention.** Decompile confirmed EBB is now attack + `EbbBlock => 33`
  (the −3 Str/−3 Dex drain was REMOVED) and Increasing Intensity no longer blocks. Fixed PhrogParasite (61→64)
  and PhantasmalGardener (28→31) to the max-roll convention.
- **DataDumper extended:** `scalars` (numeric members) on `monsters.json` + a new `enchantments.json` (type set +
  LocStrings + magnitudes). Builds against the patched game DLLs.
- Full suite **1183 tests green**; `ranwid`/`ranwid.exe` republished.

### Earlier this session (paused here)

- **Bosses + PreciseCut + perf (earlier session).** Closed the 2 boss optimistic gaps (CeremonialBeast Ringing → a
  per-turn play cap via `PowerModel.PlayCapThisTurn`/`EffectivePlayCap`; TheInsatiable FranticEscape → a Stateful
  per-instance cost-ramp on the boss-only card) and wired the run's act boss into the deck-strength `StrengthPool`
  (`Catalog.ActBossPool` + `Companion.BossEncounterForRun`). Fixed the headless harness for the 2026-06 game patch
  (the `RunManager.SetUpNewSinglePlayer`→`SetUpNewSingleplayer` rename broke the DataDumper mod; rebuilt against the
  patched DLL) and live-validated **PreciseCut** end-to-end (6-turn Silent-vs-TerrorEel trace, `--validate` 43/43 —
  the hand-size damage is exact across 10 plays). Perf: researched 3 angles in parallel (learned leaf → park;
  make/undo rollout → the durable ~1.5–2× lever, deferred; result-preserving micro-opts → ~15%); shipped the
  zero-risk `Score` LINQ-inline. See to-dos 2–4 above for detail.
- **Combat relics — batch 5 (11 relics): every-N-turns relics + damage/stars/play-count reactors.** Two patterns,
  both needing NO new engine hooks. (a) "Every N turns" relics need no state at all — they read `combat.TurnNumber`
  directly (the game's per-turn counter fires on turns N, 2N, … ≡ `TurnNumber % N == 0`): HappyFlower (energy/3),
  FakeHappyFlower (energy/5), Pendulum (draw+1 /3), PollinousCore (draw+2 /4). (b) Reactors install a hidden
  hashed-state power on the existing power hooks: CentennialPuzzle (first unblocked hit → draw 3, `AfterDamage
  Received` flag), DemonTongue (first unblocked hit/turn → heal it, per-turn flag), GalacticDust (per 10 Stars
  spent → 10 block, `AfterStarsSpent` counter), MiniRegent (first Stars spent/turn → Str, per-turn flag),
  BeatingRemnant (cap HP loss at 20/turn — `ModifyHpLost` cap + `AfterDamageReceived` accumulator), Vambrace
  (first block card/combat → ×2 block, `ModifyBlockMultiplicative` + flag set in `AfterBlockGained`), ThrowingAxe
  (first card/combat played twice — `ModifyCardPlayCount` + flag in `AfterModifyingCardPlayCount`). `RelicTests5`
  (22); full suite 1097.
- **Combat relics — batch 4 (9 relics): stateful every-Nth-play / once-per-combat counters.** Modelled via hidden
  hashed-counter relic POWERS (a relic is shared/immutable and can't hold a counter; a power is cloned + hashed,
  and present only when the relic is). `RelicPlayCounterPower` base counts qualifying plays and fires every Nth
  (counter subtracted, not modulo-grown, so it stays bounded in [0, N) ⇒ stable hash): Kunai (3 attacks/turn →
  Dex), Shuriken (3 attacks/turn → Str), OrnamentalFan (3 attacks/turn → 4 block), LetterOpener (3 skills/turn →
  5 AoE), Nunchaku (10 attacks → energy), TuningFork (10 skills → 7 block), IronClub (4 cards → draw 1). Per-turn
  counters reset at the owner's turn start; per-combat ones persist. Plus once-per-combat flag relics Permafrost
  (first Power → 7 block) and RainbowRing (all-3-types-in-a-turn → Str+Dex). All counters/flags fold into
  StateKey + HashValue. `RelicTests4` (20); full suite 1075.
- **Combat relics — batch 3 (35 relics) + stateless relic event-hook infrastructure.** Added stateless relic
  event hooks to `RelicModel` (`BeforeCardPlayed`/`AfterCardPlayed`/`AfterCardExhausted`/`BeforeSideTurnEnd`/
  `AfterSideTurnEnd`/`ModifyHandDraw(combat)`/`ModifyPowerAmountGiven`), fired alongside the power hooks. A relic
  is SHARED + immutable (Player.Clone shares instances), so only STATELESS handlers live here — they need no
  clone/hash; stateful (counter) relics will install hidden powers instead (batch 4). Ported: combat-start
  (FakeAnchor, TwistedFunnel, energy relics BloodSoakedRose/PrismaticGem/Sozu, Fiddle), turn-numbered
  (FakeBloodVial, DivineDestiny, Bread, PaelsFlesh, CaptainsWheel, HornCleat, SparklingRouge, MercuryHourglass,
  MrStruggles, RoyalPoison, RunicCapacitor), turn-1 draw (BagOfPreparation, RingOfTheSnake, RingOfTheDrake,
  BigMushroom −2), end-of-turn (Orichalcum, FakeOrichalcum, CloakClasp, RippleBasin, ScreamingFlagon,
  StoneCalendar, LunarPastry), on-play (IntimidatingHelmet, IvoryTile, DaughterOfTheWind, LostWisp, GamePiece),
  on-exhaust (CharonsAshes), power-amount (SneckoSkull). **Perf gate (important):** the event-hook loops fire in
  hot paths (per card play / power apply), so they're gated on `CombatState.HasEventRelics` — true only when a
  relic that overrides an event hook is present (event relics extend the `EventRelic` base). A deck with no such
  relic (the common case, incl. the calibration fixtures' BurningBlood) pays ZERO per-node overhead, so exact-
  solve speed and its wall-clock budgets are unchanged. `RelicTests3` (68); full suite 1055.
- **Combat relics — batch 2 (5 relics): HP-loss reducers + passive modifiers, all via existing power hooks.**
  TungstenRod (lose 1 less HP from every source — `ModifyHpLost`, the hook Intangible uses), TheBoot (your
  unblocked 1–4 hits to enemies become 5 — same `ModifyHpLost`, which also fires for damage dealt to enemies),
  SpikedGauntlets (+1 energy AND Power cards cost +1 — both modelled: `ModifyMaxEnergy` + `ModifyCardCost`, the
  latter visible to affordability in search), PaelsBlood (+1 draw every turn — `ModifyHandDraw`, like
  MachineLearning), BlessedAntler (+1 energy AND 3 Dazed shuffled into the draw pile — the dilution downside is
  modelled so the energy isn't free). New relic powers: `RelicHpLossReductionPower`, `RelicMinDamagePower`,
  `RelicPowerCostSurchargePower`, `RelicDrawPower`. RelicTests now 51; full suite 987.
  **Deferred (need engine support, not yet built):** (a) turn-conditioned passives — Bread, BagOfPreparation,
  RingOfTheSnake/Drake (the `ModifyMaxEnergy`/`ModifyHandDraw` power hooks don't receive the turn number, and
  `Creature` has no `CombatState` back-ref) → need a turn-aware relic hook; (b) PhilosophersStone (all enemies
  gain Strength incl. mid-combat SUMMONS — modelling only the initial enemies would UNDER-credit summoned enemies
  = optimistic/unsound, so it waits for a summon hook); (c) SneckoSkull (`ModifyPowerAmountGiven` hook missing);
  (d) BeatingRemnant (stateful per-turn HP-loss cap — needs a hashed counter).
- **Combat relics — batch 1 (21 relics) + the relic-modelling foundation.** Diagnosed that the to-do is almost
  entirely a CONTENT port: ranwid's relic path was already generic (`Companion.Load` maps `run.RelicIds` →
  `GameIds.ModelledRelicName` → `Catalog.IsModelledRelic`, feeds the survivors into `BuildPlayer`; warns on the
  rest), so registering a relic under its game class name makes the advisor model it with ZERO ranwid changes.
  **Architecture (no new engine pipeline):** the game implements most relics by applying powers, so each relic
  either (a) applies an existing power at combat start via `Cmd` (Vajra→Strength, BronzeScales→Thorns, Anchor→
  Block, Akabeko→Vigor, DataDisk→Focus, Gorget→Plating, BagOfMarbles/RedMask→enemy debuffs, BloodVial→Heal), or
  (b) installs a tiny hidden "relic power" carrying a passive modifier (`RelicMaxEnergyPower` for Ectoplasm;
  `RelicStrikeDamagePower` for StrikeDummy/FakeStrikeDummy; `RelicUpgradedDamagePower` for MiniatureCannon) —
  reusing the already-cloned + hashed power pipeline, so it is sound for search/memoisation for free and inert
  for any deck without the relic. Turn-numbered/recurring relics (Sai 7-block/turn, Brimstone +2 self/+1-enemy
  Str/turn, Lantern/VeryHotCocoa/Candelabra/Chandelier energy on turn 1/1/2/3, FestivePopper 9-AoE turn 1) use
  the existing `OnPlayerTurnStart` relic hook (fires after the turn-start energy reset + block clear).
  **Soundness:** all 21 are deterministic ⇒ exact for the objective (no RNG card/orb gen); enemy-side downsides
  (Brimstone's enemy Strength) ARE modelled so a relic can't read as a free upside. New: `Relics/CombatRelics.cs`,
  `Relics/RelicPowers.cs`, `Core/RelicCatalog.cs` (central `RelicFactories`, moved out of `IroncladCatalog`),
  `RelicTests.cs` (41). Full suite 977.


- **`ranwid --custom [character]` — save-less deck sandbox.** Diagnosed why ranwid couldn't see the user's
  multiplayer run: a multiplayer **GUEST** never gets a local `current_run.save` (only the HOST's game writes one),
  so there is nothing on disk for the watcher to read (confirmed: the guest's profile only updates
  `progress.save` meta-stats + a binary `replays/latest.mcr`). (A first attempt — including modded-profile saves —
  was the wrong fix and was reverted; the modded exclusion stays.) The real fix is a save-less mode: start from any
  character's starter deck + relic + HP and edit it by hand (`+card`/`add` with Tab-complete + typo auto-correct,
  `-card`/`rm`, `act <1-4>` to choose the elite pool, `char`/`hp`/`asc`/`reset`, plus `r` cuts / `c` reward check),
  with the same deck-strength dashboard updating live. Content gains a `CharacterProfile` accessor (starter specs /
  relic / HP / energy for all five) + `Catalog.ActThemes`/`ActElitePool`; `Companion.Load` refactored into
  `Load` + a shared `BuildContext`. `RanwidCustomDeckTests` (15). Both executables rebuilt with `--custom`.
- **Bosses — 8 ported + reusable engine death-phase support.** Added a survive-at-0 → telegraph → final-blow →
  die primitive (`Monster.DeathPhaseEntryMove`/`InDeathPhase`, virtual `Creature.IsAlive`, `Cmd.ApplyDamage`
  hand-off — mirrors Decimillipede Reattach) for explode/transform-on-death bosses, first used by **WaterfallGiant**
  (Steam Eruption modelled FAITHFULLY: Pressurize +15/+20, +3/move counter, EXPLODE = accumulated). **Soundness
  correction (via a pilot):** for a MONSTER, under-crediting (omitting damage) is the OPTIMISTIC/UNSAFE direction —
  the rule is now "never under-credit a monster; over-estimate threat when uncertain." Ported under this rule
  (own-file powers, conflict-free parallel orchestration): WaterfallGiant, SoulFysh, LagavulinMatriarch (Act-1),
  CeremonialBeast, Vantom (Act-2), KnowledgeDemon, TheInsatiable (Act-3 Hive), Aeonglass (Act-4 Glory).
  **Two flagged optimistic gaps to close before bosses enter the deck-strength pool:** CeremonialBeast's Ringing
  (each card once/turn — needs an engine play-restriction hook, task #11) and TheInsatiable's FranticEscape
  +1/play cost-ramp (shared card out of scope — needs a per-card combat-cost-growth primitive). Still to port:
  TestSubject + the multi-monster bosses (TheKin / KaiserCrab / Queen) + hard multi-enemy normals; combat relics
  batch-ported separately. (`BossTests` + per-boss tests; full suite 936.)
- **`ranwid` now supports all 5 characters** (was Ironclad-only — an artificial gate; the engine ports all
  88/88×5). Removed the gate (`GameIds.IsSupportedCharacter`), made relic mapping generic
  (`Catalog.IsModelledRelic` — picks up each character's combat-start starter relic that wires up Stars / Osty /
  orbs), card parsing was already generic. `RanwidMultiCharacterTests` (19) gate the mappings + an end-to-end
  solve per character.
- **Removal + reward advice now rank by the deck-strength index** (was bottleneck-survival + total-loss at
  current HP) — consistent with the dashboard headline, and HP-independent so advice is stable across a run.
  Removed the now-dead `DeckScore`/`SurvivalBand`/`ScoreDeck`. Advice still uses the 800-trial ranking budget +
  the parallel candidate grid.
- **Fixed the tab-complete UI bug**: `LineEditor.Redraw` now clears the line (CR + erase) before rewriting, so
  completions no longer garble into `cards> Xcards> XY…`.
- **Removal advice shows every cut, best-first** (was: only strictly-improving removals, which hid basic
  Strike/Defend since cutting them rarely *raises* single-combat strength). Now ranks all removable cards by
  resulting deck strength with a signed Δ (green improve / red harm) — so you can pick the least-harmful cut at
  a removal site even when nothing strictly improves.
- **Dashboard refresh is now input-cached**: a save write that doesn't change the numbers (gold / map move /
  an HP tick) no longer burns a ~6-eval refresh. The per-elite rows recompute only on deck/relic/HP/energy/asc
  change; the HP-independent deck strength recomputes only on deck/relic/energy change. `[d]` forces a refresh.
- **Explored the next speedup beyond ~30× — parked (no clean win).** `--profile` shows the rollout playout is the
  dominant cost; three probes, all negative: (A) `decimal`→`double` damage pipeline — ~2–5% ceiling (arithmetic
  is a tiny slice; `decimal` is 29× slower/op but rarely hit) AND faithfulness-blocked (`0.7m`/`0.1m` aren't
  binary-exact); (B) rollout candidate-pruning — ~20% faster but −19%+ survival error on large fights (blinds the
  greedy policy); (C) distilled cheap leaf — this is exactly the just-removed `LearnedValue` (a simple model
  reads 82%/38 vs the rollout's 100%/19.5 on big decks; matching it needs a much stronger model + carries
  systematic-bias risk). Kept a decimal-vs-double microbench in `--profile`; reverted B's knob. Decision:
  **stop at ~19s** (the ~30× already unblocks the advice features).
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

### Next to-dos (forward, ordered)

1. **Combat relics (batch port) — CORE COMPLETE (batches 1–5 done: 81 relics). Long tail deferred/out-of-scope
   below.** The user's headline ask: deck strength falls off in later acts because relics aren't modelled. **The
   ranwid side was already generic** (feeds a run's actual relics into `BuildPlayer`); the work was the engine
   port. **All common combat-relic mechanics are now modelled** (combat-start stat/block/power grants, turn-numbered
   + every-N-turns energy/block/damage/draw, on-play/on-exhaust/end-of-turn triggers, every-Nth-play counters,
   damage/stars/play-count reactors, passive damage/energy/draw/HP-loss/cost modifiers). Infrastructure added:
   stateless relic event hooks (`EventRelic` + `HasEventRelics` perf gate), the `RelicPlayCounterPower` hashed-
   counter base, and several hidden relic powers. Of 298 game relics, 180 are combat-affecting; the remaining ~95
   split into:
   - **Deferred-portable (sound, but each needs a specific new engine hook — low frequency, diminishing value):**
     **LizardTail** (first-death → survive at 50% — highest value; needs a death-prevention hook in the HP-loss
     path, the riskiest core change, do carefully); **RedSkull** (Str while HP≤50% — needs an HP-threshold toggle);
     **ToughBandages** (discard → block — needs an `AfterCardDiscarded` hook); **IceCream** (energy carries between
     turns — needs an energy-reset change); **PenNib** (×2 every 10th attack — "double next attack" marker);
     **RuinedHelmet** (first Str gain ×2 — needs `ModifyPowerAmountReceived`); **ChemicalX** (X-cost +2 — needs
     X-cost cards modelled); **VitruvianMinion** (Minion ×2 — needs a Minion card tag); **PhilosophersStone**
     (enemy Str incl. summons — needs a summon hook); **BrilliantScarf/DiamondDiadem** (per-turn play-count gates);
     **ArtOfWar/Pocketwatch** (cross-turn memory); Defect-orb ones (**Metronome/GoldPlatedCables/InfusedCore/
     SymbioticVirus** — orb-channel counters); **SturdyClamp/VelvetChoker** (block-retention / play-cap);
     **LunarPastry/GalacticDust** already done; **HandDrill** (block-break detection); **BoneFlute/BookRepairKnife**
     (Necrobinder pet/Doom hooks).
   - **Out of scope by doctrine (NOT soundly portable):** RNG card/orb GENERATION (Crossbow, OrangeDough, Toolbox,
     BigHat, ChoicesParadox, NinjaScroll, FuneraryMask, RadiantPearl, JeweledMask, VexingPuzzlebox, MusicBox,
     MummifiedHand, ForgottenSoul, Kusarigama, ParryingShield, PowerCell, FencingManual, WhisperingEarring — letting
     search SEE generated cards is the forbidden optimistic direction); **Confused/cost-randomising** (SneckoEye,
     FakeSneckoEye); **unmodelled-power** (SelfFormingClay, MysticLighter's enchantments); and the cosmetic/economy/
     map/reward/rest/potion catalog + post-combat-only heals (MeatOnTheBone, Pantograph, BeltBuckle, …) — HP-neutral
     for a single-combat evaluation. These are "cleared": correctly left unmodelled (ranwid lists them as Ignored).
2. **Boss pool — the 2 gaps CLOSED + the 8 ported bosses WIRED into deck strength.** Done this session: (a)
   CeremonialBeast **Ringing** is modelled as a per-turn play cap of 1 (`PowerModel.PlayCapThisTurn` → mined into
   `CombatState.EffectivePlayCap`; applying it sets `BoundsPlays`); (b) TheInsatiable **FranticEscape cost-ramp**
   is modelled on the (boss-only) card itself (Stateful per-instance cost counter) so the Sandpit escape grows
   prohibitively expensive. Both close optimistic over-credits. The run's act boss is now folded into the
   `StrengthPool` (`Catalog.ActBossPool` + `Companion.BossEncounterForRun`, mapped from the save's `BossId` or the
   act fallback) so the index reflects boss-readiness. **Perf note:** the boss is the long pole of the parallel
   strength eval (tankier/longer fight) — one boss keeps it bounded; revisit if the dashboard feels slow.
   **Still to port (not blocking):** TestSubject + the multi-monster bosses (TheKin / KaiserCrab / Queen) + hard
   multi-enemy normals — large faithful AI ports, deferred.
3. **Perf (explored via parallel research — verdict + one safe win shipped).** Three angles researched: (i)
   **distilled value leaf → park permanently** (the removed LearnedValue failed because it was linear over
   one-turn-sim features trained on small decks; a viable leaf needs GBM/MLP + ~CPU-weeks of big-deck exact labels
   + conservative clipping + an optimistic-bias audit — poor ROI vs a 19s budget); (ii) **clone-free rollout
   (make/undo) → the durable ~1.5–2× lever** (rollout-only undo-log is safe — rollouts are throwaway, no memo
   aliasing — but ~300 lines + heavy Play↔Undo tests; deferred until latency is a hard blocker); (iii)
   **result-preserving micro-opts → ~10–16%** (cache IncomingDamage / kill `AllPowers.ToList()` allocs / gate
   empty-choice LINQ). **Shipped** the zero-risk subset: inlined the rollout `Score`'s 3 LINQ monster passes into
   one loop. The IncomingDamage cache + AllPowers-alloc + make/undo are documented as deferred (result-changing
   risk needs dedicated validation, not the byte-identical-by-construction guarantee the Score inline has).
4. **PreciseCut live run — DONE (and the headless harness fixed for the 2026-06 game patch).** The patch renamed
   `RunManager.SetUpNewSinglePlayer`→`SetUpNewSingleplayer`, breaking the DataDumper mod (hung at the main menu);
   fixed the mod + rebuilt against the patched DLL. Recorded a 6-turn Silent (PreciseCut+Strike+Defend) vs TerrorEel
   trace; `--validate` PASS 43/43 — TerrorEel's HP trajectory (107→71→41→11→dead) is the exact cumulative damage
   over 10 PreciseCut plays at varying hand sizes, confirming the hand-size mechanic (and that the patch left
   TerrorEel/PreciseCut/Silent unchanged). Promoted as `silent_precise_cut_terror_eel.jsonl` (trace #81).

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
