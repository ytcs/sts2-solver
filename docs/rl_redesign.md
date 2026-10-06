# RL redesign: outcome heads, a decision layer outside the network, a curriculum

Status: spec of the agreed direction (user, 2026-10-06). Nothing here is built yet except where marked **exists**. Each milestone has its own gate; a
milestone that fails its gate is not adopted, and the next one does not start on top of it.

## 1. Why

| Problem | Evidence |
|---|---|
| The value net predicts one scalar (`+1 + 0.5 x HP fraction` on a win, `-1` on a loss or stall). The worth of HP and of a potion depends on the run (the route to the act boss, the next boss), not on the fight. | Conditioning the network on the run's HP-worth curve (`ucond`, `--util-prob`) failed its gate: curve vs linear E[U] -0.000 +- 0.002, new vs old linear win +0.003 +- 0.004 over 1,200 fights (`evals/gate_util_full.json`, ledger 2026-10-06). The run-level worth of HP belongs outside the network. |
| Potions: the search cannot tell "this fight" from "the boss", and throws on ties. | "Now" vs "later" scores within 0.003 at depth 2 (`evals/solver_gaps.md`). The price that answers "spend it in this fight or keep it" needs the fight's outcome distribution and the route's HP worth; today it is computed by play-outs (`agent/potion_price.py`, ~6 s per potion). |
| HP is a fraction of max HP; enemy damage is absolute. | User. A fraction mixes the deck's survivability with the character's max HP. |
| An irrelevant relic moves the network's play. | Lava Lamp / Planisphere (no combat effect) move win by about +0.3 on one scenario (`evals/solver_gaps.md`). |
| The training mix has at most 2 potions, no ancient relics, and generated (not reached) decks. | `fuzz_gen_mix.py` potions 0/1/2 (weights 1:4:5), `max_potion_slots` 2; observation `MAX_POTIONS = 4`, `MAX_RELICS = 24` (`crates/sts2sim/src/state.rs:12-13`). |

## 2. Today's pipeline (facts the milestones change)
- **Env** (`crates/sts2env/src/lib.rs`): `RewardConfig {win, loss, hp_bonus, step}`; a win pays `win + hp_bonus x hp / max_hp` (307), a loss `loss`; `max_steps` (agent steps, 600 in PPO) ends an episode as TRUNCATED with reward 0 (310), which `ppo.py` turns into -1 (185). No turn cap. `EpisodeInfo`: scenario, `hp_lost` (fraction), `hp_end` (fraction on a win), `len`.
- **PPO** (`rl/ppo.py`): GAE gamma 0.999, lambda 0.95, horizon 48, 1,024 envs (49,152 steps per iteration), value loss smooth-L1 on the scalar return, vf 0.5, entropy 0.01; HP-worth curves recomputed into the win reward with an invariant check (188-207). Fresh training 6,400 iterations (~314M steps); ~50k steps/s on an RTX 4090. Checkpoint `{net, opt, it, steps, args}`.
- **Model** (`rl/model.py`): pointer policy head; value `mlp(2d, 2d, 1)` on `gctx = [player, ctx]` (148, 273); relics as one pooled embedding bag (124, 187); each potion its own token (213); `ucond` adds the HP-worth curve's 8 features to the player token (151, 255).
- **End-HP head** **exists** as an add-on (`rl/dist.py`): 21 classes (loss, 20 HP-fraction bins) on a FROZEN trunk, trained by cross-entropy on search-played fights; as a leaf evaluator it was weaker than the 3-network ensemble (`sts2-harness`, SEARCH OBJECTIVE).
- **Search** (`crates/sts2env/src/search.rs`): value rows return one float each (`Inputs.val`); `terminal()` scores a finished play-out with `cfg.win / loss / hp_bonus` or the per-engine `cfg.util` table.
- **Training data**: `tools/gen_train.py` -> `fuzz_gen_mix.py` (starter + 4-30 added cards by act, 2-7 relics, 0-2 potions, start HP U(0.5, 1) of max); seed 1 train (30,000), seed 22 eval (1,500, `data/train/eval.json`), seed 23 high-energy eval (600).
- **Gate** (`agent.improve gate`): search play (`Solver`, 2 attempts) on the corpus holdout, `eval[:600]`, `eval_energy[:600]`; PASS needs win >= +0.01 on the corpus holdout and >= -0.01 on the others; HP lost is reported, not gated.

## 3. Target design

### 3.1 Outcome heads (the network)
For a state, under the policy, the network predicts:
- **`end`: a categorical distribution over the fight's ending**: class 0 = a loss (death, or the turn cap), classes 1..B = won with end HP in absolute bins (2 HP wide up to 150, the last bin open). P(win) = 1 - P(class 0); HP lost if won = HP now - end HP. One head carries both of the agreed outcomes (P(win), HP lost if won), and end HP is the unit the route DP already uses (`routes.continuation_values` gives V over end HP).
- **`pot[k]`: P(the potion in belt slot k is used before the fight ends)**, one logit per slot (8 slots after M3).

Real starting HP (no 999-HP normalisation): HP-conditional effects (Blood Wall, low-HP cards and relics) need no special case and death is a real outcome. A 99-player-turn cap ends stalls as a loss; `max_steps` stays as a safety net only.

Why a categorical over end HP rather than "HP lost" regression: end HP does not change along a trajectory, so the bootstrap target of a state is simply the next state's predicted distribution (lambda-mixed with the one-hot ending at the episode's end); no conditional-on-win correction is needed, and any utility of the ending is one dot product.

### 3.2 The decision layer (outside the network)

    V(s) = sum_b P_end(b | s) * U_job(b)  -  sum_k price_k * ( used_k(root -> s) + P_pot(k | s) )

- `U_job` over the classes: the run's worth of each ending, `U(0) = w_loss`, `U(b) = ` P(win the act boss | end HP of bin b) from the route DP (or the next act's boss win in an act boss, `agent/potion_price.py`).
- `price_k`: the potion's worth later in the run (its value in the route DP's potion budget); `used_k(root -> s)` charges potions a play-out already threw between the searched decision and the leaf.
- With today's weights (`U(0) = -1`, `U(b) = 1 + 0.5 x end_hp(b) / max_hp`, prices 0) V reproduces today's scalar return (up to bin width): M1 is gated against the current networks exactly.

Constraints from the search engine:
- **Combine in Rust, per job.** Value rows come back as `[rows, H]` (H = 1 + B + 8); Rust combines them with the weights of the row's job, carried per scenario (`ScenarioExtras`: the U table over classes, the potion prices). `SearchCfg.util` (one curve per engine) goes away. Python never needs to know which job a row belongs to.
- **Terminals and leaves use the same function.** `terminal()` maps a finished play-out to its class (one-hot) and its used potions and calls the same combination as a leaf; otherwise deeper search is biased toward or away from finishing fights.

### 3.3 Training targets
- `end`: cross-entropy against a lambda-mix of the next state's predicted distribution (bootstrap, target network or stop-gradient) and the one-hot ending at the episode's end; at the horizon cut, the bootstrap alone.
- `pot[k]`: binary cross-entropy against "used at this step" or else the next state's prediction (same lambda-mix), per slot that holds a potion; empty slots masked.
- Policy advantage: GAE on the scalar return of today's weights, with V(s) from the heads through the decision layer (M1: weights fixed; later milestones may randomise U and prices per episode, which then become policy inputs).

### 3.4 What stays out of the network
The worth of HP and potions for the rest of the run. The network sees the fight; the decision layer sees the run. Evidence: the failed HP-worth conditioning (section 1).

## 4. Milestones

### M1: env and model on today's mix
1. Env: per-episode components (outcome, start HP, end HP absolute, player turns, potions used per slot, cap hit); the 99-turn cap; start HP as given.
2. Model: `end` and `pot` heads on the shared trunk; the scalar value head kept only as a warm-start target for the first iterations (distil, then drop).
3. Search: `[rows, H]` value outputs, the combination and the terminal mapping in Rust with per-scenario weights; `rl/fastsearch.py` value graphs return the heads.
4. PPO: the head losses (3.3); policy advantage from the combined V.
5. Fine-tune from b128 (trunk and policy) rather than train from scratch; a fresh run only if the fine-tune fails the gate.

**Gate (paired, same seeds):** with today's weights the combined V must not regress: win >= -0.01 and HP lost <= +0.01 of max on `eval[:600]`, `eval_energy[:600]` and the corpus holdout (today's gate also checks HP lost from here on). Per-head calibration reported and kept in the ledger: for `end`, Brier of P(win) with a reliability table by decile and the coverage of the predicted end-HP quantiles (q10 / q50 / q90 on wins); for `pot`, Brier. Cost: search throughput with H outputs (`rl/bench_fast.py`) within 15 % of today. The potion regression states (`evals/solver_gaps.md`) priced from the heads alone (no play-outs) agree in sign with `agent/potion_price.py`.

### M2: relic classes and the mask
One table in `data/` classifying every relic from its decompiled hooks (`decomp/`): combat, macro-only (no combat effect: masked out of the observation and the training mix), potion-linked (belt size, potion generation or effects: scenarios must carry potions), card-linked (adds or enchants cards: scenarios must carry them). **Gate:** a macro-only relic added to any eval scenario moves its win by less than 2 se (the Lava Lamp probe, extended to every macro-only relic on 100 scenarios), plus the M1 gate.

### M3: curriculum and adaptive sampling
- Stages: easy fights, no potions, no extra relics, starter-like decks (a few Neow-like adds / removes / transforms), high HP -> harder fights, potions up to 8, relics per act like a real run (Act 1: one Neow relic; Act 2: + one Act 2 ancient relic; Act 3: + one Act 3 ancient relic), larger and more varied decks (upgrades, curses, colorless, other characters' cards, event cards). Keep a share of every earlier stage (no forgetting).
- Difficulty is adaptive for **sampling only**: prioritise fights the current policy wins 20-80 %; never in the reward.
- Advance a stage on fixed per-stage probe sets; report coverage (encounter x relic class x potion count x card type; situation probes: lethal this turn, a big-hit turn with a potion in hand, choice screens, multi-phase bosses).
- Prerequisite: observation resized for 8 potions and more relics (`MAX_POTIONS`, `MAX_RELICS`): a fresh network, trained from scratch through the curriculum.

**Gate:** the M1 gate on today's sets, plus an 8-potion / ancient-relic eval set (generated with a held-out seed) where the new network must beat the old by >= 2 se.

### M4: AutoSlay sampler
Run the game's AutoSlay mode (`decomp/MegaCrit.Sts2.Core.AutoSlay*`) with god mode and random choices; snapshot deck, relics, potions and HP at every fight. It gives the reachable distribution of real runs, between hand-made decks and random ones; it under-samples the long tail players build on purpose (engines, heavy removal), so it complements the generated mix. **Gate:** coverage report against the generated mix; M3's gate with the AutoSlay share in the mix.

## 5. Open questions
- Bin layout of `end` across characters (A10 max HP ~60-90, more with relics); whether 2-HP bins are fine enough for the route DP's cliffs.
- `pot` per slot or per potion id (duplicates).
- Randomised U and prices during training (and as policy inputs) vs fixed weights: M1 keeps them fixed; revisit only if the decision layer's potion prices disagree with play-outs.
- Search cost of H value outputs instead of one.
