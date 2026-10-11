# Plan
Single plan, current state, forward only. Evidence `docs/research/evidence.md` (E#); game facts `docs/research/game_code.md`. Each stage: goal / status / gate; a stage failing its gate is not adopted. Simulator/search/env changes pass `bash tools/gate.sh` unchanged (a deliberate behaviour change updates its checksum in the same commit).

## 1. Goal and principles
- Goal: win A10 runs with all five characters (Ironclad first); ~90%+ target (user); end HP breaks ties.
- Information contract: only the exact random state is hidden; public and tracked: move patterns + odds, potion-drop chance, rarity offset, unknown-room odds, encounter-bag narrowing, shop prices, removal cost.
- Claims: solver results are lower bounds under the stated solver; < 2 se is a tie; expert choices are evidence, never rules; no blanket bans.
- Validation (user): estimator and macro changes are judged by paired arms on the same seeds (act reach, deck strength per act, wins); the frozen decision gate (`tools/decision_gate.py`, single-run items) only flags large regressions.
- Venue: the headless server (`OracleCombat serve`) counts (mechanically indistinguishable from the game, E44); Steam copy only for live-parity work.

## 2. Live state (2026-10-11)
- Combat: player and predictor `models/solver_r9.pt` (E68); cover search + exact turn + hp_cap; potions thrown by the search at reserve cost 0.06 behind the 2-se margin gate (E61, E69); 16 rounds batched in one `decide_many` (0.1-0.3 s/decision). More search does not help the losing fights (E74).
- Macro: one `price.decide` for the scripted baseline and live `price`: stage-A search check on card rewards / shops / events, run-model rollouts elsewhere; map / rest overridden by `routes` beyond `ROUTE_OVERRIDE` 0.05 (E78). Per-arm env tunables: `ROUTE_OVERRIDE`, `TAKE_SE` 2, `SAFE_WIN` 0.9, `DEADLINE_RADIUS` 3.
- Results: one win in 20 (E77 BASE0003); E78 0/20 (act-2 reach 0.50); E80 0/40 (deaths: final pair 13, act-2 boss 9, act-2 6, act-3 elites 6, act-1 boss 5); E81 ties E80.
- What binds (E64-E83): (1) final-pair deck strength 0.14-0.28 at full HP in every arm; drafting errors are specific picks (E74); (2) HP / route discipline at HP-gated bosses (E74, E76, E78); (3) the final pair's HP / potion budget: potions held (+0.13 to +0.27) and arrival HP move it more than card choices among the offers (E83); act-3 fights drain HP faster than rests restore it.
- Validated instrument: deck strength (search vs the act's bosses, full HP, no potions) predicts replayed live boss wins (Spearman 0.73, n 28; E70). n 20 cannot resolve macro arms by run outcome; deck strength per act + act reach can.
- Instruments: section 4 `tools/` row; promotion gate = play no set worse, near-miss not worse, Brier no worse, decile bias < 0.02, obs-v3 sign gate (`tools/power_coverage.py gate`); pods `scripts/pod_job.sh` (2 processes per 32 GB GPU).

## 3. Stages, in order

**Next (before any new arm).** Goal: a first-principles review after the burn-off: game mechanics, Bellman / stochastic planning / RL theory, expert commentary (incl. Japanese Export's Spirecast and Spire Clinic). Status: not started. Gate: a written lever ranking that the next arm tests.

**A. Unified macro pricing.**
- Goal: P(win run) = product over remaining acts of P(clear act_i); every macro decision (pick, skip, removal, upgrade, shop, route, rest, event, potion buy) priced by its change. Estimator per act: search deck strength vs that act's boss(es) at arrival HP (known boss once shown, else the pool; act 3 = boss + elite pools); smooth score where wins floor. Combination d log P = sum dP_i / P_i (P_i floored ~0.05). Skip / deadline: take now only if a fight before the next comparable offer needs it, or its long-horizon gain beats the expected later offer; `BasePolicy` follows the same rule. Removal: dominance first, then search ranks the rest. Residual DSL terms (`data/macro_rules.json`) only where search is blind: max HP, gold economy, potion slots. Forks and rests: extend the hybrid override to MAP / RESTSITE if `tools/fork_check.py` shows `price` mispricing them.
- Status: built (E75 defects fixed: take/skip on search's best card, each near binding fight its own factor, urgency within `DEADLINE_RADIUS`; floored future rungs at the smallest informative HP multiple, progress when still floored). Decision gate 11/11. E80 0/40, E81 ties. ~20-25 s per screen. Target: projected-deck Bellman leaves once variance allows.
- Gate: decision gate passes; one 20-seed arm judged by deck strength per act and act-2/3 reach vs control.

**B. Fight objective prices the run-state delta.**
- Goal: terminal value = win + end HP + run-state change valued by the stage-A estimator (stolen cards, curses, max HP, potions spent, relic counters, kill bonuses).
- Status: line-end costs built with interim constants (pot_cost, STOLEN_COST 0.1, GOLD_COST 0.0005/gold, MAXHP_COST 0.008/pt; checksums unchanged at default 0); `Combat.cards_escaped` keeps escaped cards in the cost (E79). Next: price them by the macro estimator.
- Gate: near-miss not worse; Hopper / Merc / Scrolls / Pen Nib bench states resolved.

**C. Final-pair budget.**
- Goal: boss 1 of the pair under a pair-aware objective (HP and potions carried into boss 2 valued by boss-2 P(win)); potion reserve before boss 1 (E67).
- Status: pair worth built, applied only when the curve is informative (peak >= 0.3), else linear (E74's Queen start was floored).
- Gate: pair P(win) on recorded pair starts (E72, E74) up, paired.

**D. Combat model rounds.**
- Goal: stronger player / predictor.
- Status: r9 live; same-recipe rounds flatten (E37). Next round collects with r9 on decks the new macro produces and sets MAX_POTIONS 9 (user; observation layout grows). Obs-v3 sign gate still failing (Accelerant, doom).
- Gate: the promotion gate.

**Hygiene and fidelity.**
- Goal: the live sim never diverges silently. Gate: a full act with no false `DIFFERS`.
- Status: done: Battleworn Dummy turn limit (`Combat.ran_out_of_time` = loss), event pages' escalating costs (`price._stated`), FakeMerchant renders as a SHOP, potions clamped to 8 slots in scenarios (a 9th is dropped until MAX_POTIONS 9). Open sync items: re-sync hand before a mid-card selection (Survivor, Dagger Throw); card text on the rewards screen; same-id enemies pair in list order (use `combat_id`); discard/exhaust/draw sync ignore enchantments; Happy Flower counter; Kaiser Crab Surrounded facing; Rocket +99 block / +6 Strength on Crusher death; a synced state cannot set a monster's next move; hidden per-card state (Thrash bonus) re-rolled until piles match.

**Deferred** (user: revisit after runs win): persistent bridge socket (~2% of a run); bf16-clean model (~18% of GPU time), uint8 observation upload (~13%), TensorRT/FP8 on Linux pods; expert macro extraction (`tools/expert.py`, `expert-reenact`); Toy Box melt schedule in `price`; search-budget stopping rule (E56); post-burn-off performance pass (profile, port hot loops to Rust bit-identically).

## 4. Target layout
| path | keeps | goes |
|---|---|---|
| `crates/` | sts2sim, sts2env, sts2py, sts2diff | |
| `rl/` | model, heads, fastsearch, solver, exit, predictor | |
| `agent/` | bridge, screen, args, fight, engine, live, potions, proposal, tracker, runmodel, runctx, pools, price (one `decide` over the stage-A estimator), events, terms, synergy, bottleneck, crystal_sphere, harness + `__main__`, reenact, runlog, improve + hindsight, fidelity_sweep, skillgate | macro, routes, guards, card_tags, plans once their users move. Still load-bearing: macro (`narrow`, card ids, `parse_card_options`, `evaluate` behind `eval` / `restcalc`, `brief`), routes (`routes` command, `ROUTE_OVERRIDE`, fork_check, `price.run_state` map parsing), guards (operator protocol), card_tags (`brief` buckets, bench), plans (bench "plans" set) |
| `tools/` | gate.sh, bench, bench_search, nearmiss_bench, nearmiss, exact_turn_check, headroom, signal_pool, round_pool, gen_curriculum, gen_train, gen_relic_mask, gen_card_powers, fuzz_gen_mix, prune_divergent, collect.sh, power_coverage (`gate`), baseline, ab, branch, engine_ab, fork_check, postmortem, oracle, recount, decision_gate, expert, expert_macro, synergy, dashboard | serve_harness_run, serve_replay_check once baseline no longer imports `macro_choice`; one-off study scripts after their E# lands |
| `scripts/` | pod_round.sh, pod_job.sh, pod_setup.sh, skill_gate.py, porting/ | |
| `.claude/skills/` | `sts2` book, harness / operator protocol, mechanics, `expert-reenact`, `burn-off` | rules feeding only retired code |
| `data/` | catalog, pools, ancients, events, relic classes, `bench/`, `macro_rules.json` (residual terms), `synergy_candidates.json`, train/eval sets, `expert/` | `plans.json` once synergy bundles carry its measurements |
