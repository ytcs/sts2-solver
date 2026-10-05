---
name: sts2-harness
description: Use before the first action of a run and whenever unsure how to drive the game: python -m agent commands, action chaining, solver (adv/turn/combat), eval syntax and pool narrowing, speed targets, harness quirks, post-run review loop.
---

# Harness operation

Run as `.venv/Scripts/python.exe -m agent <cmd>` with `STS2_DEVICE=cuda`; details in `agent/harness.py` and `README.md`. Quote `-- why` text in double quotes (parentheses and semicolons break the shell).

## Commands
`s` state; `a <i> [target] [-- why]` act (always give the reason); chain with `;` and pick options by label: `a ~gold; ~card; ~skip; ~proceed -- why` (stops on error or combat; a map click must be last; `~text` matches option text, so avoid words that occur in card names such as `end`). `d` deck/relics, `p draw|discard|exhaust`, `m` map, `draw r1c6 ...` route on the map, `relics` counters.
`adv [secs]` advice plus the enemies' expected damage; `turn` / `combat` let the solver play a turn / the fight. `budget <s>` fixes the search time; default auto (1-15 s from the fight's predicted danger; stops early when the expected regret is under ~1 HP). `adv 20` when one turn is pivotal. `SIMULATOR DESYNC` or `DIFFERS` voids the advice: play by hand and run `status`.
`hold POWDERED_DEMISE[,ID]` keeps those potions out of the solver's choices (for the boss; `hold none` releases; set again after a daemon restart). `eval --pool <Act>:<regular|elite|boss> --hp full --attempts 256 --v "name|add=ID|remove=ID|upgrade=ID"` combat value of deck variants (ids are upper-case snake names: HEMOKINESIS); `route M E R ... --hp N`; `note`, `status`, `newrun`. Add `--smooth` for deck choices: win rate averaged over start HP x1/1.5/2/3 (4x the cost; the objective `sts2-deckbuilding` picks by).

## The skill gate `[code]`
Action commands (`a`, `turn`, `combat`, `x`, `f`, `draw`) answer `REFUSED: skills not loaded in this session: ...` until the core skills and the current screen's skills were loaded with the Skill tool (CLAUDE.md has the table). Read-only commands always work. After a compaction or clear the record is empty: load again. Daemon without a Claude Code session (human terminal): `STS2_SKILL_GATE=off` in your own shell. Code: `agent/skillgate.py`.

## Fewer calls per decision `[code]`
- **Batch**: `python -m agent - <<'EOF'` with one command per line runs them in order, each output under `>>> command`; the text is literal, so `-- why` needs no quoting (parentheses, semicolons and quotes are fine). It stops at the first `ERR` / `REFUSED` / `[chain stopped` (add `--keep-going` after `-` to continue). Use it to read and decide in one call, e.g. `brief` + `reward` + (next call) the pick.
- `brief`: header, deck by card, five-bucket line and gaps, relics, potions, the known boss and the elites that can still appear (the state every pick needs, in one call).
- `reward [--attempts N] [--hp full]` on a card reward screen: every option and skip priced against the boss (smooth objective), the elites that can still appear and the next act's elites and bosses, with the buckets each card fills. Replaces the separate `eval` calls of the pick procedure; a card whose display name has no simulator id is listed as not evaluated (use `eval`).
- `eval --boss | --elites | --next` replace `--pool <Act>:<kind>` (known boss, elites that can still appear, next act's elites and bosses, from the map and the bag logic).
- A chain now passes through a selection screen when the next step names its option: `a ~smith; ~Bash` (rest, smith, pick the card) is one call.

## Speed `[played]`
Target: a run in 30 min, fights 1-2 min. The bridge runs Instant fast mode; `combat` plays a whole fight in 5-40 s. Use `combat` for easy fights, `turn` / `adv` when the stakes are real, short reasons, no re-reading unchanged state, a whole reward screen in one chained `a`.

## Quirks `[played]`
- The solver never discards a potion (excluded in `engine.decide`; the bridge cannot).
- The fight-start prediction is made at the HP before Pantograph's heal: judge a boss with `eval --hp <HP on entry>`.
- A click onto an elite or boss below 60% HP needs `!`. Never chain map or node choices.
- If `combat` returns without playing, run `s`; an `ERR` line says why.

## Solver blind spots checked (details; open ones are tagged `[hyp]`)
- **High-energy decks `[sim]`** (`python -m agent.energy_gap`, 500 held-out scenarios per set, 4 attempts, ckpt b128): the training mix has 4+ energy in only 3% of scenarios and 5+ in 0.03%, yet the gap that search closes over the network alone is NOT wider at 4-7 energy (win +4.9 points, HP +4.8) than at 3 energy (win +7.5, HP +5.1); by energy 4/5/6/7: +5.5/+5.8/+3.0/+3.2 win points. So no sign that the policy is biased against energy-heavy decks relative to search; both share one value net, so a common bias is not excluded. Evals of energy-heavy decks need no extra discount. The scenario generator can add a high-energy slice (`tools/gen_train.py --energy-prob`).

## Review loop (after each run): numbers feed the book, the book covers the numbers' blind spots
1. `python -m agent.improve review` (writes `runs/<run>/review.md`): outcome and what ended the run; predicted vs actual per fight (Brier, HP); how often search overrode the policy (held potions excluded); fidelity divergences; card picks priced by `reward` and whether each followed the best smooth boss score; every `override:` in a `-- why`; costly fights kept under `runs/<run>/fights/`.
2. **Fidelity first**: a divergence is reproduced (`agent.validate`, `agent.fidelity_sweep`, `agent.calibrate` for real-vs-sim on a fixed deck) and fixed before any model comparison.
3. **Costly fights** (lost >= 30% max HP or lost): `python -m agent.hindsight <file> --log` says whether the loss is luck (percentile of the real loss among simulator replays) or a solver gap (decisions where a large-budget search prefers another line by >= 2 HP). A pattern of gaps goes to the corpus / fine-tune; an encounter pattern to `sts2-acts/encounters.md`.
4. **Overrides** (the judgment layer): for each, did it hold or fail in the fights that followed? Edit the blind-spot list in `sts2-deckbuilding` (narrow, tighten the test, or delete) and tag the claim `[played]` or `[hyp]`; the tally is `evals/overrides.jsonl`. `python -m agent.improve lessons` lists every open `[hyp]` in the book, including `evidence.md`: those are the experiment backlog.
5. **Model changes** only when 2-3 show a pattern: `corpus` -> `finetune` -> `gate` (candidate must gain >= 1 point on the corpus holdout and lose <= 1 on the fixed eval and the 4-7-energy eval) -> `adopt`; every step is in `evals/ledger.jsonl`. Strategy changes go through the book, model changes through the gate; neither is adopted on a single run.
- The live simulator is kept on the observed state each action: hand, discard, exhaust and (since the sync fix) the draw pile as a multiset. Cards the sim generates randomly (Stoke, Discovery, Infernal Blade ...) are replaced by the ones the game rolled, with no phantom cards left in the draw pile; a sim-created card gets no keywords of the real one (a Retain from Choices Paradox is lost until it is played) `[code]`. Fidelity sweeps (`python -m agent.fidelity_sweep`, `agent.fidelity_report`; every case a fresh run through the dev console) found no rule bugs in 159 relics, 65 potions, ~120 Ironclad cards and random decks of five characters; the leftover differences are random rolls the replay cannot reproduce (random targets, random potions, random card picks) `[sim]`.
- After every action the sim also takes the powers (id, amount) the game shows on the player and the enemies, so a hidden draw or random target that resolved differently (Hellraiser's auto-played Strikes, Havoc, Mayhem, Cascade) leaves no stale Slippery / Vulnerable / Strength. Not synced: relic counters and props (Joss Paper, Iron Club, Pael's Legion after Whispering Earring's hidden first hand) `[code]`.
- **Thorns `[sim]` `[played]`**: the Rust `ThornsPower` matches the game code (every powered hit, each hit of a multi-hit card, blockable). Calibration with one fixed deck replayed in the real game (dev console, `python -m agent.calibrate`): Spiny Toad real 23.8 HP lost (se 3.0, 14 fights, 14 wins) vs simulator 23.9 (se 1.6); Axebots real 50.4 (se 5.4, 11/14 wins) vs simulator 55.2 (se 2.8, 35/40). The earlier 37-HP Toad loss (predicted 10) was the tail of a wide distribution (sim range 4-34 HP) with that deck, not a misunderstanding of Thorns; forcing an end-turn candidate was neutral (A/B on Toad: 20.3 vs 18.9 HP).
