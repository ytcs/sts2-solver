---
name: sts2-harness
description: Use before the first action of a run and whenever unsure how to drive the game: python -m agent commands, action chaining, batch mode, solver (adv/turn/combat), eval syntax, speed targets, harness quirks, post-run review loop.
---

# Harness operation

Run as `.venv/Scripts/python.exe -m agent <cmd>` with `STS2_DEVICE=cuda` (`agent/harness.py`, `README.md`). Quote `-- why` in double quotes (parentheses and semicolons break the shell); batch mode needs none.

## Commands
- **Look (always allowed):** `s` state; `d` deck/relics; `p draw|discard|exhaust`; `m` map (`boss: <row> <ID> [+ <ID>]`); `relics` counters; `status` (run, fight, replay fidelity, engine); `brief`, `reward`: `sts2-deckbuilding` section 1.
- **Act:** `a <i> [target] [-- why]` (always give the reason). Chain with `;`, options by label: `a ~gold; ~card; ~skip; ~proceed -- why`. `~text` matches option text case-insensitively (avoid words in card names, e.g. `end`). A chain stops on error or combat, a map click must be last, and it passes through a selection screen when the next step names its option (`a ~smith; ~Bash`).
- **Route:** `draw r1c6 ...` draws the route; `route M E R ... --hp N` prices fights and rests along one hand-written route; `note`, `newrun`.
- **Whole map:** `routes [--attempts N] [--pf P] [--w E=4,M=1]` prices every route on the act map (exact DP over node x HP with the solver's fight outcomes, adaptive): per option on offer the boss win (or survival to the boss when the boss is out of reach) with at least k more elites, the representative route per k, and a reward-weighted ranking (weights are a judgment `[hyp]`: elite 5 = relic + ~3x rare odds, treasure 3.5; unknowns are a 15% regular fight; events and treasure are not simulated). A shop is worth what the gold I arrive with buys: the reward DP carries my gold (A10 income `[code]`: monster ~11, elite ~30, treasure ~35) and prices each shop as the best basket of a typical A10 shop at code prices (card ~60 with the sale, removal 100 +50 per use, relic ~225): 0 under ~60 gold, ~1 card-reward at 100, a relic at 225 (`routes.py` `shop_buy`). Potions are a budget in the route DP: each potion is thrown at most once along a route, at the elite or boss fight where it helps most (the DP chooses with the HP I arrive with, the boss keeps what is left; hallway fights are priced without potions), and every representative route prints its `potion plan`. `rmcalc [--attempts N]` prices every removable card as a removal (boss smooth, elites, next act), ranked.
- **Solver:** `adv [secs]` advice plus expected enemy damage (`adv 20` when one turn is pivotal); `turn` / `combat` play a turn / the fight, only in an AUTO fight (`DRIVE:` line at fight start; `combat !` overrides a MANUAL one; rule: `sts2-strategy`). Every `adv` is logged so the review can compare my choice with the solver's. `budget <s>` fixes search time (default auto, 1-15 s, stops when expected regret < ~1 HP). `SIMULATOR DESYNC` or `DIFFERS` voids the advice: play by hand, run `status`.
- **Potions are my call, at a gate.** The solver is unchanged and may propose a potion as often as it likes; `combat` / `turn` STOP at every proposal and print what it saves: the search value of the best line with potions minus with none (value = +1 win / -1 loss + 0.5 x HP fraction left: 0.1 is about +5% win or +16 HP), what using it now adds over the best non-potion action, and a TIE flag (the solver picks a potion on a tie, +0.00). Answer with `combat ok` (throw this one), `combat skip` (decline this one: the best non-potion action is played), `combat go` (decline every proposal this fight: the search then plans without potions, so its line is the one I play; `skip` keeps them in the search). Decide with the macro context: the boss pool, the next elites, shops, potions left. `potions` shows what each potion would add right now. `hold ID` (by hand) still takes a potion off the table. Every table (`eval`, `reward`, `rmcalc`, `routes`, `pickplan`) prices NON-boss fights without potions (a lower bound: I throw one only when it is worth it) and the boss with them.
- **Eval:** `eval --pool <Act>:<regular|elite|boss> --hp full --attempts 256 --v "name|add=ID|remove=ID|upgrade=ID|relics_add=ID"` is the combat value of deck variants (ids upper-case snake: HEMOKINESIS). `--boss`, `--elites`, `--next` replace `--pool` (known boss; elites left; next act's elites and bosses; narrowing: `sts2-acts`); `--all` whole pool; `--future` all horizons; `--smooth` averages win rate over start HP x1/1.5/2/3 (4x the cost; the pick objective).

## Batch mode `[code]`
`python -m agent - <<'EOF'` with one command per line runs them in order, output under `>>> command`; text is literal (no quoting). It stops at the first `ERR` / `REFUSED` / `[chain stopped` (`--keep-going` after `-` continues). E.g. `brief` + `reward` in one call, then the pick.

## Cost of the calculators `[played]` (warm daemon)
`reward` ~6 s, `routes` ~5 s (0.5 s again at the same deck: the tables are cached by deck, relics, belt and encounters left), `rmcalc` ~8 s, 1-variant `eval` ~3 s, `pickplan` ~27 s (1.3 s again: gains cached; a cheap screen of all cards, then the top 40 priced properly). A cold daemon costs ~35 s (networks and CUDA): do not `quit` it unless code changed. Run `pickplan` once per act and after a shop, `routes` at every fork.

## Speed `[played]`
Target a run in 30 min, fights 1-2 min (`combat` takes 5-40 s). `combat` for easy fights, `turn` / `adv` when the stakes are real; short reasons; no re-reading unchanged state; a reward screen in one chained `a`.

## Quirks `[played]`
- `REFUSED: skills not loaded: X`: invoke X, read it, repeat (`CLAUDE.md`, Rule 0).
- Never chain map or node choices (a chained click entered an elite at 37/80). A click onto an elite or boss below 60% HP needs `!`.
- Option numbers shift after every action. The harness refuses a bare `a <i>` after an earlier step in the same chain or batch, and `~text` prefers the one option that starts with the text and refuses when several still match: name options (`~gold`), or read `s` and send the number in its own call.
- Crystal Sphere (event minigame, 121 cells): the bridge's generic overlay fallback lists only 40 controls, so the cells hide Proceed. `a 0 <x> <y>` / tool / proceed come from the `CrystalSphere` case in `mods/AgentBridge/src/Decisions.cs` (installed 2026-10-05; untested live). Never use the console. Play: `sts2-crystal-sphere`. `[code]`
- `hold` is saved with the run record and survives a daemon restart (cleared on a new run); `status` lists what is held. Set it as soon as the boss potion is in the belt.
- The solver never discards a potion (excluded in `engine.decide`; the bridge cannot).
- The fight-start prediction is made at the HP before Pantograph's heal: judge a boss with `eval --hp <HP on entry>`.
- If `combat` returns without playing, run `s`; an `ERR` line says why.
- Not synced from the game `[code]`: relic counters and props (Joss Paper, Iron Club, Pael's Legion after Whispering Earring's hidden first hand); a sim-created card has none of the real card's keywords until played. Sync, fidelity, calibration results: `sts2-deckbuilding/evidence.md`. Energy-heavy evals need no extra discount `[sim]`.

## Distributions, not means `[code]`
`eval` prints the HP-lost quantiles (q10 / 50 / 90 / 97.5, a loss counts as the whole start HP); `routes` propagates the whole HP distribution; the review prints, per fight, where the real loss fell in the predicted distribution (`loss pct`, 0.50 if calibrated) and the share of fights in the worst 10% (expected 10%): the check that the simulator and the game agree. Decide on q90 and the death tail, not the mean: the spread of one elite is ~30 HP.

## Review loop (after each run)
1. `python -m agent.improve review` (writes `runs/<run>/review.md`): outcome; predicted vs actual per fight (Brier, HP); search overrides of the policy; fidelity divergences; card picks vs the best smooth boss score; every `override:`; costly fights in `runs/<run>/fights/`.
2. **Fidelity first**: reproduce a divergence (`python -m agent.fidelity_sweep --mode recorded`, `agent.fidelity_trace`, `agent.calibrate`) and fix it before any model comparison.
3. **Costly fights** (lost >= 30% max HP, or lost): `python -m agent.hindsight <file> --log` says luck (percentile of the real loss among simulator replays) or solver gap (decisions where a large-budget search prefers another line by >= 2 HP). Gap patterns go to the corpus / fine-tune; encounter patterns to `sts2-acts/encounters.md`.
4. **Overrides**: did each hold in the fights that followed? Edit the blind-spot list in `sts2-deckbuilding` (narrow, tighten the test, delete) and tag the claim `[played]` or `[hyp]`; tally in `evals/overrides.jsonl`. `python -m agent.improve lessons` lists every open `[hyp]`: the experiment backlog.
5. **Model changes** only when 2-3 runs show a pattern: `corpus` -> `finetune` -> `gate` (candidate gains >= 1 point on the corpus holdout and loses <= 1 on the fixed eval and the 4-7-energy eval) -> `adopt`; every step in `evals/ledger.jsonl`. Strategy changes go through the book, model changes through the gate; neither on a single run.
