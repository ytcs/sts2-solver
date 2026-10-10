---
name: sts2-harness
description: Use before the first action of a run and whenever unsure how to drive the game: python -m agent commands, action chaining, batch mode, solver (adv/turn/combat), eval syntax, speed targets, harness quirks, post-run review loop.
---

# Harness operation
`.venv/Scripts/python.exe -m agent <cmd>`, `STS2_DEVICE=cuda`. Double-quote `-- why`. All `[code]` (`agent/harness.py`) unless tagged.

## Commands
- Look: `s`, `d`, `p draw|discard|exhaust`, `m` (`boss:` line), `relics`, `status`, `brief`, `reward`, `plans`, `price [n] [--sat X] [--cont late|clip|prod|disc|mean]` (paired run-model rollouts; ladder P(win run) -> P(clear act) -> next-act readiness at P(clear act) >= 0.9 -> floors; `--cont` = gate-array surrogate, experimental).
- Act: `a <i> [target] [-- why]`, always a why. `loot` takes what needs no judgement (gold, potions into free slots) once the card reward, relics and a stolen card are decided (a stolen card may be one to drop anyway, or the card reward may offer it upgraded); order `[expert]`: card reward first (War Paint-like pickups act on it), relics before gold (Bowler Hat-like), and a free relic can be anti-synergy (Tungsten Rod vs Rupture/Inferno). Chain `a ~gold; ~card; ~proceed -- why` (`~text` case-insensitive; stops on error/combat; map click last; `a ~smith; ~Bash` passes the selection).
- Route: `draw r1c6 ...`; `route M E R ... --hp N [--act A] [--exclude IDS]`; `routes [--attempts N] [--hp N] [--w E=4,M=1]` (act DP on solver outcomes; reward weights `[hyp]`; unknown = 15% fight, approximation of `[code]` game_code.md C5; prints potion plan + elite arrival HP). **`routes` at every fork.**
- `restcalc`: the act boss(es) at the HP now vs after resting (Regal Pillow included): the rest decision's deciding number. `rmcalc [--hp ...]` removals ranked; `pickplan` take-threshold, once per act + after a shop; `note`, `newrun`.
- `eval --pool <Act>:<regular|elite|boss> | --boss | --elites | --next | --enc IDS [--all] [--future] [--smooth] [--hp full|N] [--attempts N] [--seed N] --v "name|add=ID|remove=ID|upgrade=ID|relics_add=ID|potions=IDS|enchant=ID:ENCH"`.
- Solver: `adv [secs]`; `turn`/`combat` in AUTO fights (`combat !` overrides MANUAL); `budget <s>|auto`. Search = 2 player turns then value net; exact turn search when the values are blind. `SIMULATOR DESYNC`/`DIFFERS`: play by hand, `status`.

## Potions (`agent/proposal.py`)
- Live search throws potions itself at a reserve cost (`agent/engine.py` POT_COST 0.06 ~ 10 HP each, half under win-only worth, 0 at the final boss; E61); set-aside potions are withheld until a boss. With a reserve cost the per-turn proposals no longer run or stop play; `potions` still prints the now / keep / save table on demand.
- `[sim]` before the final pair of bosses the constant cost undervalues holding (E67: Test Subject at 25 HP 0.24 -> 0.45 with two potions kept): `potion aside` them before boss 1 unless boss 1 is in doubt.
- Commit one per proposal: `potion use <name>` or `a <i>`; `turn`/`combat` declines. Next turn re-prices.
- `potion aside <name>[, name]|none` keeps for the boss (survives restart); bare `potion` lists set-aside potions; `potions` prints the table.
- keep/save priced within this fight: weigh the boss yourself. `[hyp]` proposals order potions well; test: the potion regression states (`docs/plan.md`).

## Fight objective (`SEARCH OBJECTIVE`)
Act boss before an ancient heal (and the final boss): P(win) only, 1% end-HP tiebreak. Others: win +1 + 0.5 x HP fraction, loss -1.

## Display
`eN now:` = the shown intent's move with its exact effects (`[me: ...]`, `[self: ...]`); `eN plan: +1 .. +3` = moves for 3 turns after it, odds marginal per turn, damage at today's modifiers. `[code]`

## Decision guards (`agent/guards.py`)
Card reward: `reward` + why fields `buckets:` `weakest:` `numbers:` `judgment:`. Map fork / Neow: `routes` this floor. Shop: a pricing call this floor (`price` counts); rest: `restcalc`, or `routes --hp <after>` + `routes` + upgrade `eval`; at >= 95% HP no pricing call (smith). Elite/boss below 60% HP: `a <i> !`.

## Batch, tables, quirks
- A reply that stopped (ERR, REFUSED, DIFFERS, DESYNC) repeats the reason as its last line (`^ stopped: ...`).
- `python -m agent - <<'EOF'` one command per line; stops at `ERR`/`REFUSED` unless `- --keep-going`.
- Variants share fights (`paired` line = se of the difference); seeded per screen, `--seed N` fresh. Decide on q90 + death tail.
- Option numbers shift after each action: bare `a <i>` after a chain step is refused; never chain map clicks.
- Crystal Sphere: `a 0 <x> <y>` (`sts2-crystal-sphere`). Pantograph heals after fight-start prediction: `eval --hp <entry HP>`.
- Never `quit` the daemon unless code changed (+20 s cold).

## Speed `[hyp]` (test: durations in `review`)
Run ~30 min, fight 1-2 min; `combat` for easy fights; short whys.

## Review loop (after each run)
1. `python -m agent.improve review` -> `runs/<run>/review.md`.
2. Fidelity first: `python -m agent.fidelity_sweep --mode recorded`; sim fixes pass `bash tools/gate.sh`.
3. Costly fights (lost, or >= 30% max HP): `python -m agent.hindsight <file> --log` -> luck vs solver gap; patterns -> `sts2-acts/encounters.md`.
4. Each `judgment:` -> edit the `sts2-deckbuilding` rule it used; `evals/judgments.jsonl`; `improve lessons` = backlog.
5. Model change only on a 2-3 run pattern: `improve corpus`, then a combat-loop round through the promotion gate (`docs/plan.md`).
