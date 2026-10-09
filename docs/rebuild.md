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
  - The stack must not depend on policy quality (E22, E23): search finds the line; the predictor learns how search play ends. Policy roles left: candidate order (live Engine searches every distinct legal action, `cover=True`, E27) and the play-out policy (fixed; a change is a gated experiment, never a training side effect). Live models: player `models/solver_gen2.pt` (d256, obs v2; E32), predictor `models/solver_td08.pt` (E27). Training effort goes to the judge (E12, E26).
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
1. Done (E29): PPO signal sampler adopted; gen-2 base = d256 obs v2 signal run (greedy 0.714 vs live 0.700).
2. Combat loop: `models/solver_r5.pt` is player and predictor (E35: gen-2 + round r5, near-miss +0.072 vs gen-2's +0.059, plans bias -0.040). Next: round r6 from r5 on a pod (`scripts/pod_round.sh`, pool via `tools/round_pool.py` scored by r5), then the promotion gate on a pod.
3. Predictor on engine/plan decks (E30: Silent poison plan predicted 0.028 vs solver 0.97/0.81 at Knowledge Demon/Insatiable; enabler removal invisible to it). Every run-level price and plan choice inherits it. Steps: (a) bench slice `plans`: plan-library target decks (data/plans.json), recorded runs' act-start decks and expert decks vs their act's elites and bosses, labelled by the live player (16 attempts); measure predictor vs solver per deck for the live model and gen-2; (b) training coverage: plan-shaped decks (archetype cores from data/plans.json + random fill) as a share of the combat-loop pool; (c) until the slice passes, fight gates for plan decks come from short solver runs, not the predictor. Gate: slice |bias| < 0.05 and per-deck ranking (Spearman) >= 0.8. Status: (a) done, 1505 fights labelled by gen-2 (E31, E33); r5 (E35): bias -0.040 (passes), deck Spearman 0.768 (fails 0.8), enabler removal still blind (pairs Spearman 0.10), worst doom -0.35, souls -0.25, osty -0.18, poison -0.18; (b) r5 done, continues in r6; (c) open.
4. S5 continuation value (blocks S7): gate-array surrogate behind `price --cont` (E30) fails its gate (4 of 93 recorded screens significantly worse, mostly rests); next: tune the floor / compare `mean`, after item 3.
5. S4 gate (potion regression states). Bench relabel done (E33).
6. S7 operator protocol + retire the calculators (gated on S5 and item 3).
7. Expert re-enactment pipeline (user request; pilot done, E34: build v0.111.0 matches, so real-game replay applies): YouTube run video -> fetch (yt-dlp, transcript + frames git-ignored) -> segment every decision (combat turns, rewards, map, shop, rest, events, ancient) -> read the run seed (always visible) and game version, transcribe every ACTION (cards + targets, potions, end turn, picks, path, shop, rest, events) -> replay in the REAL game on that seed via the bridge (exact states, no frame reconstruction; the game rejects an illegal action = transcription error) -> at each decision record the true state; compare combat vs live player + large-budget reference (clairvoyant reference allowed for diagnosis: not a scored run), macro vs `price` -> per-divergence verdict (our gap / expert error / tie, with se) -> committed notes + records. Version gate: re-enact only when the video's build equals the pinned build (v0.111.0); otherwise skip the replay (frame-based notes only). A `MODDED` stamp is fine: the creators' mods are QoL / streaming only and leave the RNG alone (user). Deliverable: tooling + a terse `expert-reenact` skill. Status: built (`tools/expert.py` fetch/build/validate/compare/report, harness `seedcheck`/`replay` in `agent/reenact.py`, run record `data/expert/baalorlord/hMrQSndDvPc.json`, skill `expert-reenact`); offline compare on the five frame-built fights reproduces the pilot's exact and R2 verdicts (E34 re-run). Open: first live `seedcheck` (standard vs custom seeding unverified; the record's floor numbering is inferred), transcription of the Baalorlord floors 2-6 fights, map columns and reward clicks (the replay stops at floor 2 until then).
8. Exact turn search when the search is blind (E34: forced lethal scored as all-loss; order-dependent setup turns misranked): when every candidate's value is a loss, or the top values tie within noise, enumerate every distinct line to end of turn (Colony turn: 141 lines, ~1 s) and score end states with the value net. Gate: near-miss bench L->W up with W->L not worse, paired vs the live player; the three pilot states solved. After round r5 frees the GPU. Status: built (`FastSearch(exact_turn=...)`, Rust `ExactCfg`: loss -0.9, tie 0, 8 paired determinizations, cap 5000 states; check `tools/exact_turn_check.py`): Colony #18 solved (Strike, 1.064 = win); Gardeners #1 plays Outbreak (exact 1.146 vs block-first 1.05, not the expert's line); Lagavulin #0 exact four-way tie (keeps the searched move). CPU, 50 mix fights: triggers on 37% of searched decisions (12% of them capped), +19% wall per decision. Next: near-miss bench arm `@x` on the GPU.
9. Burn-off after a batch of stages (`burn-off` skill).

## 5. Stages

**S0. Hygiene.** Status: open items:
- re-sync hand from screen before answering a mid-card selection (Survivor, Dagger Throw); card text on rewards screen; fight-start predictions state assumed potions.
- search `carry` never fires (speed only, ~15% more network rows).
- Caps must not cut combos (user): turns are the stall bound (99); action cap far above any legitimate fight; a play-out continues while the turn makes progress (enemy HP falling, cards/energy generated); per-step loop guard (20,000 work units, observed max 174) stays.
- Same-id enemies pair in list order (assumes the game lists by slot; summons unverified); the bridge's per-enemy `combat_id` is the robust key. Discard/exhaust/draw sync ignore enchantments.
- Gate: a full act with no false `DIFFERS`.

**S1. Observation and fidelity.** Status: done. Look-ahead plays the next turns on a projected copy (`LOOK_H` = 4, joint encounter projection, pending node + stored follow-up in `enemy_moves`); cross-character cards in fuzzers. Observation v2 (calculated card numbers, affliction amounts, selection purpose, power secondary numbers, 64 candidates; E20) is used by the live player (gen-2); v1 networks keep working, bit-identical (gate). Gate: fuzz 0 residual mismatches; look-ahead probability test within tolerance.

**S2. Benchmark before training.** Status: done (`tools/bench.py`, near-miss bench). Frozen sets labelled by the live player (5x32 cover; labeller in `data/bench/labels.json`): eval, high energy, real-run corpus, cross-character/big belt, per-character. Local RTX 4070 Super: 100k labelled fights ~2.5 h (no pod needed). Gate: re-runs within se.

**S3. Predictor trained on search play.** Status: `models/solver_r5.pt` is player and predictor (E35). Open: underprediction on hard fights (bias -0.035 mix, -0.056 tail; decile gate fails); enabler-removal pairs blind.
- Combat loop (the engine of combat improvement): each round collects with the live player (its own cover search) on a signal-weighted pool, trains value with TD(0.8) from the live model, then applies the promotion gate; promote on pass. Policy stays fixed except via a new PPO base (no policy distillation target: E15, E22). Value targets are realized/TD outcomes, never max of search Q (winner's curse); HL-Gauss categorical targets; Reanalyse of stored fights.
- Curriculum by signal: each round draws fresh candidates (`tools/gen_curriculum.py` + corpus), scores at fight start with the current predictor, samples 15% uniform anchor + rest by p(1-p) (`tools/signal_pool.py`); selection before a seed is played (labels unbiased). Measured by A/B vs a uniform pool, calibration on the natural distribution. ExIt pool A/B (E18) inconclusive: run under the policy-target bottleneck; re-test for value-only training.
- Data hygiene after a simulator change: `tools/prune_divergent.py` drops fights that no longer replay (backup kept); a part losing > 1% is regenerated instead.
- Privileged inputs only if label noise proves the bottleneck.
- Open data source: snapshots of deck/relics/potions at each fight from the game's AutoSlay mode (god mode, random choices; `decomp/MegaCrit.Sts2.Core.AutoSlay*`) as realistic setups alongside the generated mix.
- Gate: calibration bias under search play < 0.02 in every decile; ranking no worse than the live predictor; live-width win not lower on any set; near-miss bench not worse.

**S4. Potion flow.** Status: done, gate pending (`agent/proposal.py`).
- Per turn, per potion, on the same futures and job seeds, other potions out of every arm: now (best target chosen on other futures) / keep (usable from next turn) / save (`without_potions`). Stop iff now beats keep and save by > 2 paired se in the fight's score, or the better of now/keep wins more than save by > 2 paired se. Live card search plans without potions. One potion per commit (`potion use`); `potion aside` keeps one for the boss.
- Per-fight objective via the live per-job `Worth` plumbing (`rl/fastsearch.py` `worth_row`, `search.rs` `Worth`): act boss followed by an ancient heal = win-only table (1% end-HP tiebreak), for fight-start prediction, `Engine.decide` and proposal play-outs; other fights linear.
- Open tests: self-damaging potions (Foul Potion: per potion id, boss fight with only it vs none; below "none" = misuse); potion timing before scheduled big hits (Vantom Dismember, Kaiser Laser, Byrdonis: training coverage of potion x big-hit pairs; value of use-now vs hold vs long-search truth two turns before).
- Gate: on the potion regression states (recorded run 20261005-201805: Vantom T1 Weak+Speed, Soul Nexus T4 Strength+Colorless), proposals agree with large-budget references.

**S5. Run model.** Status: implemented (`agent/runmodel.py`, `agent/price.py`, `agent/tracker.py`, `agent/events.py`); `price` live.
- Paired rollouts per option under a base policy with common random numbers: fights from the predictor's fight-start distribution (4-8 opening shuffles), rewards/shops/potions/unknown rooms at coded odds from the tracker's counters (`game_code.md` C), events from `data/events.json` (unknown stubbed). Current act on its real map; later acts from a template (room counts C1, known or sampled boss, ancient 80% heal + relic). V needs no terminal value.
- Base policy: path by rests-before-elites at low HP / elites at high HP; picks greedy on the predictor vs the act boss (plan cards when a plan is set); rest below 50% HP else smith the first unupgraded non-basic; shop removal first then best item within budget, else carry gold; potions: the whole belt is allowed (and spent) at every elite and boss.
- Encounter draws: weak pool first 3/2/2, bag per pool (`runmodel.draw_encounter`). Open: the tag rule (`AddWithoutRepeatingTags`; needs pool tags from decomp).
- Horizon ladder in `price`: significance 2 paired se; P(clear act) >= `ACT_SATURATED` 0.9 (`price --sat`) -> next-act readiness (`runmodel.readiness`: P(win) vs next act's boss pool 0.5 + elite pool 0.5 at HP after the ancient heal; Glory as pairs of distinct bosses), floors tiebreak.
- Cost: ~8k fight-start predictions/s; 4 options x 64 rollouts x ~25 predictions ≈ 10-20 s.
- Continuation value (priority; E30). P(win run) is flat for a genuine reason: a current deck's far gates (act-3 bosses, Knowledge Demon) are ~0 for the solver too, and full-run rollouts separate 1 of 266 known-ranking pairs at 512 per option. Potion-use variants of the base policy (none / bosses only, 8 states) do not lift the next act above 0.012; a smarter policy was not built. Surrogate = an array of proxy objectives combined as a product:
  - `runmodel.gates`: per remaining act, P(win) vs its elite pool and P(clear its boss gate) (A10 double boss as ordered pairs, second at the first's end HP), arrival HP chained through the ancient heal, belt at the current boss only.
  - `runmodel.combine`: `late` (default) = product with later acts' gates floored at `FLOOR` 0.05: passed gates (~1) and later gates the current deck cannot pass yet drop out, contested gates decide; `clip`, `prod`, `disc`, `mean` for comparison. Unfloored products are ruled by 1e-11-level act-3 tails (E30). `mean` scores best on every empirical test but is not a probability of clearing the run.
  - `price --cont late` (or `STS2_PRICE_CONT`): rollouts to the current act's boss, then V = its boss gate x every later gate on the deck at arrival; ranks by V and prints the closed-form gate table per option (0.1-0.5 s). Default ladder unchanged. `python -m agent.price <events.jsonl>` replays a recorded run's card rewards and rests through both.
  - Limits: the predictor misses engine decks (poison plan 0.03 vs solver 0.97; Accelerant invisible), and proxies score the current deck; plan-directed choices stay on the plan library's solver tables (S6). The run model is pessimistic against real play at fixed decks (act 1 cleared 0.15-0.56 from recorded starts vs 7 of 9 real runs).
  - Surrogate gate (to make `--cont late` the default): on recorded screens, its choice is never significantly worse than the current ladder's on the ladder's own horizon while unsaturated (P(clear act) within 2 paired se), separates the best on most screens, and is stable under fresh seeds; plus the predictor check on engine decks. Status: fails (4 of 93 recorded screens significantly worse on P(clear act); separates 81%; E30).
  - Per-fight worth tables U(ending) = V(state after) can use the same surrogate once it passes.
- Open: route-dependent options (Dowsing Rod, unknown/treasure/elite rewards) undervalued while the base policy routes the same: price (option, best route) pairs or deck-aware route preferences.
- Open (shop/draft): bundles within budget; saving gold vs small gains; speculative drafting is for plans (S6) and the operator.
- Gate: real runs fall inside the simulated distribution (act reached, HP at act boundaries); a price is stable under fresh draws (se reported).

**S6. Plan library.** Status: started (`data/plans.json`, `agent/plans.py`, `plans`; `plans check` validates entries). 16 entries, 2-4 per character (silent-poison measured, the rest proposed, unmeasured); with them the plans slice dry-runs at 1505 fights (plan 1260) vs 341 before.
- Entry: character, archetype, threats answered (bosses, elites, mechanics), core cards/relics, enablers/payoffs, substitutes, predictor-measured win of full and partial plans vs each threat at stated HP/act, status `proposed`/`measured`/`demoted` with sample sizes.
- Use: at each ancient and whenever the boss is far out of reach, pick the plan with best V = P(reach) x P(win | it); the run model prices picks/shops/routes by progress toward it via the partial-version table.
- Learning: operator proposes (first principles, source, `[expert]` runs); measured before use; demoted only by measurement, never one run.
- Self-improvement loop (to wire up; after roadmap item 3): (1) auto-measure every `proposed` entry with solver-labelled tables (`bench.py build-plans` machinery: full/core/minus-one vs each threat at stated HP/act; solver, not the predictor, until E31 passes) -> `measured`/`demoted`; (2) mine new entries from play: decks the solver wins hard fights with (act-2/3 bosses and elites in collections and real runs), cores found by remove-one-card ablation (enabler pairs, E31); (3) `improve review` checks each run's plan choices against the tables (gaps, not evidence).
- Gate: every plan used live is `measured`; plan choice logged with V and checked in review.

**S7. Operator and self-improvement.** Status: draft protocol below; becomes the operator skill once S5's surrogate gate passes (S3 is effectively passing). The per-screen protocol (`price` with the gate table) can start then; retiring the plan-related calculators (`pickplan`, plan-directed picks) also waits on a predictor check on engine decks, since "the predictor is the authority on fights" fails there (E30). Until then the current skills + calculators govern live play.
- Principles: one currency (P(win run); flat -> next horizon -> a measured plan; never unmeasured intuition). The predictor is the authority on fights: question it only for a reason the model cannot see, logged as a gap. Every disagreement or tail outcome is a typed gap (fidelity / calibration slice / missing model / tool) -> fix or experiment through its stage's gate.
- Per screen: Neow/ancient `plans` + `price` (unmodelled relic effects by judgment, logged); map `price` at every fork; card reward `price`, flat -> the plan's card else skip; shop `price` (nothing vs best affordable bundles of <= 3 purchases); full belt + potion offer `price`; rest `price` (rest vs each smith); event `data/events.json` + `price`, unmodelled -> judgment + gap; combat `combat`/`turn`, potions per S4.
- Skills shrink to: operator protocol, gap taxonomy, verified mechanics, plan reasoning with predictor-tested target decks. Decision guards replaced by prediction-vs-outcome logging. `improve review` becomes the gap review (predicted vs real fights, `price` decisions vs outcomes, plan choices vs tables, gap list).
- Open: auto-combat hands back to manual before damage lands (next enemy turn's predicted loss beyond the fight-start q90 pace, or predicted win drop > X); design on `runs/*/fights/` costly fights.
- Open solver-gap tests: early-stop on ties in manual fights (`agent.hindsight` large budget vs the plan in the why); missed lethal (states where a 1-2 card line kills the last enemy: solver's first choice should reach lethal ~100%).
- Open: harness refactor + bug sweep of `agent/` after the RL work.

**Expert claims backlog** (`data/expert/japaneseexport/`: 11 coaching videos, 170 [expert] claims with tests; transcripts/frames local, git-ignored). Claims are hypotheses until a test decides. Macro claims are validation cases for `price` (does it reproduce the expert's choice in his exact state? if not, which is wrong?), not rules to encode: the right choice should fall out of the model. Verdicts so far: unknown-room fight odds 10% +10%/non-fight unknown (supported `[code]` C5). Open, highest value (they contradict a current skill rule):
  - pre-boss smith at very low HP (Defect vs Lagavulin Matriarch at 13/75): conditional, not categorical: only while the boss stays winnable at that HP (harmless opening turns -> low marginal HP value). Rule = price it: smith iff P(win | HP now, smithed) >= P(win | rested HP); test on that state; ties to S5's rest-vs-smith misses (E30).
  - skip cards in 13-18-card decks once the plan works (9 videos): run-model rollouts of a skip-off-plan policy vs smooth greedy on P(clear 2 acts); the current-deck proxy bias (E30) pushes the other way.
  - Skulking Colony rewards block rather than front-loaded damage; removal value (his videos split 3/3); ancients: immediate power over long-term access.
  - 30 reconstructable fight/screen states: compare his line with the live solver's on the same state.
**Expert data.** Pipeline: roadmap item 7. NaveGreed pilot (`data/expert/navegreed_2026-10-07.md`): 41 macro decisions + 3 fights transcribed from one A10 Ironclad win.

## 6. Retired / live / kept
- Retired: near-miss restarts (`exit.py collect --restarts`; E25/E26: little honest flip, errors spread over setup turns); potion `search_keep`, alert thresholds, keep/allow/deny/hold; the HP-worth util curve (failed its gate: curve vs linear E[U] -0.000 +- 0.002); Gumbel root (E16); value ensembles; policy distillation targets (E15, E22); duplicate route pricers and model gates.
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
| `agent/` | bridge, screen, fight, engine, live, proposal, tracker, runmodel, price, events, plans, harness + `__main__`, reenact, runlog, improve + hindsight (review), fidelity_sweep (only fidelity tool), skillgate | guards, macro, routes, pickplan, potions, card_tags once price/plans replace them |
| `tools/` | gate.sh, bench, bench_search, nearmiss_bench, nearmiss, signal_pool, round_pool, gen_curriculum, gen_train, fuzz_gen_mix, prune_divergent, collect.sh, headroom, dashboard, expert | one-off A/B scripts (results go to `evidence.md`); next burn-off (once the running jobs finish): tools/policy_agree.py, tools/target_noise.py (policy study closed); `exit.py collect --restarts` + `policy_only` parts and the restart writer in tools/nearmiss.py (restarts retired) |
| `.claude/skills/` | `sts2` (rules, information contract), operator protocol + gap logging, plan-library use, verified mechanics | procedures that only feed the old calculators (buckets, smooth score, section-3 bar, guard fields) |
| `data/` | catalog, pools, ancients, events, relic classes, `bench/`, `plans.json`, train/eval sets | |
