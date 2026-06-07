# Per-Character Combat Heuristic — Research Program

Status: **research / design** (no code changes yet). Owner: Steven + Claude.
Companion to `docs/mcts-solver-design.md` (the solver this heuristic feeds).

---

## 0. The bug that started this

The Ranwid removal advice ranks **Bodyguard** (a premier Osty-scaling card) as the *#1 card to
cut* from the Necrobinder starter deck — and the whole character evaluates as the weakest of the
five (deck-strength 18.6 vs Ironclad 23.9 / Silent 26.6 / Regent 21.9 / Defect 28.6 against the
same Act-1 pool; near-certain death vs 2 of 4 elites).

Root cause (verified, `solver/Sts2Solver.Search/CombatHeuristic.cs`): the MCTS rollout/leaf
heuristic `Score()` is **structurally blind to Osty**. Its only features are `incoming/unblocked`,
`overblock`, `enemyHp`, *player* `Strength`, and enemy `Vulnerable`. Osty lives at
`Player.Osty` (separate from `s.Monsters`) and never enters the score. Consequences:

- **Survival ignores `DieForYou`** — Osty intercepts one full post-block enemy hit per turn
  (and re-summons every turn via Bound Phylactery). `IncomingDamage()` counts every telegraphed
  hit as landing on the player ⇒ the rollout over-blocks and plays too defensively.
- **No Osty-HP term** — `Bodyguard` (Summon +5 Osty HP) raises every future `Unleash` *and*
  every future `DieForYou` block, but produces **zero** change in `Score()` (no enemy damage, no
  block, no player Strength). In the greedy rollout (`PlayTurn`, plays only a *strictly-better*
  action) it is **never played** → reads as dead weight → cutting it "thins the deck" → strength
  goes up → ranked #1 to remove.

Unleash itself is correctly *kept* (its immediate `6 + Osty.CurrentHp` damage is really applied
in the rollout, so the enemy-HP drop is visible). Only the **delayed / indirect** value of Osty
HP is invisible — which is exactly the value the character is built on.

**Generalisation:** one fixed, offense-flavoured heuristic mis-serves any character whose win
condition routes through a resource the score can't see. This program redesigns the heuristic to
be character/archetype-aware.

---

## 1. Problem statement & scope

**Goal.** Make the MCTS leaf/rollout evaluation value each character's resources correctly, so
that (a) absolute deck-strength numbers are trustworthy per character and (b) relative advice
(remove / upgrade / reward) stops mis-ranking build-defining cards.

**In scope:** the combat *position score* (`CombatHeuristic.Score`) and the rollout policy that
consumes it; the feature set it reads; how its weights are conditioned (character / archetype);
the tuning + validation methodology.

**Out of scope (for now):** the MCTS tree mechanics themselves (UCB/PUCT/widening — already
tuned), the exact oracle, save-parsing, the TUI.

**Hard principle — calibrate to the *engine*, not to the wiki.** The pro-playstyle research in
§3 is a source of **priors and hypotheses**, not ground truth. The arbiter of "correct play" is
the **exact solver** (`Solver.cs`) on fights small enough to solve, and faithful engine
simulation everywhere else. Where the real game and our engine model differ (e.g. the wiki says
Osty *overflow* spills to the player; our `DieForYou` redirects the whole post-block hit to Osty
for powered attacks only), **the engine wins** — the heuristic must approximate what the engine
actually does, because that is what the MCTS is solving.

---

## 2. What we already have (don't rebuild)

From the codebase audit — the tuning/validation loop is largely built:

| Asset | File | Use |
|---|---|---|
| Exact oracle (expectimax, lexicographic (Win,Loss), memoised) | `Search/Solver.cs` | ground truth on small fights |
| Calibration harness (exact-budgeted vs MCTS, multi-seed noise floor) | `Search/CalibrationHarness.cs` | measure MCTS-vs-exact Δsurv/Δloss |
| CLI `--calibrate` | `Cli/Program.cs` | run harness over fixtures, report mean abs error |
| Fixtures: 6 archetypes, elite sweep, random decks, bridge ladder (8→30 cards) | `Content/Core/CalibrationFixtures.cs` | graded test fights |
| Quality tests (Δsurv ≤ 6–12%, Δloss ≤ 2.5 HP; seed-stability ≤ 3%) | `Tests/CalibrationTests.cs`, `MctsTests.cs`, `BridgeInstrumentTests.cs` | regression gates |
| **All heuristic weights env-overridable, no rebuild** (`STS2_WENEMYHP`, `STS2_WSTRENGTH`, `STS2_WLOSS`, `STS2_WVULN`, `STS2_WOVERBLOCK`, `STS2_LAMBDA_LO/HI`, …) | `CombatHeuristic.cs`, `MctsSolver.cs` | external weight sweeps |
| **Phase-C learned value function (scaffolded)**: `VfTrainer`, CLI `--train-vf`, `TrainingFixtures` (exact-labelled deck×enemy×HP grid) | `Content/Core/TrainingFixtures.cs` | an *alternative* to hand-tuned weights |

**Implication:** we already have an oracle, a measuring harness, env-var knobs, and a latent
learned-VF track. The missing piece is **features** (the score can't see Osty/poison/stars/orbs)
and a **per-character conditioning + tuning plan**. That is what this program adds.

---

## 3. Pro-playstyle findings, distilled to evaluable principles

Sourcing caveat: STS2 is early-access (mid-2026); there is **no settled pro/tournament meta**.
Transferable *combat-theory* principles (below) are high-confidence (game-theoretic, STS1
pro-validated). STS2 *card-specific* and *tier* claims are guide-sourced and patch-volatile.
Treat all of this as priors to be confirmed against the oracle.

### 3.1 Universal principles (encode once, in the shared feature set)

1. **Killing is blocking, permanently.** A dead attacker removes its damage *every* future turn.
   Value lethal/near-lethal on a single enemy above face — reward states where an enemy is dead
   or in kill range; **enemy HP is not linear** (chip on a survivor ≈ worthless this turn).
2. **Kill order — collapse threat count.** One enemy at 2 HP beats two at 50%.
3. **Block is a consumable; overblock ≈ 0.** Value block ≈ `min(block, incoming)`; penalise the
   surplus. (Exception: `Barricade` makes block a scaling stat — see §4.)
4. **HP is a fungible buffer, not sacred.** Convex penalty: cheap at high HP, a cliff near death.
5. **Scaling value = per-turn payoff × expected remaining turns.** This single rule explains
   Strength, Poison, Dexterity, Focus, Dark-orb, Osty growth, Doom. It requires a **fight-horizon
   estimate** the current score lacks (§4, §5-Q3).
6. **Tempo by win-probability.** Ahead → solidify (survival weight up); behind → race (burst up).

### 3.2 Per-character deltas (priors for the weight vectors)

| Character | Identity / lean | Primary scaling resource → value rule | Eval must additionally reward / penalise |
|---|---|---|---|
| **Ironclad** | Offense bruiser; block-poor; heals between fights | **Strength** ≈ (hits/turn) × (remaining turns) | Shallow HP penalty at high HP; `Barricade`-block at full carry; apply `Vulnerable` *before* burst; self-damage OK when healthy |
| **Silent** | Patient setup → burst / DoT | **Poison** = full future ticks, HP-capped (banked damage); **Dexterity** defensively | Reward early shivs/AOE; **strict** overblock penalty (turtle trap); "race override" when enemy out-scales DoT |
| **Defect** | Per-turn balanced; **STS2 Focus is mostly temporary** | **Orb-board width × Focus**; `TempFocus` = *this-turn-only* value × current orbs; permanent `Focus` = ×remaining turns | Reward orb count (slots, cap 10); Dark-orb = HP-capped ramp gated on fight length; energy(Plasma) at marginal use |
| **Regent** | Setup-oriented burst; two banked engines | **Stars** (persist, uncapped) = value of best assemblable *future* burst, **tapered** above what's castable next 1–2 turns; **Forge/Sovereign Blade** ≈ persistent Strength-like stat | Penalise hoarded-but-unusable Stars and gen/spend *mismatch*; risk-gate: convert Stars when incoming > block |
| **Necrobinder** | Defensive-investment → offense; forgiving | **Osty HP double-counted**: per-turn shield `min(OstyHp, incoming) × remaining turns` **plus** Unleash-multiplier `OstyHp × P(draw an Osty-HP attack)`; **Doom** = banked, armour-piercing execution | **Alive→dead Osty = cliff**; reward re-summon capacity in hand; **do NOT credit player Strength on Osty attacks**; don't value Osty HP as one-time block |

Full source notes and citations live in the research-agent transcripts (Jorbs "jobs" framing,
Untapped/Mobalytics/wiki for STS2 specifics, scumthespire / bottled_ai / spirecomm / Colin-Lu
Nibbit for AI prior art).

### 3.3 AI prior-art takeaways

- **Every serious STS bot conditions evaluation on character/archetype** (scumthespire per-char
  `CARD_RANKS`+`PlayOrder`; bottled_ai 5 named strategies; spirecomm class `Priority` objects;
  Slay-I trains per-character damage-saved models; STS2's BoberInSpire uses
  `guide_archetypes.json`). Nobody ships one flat heuristic across characters.
- They chose *separate hardcoded* logic because they are **hand-authored decision trees**, not
  optimisers — not because unified-with-weights loses.
- The asymmetric-game ML pattern is **shared feature set + per-role weight vector**, tuned by
  self-play (CMA-ES is the standard derivative-free weight optimiser; per-archetype runs are
  cheap). This matches our env-var-weights + calibration-harness setup exactly.
- Cleanest single objective in the ecosystem: **"damage taken / damage saved"** (Colin-Lu
  expectimax value; Slay-I) — but it must be **horizon-aware**, not greedy per-turn.

---

## 4. Architecture decision

**Recommendation: one unified, feature-based leaf evaluation with a character/archetype-conditioned
weight vector — NOT N separate hardcoded heuristics.** Reasons:

1. **MCTS wants a single scalar leaf value to back-propagate.** N independent evals fight the
   architecture and explode maintenance.
2. **Mechanics travel across characters.** Steven's own example: Ironclad can pick up Osty via an
   event; Silent/anyone can acquire poison or orbs through events/colourless cards. A *separate*
   Ironclad heuristic would be Osty-blind again the moment that happens. The fix is a **universal
   feature set every character can read**, where only the *weights* differ.
3. **Conditioning should arguably be on ARCHETYPE / present mechanics, not character ID.** The
   right question is "does this deck contain Osty-scalers / poison-appliers / orbs / star-spenders?"
   not "is this the Necrobinder?". Character ID is a cheap **prior** for the weights; deck-content
   is the **true** signal (and handles cross-character mixing for free). → *Key research question
   Q1.*
4. **It composes with the existing Phase-C learned VF.** A richer, character-aware feature set is
   the shared prerequisite for *both* hand-tuned weights *and* the learned value function. Whatever
   wins the weighting method, the **features** are the same investment.

**Therefore the highest-leverage first work is FEATURE ENRICHMENT**, independent of how weights
are ultimately set.

### Proposed universal feature set (superset of today's `Score`)

Today's score has: `unblocked`, `overblock`, `enemyHp` (linear — flag for redesign), player
`Strength`, enemy `Vulnerable`. Add:

- **Survival, summon-aware:** effective incoming after Osty interception
  (`max(0, incoming − f(OstyHp, DieForYou))`); Osty alive flag (discrete); Osty re-summon
  capacity in hand.
- **Stored/banked damage:** enemy `Poison` valued at future ticks (HP-capped); `Doom` valued as
  banked armour-piercing execution weighted by proximity to threshold.
- **Resource pools:** `Player.Stars` (tapered by castable spenders); orb-board state
  (`Player.Orbs` count/types, `OrbSlots`), `Focus` vs `TempFocus` (permanent vs this-turn);
  Dark-orb stored ramp.
- **Scaling powers as forward value:** `Strength`/`Dexterity`/`Focus`/`Calcify`(Osty growth)/
  `ReaperForm`/`Lethality`/`Barricade` — each valued by **payoff × expected remaining turns**.
- **Enemy HP, non-linear:** kill / breakpoint credit instead of linear `enemyHp`.
- **Fight-horizon estimate:** expected remaining turns (drives every "× remaining turns" term).

All accessors are confirmed O(1) on engine state: `Player.Osty?.CurrentHp/.MaxHp`,
`IsOstyAlive`, `Player.Stars`, `CanAffordStars`, `Player.Orbs`, `Player.OrbSlots`,
`GetPowerAmount("Poison"|"Doom"|"Focus"|"TempFocus"|"Calcify"|"Strength"|"Dexterity"|…)`.

---

## 5. Key research questions

Ordered roughly by leverage. Each names the data/experiment that answers it.

- **Q1 — Conditioning variable: character ID vs deck-archetype vs both?**
  Does conditioning weights on *present mechanics* (deck contains Osty-scalers / poison / orbs /
  stars) dominate conditioning on character ID, especially for cross-character event cards?
  *Experiment:* build a few mixed-mechanic fixtures (Ironclad + Osty; Silent-poison on another
  char) and check which conditioning predicts the oracle's play.

- **Q2 — How wrong is the current heuristic, per character, vs the oracle?**
  Quantify the gap the redesign must close. *Data:* run `--calibrate` per character on
  exact-solvable fixtures (need to *build per-character fixtures* — §6); report Δsurv/Δloss and,
  crucially, **policy-agreement** (does the rollout's first play match the oracle's?).

- **Q3 — How to estimate "expected remaining turns" cheaply inside the score?**
  Every scaling term needs it. Candidates: `enemyHpTotal / expected-damage-per-turn`; a fixed
  per-enemy prior; intent-aware. *Experiment:* correlate each estimator with actual fight length
  in oracle/rollout playouts.

- **Q4 — Osty valuation form.** What functional form for Osty HP best matches the oracle?
  Linear-per-turn-shield + Unleash-multiplier (§3.2) vs a learned term. Is the alive→dead cliff
  worth an explicit discrete bonus? *Experiment:* sweep an `STS2_W_OSTY*` family on Osty fixtures
  vs exact; check the Bodyguard-removal probe (§7) flips to "keep".

- **Q5 — Non-linear enemy-HP / kill-credit.** Does replacing linear `enemyHp` with
  kill/breakpoint credit improve policy agreement without destabilising the tuned tree?
  *Experiment:* A/B on the existing 6 archetypes (must not regress) + multi-enemy fixtures.

- **Q6 — Overblock penalty calibration per archetype.** Silent/Necrobinder over-survive; Ironclad
  under-blocks. Is one `WOverblock` enough, or does it need to be per-archetype / horizon-scaled?

- **Q7 — Hand-tuned weights vs learned value function (Phase-C).** Given enriched features, does
  CMA-ES-tuned per-archetype weights beat / match / underperform the learned VF? Which is more
  robust out-of-distribution (new cards, ascension)? *This is the architecture fork; defer the
  decision until features exist and Q2 baselines are measured.*

- **Q8 — Does fixing the rollout/leaf suffice, or must the tree see these features too?**
  The leaf seeds are biased today; with more trials the tree can partly recover. Measure how much
  of the gap is leaf-seed bias vs genuinely unreachable in-budget.

- **Q9 — Regression safety.** Can per-character enrichment be added without moving the 5 existing
  tuned Ironclad/generic fixtures outside tolerance? (Default weights for absent mechanics must be
  no-ops.)

---

## 6. Data to collect / instrumentation to build

1. **Per-character exact-solvable fixtures** (the central missing asset). For each character: 4–8
   small decks (≤14 cards so exact is tractable) that *exercise the defining mechanic* —
   Necrobinder Osty-growth fights, Silent poison ramps, Defect orb boards, Regent star-bank turns,
   Ironclad Strength ramps — each vs 2–3 representative enemies at a couple of HP levels. Extend
   `CalibrationFixtures.cs`. These are the oracle-labelled training/validation set.

2. **Policy-agreement metric** (new). Beyond Δsurv/Δloss, log: *does the rollout/greedy policy's
   chosen first action equal the exact solver's optimal action?* Per fixture, per character. This
   is far more sensitive to "plays the wrong card" bugs than aggregate HP loss.

3. **Oracle-free diagnostic probes** (cheap regression gates, runnable on big decks): assertions of
   the form *"the heuristic must NOT rank this build-defining card as the best cut"* —
   Necrobinder/Bodyguard (the originating bug), Silent/Noxious-Fumes, Defect/orb-power,
   Regent/Forge. And *"deck-strength must rank these obviously-better cards above a basic Strike."*
   These don't need an oracle and catch the exact class of failure we're fixing.

4. **Per-character deck-strength baselines** (already producible via the custom-mode path used in
   the diagnostic). Snapshot now; re-measure after each change to track movement.

5. **Weight-sweep tooling.** A thin external script that sets `STS2_W*` env vars, runs
   `--calibrate` (and the probes), and records Δsurv/Δloss/policy-agreement — the substrate for a
   CMA-ES / grid optimiser per archetype. Harness already supports this; only the sweep driver is
   new.

6. **Fight-length / resource-trajectory logs** from rollouts (for Q3/Q4): turns-to-win, Osty-HP
   over time, poison realised vs applied, orbs slotted over time.

---

## 7. Methodology — the tuning loop

```
        ┌─────────────────────────────────────────────────────────┐
        │ 1. ENRICH FEATURES  (Score sees Osty/poison/stars/orbs/   │
        │    Doom/Focus/horizon; default weights = no-op for absent)│
        └───────────────┬─────────────────────────────────────────┘
                        ▼
        ┌─────────────────────────────────────────────────────────┐
        │ 2. MEASURE  per-char fixtures → exact oracle              │
        │    metrics: Δsurv, Δloss, POLICY-AGREEMENT + probes       │
        └───────────────┬─────────────────────────────────────────┘
                        ▼
        ┌─────────────────────────────────────────────────────────┐
        │ 3. TUNE  per-archetype weight vectors (grid → CMA-ES)     │
        │    via STS2_W* env sweep; fitness = agreement − error     │
        └───────────────┬─────────────────────────────────────────┘
                        ▼
        ┌─────────────────────────────────────────────────────────┐
        │ 4. GATE  existing tolerances must hold; probes must pass; │
        │    full test suite green                                  │
        └───────────────┬─────────────────────────────────────────┘
                        ▼
        ┌─────────────────────────────────────────────────────────┐
        │ 5. (fork, Q7) hand-tuned weights  ←→  learned VF (Phase-C)│
        │    on the SAME enriched features                          │
        └─────────────────────────────────────────────────────────┘
```

Ground rules: the **exact oracle is the arbiter**; pro-playstyle priors only seed initial weights
and explain disagreements. Every change is gated by the existing convergence tolerances *plus* the
new policy-agreement metric and oracle-free probes. Default weights for a mechanic must be no-ops
when the mechanic is absent (protects the 5 tuned baselines, Q9).

---

## 8. Validation & regression gates

- **Existing** (must never regress): `CalibrationTests` (Δsurv ≤ 6–12%, Δloss ≤ 2.5 HP over 6
  archetypes + elites + random decks), `MctsTests` convergence, `BridgeInstrumentTests`
  seed-stability ≤ 3%.
- **New oracle gates:** policy-agreement ≥ target on per-character fixtures.
- **New oracle-free probes:** build-defining cards are never the top cut; deck-strength orderings
  respect obvious dominance (Unleash > Strike, etc.).
- **Smoke:** per-character starter deck-strength must land in a sane band and the
  Bodyguard-removal bug must be gone.

---

## 9. Phased roadmap

- **Phase 0 — Baseline & instrumentation** *(small, do first)*
  Build per-character exact fixtures (§6.1); add the policy-agreement metric (§6.2) and the
  oracle-free probes (§6.3) as tests; snapshot current per-character Δsurv/Δloss/agreement and
  deck-strength. *Exit:* we can quantify "how wrong, per character" and have red probes for the
  known bugs.

- **Phase 1 — Feature enrichment (Necrobinder first, as the proof case)**
  Make `Score` summon-aware (Osty interception in survival; Osty-HP dual value; alive cliff;
  re-summon capacity). Keep all other characters no-op. *Exit:* Bodyguard probe flips to "keep";
  Necrobinder deck-strength rises; existing baselines unmoved; suite green.

- **Phase 2 — Remaining resources** Poison-as-banked-ticks, Stars (tapered), Orbs/Focus-vs-TempFocus,
  Doom, Barricade, plus the fight-horizon estimator (Q3) and non-linear enemy-HP (Q5). One mechanic
  at a time, each gated.

- **Phase 3 — Conditioning & tuning** Decide Q1 (character vs archetype conditioning); wire the
  weight-sweep driver; CMA-ES per-archetype weight vectors against the Phase-0 metrics.

- **Phase 4 — Architecture fork (Q7)** With enriched features stable, compare hand-tuned weights
  vs the Phase-C learned VF on identical features; pick the production path (or a hybrid: learned
  VF as leaf, hand probes as guardrails).

---

## 10. Risks & open problems

- **No exact oracle where the mechanics matter most.** Osty/poison/orb payoffs compound over
  *long* fights / *big* decks — exactly the regime exact can't reach. Mitigation: oracle-free
  probes + policy-agreement on the small fights that *do* exercise the mechanic in miniature;
  accept that long-fight tuning is partly self-consistency, not oracle-verified.
- **Engine ≠ wiki.** Must verify each mechanic's *modelled* behaviour (e.g. Osty overflow,
  Doom trigger threshold, whether Focus touches Dark) from the engine + tests, not from guides.
- **Tuning instability / overfitting to fixtures.** Per-archetype weights can overfit a handful of
  fixtures. Mitigation: held-out random per-character decks (mirror the existing
  `MctsRoll_Tracks_Exact_On_Random_Decks` discipline).
- **Interaction with the tuned tree.** Leaf/score changes can shift APW/PUCT behaviour. Re-run the
  bridge-ladder noise gates after each phase.
- **Cost.** Per-character fixtures × exact solves × weight sweeps is compute-heavy. Budget the grid;
  use the multi-seed harness to separate signal from noise.

## 11. Decisions / non-goals (for now)

- **Decided:** unified features + conditioned weights (not separate hardcoded heuristics);
  feature enrichment precedes the weighting-method choice; exact oracle is the arbiter; ship
  Necrobinder/Osty first as the proof case.
- **Deferred (data decides):** Q1 conditioning variable; Q7 hand-tuned vs learned VF.
- **Not doing now:** redesigning the tree search; changing the lexicographic (Win, Loss)
  objective; touching save-parsing/TUI.
