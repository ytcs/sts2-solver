---
name: sts2-harness
description: Use before the first action of a run and whenever unsure how to drive the game: python -m agent commands, action chaining, batch mode, solver (adv/turn/combat), eval syntax, speed targets, harness quirks, post-run review loop.
---

# Harness operation
`.venv/Scripts/python.exe -m agent <cmd>`, `STS2_DEVICE=cuda`. Double-quote `-- why`. All `[code]` (`agent/harness.py`) unless tagged.

## Commands
1. Look: `s`, `d`, `p draw|discard|exhaust`, `m` (`boss:` line), `relics`, `status`, `brief` (deck, buckets + gaps, boss / elites left, bottleneck line).
2. `price [n] [--no-search]`: paired run-model rollouts (n 128) on this screen's options (card reward, shop incl. removal, catalogued event / ancient, rest, map, potion swap on rewards); on card reward / shop / event also the search check of the top options + skip + best removal: log ratio vs skip over the factors of P(win run), per factor P(win) (near binding fights at current HP, future boss rungs). Prints `decision` (shared with the scripted baseline).
3. Act: `a <i> [target] [-- why]`, always a why. Chain `a ~gold; ~card; ~proceed -- why` (`~text` case-insensitive; stops on error/combat; map click last; `a ~smith; ~Bash` passes the selection).
4. `loot` after the card reward (`price` on it), relics and a stolen card are decided: takes gold, then potions into free slots. `[expert]` card reward before relics (War Paint-like pickups act on it), relics before gold (Bowler Hat-like); a free relic can be anti-synergy (Tungsten Rod vs Rupture/Inferno).
5. Route: `draw r1c6 ...`; `routes [--attempts N] [--hp N] [--pf 0.15] [--w E=4,M=1] [--seed N]`: act DP on solver outcomes (reward weights `[hyp]`; unknown room = 15% fight, approximates `[code]` game_code.md C5); potion plan + elite arrival HP. **`routes` at every fork.**
6. `restcalc [eval flags]`: act boss(es) at the HP now vs after resting (Regal Pillow included).
7. `eval --pool <Act>:<regular|elite|boss>[:n] | --boss | --elites | --next | --enc IDS | --future [--all] [--smooth] [--hp full|current|N] [--attempts N] [--seed N] --v "name|add=ID|remove=ID|upgrade=ID|relics_add=ID|potions=IDS|enchant=ID:ENCH"`, or a JSON spec.
8. Solver: `adv [secs]`; `turn`/`combat [secs]` in AUTO fights (`!` overrides MANUAL); `budget <s>|auto`. Search = 2 player turns then value net; exact turn search when values are blind. `SIMULATOR DESYNC`/`DIFFERS`: play by hand, `status`.
9. `note <text>`, `newrun` (force new-run detection). A changed relic bar is logged (`relics` event).

## Potions (`agent/proposal.py`)
1. Search throws potions at a reserve cost (`agent/engine.py` POT_COST 0.06 ~ 10 HP; half under win-only worth; 0 at the final boss) behind a 2-se margin gate; set-aside potions are withheld until a boss. `[sim]` E61, E69.
2. Before the final pair: `potion aside` them before boss 1 unless boss 1 is in doubt. `[sim]` E67 (Test Subject at 25 HP 0.24 -> 0.45 with two kept).
3. `potion use <name>` or `a <i>` commits one; `turn`/`combat` decline; next turn re-prices. `potion aside <name>[, name]|none` (survives restart); bare `potion` lists them; `potions` prints now / keep / save.
4. keep/save is priced within this fight: weigh the boss yourself. `[hyp]` test: potion regression states (`docs/plan.md`).

## Fight objective (`SEARCH OBJECTIVE`)
Act boss before an ancient heal and the final boss: P(win) only, 1% end-HP tiebreak. Others: win +1 + 0.5 x HP fraction, loss -1.

## Display
`eN now:` = shown intent's move with exact effects (`[me: ...]`, `[self: ...]`); `eN plan: +1 .. +3` = next 3 turns, odds marginal per turn, damage at today's modifiers.

## Decision guards (`agent/guards.py`)
1. Card reward: `price` on this screen; why holds `numbers:` and `judgment:` and names the chosen card (`a ~Card`).
2. Map fork / Neow: `routes` this floor.
3. Shop / rest: `price`, `eval`, `routes` or `restcalc` this floor, why with `numbers:` and `judgment:`. Rest below 95% HP: `restcalc`, or `routes` + `eval`; at >= 95% no pricing call, the why fields still.
4. Elite/boss below 60% HP: `a <i> !`.

## Batch, quirks
1. `python -m agent - <<'EOF'` one command per line; stops at `ERR`/`REFUSED` unless `- --keep-going`. A stopped reply repeats the reason last (`^ stopped: ...`).
2. Variants share fights (`paired` = se of the difference); seeded per screen, `--seed N` fresh. Decide on q90 + death tail.
3. Option numbers shift after each action: a bare `a <i>` after a chain step is refused; never chain map clicks.
4. Crystal Sphere: `a 0 <x> <y>` (`sts2-crystal-sphere`). Pantograph heals after the fight-start prediction: `eval --hp <entry HP>`.
5. Never `quit` the daemon unless code changed (+20 s cold).

## Speed `[hyp]` (test: durations in `review`)
Run ~30 min, fight 1-2 min; `combat` for easy fights; short whys.

## Review loop (after each run)
1. `python -m agent.improve review` -> `runs/<run>/review.md`.
2. Fidelity first: `python -m agent.fidelity_sweep --mode recorded`; sim fixes pass `bash tools/gate.sh`.
3. Costly fights (lost, or >= 30% max HP): `python -m agent.hindsight <file> --log`; patterns -> `sts2-acts/encounters.md`.
4. Each `judgment:` -> edit the `sts2-deckbuilding` rule it used; `evals/judgments.jsonl`; `improve lessons` = backlog.
5. Model change only on a 2-3 run pattern: `improve corpus`, then a combat-loop round through the promotion gate (`docs/plan.md`).
