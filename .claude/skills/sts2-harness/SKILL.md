---
name: sts2-harness
description: Use before the first action of a run and whenever unsure how to drive the game: python -m agent commands, action chaining, batch mode, solver (adv/turn/combat), eval syntax, potions, guards, quirks, post-run review loop.
---

# Harness operation
`.venv/Scripts/python.exe -m agent <cmd>`, `STS2_DEVICE=cuda`. Operator run (headless, scored): `bash scripts/operator_server.sh BASE01NN [port 15700]`, then every command with `STS2_BRIDGE=127.0.0.1:15700`; seeds BASE0121+ only (1-120 are training / arm seeds). Double-quote `-- why`. All `[code]` (`agent/harness.py`) unless tagged.

## Commands
1. Look: `s`, `d`, `p draw|discard|exhaust`, `m` (`boss:` line), `relics`, `status`, `brief` (target missing, deck, buckets + gaps, boss / elites left, bottleneck: energy-bound share, draw-bound share (turns ending with energy and nothing playable), most ignored cards; printed after every fight).
2. `price [--run] [--hp N] [n]`: per option, log ratio vs skip over the factors of P(win run): near binding fights at current HP (`--hp N`: at an expected arrival HP, P9), later boss rungs at full HP or an HP multiple when floored. `--run` adds run-model rollouts (n 128; weak in acts 1-2). Shop: run model + bundles + the search check of the top ones. No verdict. An event option with a player-chosen enchant is priced per legal target (`Option -> CARD`).
3. Act: `a <i> [target] [-- why]`, always a why. Chain `a ~gold; ~card; ~proceed -- why` (`~text` case-insensitive; stops on error/combat; map click last; `a ~smith; ~Bash` passes the selection).
4. `loot` once the card reward (`price` it), relics and a stolen card are decided: takes gold, then potions into free slots. `[expert]` Card reward before relics (War Paint-like pickups act on it), relics before gold (Bowler Hat-like); a free relic can be anti-synergy (Tungsten Rod vs Rupture / Inferno).
5. Route: `draw r1c6 ...`; `routes [--attempts N] [--hp N] [--pf 0.15] [--w E=4,M=1] [--seed N]`: act DP on solver outcomes (reward weights `[hyp]`; unknown room = 15% fight, `game_code.md` C5); potion plan + elite arrival HP. **`routes` at every fork.** Its "with elite rewards" block (`[sim]` estimate, outside the DP) scales each k>= column by k elites' relic + card value (`data/reward_values.json`).
6. `restcalc [eval flags]`: act boss(es) at the HP now vs after resting (Regal Pillow included).
7. `eval --pool <Act>:<regular|elite|boss>[:n] | --boss | --elites | --next | --enc IDS | --future [--all] [--smooth] [--hp full|current|N] [--attempts N] [--seed N] --v "name|add=ID|remove=ID|upgrade=ID|relics_add=ID|potions=IDS|enchant=ID:ENCH"`, or a JSON spec. `--potions`: potions usable (default holds them; price potion-making relics this way). `--target`: baseline = the plan's target deck (the current act-1 deck vs the pair misleads, E86). `[sim]` Play-outs are the weak player (`solver.fs`): act-3 bosses read ~0.10 below live play (E136), the act-2 boss ~0.07 low, the rest within 0.04 (E96); compare variants, not levels.
8. Solver: `turn [secs]` plays one turn, printing every play with its alternatives (AUTO and MANUAL); `combat [secs]` the whole fight (MANUAL: `combat !` with the reason); `adv [secs]` the next play only; `budget <s>|auto`. `SIMULATOR DESYNC` / `DIFFERS`: `status` resyncs, then `turn` again; play by hand while it repeats.
9. `target add=..|remove=..|upgrade=..|relics_add=..` sets the plan's target deck vs the current deck (`none` clears; bare prints). `note <text>` (`note seen_seed <seed>` at the start of a replayed seed: its ledger rows stay out of `improve rules`). `newrun` forces new-run detection.
10. `prior <ENC|ACT3|pair> [CARD|RELIC ...]`: community win rate with vs without each item (deck at that fight; `[community]`, correlational, never evidence for promotion); bare: top / bottom items.

## Potions (`agent/proposal.py`, `agent/engine.py`)
- The search values potions at a reserve cost (POT_COST 0.06 ~ 10 HP each, half under win-only worth, E61) behind a 2-se margin gate; set-aside potions are withheld until a boss.
- [expert: user] No scripted default answer to a `POTION:` stop: every throw or hold is an operator decision with its why.
- When the search's play is a potion, `turn` / `combat` stop with `POTION: ...` (its q and the best play without a potion): `potion yes` throws exactly that, `potion no` plays the turn without potions.
- Final pair: boss 1 is valued by P(win boss 2 | HP, potions left), so each kept potion is priced by its boss-2 gain; no reserve cost at boss 2 (E89, E90).
- `potion aside <name>[, name]|none` keeps for a boss (survives restart); bare `potion` lists them; `potions` prints the now / keep / save table. Commit one at a time: `potion use <name>` or `a <i>`; the next turn re-prices.

## Fight objective (`SEARCH OBJECTIVE`)
Act boss before an ancient heal and the final boss: P(win) only, 1% end-HP tiebreak. Others: win +1 + 0.5 x HP fraction, loss -1.

## Display
`eN now:` = the shown intent's exact effects (`[me: ...]`, `[self: ...]`); `eN plan: +1 .. +3` = next 3 turns, odds marginal per turn, damage at today's modifiers. `price` prints `win x` per factor; `(scored on progress y)` = skip win rate floored, ranked by enemy HP dealt. `WARNING: harness code changed` = stale daemon: `quit` and repeat.

## Decision guards (`agent/guards.py`)
Macro `-- why` = `rules: ... ; numbers: ... ; judgment: ...`. Card reward: `price` this screen first; name the chosen card (`a ~Card`). Map fork / Neow: `routes` this floor. Shop: a pricing call this floor. Rest: `restcalc`, or `routes --hp <after>` + `routes` + upgrade `eval`; >= 95% HP needs none (smith). Elite / boss below 60% HP on a fork: `a <i> !`. New run: refused until the previous run is reviewed.

## Batch, quirks
1. `python -m agent - <<'EOF'` one command per line; stops at `ERR` / `REFUSED` unless `- --keep-going`; the reason repeats last (`^ stopped: ...`).
2. Variants share fights (`paired` = se of the difference); seeded per screen, `--seed N` fresh. Decide on q90 + death tail.
3. Unknown command: `ERR unknown command`, exit 1 (discard a potion: `a dp <slot>`); chain shell calls with `&&`, never `;`.
4. Option numbers shift after each action: a bare `a <i>` after a chain step is refused; never chain map clicks.
5. Crystal Sphere: `a 0 <x> <y>` (`sts2-crystal-sphere`). The sim applies Pantograph's +25 at boss start: give `eval --hp` the pre-heal HP.
6. Never `quit` the daemon unless code changed (+20 s cold).

## Speed
- [hyp] Run ~30 min, fight 1-2 min; `combat` for easy fights; short whys. Test: outcome: durations in `review`.

## Review loop
After each run: `sts2-improve`. Fidelity first (`python -m agent.fidelity_sweep --mode recorded`; sim fixes pass `bash tools/gate.sh`); costly fights `python -m agent.hindsight <file> --log`; model change only on a 2-3 run pattern (`improve corpus`, promotion gate in `docs/plan.md`).
