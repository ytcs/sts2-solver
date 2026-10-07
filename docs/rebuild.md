# Rebuild plan

Status: plan, 2026-10-06. Direction from the user; methods chosen from the literature (`docs/research/literature.md`), the game code (`docs/research/game_code.md`), a playtest (`docs/research/playtest.md`) and measurements (`docs/research/evidence.md`). Each stage has a gate. A stage that fails its gate is not adopted, and nothing is built on top of it.

## 1. Goal and contracts

**Goal.** Win A10 runs with all five characters. Ending HP only breaks ties.

**Information contract.** The only hidden information is the exact random state. Anything that follows from the game source plus what has been observed is public and is tracked as state:
- enemy move patterns, with the odds at random branches;
- the potion-drop chance of this fight;
- the card-rarity offset;
- unknown-room odds;
- the narrowing of the encounter bag;
- shop prices and removal cost.

The operator may read the game source.

**Layers.**

| Layer | Who | Job |
|---|---|---|
| Predictor | network (+ search for action choice) | P(win) and the end-HP distribution (with potions left) of a fight, conditioned on the full visible state and an explicit set of allowed potions |
| Combat play | search with the predictor at the leaves | picks card plays. It sees every potion and PROPOSES potion use (use now / keep / save, each priced); it never commits one |
| Run model | simulator over the act, with the deck changing | V = P(win the run), chained at act boundaries (ancients heal 80% of missing HP at A2+). Prices routes, picks, shops, rests and potion keep/spend in one currency |
| Operator (LLM) | Claude | High-level decisions, and game plans where numbers are flat: when the deck is far from beating what lies ahead, greedy pricing has no gradient, so the operator proposes target decks and plans from first principles and the predictor tests them. Commits potions. Collects gaps: fidelity, calibration slices, missing models, tool friction |

## 2. Predictor contract

f(state, allowed potions) → a joint distribution over:
- outcome: a loss (death or the turn cap), or a win with an end-HP bin;
- which allowed potions are used.

It predicts play under the best combat policy we have (search), not the raw network.

- **Inputs:** everything visible:
  - enemies with powers, intents and the multi-turn pattern look-ahead;
  - the character: HP, block, energy, Stars, orbs, Osty;
  - relics and their counters;
  - hand, and the draw, discard and exhaust piles as multisets;
  - the belt, the turn, the act and the encounter.
- **Fight-start predictions** average f over sampled opening shuffles and starting rolls. This is exact, because those are revealed before the first decision.
- **Every character and cross-character cards.** One network covers all of them.
- **The allowed set is the belt the state carries.** A potion not allowed in this fight is removed from the state (`Sim.without_potions`), and the remaining belt is what the predictor sees. Belts of every size are in the training mix, so the allowed set needs no extra input and no masks.
- **Auxiliary outputs** (not decision currencies): enemy HP left when a fight is lost, and turns survived. They are diagnostics, features for plan testing, and inputs to the run model's base policy. Decisions maximise V only.
- **Never a clairvoyant target.** If a privileged model is trained on determinized states, it predicts the honest policy's outcome. Averaging such a model over sampled hidden states is then unbiased (Baisero & Amato 2022). The value of a clairvoyant player is optimistic and is never used.

## 3. Evidence so far
- **E1** (`evidence.md`): the h128 head predicts the raw policy (bias -0.001) but underestimates search play by 6.5 points overall and by 19-23 points in the 0.2-0.8 band. The predictor must be trained on search-played outcomes.
- **E-KD (E2):** under h128 search, the playtest deck had 0.00 against Knowledge Demon, while two hand-built decks scored 1.00 (poison engine) and 0.98 (power scaling), and one shiv package 0.22. These are lower bounds under this solver, not verdicts on the decks; they show that whole-deck plans give a signal where single-pick pricing gives none.
- **Playtest:** potion pricing in three units was unreadable. Fight-start numbers didn't say which potions they assumed. Event pricing needed the source. Shops are budget-basket problems. Mid-card selections desync the hand.

## 4. Stages

**S0. Hygiene (no retrain).**
- Re-sync the hand from the screen before answering a mid-card selection (Survivor, Dagger Throw).
- Starter-card name matching (done, `agent/live.py`).
- Fight-start predictions state which potions they assume.
- Card text on the rewards screen.
- *Gate:* a full act played with no false `DIFFERS`.

**S1. Observation and fidelity (forces a retrain, so it goes first).**
- Look-ahead fixes (`game_code.md` A):
  - advance each monster's own state along every projected path (sleep and summon countdowns, spawns, buffs on itself);
  - project the encounter jointly;
  - put the current state-machine node in the observation;
  - add the other enemies' pending moves to the cache key;
  - add a probability-accuracy test;
  - extend the horizon to 4-5 turns.
- Cross-character cards in the fuzzers, and the `base_orb_slots` default fixed (`game_code.md` B).
- *Gate:* fuzz rounds with 0 residual mismatches on the new mixes; the look-ahead's probability test within tolerance.

**S2. Benchmark before training.**
- Frozen sets with search-played labels at live width (5x32 adaptive), covering:
  - eval;
  - high energy;
  - real-run corpus fights;
  - a cross-character and big-belt set;
  - per-character slices.
- Metrics:
  - calibration: P(win) reliability, end-HP log loss / CRPS, PIT;
  - ranking accuracy against a large-budget reference, on deck-variant pairs, potion use-now / keep / save triples, and action pairs.
- **Compute (measured):** local RTX 4070 Super, `rl/bench_fast.py`, 200 eval fights:
  - 3x8: 39 fights/s (about 140k per hour), win 0.740;
  - 5x32: 12 fights/s (about 44k per hour), win 0.745.

  A 100k-fight label set at live width takes about 2.5 h locally, so RunPod is not needed.
- *Gate:* the benchmark is reproducible (re-runs within se).

**S3. Predictor trained on search play.**
- Distillation / expert iteration:
  - value targets = realized outcomes, never the max of search Q values (winner's curse);
  - policy targets = Gumbel-style improved policy;
  - HL-Gauss categorical targets;
  - Reanalyse of stored fights.
- Curriculum by learnability p(1-p) with a uniform share. Calibration measured on the natural distribution, with importance weights.
- A privileged-input stage only if label noise proves to be the bottleneck.
- *Gate:* calibration bias under search play below 0.02 in every decile; ranking accuracy no worse than h128; live-width win not lower on any set.

**S4. Potion flow.**
- Every turn, use now / keep in this fight / save, each priced by paired search play-outs on the same futures, with the predictor at the leaves (not two forward passes: a shared network understates the gap).
- `turn` / `combat` stop on a proposal; the operator commits.
- Remove `search_keep`, the alert thresholds, and keep / allow / deny / hold.
- *Gate:* on the potion regression states, the proposals agree with large-budget references.

**S5. Run model.**
- An act simulator in which the deck changes:
  - map;
  - rewards, shops, potions and unknown rooms at the coded odds (`game_code.md` C);
  - events from a catalog generated from the decomp, with unknown ones stubbed;
  - fights through the predictor.
- A base macro policy for the rollouts. Each operator decision is priced as V(option) by paired rollouts.
- First output: the simulated run win rate of the base policy, the baseline every later macro change must beat.
- *Gate:* real runs fall inside the simulated distribution (act reached, HP at each act boundary).

**S5 design.** A macro decision is priced by paired rollouts: one step of policy improvement over a base policy, the same idea as the combat search one level up.
- **Rollouts.** For each option, M continuations under a base policy, with common random numbers across options (the same map, the same reward and fight draws).
- **What each rollout samples:**
  - fights from the predictor's fight-start distribution (averaged over 4-8 opening shuffles);
  - rewards at the coded odds, with the tracker's counters as the starting state (`game_code.md` C, `agent/tracker.py`);
  - unknown rooms from their odds;
  - events: catalogued ones from the catalog, unknown ones stubbed as nothing.
- **Horizon.**
  - The current act uses its real map.
  - Later acts use a template act: their room counts and order distribution (C1), the known or sampled boss, and the ancient's 80% heal and a relic draw.
  - The run ends with a win or a death, so V = P(win run) needs no hand-made terminal value.
- **Base policy** (cheap, deterministic given the draws):
  - path: the child whose subtree has the most rests before elites at low HP, else the most elites at high HP;
  - card picks: greedy on the predictor against the act boss, restricted to the chosen plan's cards when a plan is set (S6);
  - rest below 50% HP, otherwise smith the card with the best predicted gain;
  - shop: removal first, then the best predicted card within budget;
  - potions: allowed at elites and bosses when the predictor's win gain exceeds 0.05.
- **Cost (measured):** about 10k fight builds per second plus 55k network evaluations per second on the local GPU, so about 8k fight-start predictions per second. A decision with 4 options x 64 rollouts x ~25 predictions each takes about 10-20 s.
- **Validation:**
  - the base policy's simulated run win rate is the baseline;
  - the act-boundary HP and the act reached by real runs fall inside the simulated distribution;
  - a decision's price is stable under fresh draws (se reported).

**S6. Plan library (codified high-level reasoning).** When the deck is far from what lies ahead, decisions come from game plans, not from greedy numbers. Plans must not drift between sessions, so they live in a versioned database (`data/plans/`), not in each agent's intuition.
- **Entry:**
  - character;
  - archetype (poison engine, power scaling, ...);
  - the threats it answers: bosses, elites, mechanics such as heal-and-Strength races, curses, Artifact;
  - core cards and relics;
  - enablers and payoffs;
  - acceptable substitutes;
  - the predictor-measured win rate of the full plan and of partial versions (core only, core plus one) against each threat at stated HP and act;
  - status (`proposed` / `measured` / `demoted`) with dates and sample sizes.
- **Use:** at each act start (the ancient), and whenever the boss is far out of reach, the operator picks the plan with the best V (P(reach it) x P(win | it)) for this character, deck and known boss, and records the choice. The run model then prices picks, shops and routes by progress toward the chosen plan, with the plan's partial-version table as the bridge between where the deck is and where it needs to be.
- **Learning loop:** new archetypes are proposed by the operator (from first principles or the game source) and enter as `proposed`, becoming `measured` once the predictor's table is filled. After each run, the review compares chosen plans with outcomes; plans that fail their tables are re-measured or demoted. A plan changes only through a measurement, never on one run's anecdote.
- *Gate:* every plan used in live play is `measured`; the operator's plan choice is logged with V and checked in review.

**S7. Operator and self-improvement.**
- Skills shrink to:
  - the operator protocol;
  - the gap taxonomy;
  - verified mechanics;
  - game-plan reasoning, with target decks tested by the predictor.
- Decision guards are replaced by prediction-vs-outcome logging.
- Every gap is typed (fidelity / calibration slice / missing model / tool) and leads to a fix or an experiment, adopted only through the gate of its stage.

## 5. What is retired
- The potion machinery listed in S4.
- `DRIVE` thresholds and the danger budget formula.
- The decision guards (`agent/guards.py` decision/pick guards; for now `STS2_DECISION_GUARDS=off` switches them off).
- Smooth score, buckets, the section-3 bar, pickplan's ρ, and the `routes` reward weights (all replaced by V).
- The two route pricers and the two model gates (one of each remains).
- The dormant HP-worth / `Worth` / `ucond` plumbing.

## 6. Target layout (lean)
Each stage lands in one place and deletes what it replaces.

| Path | Keeps | Goes |
|---|---|---|
| `crates/` | sts2sim, sts2env, sts2py, sts2diff (the simulator, search, bindings, differential tester) | the `util` / `Worth` plumbing once S4 lands |
| `rl/` | model, heads, fastsearch, solver, exit (training by expert iteration) | utility.py (HP-worth), dist.py (old end-HP head), pot_check, heads_check (folded into tools/bench), ppo.py once ExIt trains from h128 alone |
| `agent/` | bridge, screen, fight (replay and sync), engine (search and predictor service), live (combat driver with proposals), tracker (public counters), runmodel (S5), plans (S6), harness + `__main__` (commands, daemon), runlog, review (S7), skillgate | guards, macro, routes, pickplan, potion_price, potions, deckstudy, card_tags, improve, autopilot, calibrate, fidelity_report / fidelity_trace (one fidelity tool stays), args |
| `tools/` | bench, fuzz generators, gen_curriculum | one-off A/B and payoff scripts (results stay in `evals/` summaries) |
| `.claude/skills/` | `sts2` (rules, information contract), operator protocol and gap logging, plan-library use, verified mechanics | procedures that existed only to feed the old calculators (buckets, smooth score, section-3 bar, guard fields) |
| `data/` | catalog, pools, ancients, relic classes, `bench/`, `plans/`, training and eval sets | |
