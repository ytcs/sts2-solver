# Rebuild plan
Single plan, current state. Evidence `docs/research/evidence.md` (E#); game facts `docs/research/game_code.md`. Each stage: goal / status / gate. A stage failing its gate is not adopted; nothing builds on it. Simulator/search/env changes pass `bash tools/gate.sh` unchanged (a deliberate behaviour change updates its checksum in the same commit).

## 1. Goal, contracts, principles
- Goal: win A10 runs with all five characters; end HP breaks ties.
- Information contract: only the exact random state is hidden. Public, tracked as state: enemy move patterns + branch odds, potion-drop chance, card-rarity offset, unknown-room odds, encounter-bag narrowing, shop prices, removal cost. The operator may read the game source.

| layer | who | job |
|---|---|---|
| Predictor | network (+ search) | P(win) and end-HP distribution (potions left) of a fight, given visible state and the allowed belt |
| Combat play | search, predictor at leaves | card plays; PROPOSES potion use (now / keep / save, priced), never commits |
| Run model | act simulator | V = P(win run), chained at act boundaries (ancient heals 80% of missing HP at A2+); prices routes, picks, shops, rests, potions in one currency |
| Operator | Claude | high-level decisions, plans where numbers are flat, commits potions, logs gaps |

- The stack must not depend on policy quality (E22, E23): search finds the line; the predictor learns how search play ends; training effort goes to the judge (E12, E26). Live: `models/solver_r5.pt` is player and predictor (`models/current.json`); the live `Engine` searches every distinct legal action (cover, E27) with exact turn search on (E36).
- Only objective: P(clear run). HP matters through V: U(ending) = V(state after the fight); a potion's price = V drop without it. Price choices as P(win fight) x V(next act), never the myopic fight delta.
- Horizon ladder: the longest objective that is estimable and not saturated: fight -> clear act -> next-act readiness -> clear run.
- Claims: solver results are lower bounds under the stated solver; state exactly what was tested.

## 2. Predictor contract
f(state, allowed potions) -> joint distribution over outcome (loss = death or turn cap; win x end-HP bin) and potions used, under search play.
- Inputs: everything visible (enemies with intents and look-ahead, character incl. Stars/orbs/Osty, relics + counters, hand + pile multisets, belt, turn, act, encounter). One network for every character.
- Fight-start predictions average over sampled opening shuffles/rolls.
- Allowed set = the belt the state carries (`Sim.without_potions`); all belt sizes in training; no masks.
- Auxiliary outputs are diagnostics only. Never a clairvoyant target (a privileged model predicts the honest policy's outcome).

## 3. Metrics
- Near-miss bench (`tools/nearmiss_bench.py`, `data/bench/nearmiss.json`): near-miss losses won / close wins lost, the candidate paired directly with the live player (first arm; v1 baselines like r3 no longer load) (E21). User's key metric.
- Optimality bracket (E25): avoidable share of near-miss losses in [~0.02, ~0.19]; errors spread over setup turns (E26).
- Predictor bench (`tools/bench.py`): calibration (reliability, Brier, decile bias), ranking vs large-budget references.
- Promotion gate (new live model): `bench.py play` no set worse than the live player (paired with its labels); near-miss not worse; `bench.py score` Brier no worse on most sets; S3 decile bias < 0.02 (fails for r5, E35).

## 4. Roadmap (current order; review at each gate)
1. Done (E29): PPO signal sampler; gen-2 base.
2. Combat loop (S3): r5 live (E35). Next: round r6 from r5 on a pod (`scripts/pod_round.sh`, pool `tools/round_pool.py` scored by r5), then the promotion gate on a pod.
3. Predictor on engine/plan decks (E30, E31); every run-level price inherits it. (a) bench slice `plans`: done; (b) plan-shaped decks (archetype cores + fill, thin act-2 decks with and without a core) in the round pool: r5, continues in r6; (c) solver-run fallback for plan-deck gates: deprioritized (user): the search plays plan decks well (E30); the predictor gap closes through the rounds' plan-deck share, not a patch. Gate: slice |bias| < 0.05 and deck Spearman >= 0.8. Status (r5): bias -0.040 passes; Spearman 0.768 fails; enabler-removal pairs blind (0.10); worst archetypes doom -0.35, souls -0.25, osty -0.18, poison -0.18.
4. S5 continuation value (blocks S7): `price --cont` surrogate fails its gate (E30); next: tune the floor / compare `mean`, after item 3.
5. S4 gate (potion regression states).
6. S7 operator protocol + retire the calculators (after S5 and item 3).
7. Expert re-enactment (skill `expert-reenact`, E34): video -> record -> real-game replay on the seed (pinned build v0.111.0 only) -> per-decision verdicts vs the live player and `price`. Built (`tools/expert.py`, `agent/reenact.py`). Live seedcheck passed on the vanilla game (standard seeded run): Baalorlord YMY1KELG18SC reproduces Neow, boss, the unique room-consistent path, the first fight. Open: transcription of the rest of the video (running), then the full replay; gaps and rejections recovered by lookahead against the next recorded state.
8. Exact turn search when the search is blind: live (E36). Open: order-dependent setup turns (Gardeners #1, Lagavulin #0) need a horizon past the turn end.
9. Burn-off after a batch of stages (`burn-off` skill).

## 5. Stages

**S0. Hygiene.** Goal: the live sim never diverges silently. Open:
- re-sync hand from screen before a mid-card selection (Survivor, Dagger Throw); card text on the rewards screen; fight-start predictions state assumed potions.
- the search's chosen-line prefix reuse rarely fires (speed only, ~15% more network rows).
- caps must not cut combos (user): turns are the stall bound (99); action cap far above any legitimate fight; a play-out continues while the turn makes progress; per-step loop guard (20,000 units, max seen 174) stays.
- same-id enemies pair in list order (summons unverified; the bridge's `combat_id` is the robust key); discard/exhaust/draw sync ignore enchantments (hand sync and identical-monster slots fixed, E34).
Gate: a full act with no false `DIFFERS`.

**S1. Observation and fidelity.** Done. Look-ahead projects the next turns on a copy (`LOOK_H` = 4, joint encounter projection, `enemy_moves`); observation v2 (E20) is the only observation. Gate: fuzz 0 residual mismatches; look-ahead probability test within tolerance.

**S2. Benchmark before training.** Done (`tools/bench.py`, near-miss bench). Frozen sets (eval, high energy, corpus, cross-character/big belt, per-character, plans) labelled by gen-2 5x32 cover (E33; `bench.py relabel` relabels with the live model). Local RTX 4070 Super: 100k labelled fights ~2.5 h. Gate: re-runs within se.

**S3. Predictor trained on search play.** Status: r5 player and predictor (E35). Open: underprediction on hard fights (bias -0.035 mix, -0.056 tail; decile gate fails); enabler-removal pairs blind.
- Combat loop: each round collects with the live player (its cover search) on a signal-weighted pool (`tools/round_pool.py`), trains from the live model the policy (anchored c=2 target, E10) and the value (TD(0.8), E24) (`rl/exit.py train`), then applies the promotion gate. Value targets are realized/TD outcomes, never max of search Q; HL-Gauss categorical targets.
- Curriculum: fresh candidates (`tools/gen_curriculum.py` + corpus) scored by the current predictor; 15% uniform + rest by p(1-p) (`tools/signal_pool.py`), chosen before play. Open: A/B vs a uniform pool under the current recipe (E18 inconclusive).
- After a simulator change `tools/prune_divergent.py`; a part losing > 1% is regenerated.
- Privileged inputs only if label noise proves the bottleneck.
- Open data source: deck/relic/potion snapshots from the game's AutoSlay mode (`decomp/MegaCrit.Sts2.Core.AutoSlay*`).
- Gate: decile bias under search play < 0.02; ranking no worse than the live predictor; play not worse on any set; near-miss not worse.

**S4. Potion flow.** Done, gate pending (`agent/proposal.py`).
- Per turn, per potion, shared futures, other potions out: now / keep / save; stop iff now beats keep and save by > 2 paired se, or win at stake. Card search plans without potions; one potion per commit.
- Per-job `Worth` (`worth_row`, `search.rs`): act boss before an ancient heal = win-only (1% end-HP tiebreak) in prediction, `Engine.decide` and proposals; other fights linear.
- Open tests: self-damaging potions (Foul Potion: boss with only it vs none); timing before scheduled big hits (Vantom Dismember, Kaiser Laser, Byrdonis).
- Gate: on the potion regression states (run 20261005-201805: Vantom T1 Weak+Speed, Soul Nexus T4 Strength+Colorless) proposals agree with large-budget references.

**S5. Run model.** Implemented (`agent/runmodel.py`, `price.py`, `tracker.py`, `events.py`); `price` live.
- Paired rollouts per option (common random numbers): fights from the predictor's fight-start distribution; rewards, shops, potions, unknown rooms at coded odds from the tracker (`game_code.md` C); events `data/events.json` (unknown stubbed); real current map, template later acts. Base policy: rest-before-elites at low HP, greedy picks vs the act boss, whole belt at every elite/boss.
- Ladder: 2 paired se; P(clear act) >= `ACT_SATURATED` 0.9 -> next-act readiness -> floors. ~10-20 s per screen.
- Continuation value (priority; E30): P(win run) is genuinely flat. Surrogate `runmodel.gates` combined by `runmodel.combine` (`late` default, floor 0.05); `price --cont` / `STS2_PRICE_CONT`; `python -m agent.price <events.jsonl>` replays recorded screens. Surrogate gate (to make `--cont late` default): never significantly worse than the ladder on its own horizon while unsaturated, separates the best on most screens, stable under fresh seeds, plus the engine-deck predictor check. Status: fails (E30). Per-fight worth tables can use it once it passes.
- Open: the tag rule (`AddWithoutRepeatingTags`; needs pool tags); route-dependent options (Dowsing Rod, unknown/treasure/elite rewards): price (option, best route) pairs; shop bundles within budget vs saving gold.
- Gate: real runs fall inside the simulated distribution (act reached, HP at boundaries); prices stable under fresh draws.

**S6. Plan library.** Started (`data/plans.json`, `agent/plans.py`; `python -m agent.plans check` validates). 16 entries, 2-4 per character (silent-poison measured, the rest proposed).
- Entry: archetype, threats answered, core, enablers/payoffs, substitutes, measured win of full/partial plans per threat, status `proposed`/`measured`/`demoted` with n. Use: at each ancient or when the boss is out of reach, max V = P(reach) x P(win | plan).
- Self-improvement (after item 3): auto-measure `proposed` entries with solver tables (`bench.py build-plans` machinery); mine entries from decks the solver wins hard fights with and from remove-one-card ablation; `improve review` checks plan choices.
- Gate: every plan used live is `measured`; plan choice logged with V and reviewed.

**S7. Operator and self-improvement.** Draft; becomes the operator skill once S5's surrogate gate passes and the predictor passes on engine decks (item 3). Until then the current skills + calculators govern live play.
- One currency (P(win run); flat -> next horizon -> a measured plan). The predictor is the authority on fights except for a reason it cannot see, logged as a typed gap (fidelity / calibration slice / missing model / tool). Per screen: `plans` + `price` (events: `data/events.json`, else judgment + gap); combat `combat`/`turn`, potions per S4. Skills shrink to protocol, gap taxonomy, verified mechanics, plan reasoning; guards become prediction-vs-outcome logging; `improve review` becomes the gap review.
- Open: auto-combat hands back to manual before damage lands (predicted loss beyond the fight-start q90 pace); solver-gap tests (early stop on ties in manual fights, missed lethal); harness refactor + bug sweep of `agent/`.
- Calculators kept until `price`/`plans` replace them (then delete with their skill sections): `reward`, `routes`, `eval`, `rmcalc`, `pickplan`, `brief`, the `potions` command, `DRIVE` thresholds, decision guards (`agent/guards.py`; `STS2_DECISION_GUARDS=off`), smooth score, buckets, section-3 bar. Open while kept: weak CRN coupling for card/potion variants (paired se vs independent: upgrade 2.0x, removal 1.3x, add 1.0x); depth-2 fresh-seed baseline for table depth; paired-regret early stop in `engine._opportunity_loss`; upgrade debt for rest-vs-smith.
- Expert claims (`data/expert/japaneseexport/`; hypotheses; macro claims are `price` validation cases). Open: pre-boss smith at very low HP (smith iff P(win | smithed, HP now) >= P(win | rested)); skipping cards in 13-18-card decks once the plan works (skip-off-plan rollouts vs smooth greedy on P(clear 2 acts)); Skulking Colony rewards block; removal value; immediate-power ancients; 30 reconstructable states vs the live solver. Resolved: unknown-room fight odds (`[code]` C5). NaveGreed pilot: `data/expert/navegreed_2026-10-07.md`.

## 6. Target layout
| path | keeps | goes |
|---|---|---|
| `crates/` | sts2sim, sts2env, sts2py, sts2diff | |
| `rl/` | model, heads, fastsearch, solver, exit, ppo, predictor | |
| `agent/` | bridge, screen, args, fight, engine, live, potions, proposal, tracker, runmodel, runctx, pools, price, events, plans, crystal_sphere, harness + `__main__`, reenact, runlog, improve + hindsight, fidelity_sweep, skillgate | guards, macro, routes, pickplan, card_tags, the `potions` command (`live.potions_now`) once price/plans replace them |
| `tools/` | gate.sh, bench, bench_search, nearmiss_bench, nearmiss, exact_turn_check, signal_pool, round_pool, gen_curriculum, gen_train, gen_relic_mask, fuzz_gen_mix, prune_divergent, collect.sh, headroom, dashboard, expert | one-off A/B scripts (results go to `evidence.md`) |
| `scripts/` | pod_*.sh, skill_gate.py, porting/ | |
| `.claude/skills/` | `sts2`, operator protocol + gap logging, plan-library use, verified mechanics | procedures feeding only the old calculators |
| `data/` | catalog, pools, ancients, events, relic classes, `bench/`, `plans.json`, train/eval sets | |
