# Phase 0 — Baseline & Instrumentation (per-character heuristic redesign)

Companion to `docs/per-character-heuristic-research.md`. Records the measurement tools built in Phase 0 and the
**baseline numbers** the later phases are measured against. Re-run the commands here after each phase to verify.

## Instruments added

1. **Policy-regret** (`CalibrationHarness.MeasurePolicyRegret`) — the greedy LEAF policy's single-turn regret vs
   the exact oracle, probability-weighted over a fixture's opening decision states:
   - `WinRegret` = optimal win-prob − greedy win-prob (≥0; dominant term).
   - `LossRegret` = greedy E[HP loss] − optimal E[HP loss] (≥0; tie-breaker).
   - Both 0 ⇔ the greedy turn plays as well as the oracle. The exact side plays the real engine, so any regret is
     the **leaf heuristic's**, not the search's. Backed by `Solver.StateValue` / `Solver.GreedyTurnValue` and
     `MctsSolver.GreedyPlayTurn` (the faithful leaf policy, exposed).
2. **Per-character fixtures** (`CalibrationFixtures.PerCharacter`) — exact-solvable fights that exercise a class's
   defining mechanic. Draw-free (deterministic greedy turn + exact-tractable).
3. **CLI**: `sts2solve --calibrate --percharacter [--regret]` — value-convergence + regret table.
4. **Gating tests**: `PerCharacterCalibrationTests` (MCTS value tracks exact per character) and
   `PerCharacterProbes` (oracle-free deck-strength probes).

## Baseline (commit prior to Phase 1)

`sts2solve --calibrate --percharacter --regret --trials 40000 --exact-budget 150`

| fixture | exact loss | MCTS@40k loss (Δ) | greedy-leaf loss | **regret (loss)** |
|---|---|---|---|---|
| necro/Osty-vs-Byrdonis | 20.6 | 20.5 (0.1) | 21.2 | **0.63** |
| necro/Unleash-race-vs-Byrdonis | 21.2 | 21.3 (0.1) | 22.9 | **1.70** |
| iron/Starter-vs-Cultist (control) | 0.9 | 0.9 (0.0) | 0.9 | **0.00** |

Reading: **MCTS *value* already tracks exact** (Δloss ≤ 0.1) — the tree corrects the biased leaf at 40k trials.
But the **leaf policy itself is wrong for Necrobinder** (regret 0.6–1.7), and advice runs at only 800 trials where
that leaf bias dominates — which is why the deck-strength advice mis-ranks Osty cards. The **Ironclad control is
0.00**, confirming the instrument isolates the heuristic (the tuned class plays optimally; the untuned one doesn't).

### Oracle-free probe state

Necrobinder starter deck strength vs the act-1 elite pool, removing one card (advice budget, 800 trials):

| cut | Δ strength | probe |
|---|---|---|
| Unleash | ≈ −5.8 (cutting it clearly weakens) | `Necrobinder_CuttingUnleash_Weakens_Deck` — **LIVE / green** |
| Bodyguard | ≈ **+2** (cutting it wrongly *improves*) | `Necrobinder_CuttingBodyguard_DoesNotImprove_Deck` — **SKIPPED, Phase-1 target (RED)** |

The Bodyguard probe is the originating bug in test form: the Osty-blind leaf treats a +5-Osty-HP summon as a dead
play, so cutting it raises measured strength. It is `[Fact(Skip=…)]` until Phase 1 flips it green.

## Phase-1 exit criteria (what "done" looks like)

- `necro/*` regret (loss) drops materially toward 0; **`iron/*` control stays 0.00** (no regression).
- `Necrobinder_CuttingBodyguard_DoesNotImprove_Deck` un-skipped and **green**.
- `PerCharacterCalibrationTests` value-convergence still green; full suite green.
- Necrobinder starter deck-strength rises (re-measure via the custom-mode advice path).
