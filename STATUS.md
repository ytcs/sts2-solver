# STS2 Solver — Project Status

_Last updated: 2026-06-03. Recent: an OPTIMISTIC-gap sweep that closed EVERY over-crediting spot —
**Void** −1-energy-on-draw (new per-card `OnDraw` hook on all draw paths), **Hex** (all player cards Ethereal →
whole hand exhausts under Hex), **Dampen** (MagiKnight downgrades the player's upgraded cards), and **Reattach**
(Decimillipede segment revival — a downed segment strips its non-Reattach powers, skips one enemy turn, then
reattaches to 25; the board only clears when all segments are down together). Reattach took two cuts: the first
revived one turn too early and the oracle trace rejected it; the decompile's DEAD_MOVE→REATTACH_MOVE machine gave
the exact 2-turn delay, now trace-validated. Also MODELLED the previously-deferred draw-scaling:
**DeathMarch** `(8+U)+(4+2U)×mid-turn-draws` (new gated `CardsDrawnMidTurn` counter threaded through all four
draw paths) and **MachineLearning** +1 turn-start draw (`ModifyHandDraw`/`TurnStartDrawCount`). Earlier: a 37-card
scaling/conditional audit; Sly/Innate/Retain keywords 1:1; HiddenDaggers promotion; Murder live-validation._

## Goal

A **standalone solver** that, given a deck and an encounter, computes the **HP lost under optimal play** in Slay
the Spire 2. Optimal play is **lexicographic: maximize win probability first, then minimize expected HP loss**.
Combat is stochastic (monster-move RNG, draw order), so the answer is an expectation over outcomes. The solver
is a faithful re-implementation of the game's combat engine (ported 1:1 from decompiled C#), driven by
expectimax + MCTS search, and **differentially validated against the real game** via a recorder mod + a headless
harness. The decompile is the **spec**; the real game is the **oracle** (see "Why reimplement" below).

## Current state

- **Content — every card ported (573/577), including all 21 multiplayer-only cards.** Five characters complete
  (88/88 each): Ironclad, Silent, Regent (Stars + Forge/Sovereign Blade), Necrobinder (Osty pet + Doom),
  **Defect (orbs + Focus)** — plus the **full Colorless pool (53/53)**, the **complete Status (12) + Token (14)
  pools**, the Event/Ancient "Special" pool, and curses (18). The **21 multiplayer-only cards** are modelled as
  their single-player projection (effects on other players dropped; "all allies / any ally" resolves to you;
  self/enemy payloads kept) — see the Colorless block in `ColorlessCards.cs` and their home-pool files. The
  only 4 unported in-scope cards are 3 Quest/map items (ByrdonisEgg/LanternKey/SpoilsMap — not combat cards)
  and MadScience (documented out-of-scope RNG card-gen). Act-1 elites 12/12 + the normal-monster set,
  trace-validated against live recordings.
- **Defect — COMPLETE (88/88).** The full **orb subsystem** is built and gated (engine `Orbs.cs`): all five
  orb types (Lightning damage / Frost block / Dark accumulate→evoke-weakest / Plasma turn-start energy / Glass
  all-enemy decay), the FIFO slot queue (channel + overflow-evokes-oldest, evoke front/back, slots), **Focus**,
  turn-boundary passive triggers, the **AfterOrbEvoked** hook (Thunder) and the **AfterCardGenerated** hook
  (status-card generation → Smokestack/TrashToTreasure) + the CrackedCore starter relic. All 88 cards ported
  1:1, plus the status/token cards they make (Wound/Slimed/Void/Fuel). Orb-reactive powers modelled:
  Thunder/Hailstorm/Storm/Subroutine/Coolant/Smokestack/Loop/Spinner/LightningRod/BiasedCognition/
  ConsumingShadow/Buffer/Iteration + temporary-Focus (Hotfix/FocusedStrike/Synchronize), FreePower (Synthesis),
  SignalBoost/EchoForm replay. Two new **gated combat counters** (mirroring Murder/BeatIntoShape): Voltaic
  (Lightnings channeled this combat) and HelixDrill (energy spent this turn). X-cost orb cards (MultiCast/
  Tempest), Stateful cost-mutators (AdaptiveStrike free copy, MomentumStrike, Modded, Claw scaling), and
  `Monster.IntendsToAttack` (GoForTheEyes).
  - **MachineLearning's +1 turn-start draw is now MODELLED** (`ModifyHandDraw` hook → `CombatManager.TurnStartDrawCount`,
    wired at every turn-start draw site); upgraded MachineLearning is Innate. **Void's on-draw −1 energy is now
    MODELLED** (per-card `OnDraw`/`HasOnDraw` hook fired on every draw path) — the formerly-acknowledged lone
    optimistic gap is closed; there are now **zero known optimistic gaps in the catalog**.
  - **Documented PESSIMISTIC-sound gaps (under-credit, never optimistic):** Feral (free-replay return-to-hand),
    CreativeAi/WhiteNoise/Chaos (RNG card/orb generation — never a search decision), Uproar's auto-play,
    TrashToTreasure's random orb, RocketPunch's status-gen cost-reduction, GeneticAlgorithm's cross-combat
    scaling, Scrape's draw-then-selective-discard (modelled only under a concrete Rng; skipped in pure search),
    Hologram/Scavenge choices (fixed defaults).
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
- **Tests: 698 passing, 0 skipped/failed. Traces: 78 recorded game traces, all PASS** — including **a live
  Necrobinder DeathMarch run (#77)** that oracle-confirms the new draw-scaling: 7 Parse draws feed 4 DeathMarch
  plays across turns 3–5 vs Byrdonis, matching the game's HP/Strength/Osty-DieForYou 85/85 (this validates both
  the `CardsDrawnMidTurn` scaling AND the `ReplayMode` mid-turn-draw reconstruction). Plus a live
  headless Defect-vs-Byrdonis run (#74) that exercises the orb subsystem end-to-end (CrackedCore's starting
  orb, Zap channel, Dualcast evoke, ColdSnap Frost channel+block, BallLightning, and the Lightning/Frost
  turn-end passives), matching the game's HP/block/Strength across 4 turns 35/35. (The orb random-target
  default is exact for single-enemy, an approximation for multi-enemy — by design.) **Two new Colorless
  validations (#75–76):** a Whistle-stun deck where the game (like our engine) shows Byrdonis taking zero
  attacks across the fight — live-confirming the new bounded stun AND that Territorial Strength still ramps
  during a stunned turn (25/25); and a Prowess/EternalArmor/Shockwave power deck confirming the new powers'
  Weak/Vulnerable, Plating block, and Strength/Dexterity against the real game across 4 turns (43/43).

---

## Repo layout

```
solver/                         C#/.NET 9 solution
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
data/  game_data/*.json (dumped metadata) · combat_traces/*.jsonl (78 ground-truth traces)
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
  (Skills/Stars/Attacks/Discarded/Drawn[gated]/Ethereal/Osty/Doom, per-target PoweredHitsThisTurn[gated:
  BeatIntoShape], PendingDiscard + its continuation card).

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

**Priority: CORRECTNESS — 1:1-model the mechanics of every ported card.** The per-module fidelity audit plus a
follow-up **scaling/conditional-card audit** (all 37 ported cards that use a `WithMultiplier`/history-based
damage or block term, cross-checked formula-by-formula vs the decompile) found the catalog fidelity-correct.
**Crucially, no OPTIMISTIC (over-crediting / dangerous) gap exists** anywhere in the scaling set — every
remaining mismatch is PESSIMISTIC (under-credit, sound). Resolved this pass:

- **CrescentSpear — verified correct, no change.** `AllCards.Count(c => CanonicalStarCost >= 0 || HasStarCostX)`:
  base default −1 and no card has `CanonicalStarCost == 0`, so `>= 0` ≡ our `StarCost > 0`; `AllCards` includes
  the in-play PlayPile card ⇒ our `+1` is exact.
- **BeatIntoShape — history-based Forge modelled exactly.** Forge = `CalcBase + CalcExtra×priorHits`
  (`CalcBase==CalcExtra==5, +2/upg`), priorHits = powered Move hits the player dealt this target this turn
  before this card. (Was the priorHits=0 floor — a *pessimistic* under-forge of the beneficial Sovereign Blade,
  not the "over-forge" STATUS once claimed.) Gated per-creature counter `Creature.PlayerPoweredHitsThisTurn`
  (`CombatState.TracksPoweredHits` / `CardModel.TracksTargetPoweredHits`, mirroring the Murder gate).
- **Stomp — upgrade number fixed** `12 + 2·U → 12 + 3·U` (decompile `Damage.UpgradeValueBy(3)`); upgraded Stomp
  now deals 15 to all enemies. Same wrong-number class as commit d2bb41e.

Remaining (all lower-value; #1–2 inert-but-sound, #3 needs the game you're playing):

1. **Single-turn Sly grant** (HandTrick selects a Skill, MasterPlannerPower grants Sly to played Skills) — still
   inert (pessimistic: under-credits a beneficial free auto-play). A full fix is high-cost (mutable per-card Sly
   flag ⇒ Stateful; HandTrick's selection is a player CHOICE node; benefit only materialises via a mid-turn
   discard) and a careless partial fix risks the forbidden optimistic direction. Deferred deliberately. Low freq.
2. **`Purity` variable-count exhaust-of-choice** (0..N) — Retain now set, but the exhaust selection is a fixed
   default; HP-neutral without on-exhaust powers (Feel No Pain / Dark Embrace). Low value.
3. **Live-validate the new mechanics** via headless single-enemy runs: a Sly-discard fight (e.g. CalculatedGamble
   + Untouchable/FlickFlack), an Innate deck, a Retain deck. Murder is already live-validated (trace #73). The
   random-target (SerpentForm/Ricochet/RipAndTear) and hand-size (PreciseCut) cards need single-enemy encounters.

**Move-legality + stun — NOW MODELLED (closed the two soundness gaps).** `Normality` (≤3 plays/turn while in
hand) and `Enthralled` (hand-lockout: only Enthralled is playable until played) were the only spots where an
unmodelled HARM could OVER-credit the search. Both are now enforced 1:1 by every move generator (the exact
Solver, MCTS, and the rollout/heuristic) via `CombatState.EffectivePlayCap()` (a per-turn play cap tightened by
in-hand cards) and `CardPlayAllowed()` (Unplayable + lockout). A deck holding a play-cap card sets `BoundsPlays`
so `PlaysThisTurn` memoises soundly. **Whistle's stun** is modelled via a bounded one-turn monster stun
(`Cmd.Stun` + `Monster.StunnedTurns`): the target's telegraphed move is DELAYED a turn (never permanently
disabled — gated hash/StateKey, so un-stunned monsters are byte-identical to before), so the player correctly
avoids one enemy action and the model can't over-credit.

**Monster-side OPTIMISTIC gaps — ALL CLOSED (gated → byte-identical for every other fight).** A sweep of the
inert-marker monster powers found three over-crediting spots, all now modelled + trace-validated:
- **Hex (SpectralKnight).** Makes all player cards Ethereal (a deck-thinning HARM). Was inert ⇒ forward search
  kept cards the real game exhausts. Now `EndPlayerTurn` exhausts the whole hand under Hex — faithful + sound.
- **Dampen (MagiKnight).** Downgrades the player's upgraded cards. Was inert ⇒ an UPGRADED deck kept stripped
  damage/block. Now `DampenPower.AfterApplied` downgrades every upgraded player card to base across all piles
  (no restore on caster death — the sound/pessimistic over-statement, never optimistic).
- **Reattach (Decimillipede).** Segment revival was inert ⇒ the search could clear the board one segment at a
  time across turns, over-crediting WIN PROBABILITY. Now ported 1:1 from the decompile's move machine: on downing
  (0 HP, another segment alive) the segment strips its non-Reattach powers (death cleanup) and sets
  `Monster.ReattachIn=2`; it sits at 0 (untargetable, doesn't act) for one enemy turn (DEAD_MOVE), then reattaches
  to 25 on the next (REATTACH_MOVE) if a segment still lives — all in `RunEnemyTurn`, deterministic so convergence
  holds. The board clears only when all segments are down together. The first cut revived one turn early and trace
  `combat-20260530-203121` rejected it; the 2-turn DEAD→REATTACH delay now validates that trace (76/76). A
  later long-fight capture (Ironclad, 6 turns, both reattach directions) **oracle-confirms the heal-to-25 itself**
  (every `REATTACH_MOVE` restores exactly 25 and the power persists — our engine matches those checks).

**Decimillipede middle-segment auto-bury — newly characterized, deliberately UNMODELLED (sound/pessimistic).**
That same long-fight trace surfaced a separate, previously-unknown mechanic: the MIDDLE segment is downed
(→ `DEAD_MOVE`) by the end of enemy turn 1 with **no player damage**, then reattach-heals to 25 and rejoins,
cycling. Proven scripted (a zero-damage defensive-deck control run still buries it) and not a recorder artifact
(trace #78 records a clean no-bury fight at 82/82). The game's bury **helps the player** (a segment stops
attacking, returns at only 25 HP); our engine keeps it alive-and-attacking, so the model is **strictly
pessimistic** (engine player HP ≤ game's in every check). The exact trigger isn't recoverable from the
name-obfuscated decompile, so modeling it would be speculative and risk the forbidden OPTIMISTIC direction —
left documented with the failing trace in `data/combat_traces_unresolved/` (see its README) for future work.

**Gold — verified HP-neutral, intentionally inert (NOT a missing subsystem).** Audited every combat-relevant
gold reader: `RoyaltiesPower` only fires `AfterCombatEnd` (post-combat reward), and HandOfGreed / the gold
relics have NO in-combat HP/block/damage feedback. So leaving gold unmodelled is not merely sound but
**HP-exact** for the survival/E[HP-loss] objective — building gold state would add memo-key surface for zero
objective impact. Documented, not built.

**Full-pool RNG card-generation — inert-in-search IS the sound model (not a deferrable bug).** Letting the
search SEE generated cards as a benefit is exactly the optimistic-unsound direction the doctrine forbids (it
would cherry-pick favourable generations); a faithful chance-node over the 50+-card pool would also explode the
state space. For replay/validation the generated cards' own plays are recorded individually, so inert is
trace-compatible. The tractable selection cards (SecretWeapon/SecretTechnique/SeekerStrike/Wish) use a sound
fixed-default move-to-hand. Affected (deliberately inert): Discovery, JackOfAllTrades, Splash, Jackpot's gen,
Catastrophe/BeatDown auto-play, Calamity/Entropy/CreativeAi/WhiteNoise/Chaos/Metamorphosis/Distraction/DualWield/
Begone/Quasar/Supermassive. Random card-selection is deliberately NEVER a search decision node.

**Multiplayer-only cards (all 21 ported as single-player projections, sound):** the ally-facing payloads are
dropped (no other players exist), so the powers that only buff/strike *via* other players are inert markers
(BeaconOfHope/Flanking/Knockdown/Sneaky/HammerTime/TagTeam); the self/enemy slices are kept verbatim. The one
self-downside — **Tank** (you take ×2 attack damage) — IS modelled, because an inert Tank would let the search
play it for free (an over-credit). Largesse's random card-gen stays inert (RNG selection is never a search
node), and Coordinate (temp Strength on the only ally — you) is a real buff.

**Documented out-of-scope (need a new subsystem — left inert/approximated, all sound):** Rupture end-of-turn
self-damage edge, MadScience TinkerTime, true-RNG card-selection for Cinder / base True Grit (kept
deterministic-default — promoting would be optimistically unsound).

**DeathMarch draw-scaling — NOW MODELLED.** `(8+U)+(4+2U) × mid-turn cards drawn this turn` via a new gated
`CombatState.CardsDrawnMidTurn` counter (set by `CardModel.TracksMidTurnDrawScaling`), threaded through all four
draw paths with a `fromHandDraw` flag so only mid-turn effect-draws count (the game's `!FromHandDraw` filter):
concrete `DrawCards`, exact `EnumerateDraw`, MCTS `SampleDraw`/exact, and rollout. During trace replay the count
is reconstructed via `CombatState.ReplayMode` (draws are no-ops there, so `Cmd.Draw` credits the requested count).
The full suite confirms exact↔MCTS convergence is preserved. (This also fixed a latent bug: the MCTS DPW
`SampleDraw` path never applied the Murder counter — now consistent with the exact oracle.)

**Audit-confirmed PESSIMISTIC (sound) gaps left as-is** (under-credit the player; fixing is low-value):
**Fetch** card-draw, **MakeItSo** return-to-hand recursion, **Pinpoint**/**Stomp** per-card cost reduction
(cost-only ⇒ HP-neutral), **Squeeze** omits the transient PlayPile from its OstyAttack count (exotic, ≤1 under).

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
keyword mechanics 1:1** + a full per-module catalog fidelity audit (Predator/FlashOfSteel upgrade-number fixes);
**the audit's two Regent gaps closed** (CrescentSpear verified correct; BeatIntoShape history-based Forge via a
gated per-target powered-hits counter); **a 37-card scaling/conditional audit** (no optimistic over-crediting;
Stomp upgrade number fixed).
