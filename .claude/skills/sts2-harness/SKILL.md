---
name: sts2-harness
description: How to operate the STS2 self-play harness (python -m agent): commands, action chaining, the solver (adv/turn/combat), eval syntax, speed targets, harness quirks and the post-run review loop. Load at the start of any run.
---

# Harness operation

Run as `.venv/Scripts/python.exe -m agent <cmd>` with `STS2_DEVICE=cuda`; details in `agent/harness.py` and `README.md`. Quote `-- why` text in double quotes (parentheses and semicolons break the shell).

## Commands
`s` state; `a <i> [target] [-- why]` act (always give the reason); chain with `;` and pick options by label: `a ~gold; ~card; ~skip; ~proceed -- why` (stops on error or combat; a map click must be last; `~text` matches option text, so avoid words that occur in card names such as `end`). `d` deck/relics, `p draw|discard|exhaust`, `m` map, `draw r1c6 ...` route on the map, `relics` counters.
`adv [secs]` advice plus the enemies' expected damage; `turn` / `combat` let the solver play a turn / the fight. `budget <s>` fixes the search time; default auto (1-15 s from the fight's predicted danger; stops early when the expected regret is under ~1 HP). `adv 20` when one turn is pivotal. `SIMULATOR DESYNC` or `DIFFERS` voids the advice: play by hand and run `status`.
`eval --pool <Act>:<regular|elite|boss> --hp full --attempts 256 --v "name|add=ID|remove=ID|upgrade=ID"` combat value of deck variants (ids are upper-case snake names: HEMOKINESIS); `route M E R ... --hp N`; `note`, `status`, `newrun`.

## Speed `[played]`
Target: a run in 30 min, fights 1-2 min. The bridge runs Instant fast mode; `combat` plays a whole fight in 5-40 s. Use `combat` for easy fights, `turn` / `adv` when the stakes are real, short reasons, no re-reading unchanged state, a whole reward screen in one chained `a`.

## Quirks `[played]`
- The solver never discards a potion (excluded in `engine.decide`; the bridge cannot).
- The fight-start prediction is made at the HP before Pantograph's heal: judge a boss with `eval --hp <HP on entry>`.
- A click onto an elite or boss below 60% HP needs `!`. Never chain map or node choices.
- If `combat` returns without playing, run `s`; an `ERR` line says why.

## Review loop (after each run)
`python -m agent.improve review` (predicted vs actual, search overrides, fidelity); fix fidelity first; test the `[hyp]` claims the run touched (`python -m agent.improve lessons`); update the book at the right level; only then `corpus` / `finetune` / `gate`.
