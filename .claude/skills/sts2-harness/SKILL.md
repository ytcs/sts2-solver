---
name: sts2-harness
description: Use before the first action of a run and whenever unsure how to drive the game: python -m agent commands, action chaining, batch mode, solver (adv/turn/combat), eval syntax, speed targets, harness quirks, post-run review loop.
---

# Harness operation
`.venv/Scripts/python.exe -m agent <cmd>`, `STS2_DEVICE=cuda`. Operator run (headless, scored): `bash scripts/operator_server.sh BASE01NN [port 15700]`, then every command with `STS2_BRIDGE=127.0.0.1:15700`; seeds BASE0121+ only (1-120 are training / arm seeds). Double-quote `-- why`. All `[code]` (`agent/harness.py`) unless tagged.

## Commands
1. Look: `s`, `d`, `p draw|discard|exhaust`, `m` (`boss:` line), `relics`, `status`, `brief` (deck, buckets + gaps, boss / elites left, bottleneck line).
2. `price [--run] [--hp N] [n]`: card reward / event: the search check of every option (log ratio vs skip over the factors of P(win run): near binding fights at current HP, future boss rungs at full HP or an HP multiple when floored); `--run` adds the run-model rollouts (n 128; weak in acts 1-2). Shop: run model + bundles + the search check of the top ones. No verdict in operator play. Near fights are scored at current HP; `--hp N` scores them at an expected arrival HP (P9). An event option with a player-chosen enchant is priced once per legal target (`Option -> CARD`).
3. Act: `a <i> [target] [-- why]`, always a why. Chain `a ~gold; ~card; ~proceed -- why` (`~text` case-insensitive; stops on error/combat; map click last; `a ~smith; ~Bash` passes the selection).
4. `loot` after the card reward (`price` on it), relics and a stolen card are decided: takes gold, then potions into free slots. `[expert]` card reward before relics (War Paint-like pickups act on it), relics before gold (Bowler Hat-like); a free relic can be anti-synergy (Tungsten Rod vs Rupture/Inferno).
5. Route: `draw r1c6 ...`; `routes [--attempts N] [--hp N] [--pf 0.15] [--w E=4,M=1] [--seed N]`: act DP on solver outcomes (reward weights `[hyp]`; unknown room = 15% fight, approximates `[code]` game_code.md C5); potion plan + elite arrival HP. **`routes` at every fork.** Its "with elite rewards" block (`[sim]` estimate beside the DP, never in it) scales each k>= column by the deck-strength gain of k elites' measured relic + elite-vs-hallway card value (`data/reward_values.json`, `tools/reward_values.py`).
6. `restcalc [eval flags]`: act boss(es) at the HP now vs after resting (Regal Pillow included).
7. `eval --pool <Act>:<regular|elite|boss>[:n] | --boss | --elites | --next | --enc IDS | --future [--all] [--smooth] [--hp full|current|N] [--attempts N] [--seed N] --v "name|add=ID|remove=ID|upgrade=ID|relics_add=ID|potions=IDS|enchant=ID:ENCH"`, or a JSON spec. `--potions`: potions usable (default holds them; price potion-making relics this way). `--target`: the baseline is the plan's target deck, not the current one (deck strength of the current act-1 deck vs the pair misleads, E86). `[sim]` Search-playout win vs live realised (E96, 3,170 fights): act-1 bosses 0.86 vs 0.87, act-2 bosses 0.65 vs 0.72 (pessimistic), act-3 bosses 0.26 vs 0.22; elites and hallways within 0.01-0.04.
8. Solver: `turn [secs]` plays one turn and prints every play with its alternatives (AUTO and MANUAL); `combat [secs]` the whole fight (MANUAL: `combat !` with the reason); `adv [secs]` the next play only; `budget <s>|auto`. Search = 2 player turns then value net; exact turn search when values are blind. `SIMULATOR DESYNC`/`DIFFERS`: play by hand, `status`.
9. `target add=..|remove=..|upgrade=..|relics_add=..` sets the plan's target deck relative to the current deck (`none` clears; bare prints it); `note <text>` (`note seen_seed <seed>` at the start of a replayed seed: its ledger rows stay out of `improve rules`), `newrun` (force new-run detection). A changed relic bar is logged (`relics` event).

## Potions (`agent/proposal.py`, `agent/engine.py`)
- The search values potions at a reserve cost (POT_COST 0.06 ~ 10 HP each, half under win-only worth, E61) behind a 2-se margin gate; set-aside potions are withheld until a boss.
- Operator confirms every throw: when the search's play is a potion, `turn` / `combat` stop with `POTION: ...` (its q and the best play without a potion); `potion yes` throws exactly that, `potion no` plays the rest of the turn without potions.
- Final pair: boss 1 is valued by P(win boss 2 | HP, potions left), so each kept potion is priced by its boss-2 gain; no reserve cost at boss 2 (E89, E90).
- `potion aside <name>[, name]|none` keeps for a boss (survives restart); bare `potion` lists them; `potions` prints the now / keep / save table. Commit one at a time: `potion use <name>` or `a <i>`; the next turn re-prices.

## Fight objective (`SEARCH OBJECTIVE`)
Act boss before an ancient heal and the final boss: P(win) only, 1% end-HP tiebreak. Others: win +1 + 0.5 x HP fraction, loss -1.

## Display
`eN now:` = shown intent's move with exact effects (`[me: ...]`, `[self: ...]`); `eN plan: +1 .. +3` = next 3 turns, odds marginal per turn, damage at today's modifiers. `price` search check: each factor prints `win x` (P(win), playouts); `(scored on progress y)` marks a factor whose skip win rate is floored, ranked by enemy HP dealt instead. A reply headed `WARNING: harness code changed` came from a stale daemon: `quit` and repeat.

## Decision guards (`agent/guards.py`)
Macro `-- why` = `rules: <IDs or new: claim> ; numbers: <tool and margin> ; judgment: <what decided it>` (`sts2` decision loop). Card reward: `price` this screen first; the why names the chosen card (`a ~Card`). Map fork / Neow: `routes` this floor. Shop: a pricing call this floor. Rest: `restcalc`, or `routes --hp <after>` + `routes` + upgrade `eval`; at >= 95% HP no pricing call (smith). Elite/boss below 60% HP on a fork: `a <i> !`. New run: refused until the previous run is reviewed (`sts2-improve`).

## Batch, quirks
1. `python -m agent - <<'EOF'` one command per line; stops at `ERR`/`REFUSED` unless `- --keep-going`. A stopped reply repeats the reason last (`^ stopped: ...`).
2. Variants share fights (`paired` = se of the difference); seeded per screen, `--seed N` fresh. Decide on q90 + death tail.
3. Option numbers shift after each action: a bare `a <i>` after a chain step is refused; never chain map clicks.
4. Crystal Sphere: `a 0 <x> <y>` (`sts2-crystal-sphere`). The sim applies Pantograph's +25 at boss start: give `eval --hp` the pre-heal HP, never add 25.
5. Never `quit` the daemon unless code changed (+20 s cold).

## Speed `[hyp]` (test: durations in `review`)
Run ~30 min, fight 1-2 min; `combat` for easy fights; short whys.

## Review loop
After each run: `sts2-improve`. Fidelity first (`python -m agent.fidelity_sweep --mode recorded`; sim fixes pass `bash tools/gate.sh`); costly fights `python -m agent.hindsight <file> --log`; model change only on a 2-3 run pattern (`improve corpus`, promotion gate in `docs/plan.md`).
