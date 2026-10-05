---
name: sts2-harness
description: Use before the first action of a run and whenever unsure how to drive the game: python -m agent commands, action chaining, batch mode, solver (adv/turn/combat), eval syntax, speed targets, harness quirks, post-run review loop.
---

# Harness operation

Run as `.venv/Scripts/python.exe -m agent <cmd>` with `STS2_DEVICE=cuda` (`agent/harness.py`, `README.md`). Quote `-- why` in double quotes (parentheses and semicolons break the shell); batch mode needs none.

## Commands
- **Look (always allowed):** `s` state; `d` deck/relics; `p draw|discard|exhaust`; `m` map (`boss: <row> <ID> [+ <ID>]`); `relics` counters; `status` (run, fight, replay fidelity, engine); `brief`, `reward`: `sts2-deckbuilding` section 1.
- **Act:** `a <i> [target] [-- why]` (always give the reason). Chain with `;`, options by label: `a ~gold; ~card; ~skip; ~proceed -- why`. `~text` matches option text case-insensitively (avoid words in card names, e.g. `end`). A chain stops on error or combat, a map click must be last, and it passes through a selection screen when the next step names its option (`a ~smith; ~Bash`).
- **Route:** `draw r1c6 ...` draws the route; `route M E R ... --hp N` prices fights and rests; `note`, `newrun`.
- **Solver:** `adv [secs]` advice plus expected enemy damage (`adv 20` when one turn is pivotal); `turn` / `combat` play a turn / the fight. `budget <s>` fixes search time (default auto, 1-15 s, stops when expected regret < ~1 HP). `SIMULATOR DESYNC` or `DIFFERS` voids the advice: play by hand, run `status`.
- **Potions:** `hold POWDERED_DEMISE[,ID]` keeps those potions from the solver (for the boss; `hold none` releases; set again after a daemon restart). Automatic `[code]`: at fight start the harness keeps all potions when the prediction without them is comfortable (win - se >= 0.95 and expected HP loss <= 40% of current HP; `agent/harness.py` `_fight_start`, `comfortable`).
- **Eval:** `eval --pool <Act>:<regular|elite|boss> --hp full --attempts 256 --v "name|add=ID|remove=ID|upgrade=ID|relics_add=ID"` is the combat value of deck variants (ids upper-case snake: HEMOKINESIS). `--boss`, `--elites`, `--next` replace `--pool` (known boss; elites left; next act's elites and bosses; narrowing: `sts2-acts`); `--all` whole pool; `--future` all horizons; `--smooth` averages win rate over start HP x1/1.5/2/3 (4x the cost; the pick objective).

## Batch mode `[code]`
`python -m agent - <<'EOF'` with one command per line runs them in order, output under `>>> command`; text is literal (no quoting). It stops at the first `ERR` / `REFUSED` / `[chain stopped` (`--keep-going` after `-` continues). E.g. `brief` + `reward` in one call, then the pick.

## Speed `[played]`
Target a run in 30 min, fights 1-2 min (`combat` takes 5-40 s). `combat` for easy fights, `turn` / `adv` when the stakes are real; short reasons; no re-reading unchanged state; a reward screen in one chained `a`.

## Quirks `[played]`
- `REFUSED: skills not loaded: X`: invoke X, read it, repeat (`CLAUDE.md`, Rule 0).
- Never chain map or node choices (a chained click entered an elite at 37/80). A click onto an elite or boss below 60% HP needs `!`.
- After a failed chain step the earlier screen stays open: re-read `s` before a bare `a <i>` (a blind `a 0` took the wrong card).
- The solver never discards a potion (excluded in `engine.decide`; the bridge cannot).
- The fight-start prediction is made at the HP before Pantograph's heal: judge a boss with `eval --hp <HP on entry>`.
- If `combat` returns without playing, run `s`; an `ERR` line says why.
- Not synced from the game `[code]`: relic counters and props (Joss Paper, Iron Club, Pael's Legion after Whispering Earring's hidden first hand); a sim-created card has none of the real card's keywords until played. Sync, fidelity, calibration results: `sts2-deckbuilding/evidence.md`. Energy-heavy evals need no extra discount `[sim]`.

## Review loop (after each run)
1. `python -m agent.improve review` (writes `runs/<run>/review.md`): outcome; predicted vs actual per fight (Brier, HP); search overrides of the policy; fidelity divergences; card picks vs the best smooth boss score; every `override:`; costly fights in `runs/<run>/fights/`.
2. **Fidelity first**: reproduce a divergence (`python -m agent.fidelity_sweep --mode recorded`, `agent.fidelity_trace`, `agent.calibrate`) and fix it before any model comparison.
3. **Costly fights** (lost >= 30% max HP, or lost): `python -m agent.hindsight <file> --log` says luck (percentile of the real loss among simulator replays) or solver gap (decisions where a large-budget search prefers another line by >= 2 HP). Gap patterns go to the corpus / fine-tune; encounter patterns to `sts2-acts/encounters.md`.
4. **Overrides**: did each hold in the fights that followed? Edit the blind-spot list in `sts2-deckbuilding` (narrow, tighten the test, delete) and tag the claim `[played]` or `[hyp]`; tally in `evals/overrides.jsonl`. `python -m agent.improve lessons` lists every open `[hyp]`: the experiment backlog.
5. **Model changes** only when 2-3 runs show a pattern: `corpus` -> `finetune` -> `gate` (candidate gains >= 1 point on the corpus holdout and loses <= 1 on the fixed eval and the 4-7-energy eval) -> `adopt`; every step in `evals/ledger.jsonl`. Strategy changes go through the book, model changes through the gate; neither on a single run.
