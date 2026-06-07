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
| Bodyguard | ≈ +4 (cutting it *improves*) | (no probe — see Phase-1 finding: this is **correct**, not a bug) |

## Phase 1 finding — the "Osty-blindness bug" was REFUTED (no code change shipped)

Phase 1 set out to make the leaf heuristic Osty-aware and expected that to (a) drop Necrobinder regret and
(b) flip "cutting Bodyguard improves the deck" (the assumed bug). Built behind env knobs and measured against the
oracle, **both expectations failed**, and the investigation overturned the premise:

1. **Engine model corrected (real finding).** Osty's `DieForYou` does **not** absorb a whole hit — it absorbs up
   to its current HP and the **overkill spills back to the player** (`Cmd.ApplyDamage` line ~146). An earlier read
   stopped before that line and concluded "no overflow"; the regret instrument's disagreement with the oracle is
   what surfaced the mistake. So a 1-HP Osty stops only 1 of a 17 hit. *(This is why we validate against the
   oracle, not the wiki — though here the wiki was right and our first reading wrong.)*

2. **An Osty-HP value term makes play WORSE.** Rewarding Osty `CurrentHp` pushes the greedy to play Bodyguard
   over the Defend/Strike the oracle actually wants; regret rose (necro/Osty 0.6 → 2.6) and the policy lines
   diverged from exact. Sweeping the weight only widened the gap.

3. **A defensive lean doesn't help either.** Lowering rollout aggression (more block) raised regret monotonically
   (0.0 was catastrophic and broke the Ironclad control); the residual ~2 HP gap is **myopic-greedy-vs-lookahead**,
   not a block-weight miscalibration.

4. **"Cut Bodyguard" is CORRECT.** Forcing the rollout to value Osty HP left `−Bodyguard` strength flat (21.07)
   while *lowering* full-deck strength — i.e. it made the cut look better, not worse. The exact oracle agrees: in
   the **starter** deck it never grows Osty. Osty starts at 1 HP, dies/resets each turn, and absorbs only its own
   HP — so Bodyguard's scaling can't pay off without the protect-and-grow engine a **built** deck has. The advice
   ranking Bodyguard as the weakest starter card is right.

**Conclusion.** On every oracle-verifiable fight the Necrobinder leaf is already near-optimal (baseline regret
0.6–1.7) and the advice is correct; the refuted changes were reverted. The hypothesized Osty-scaling value is a
**built-deck / long-fight** phenomenon that the exact oracle cannot reach (research-doc §10) — so it cannot be
confirmed *or* fixed by oracle-grounded tuning. Phase 1 ships **no heuristic change**, only the diagnostic
tooling that produced the finding (`sts2solve --plans`; `STS2_REGRET_AGGRO`; `MctsSolver.GreedyTurnPlan`).

### What remains true / open

- `PerCharacterCalibrationTests` (value-convergence) and `Necrobinder_CuttingUnleash_Weakens_Deck` stay green.
- The regret instrument + per-character fixtures are validated and reusable for the other classes (poison / orbs
  / stars), where the defining mechanic may matter *within* exact-tractable fights (unlike Osty's 1-HP start).
- Open question for a later phase: validating built-deck / long-fight Osty value needs a non-oracle signal
  (self-play, or larger exact budgets on mid-size decks) — this is the §10 "no oracle where the mechanic matters"
  gap made concrete.
