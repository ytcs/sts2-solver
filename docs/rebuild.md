# Rebuild plan
Single plan, current state. Evidence: `docs/research/evidence.md` (E#). Game-code facts: `docs/research/game_code.md`. Each stage: goal / status / gate. A stage failing its gate is not adopted; nothing builds on it. Simulator/search/env changes pass `bash tools/gate.sh` unchanged (deliberate behaviour change: update its checksum in the same commit).

## 1. Goal, contracts, principles
- Goal: win A10 runs with all five characters; end HP only breaks ties.
- Information contract: only the exact random state is hidden. Public and tracked as state: enemy move patterns + odds at random branches, potion-drop chance, card-rarity offset, unknown-room odds, encounter-bag narrowing, shop prices, removal cost. The operator may read the game source.
- Layers:

| layer | who | job |
|---|---|---|
| Predictor | network (+ search) | P(win) and end-HP distribution (with potions left) of a fight, given full visible state and the allowed belt |
| Combat play | search, predictor at leaves | card plays; sees every potion, PROPOSES potion use (now / keep / save, priced), never commits |
| Run model | act simulator, deck changing | V = P(win run), chained at act boundaries (ancient heals 80% of missing HP at A2+); prices routes, picks, shops, rests, potions in one currency |
| Operator | Claude | high-level decisions, plans where numbers are flat, commits potions, logs gaps |

- Principles:
  - The stack must not depend on policy quality (E22, E23): search finds the line; the predictor learns how search play ends. Policy roles left: candidate order (live Engine searches every distinct legal action, `cover=True`, E27) and the play-out policy (fixed; a change is a gated experiment, never a training side effect). Live model: `models/solver_td08.pt` (TD(0.8) targets; policy + predictor; E27). Training effort goes to the judge (E12, E26).
  - Only true objective: P(clear run). HP matters only through V: U(ending) = V(state after the fight); worth of HP = dV/dHP (≈0 before an ancient heal); a potion's price = V drop without it. Price choices as P(win fight) x V(next act), never the myopic fight delta.
  - Horizon ladder: use the longest objective that is estimable and not saturated: fight -> clear act -> next-act readiness -> ... -> clear run.
  - Claims: solver results are lower bounds under the stated solver, never verdicts on plans; state exactly what was tested.

## 2. Predictor contract
f(state, allowed potions) -> joint distribution over outcome (loss = death or turn cap; win with end-HP bin) and which allowed potions are used, under the best combat policy (search), not the raw network.
- Inputs: everything visible: enemies (powers, intents, multi-turn look-ahead), character (HP, block, energy, Stars, orbs, Osty), relics + counters, hand + draw/discard/exhaust multisets, belt, turn, act, encounter.
- Fight-start predictions average f over sampled opening shuffles/rolls (exact: revealed before the first decision).
- One network for every character and cross-character cards.
- Allowed set = the belt the state carries (`Sim.without_potions` removes the rest); all belt sizes in training; no masks.
- Auxiliary outputs (enemy HP left on a loss, turns survived): diagnostics/features only; decisions maximise V.
- Never a clairvoyant target: a privileged model must predict the honest policy's outcome (Baisero & Amato 2022).

## 3. Metrics
- Near-miss bench (`tools/nearmiss_bench.py`, `data/bench/nearmiss.json`, held out from r4s-trained arms): near-miss losses won / close wins lost, paired with the collecting player (E21). User's key metric.
- Optimality bracket (E25): honest 5x256 flips = lower bound on avoidable losses, clairvoyant = upper bound; avoidable share in [~0.02, ~0.19]; 48% of near-miss losses unwinnable with perfect information. Avoidable errors are spread over setup turns (E26): sharpen the judge, not depth.
- Predictor bench (`tools/bench.py`): calibration (reliability, log loss/CRPS, PIT), ranking vs large-budget reference.
- Promotion gate (a new live model, E27): `bench.py play` no set worse than the live player (paired with labels); near-miss bench not worse; `bench.py score` Brier no worse on most sets; S3 decile bias < 0.02 (not yet run for solver_td08).

## 4. Roadmap (current order; review at each gate)
1. PPO sampler A/B (uniform vs p(1-p) signal; d256, obs v2, 3200 it, from scratch): pick the sampler by eval curve at matched iterations + greedy eval; the winner is the gen-2 base.
2. Gen-2 model: gen-2 base + outcome head -> combat loop rounds (S3) on obs v2 (old collections replay to v2: actions are stored, not observations) -> promote when it beats solver_td08 on the promotion gate.
3. S5 continuation value (top priority for run-level decisions; blocks S7): a non-flat V(act-start state).
4. Relabel the bench with the live player (labels are h128's play; the predictor's calibration target is its own player, E28); S4 gate (potion regression states).
5. S7 operator protocol + retire the calculators (gated on S5).
6. Expert re-enactment (queued); burn-off after a batch of stages (`burn-off` skill).

## 5. Stages

**S0. Hygiene.** Status: open items:
- re-sync hand from screen before answering a mid-card selection (Survivor, Dagger Throw); card text on rewards screen; fight-start predictions state assumed potions.
- search `carry` never fires (speed only, ~15% more network rows).
- Caps must not cut combos (user): turns are the stall bound (99); action cap far above any legitimate fight; a play-out continues while the turn makes progress (enemy HP falling, cards/energy generated); per-step loop guard (20,000 work units, observed max 174) stays.
- Gate: a full act with no false `DIFFERS`.

**S1. Observation and fidelity.** Status: done. Look-ahead plays the next turns on a projected copy (`LOOK_H` = 4, joint encounter projection, pending node + stored follow-up in `enemy_moves`); cross-character cards in fuzzers. Observation v2 (calculated card numbers, affliction amounts, selection purpose, power secondary numbers, 64 candidates; E20) exists behind the version switch for the next from-scratch run; v1 bit-identical (gate). Gate: fuzz 0 residual mismatches; look-ahead probability test within tolerance.

**S2. Benchmark before training.** Status: done (`tools/bench.py`, near-miss bench). Frozen sets with search labels at live width (5x32): eval, high energy, real-run corpus, cross-character/big belt, per-character. Local RTX 4070 Super: 100k labelled fights ~2.5 h (no pod needed). Gate: re-runs within se.

**S3. Predictor trained on search play.** Status: live predictor + player `models/solver_td08.pt` (TD(λ=0.8) value targets, E24; with cover search, E27); passes the promotion gate except decile bias: +0.03 pessimistic at P(win) 0.27-0.81 vs h128's labels (E28).
- Combat loop (the engine of combat improvement): each round collects with the live player (its own cover search) on a signal-weighted pool, trains value with TD(0.8) from the live model, then applies the promotion gate; promote on pass. Policy stays fixed except via a new PPO base (no policy distillation target: E15, E22). Value targets are realized/TD outcomes, never max of search Q (winner's curse); HL-Gauss categorical targets; Reanalyse of stored fights.
- Curriculum by signal: each round draws fresh candidates (`tools/gen_curriculum.py` + corpus), scores at fight start with the current predictor, samples 15% uniform anchor + rest by p(1-p) (`tools/signal_pool.py`); selection before a seed is played (labels unbiased). Measured by A/B vs a uniform pool, calibration on the natural distribution. ExIt pool A/B (E18) inconclusive: run under the policy-target bottleneck; re-test for value-only training.
- Near-miss restarts (`tools/nearmiss.py`, `exit.py collect --restarts`, parts `policy_only`): deprioritized: stronger honest search flips only 2-4 points and avoidable errors are spread over setup turns (E25, E26).
- Data hygiene after a simulator change: `tools/prune_divergent.py` drops fights that no longer replay (backup kept); a part losing > 1% is regenerated instead.
- Privileged inputs only if label noise proves the bottleneck.
- Open data source: snapshots of deck/relics/potions at each fight from the game's AutoSlay mode (god mode, random choices; `decomp/MegaCrit.Sts2.Core.AutoSlay*`) as realistic setups alongside the generated mix.
- Gate: calibration bias under search play < 0.02 in every decile; ranking no worse than h128; live-width win not lower on any set; near-miss bench not worse.

**S4. Potion flow.** Status: done, gate pending (`agent/proposal.py`).
- Per turn, per potion, on the same futures and job seeds, other potions out of every arm: now (best target chosen on other futures) / keep (usable from next turn) / save (`without_potions`). Stop iff now beats keep and save by > 2 paired se in the fight's score, or the better of now/keep wins more than save by > 2 paired se. Live card search plans without potions. One potion per commit (`potion use`); `potion aside` keeps one for the boss.
- Per-fight objective via the live per-job `Worth` plumbing (`rl/fastsearch.py` `worth_row`, `search.rs` `Worth`): act boss followed by an ancient heal = win-only table (1% end-HP tiebreak), for fight-start prediction, `Engine.decide` and proposal play-outs; other fights linear.
- Open tests: self-damaging potions (Foul Potion: per potion id, boss fight with only it vs none; below "none" = misuse); potion timing before scheduled big hits (Vantom Dismember, Kaiser Laser, Byrdonis: training coverage of potion x big-hit pairs; value of use-now vs hold vs long-search truth two turns before).
- Gate: on the potion regression states (recorded run 20261005-201805: Vantom T1 Weak+Speed, Soul Nexus T4 Strength+Colorless), proposals agree with large-budget references.

**S5. Run model.** Status: implemented (`agent/runmodel.py`, `agent/price.py`, `agent/tracker.py`, `agent/events.py`); `price` live.
- Paired rollouts per option under a base policy with common random numbers: fights from the predictor's fight-start distribution (4-8 opening shuffles), rewards/shops/potions/unknown rooms at coded odds from the tracker's counters (`game_code.md` C), events from `data/events.json` (unknown stubbed). Current act on its real map; later acts from a template (room counts C1, known or sampled boss, ancient 80% heal + relic). V needs no terminal value.
- Base policy: path by rests-before-elites at low HP / elites at high HP; picks greedy on the predictor vs the act boss (plan cards when a plan is set); rest below 50% HP else smith best gain; shop removal first then best item within budget, else carry gold; potions at elites/bosses when win gain > 0.05.
- Encounter draws: weak pool first 3/2/2, bag per pool (`runmodel.draw_encounter`). Open: the tag rule (`AddWithoutRepeatingTags`; needs pool tags from decomp).
- Horizon ladder in `price`: significance 2 paired se; P(clear act) >= `ACT_SATURATED` 0.9 (`price --sat`) -> next-act readiness (`runmodel.readiness`: P(win) vs next act's boss pool 0.5 + elite pool 0.5 at HP after the ancient heal; Glory as pairs of distinct bosses), floors tiebreak.
- Cost: ~8k fight-start predictions/s; 4 options x 64 rollouts x ~25 predictions ≈ 10-20 s.
- Open (priority): continuation value. P(win run) is flat under the base policy, so `price` falls back to act/fight horizons (myopic). Need a non-flat V(act-start state): plan-directed base policy, a learned value over act-start states, or measured next-act boss-pool win rates as proxy. Per-fight worth tables U(ending) = V(state after) wait on it.
- Open: route-dependent options (Dowsing Rod, unknown/treasure/elite rewards) undervalued while the base policy routes the same: price (option, best route) pairs or deck-aware route preferences.
- Open (shop/draft): bundles within budget; saving gold vs small gains; speculative drafting is for plans (S6) and the operator.
- Gate: real runs fall inside the simulated distribution (act reached, HP at act boundaries); a price is stable under fresh draws (se reported).

**S6. Plan library.** Status: started (`data/plans.json`, `agent/plans.py`, `plans`).
- Entry: character, archetype, threats answered (bosses, elites, mechanics), core cards/relics, enablers/payoffs, substitutes, predictor-measured win of full and partial plans vs each threat at stated HP/act, status `proposed`/`measured`/`demoted` with sample sizes.
- Use: at each ancient and whenever the boss is far out of reach, pick the plan with best V = P(reach) x P(win | it); the run model prices picks/shops/routes by progress toward it via the partial-version table.
- Learning: operator proposes (first principles, source, `[expert]` runs); measured before use; demoted only by measurement, never one run.
- Gate: every plan used live is `measured`; plan choice logged with V and checked in review.

**S7. Operator and self-improvement.** Status: draft protocol below; becomes the operator skill once S5 passes (S3 is effectively passing; S5's flat V is the blocker). Until then the current skills + calculators govern live play.
- Principles: one currency (P(win run); flat -> next horizon -> a measured plan; never unmeasured intuition). The predictor is the authority on fights: question it only for a reason the model cannot see, logged as a gap. Every disagreement or tail outcome is a typed gap (fidelity / calibration slice / missing model / tool) -> fix or experiment through its stage's gate.
- Per screen: Neow/ancient `plans` + `price` (unmodelled relic effects by judgment, logged); map `price` at every fork; card reward `price`, flat -> the plan's card else skip; shop `price` (nothing vs best affordable bundles of <= 3 purchases); full belt + potion offer `price`; rest `price` (rest vs each smith); event `data/events.json` + `price`, unmodelled -> judgment + gap; combat `combat`/`turn`, potions per S4.
- Skills shrink to: operator protocol, gap taxonomy, verified mechanics, plan reasoning with predictor-tested target decks. Decision guards replaced by prediction-vs-outcome logging. `improve review` becomes the gap review (predicted vs real fights, `price` decisions vs outcomes, plan choices vs tables, gap list).
- Open: auto-combat hands back to manual before damage lands (next enemy turn's predicted loss beyond the fight-start q90 pace, or predicted win drop > X); design on `runs/*/fights/` costly fights.
- Open solver-gap tests: early-stop on ties in manual fights (`agent.hindsight` large budget vs the plan in the why); missed lethal (states where a 1-2 card line kills the last enemy: solver's first choice should reach lethal ~100%).
- Open: harness refactor + bug sweep of `agent/` after the RL work.

**Expert data (queued).** Top players' runs (NaveGreed, OpemSpire) allowed. Macro decisions seed the plan library as `[expert]`; a few hundred pivotal combat decisions form a test set (replay both lines where search disagrees); imitation only if that set shows gaps. Pilot (`data/expert/navegreed_2026-10-07.md`): one A10 Ironclad win on v0.111.0 -> 41 macro decisions + 3 fights in ~55 min; accuracy ~95% visible choices, ~60% encounter names. Next: decision-screen detector, OCR limited to catalog ids, caption alignment, seed-replay test; encounter id by sprite matching (`SlayTheSpire2.pck`). Re-enactment: start his seed, replay his transcribed actions, export each fight start, compare his line with the solver's on the same hidden state per decision (risks: one transcription error breaks later fights; 3 unidentified mods).

## 6. Retired / live / kept
- Retired: potion `search_keep`, alert thresholds, keep/allow/deny/hold; the HP-worth util curve (failed its gate: curve vs linear E[U] -0.000 +- 0.002); Gumbel root (E16); value ensembles; policy distillation targets (E15, E22); duplicate route pricers and model gates.
- Live: per-job `Worth` (win-only act-boss objective, S4).
- Kept until `price`/`plans` replace them (then delete with their skill sections): calculators `reward`, `routes`, `eval`, `rmcalc`, `pickplan`, `brief`, `potions`; `DRIVE` AUTO/MANUAL thresholds; decision guards (`agent/guards.py`; `STS2_DECISION_GUARDS=off` disables); smooth score, buckets, section-3 bar, pickplan ρ, `routes` reward weights. Open calculator items while kept:
  - CRN coupling weak for card/potion variants (paired se vs independent: upgrade 2.0x smaller, removal 1.3x, added card 1.0x, belt 1.0-1.7x). Candidates: future keys from (job seed, turn, decision index); eval-only shuffle by per-card-instance random keys.
  - Table depth: depth 2 tables 2.3x slower than depth 1; depth 1 picks within 2 paired se of depth 2's best on 30/30 screens (same best 77%). Missing baseline: depth 2 vs depth 2 with fresh seeds.
  - Live stopping rule (`engine._opportunity_loss`): paired regret test may stop earlier; measure rounds-to-stop.
  - Upgrade debt: rank deck cards by upgrade gain / draw frequency; rest-vs-smith may need to compare against summed debt.

## 7. Target layout
| path | keeps | goes |
|---|---|---|
| `crates/` | sts2sim, sts2env, sts2py, sts2diff | |
| `rl/` | model, heads, fastsearch, solver, exit, ppo, predictor | |
| `agent/` | bridge, screen, fight, engine, live, proposal, tracker, runmodel, price, events, plans, harness + `__main__`, runlog, improve + hindsight (review), fidelity_sweep (only fidelity tool), skillgate | guards, macro, routes, pickplan, potions, card_tags once price/plans replace them |
| `tools/` | gate.sh, bench, bench_search, nearmiss_bench, nearmiss, signal_pool, gen_curriculum, gen_train, fuzz_gen_mix, prune_divergent, collect.sh, headroom, dashboard | one-off A/B scripts (results go to `evidence.md`); next burn-off: policy_agree, target_noise (policy study closed) |
| `.claude/skills/` | `sts2` (rules, information contract), operator protocol + gap logging, plan-library use, verified mechanics | procedures that only feed the old calculators (buckets, smooth score, section-3 bar, guard fields) |
| `data/` | catalog, pools, ancients, events, relic classes, `bench/`, `plans.json`, train/eval sets | |
