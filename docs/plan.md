# Plan
Single plan, current state, forward only. Evidence `docs/research/evidence.md` (E#: history, numbers, superseded threads); game facts `docs/research/game_code.md`. Each stage: goal / status / gate; a stage failing its gate is not adopted. Simulator/search/env changes pass `bash tools/gate.sh` unchanged (a deliberate behaviour change updates its checksum in the same commit).

## 1. Goal and principles
- Goal: win A10 runs with all five characters (Ironclad first); ~90%+ target (user); end HP breaks ties.
- Information contract: only the exact random state is hidden; public and tracked: move patterns + odds, potion-drop chance, rarity offset, unknown-room odds, encounter-bag narrowing, shop prices, removal cost.
- Claims: solver results are lower bounds under the stated solver; < 2 se is a tie; expert choices are evidence, never rules; no blanket bans.
- Venue: the headless server (`OracleCombat serve`) counts once mechanically indistinguishable from the game (E44); Steam copy only for live-parity work.

## 2. Live state (2026-10-10)
- Combat: player and predictor `models/solver_r9.pt` (E68); cover search + exact turn + hp_cap; potions thrown by the search at reserve cost 0.06 with the 2-se margin gate (E61, E69); 16 rounds batched in one `decide_many` (0.1-0.3 s/decision). More search does not help the losing fights (E74: Queen 64 vs 16 rounds, end HP -1.0 +- 3.8).
- Macro (scripted baseline + live `price`): one `price.decide` for both (run-model rollouts, search check of the top options with the belt each option leaves, hybrid override beyond 2 se, macro-terms DSL `data/macro_rules.json` + `agent/terms.py`). No arm wins: control / new2 / E71 / E73 all 0/20, mean death floor 31-36 (E70, E71, E73).
- What the evidence says binds (E64-E74): (1) final-pair deck strength 0.14-0.28 at full HP in every arm; drafting errors are specific picks, not raw size (E74: removing the three weakest cards +0.188 +- 0.073 win vs the Queen, three random cards +0.06-0.08); (2) HP / route discipline at HP-gated bosses (Knowledge Demon 0.31 at 38 HP vs 0.71 at 59; script took fights where `routes` said rest); (3) the final pair's HP/potion budget (both operator branches beat the Queen and lost Test Subject at 10-30 HP).
- Validated instrument: deck strength (search vs the act's bosses at full HP, no potions) predicts replayed live boss wins (Spearman 0.73, n 28; E70). Run outcome at n 20 cannot resolve macro arms; deck strength per act + act reach can.
- Instruments: `tools/baseline.py` (arms; rows carry deck strength, bottlenecks, active terms), `tools/ab.py` (paired arm comparison), `tools/branch.py` (replay a run to a floor for operator diagnosis), `tools/engine_ab.py`, `tools/fork_check.py` (price vs routes at recorded forks), near-miss / predictor benches and the promotion gate (`tools/nearmiss_bench.py`, `tools/bench.py`; gate: play no set worse, near-miss not worse, Brier no worse, decile bias < 0.02, sign gate for obs v3), pods (`scripts/pod_job.sh`, 2 processes per 32 GB GPU).

## 3. Stages, in order

**A. Unified macro pricing (the framework; advisor + user 2026-10-10).**
- Objective: P(win run) = product over remaining acts of P(clear act_i). Every macro decision (pick, skip, removal, upgrade, shop, route, rest, event, potion buy) is priced by its change to that product.
- Estimator per act: search deck strength vs that act's boss(es) at arrival HP: the known boss(es) once the map shows them, else the pool (act 3 from act 1 on: its boss + elite pools); smooth score (HP multiples) where wins floor at 0. Screens use search (adaptive `macro.evaluate_many`); rollouts keep the predictor (cost) and get only the structural rules below. State the split; screens and rollouts may disagree.
- Combination: d log P(win run) = sum_i dP_i / P_i (P_i floored ~0.05): the binding act gets the weight (act 1 with a starter deck: front damage; act 1 safe: the act-3 term decides, ties break toward what scales).
- Skip / deadline (user: a later floor knows more, an early draft carries more regret): take a card now only if a fight before the next comparable offer needs it (next elite/boss under ~0.9 or a large HP cost), or its long-horizon gain beats the expected later offer; rare core pieces unlikely to recur are the exception. `BasePolicy` (rollouts) follows the same take/skip rule so `price`'s skip row prices the option to wait.
- Removal: dominance first (a basic dominated by a stronger copy goes first: Strike before Headbutt), then search on the same horizon ranks the rest (E74: Headbutt/Breakthrough over a Defend); `removal_first` stops meaning "a basic".
- Residual DSL terms only where search is structurally blind: max HP as a whole-run resource (`max_hp_value`), gold economy (removal price escalation, shop access), potion slots. The proxy terms (density, skip_margin, gap_fill, consistency, bundle_bonus, stranding_*, anti_veto) are deleted when this lands (E73: additive proxies grew decks +2.65 +- 0.80 cards with no strength; anti_veto would have skipped Battle Trance, measured +0.15).
- Forks and rests are the same decision at arrival HP: if `tools/fork_check.py` shows `price` systematically misprices act-2 forks/rests where `routes` was right, extend the hybrid override to MAP and RESTSITE (routes tables, one cached batch).
- Status (2026-10-10, built): `price.search_check` = Bellman log-ratio over the factors of P(win run): near = radius ladder (3/6/10/act end) of reachable known pools, the 2 most dangerous fights where the skip deck is unsafe, at current HP; far = the act-2 / act-3 bosses (mandatory only; elites optional), a floored rung at 2x max HP (interim, user: ad hoc; target = projected-deck Bellman leaves: rollouts project the deck to each future boss, search plays the leaf; prototype right in kind but se 0.04-0.09 at 16 rollouts, ~25k fights/screen for 0.02: needs variance reduction or per-act use). Deadline skip rule at card rewards on that score (2 se, or <= 4 floors before an unsafe boss when positive); rollouts use `safe_now`; removal candidates dominance-filtered; proxy terms deleted (rollout decks smaller: 23 vs 28 cards at the act-2 boss). ~15-25 s per screen on a 5090. Gate `tools/decision_gate.py` 11/11 (frozen; the first 8 items shaped the estimator, so the arm is the independent test).
- Gate: offline replay of the ~20 measured decisions from the operator branches and scripted probes (E72, E74: take Demon Form, Battle Trance, Heart of Iron, Crossbow; skip Vicious, Omnislice, the third Tablet page; remove Headbutt/Breakthrough before a Defend) passes; then one 20-seed arm judged by deck strength per act (validated) and act-2/act-3 reach vs control / E71.

**B. Fight objective prices the run-state delta (user: micro still weak).** Terminal value = win + end HP + the run-state change valued by the stage-A estimator: stolen/added cards (Thieving Hopper escaping with Battle Trance+), curses, max HP, potions spent, relic counters (Pen Nib), kill bonuses (Feed). The search then trades HP against them itself. Gate: near-miss not worse; Hopper/Pen Nib bench states resolved.

**C. Final-pair budget.** First boss of the pair under a pair-aware objective (HP and potions carried into boss 2 valued by boss-2 P(win)), potion reserve before boss 1 (E67: Test Subject at 25 HP 0.24 -> 0.45 with two kept). Gate: pair P(win) on recorded pair starts (E72, E74) up, paired.

**D. Combat model rounds** (r9 live; same-recipe rounds flatten, E37). Next round collects with r9 on decks the new macro produces; obs v3 sign gate still failing (Accelerant, doom). Gate: the promotion gate above.

**Hygiene and fidelity** (goal: the live sim never diverges silently; gate: a full act with no false `DIFFERS`):
- Simulator: Battleworn Dummy turn limit (BattleFriend, RanOutOfTime after 3 turns) unmodelled: eval says 1.00, live search plays slow and fails (E74).
- Bridge/server: FakeMerchant ("The Merchant???", act 2+, gold >= 100) is a custom-layout shop rendered as "Placeholder" + Proceed: expose its inventory as shop options.
- Open sync items: re-sync hand before a mid-card selection (Survivor, Dagger Throw); card text on the rewards screen; same-id enemies pair in list order (use `combat_id`); discard/exhaust/draw sync ignore enchantments; Happy Flower counter; Kaiser Crab Surrounded facing; Rocket +99 block / +6 Strength on Crusher death; a synced state cannot set a monster's next move; hidden per-card state (Thrash bonus) re-rolled until piles match (6555b08).

**Deferred** (user: revisit after runs win): persistent bridge socket (reply framing in the mod + server; ~2% of a run); bf16-clean model (autocast re-casts ~18% of GPU time; fp32 matmuls outside autocast block it), uint8 observation upload (~13%), TensorRT/FP8 on Linux pods; expert macro extraction (`tools/expert.py`, `expert-reenact`; not on the critical path); Toy Box melt schedule in `price`; search-budget stopping rule (E56, small).

## 4. Target layout
| path | keeps | goes |
|---|---|---|
| `crates/` | sts2sim, sts2env, sts2py, sts2diff | |
| `rl/` | model, heads, fastsearch, solver, exit, ppo, predictor | |
| `agent/` | bridge, screen, args, fight, engine, live, potions, proposal, tracker, runmodel, runctx, pools, price (one `decide` over the stage-A estimator), events, terms, synergy, bottleneck, crystal_sphere, harness + `__main__`, reenact, runlog, improve + hindsight, fidelity_sweep, skillgate | macro, routes, pickplan, card_tags, guards and the calculators `reward` / `eval` / `rmcalc` / `pickplan` / `brief` buckets once stage A's `decide` covers them (their search pieces move into `price`); plans (superseded by synergy bundles) |
| `tools/` | gate.sh, bench, bench_search, nearmiss_bench, nearmiss, exact_turn_check, signal_pool, round_pool, gen_curriculum, gen_train, gen_relic_mask, fuzz_gen_mix, prune_divergent, collect.sh, headroom, dashboard, expert, baseline, ab, branch, engine_ab, fork_check, postmortem, synergy | one-off A/B scripts |
| `scripts/` | pod_*.sh, skill_gate.py, porting/ | |
| `.claude/skills/` | `sts2`, operator protocol + gap logging, mechanics, `expert-reenact` | procedures feeding only the retired calculators |
| `data/` | catalog, pools, ancients, events, relic classes, `bench/`, `macro_rules.json` (residual terms), `synergy_candidates.json`, train/eval sets, `expert/` | `plans.json` once synergy bundles carry its measurements |
