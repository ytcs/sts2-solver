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
  30-card-vs-elite solve runs ~1.5–2.7 s. Sound `HorizonBound` + `LossCertificate` pruning (value-preserving);
  opt-in Phase-C `LearnedValue` MCTS leaf for the razor-thin survival regime.
- **Advisor:** `ranwid` live companion — reads the unmodded save, benchmarks the deck vs the Act's elites,
  recommends card removals + reward take/skip.
- **Tests: 707 passing, 0 skipped/failed. Traces: 78 recorded game traces, all PASS.**

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
                                · HorizonBound/DeckProfile/MultiEnemy/LossCertificate · LearnedValue/VfTrainer
  Sts2Solver.Ranwid/ (ranwid)   live-run advisor (SaveLocator/RunSave/GameIds/Reporting/Advisor/Companion)
  Sts2Solver.Cli/   (sts2solve) solve · --validate · --calibrate · --profile/--converge · --train-vf
  Sts2Solver.Tests/             xUnit: pipeline + per-card + solver + trace-replay + MCTS + calibration + horizon
mods/DataDumper/DataDumperCode/ Godot C# mod: MainFile · CombatOracle (records traces) · AutoPilot (`autopilot`
                                console cmd) · HeadlessBatch (STS2_BATCH: headless run+fight+record+quit)
data/  game_data/*.json (dumped metadata) · combat_traces/*.jsonl (78 ground-truth traces) ·
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

Ordered by priority. Correctness work (the spine) is complete — the catalog + engine have **zero known optimistic
gaps**, so what follows is feature/quality expansion, all behind the standing correctness bar.

1. **Live-validate the still-unverified mechanics** via headless single-enemy runs (cheap, high-confidence):
   - A Sly-discard fight (e.g. CalculatedGamble + Untouchable/FlickFlack).
   - An Innate deck and a Retain deck.
   - The random-target (SerpentForm/Ricochet/RipAndTear/FlakCannon) and hand-size (PreciseCut) cards.
   - A VoidForm fight (confirm the first-2-cards-free + forced-end-turn against the oracle).
2. **Calibration expansion:** all 12 Act-1 elites + a random-deck generator feeding `--calibrate`, to widen the
   exact↔MCTS convergence evidence beyond the current archetype fixtures.
3. **Search-soundness audit (optional, different class):** the horizon bound, loss-prune, and MCTS widening have
   their own correctness proofs + test suites (`HorizonBoundTests`, `LossPruningTests`). Audit only if we want
   defense-in-depth on the algorithmic layer (not content fidelity).
4. **VF / advisor quality:** VF distillation + survival recalibration (Platt/isotonic) + deck-composition
   features; tighten `ranwid`'s `SurvivalBand`.
5. **Scope expansion (needs new subsystems):** more relics (combat-relevant relic hooks), Act-1 bosses, potions.
6. **Perf lever (deferred behind correctness):** cheaper rollout policy — top-k clone-free prior / learned
   action-value, ≤3–4 HP accuracy budget.

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
DyingStar/SevenStars star-gates closed; VoidForm modelled 1:1; Decimillipede auto-bury characterized).
