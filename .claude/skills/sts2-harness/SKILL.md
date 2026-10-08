---
name: sts2-harness
description: Use before the first action of a run and whenever unsure how to drive the game: python -m agent commands, action chaining, batch mode, solver (adv/turn/combat), eval syntax, speed targets, harness quirks, post-run review loop.
---

# Harness operation
`.venv/Scripts/python.exe -m agent <cmd>`, `STS2_DEVICE=cuda` (`agent/harness.py`). Double-quote `-- why` (`;`/parens break the shell); batch mode needs none.

## Commands
- Look (always allowed): `s`; `d` deck/relics; `p draw|discard|exhaust`; `m` map (`boss: <row> <ID> [+ <ID>]`); `relics`; `status` (run, fight, fidelity, engine, aside); `brief`, `reward` (`sts2-deckbuilding` section 1); `plans`; `price [n] [--sat X]` (screen options by paired run-model rollouts; shop = bundles in budget; horizon ladder P(win run) -> P(clear act) -> next-act readiness once P(clear act) >= 0.9 -> floors).
- Act: `a <i> [target] [-- why]` (always a why). Chain `a ~gold; ~card; ~skip; ~proceed -- why`; `~text` case-insensitive (avoid words inside card names, e.g. `end`); stops on error/combat; map click last; passes a selection when the next step names it (`a ~smith; ~Bash`).
- Route: `draw r1c6 ...`; `route M E R ... --hp N [--act A] [--exclude IDS]` (one route, fights + rests); `note <text>`; `newrun`.
- `routes [--attempts N] [--pf P] [--w E=4,M=1] [--hp N]`: whole act map, exact DP node x HP on solver outcomes. Per option: boss win (or survival) with >= k more elites, route per k, reward ranking (weights `[hyp]`: elite 5, treasure 3.5; unknown = 15% regular fight; events/treasure unsimulated). Gold carried (A10 `[code]` monster ~11, elite ~30, treasure ~35); shop = best basket (card ~60, removal 100 +50/use, relic ~225). Each potion thrown at most once at its best elite/boss (hallways without); prints `potion plan` + elite arrival HP (alive, mean, q10). `--hp` = what-if HP.
- `rmcalc [--attempts N] [--hp full|current|N]`: each removable card as a removal, ranked.
- `pickplan [--screens K] [--shops S]`: take-threshold (`sts2-deckbuilding` section 1).
- `eval --pool <Act>:<regular|elite|boss> | --boss | --elites | --next | --enc IDS [--all] [--future] [--smooth] [--hp full|N] [--attempts N] [--seed N] --v "name|add=ID|remove=ID|upgrade=ID|relics_add=ID|potions=IDS|enchant=ID:ENCH"` (ids upper snake). `--next` = next act elites+bosses; `--future` all horizons; `--smooth` win over start HP x1/1.5/2/3 (4x cost).
- Solver: `adv [secs]` (+ expected enemy damage; `adv 20` pivotal turn); `turn`/`combat` only in AUTO fights (`combat !` overrides MANUAL; `sts2-strategy`). `budget <s>` fixed, searched in full; `budget auto` 1-15 s by danger, stops at regret < ~1 HP. Search plays 2 player turns then value net `[sim]`; more futures/time at same depth ~nothing. `SIMULATOR DESYNC`/`DIFFERS` voids advice: play by hand, `status`.
- `routes` at every fork; `pickplan` once per act + after a shop.

## Potions `[code]` (`agent/proposal.py`): solver proposes, I commit one per commit
- Live search plans without potions, never throws.
- Each turn, each potion on the same 32 futures to fight end, other potions out: now (best target) / keep (from next turn) / save. Table: P(win), end HP mean p10/p50/p90 (loss 0), score, paired diff to save; verdict USE NOW / KEEP / SAVE.
- `turn`/`combat` stop with `POTION PROPOSAL` iff now > keep AND save by 2 paired se in score, or win at stake (max(now, keep) wins more than save, > 2 se).
- Answer `potion use <name>` / `a <i>`, or `turn`/`combat` (no new proposal that turn). Next turn re-prices; never a second throw on one proposal.
- `potion aside <name>[, name]` = keep for boss (elsewhere stops only when win at stake); `potion aside none`; `potion` lists; `potions` table; `adv` prints it.
- keep/save priced within this fight: weigh SAVE vs the boss yourself. Tables price non-boss fights without potions (lower bound), boss with.
- `[hyp]` proposals order potions well. Test (S4 gate): regression states vs large-budget references.

## Fight objective `[code]` (`SEARCH OBJECTIVE`)
Act boss before an ancient heal (Acts 1-2; Act 3 final, A10 the second): P(win) only (+1% end-HP tiebreak) in prediction, search, potion arms. Others linear: win +1 + 0.5 x HP fraction, loss -1.

## Combat display `[code]`
`draw (N)` multiset; `eN plan: +1 .. +2 .. +3 ..` moves for 3 turns after the intent, odds, damage at today's modifiers, effects (`[me: VULNERABLE +3]`, `[discard: +3 WOUND]`, `[self: ...]`, `summons`); odds marginal per turn.

## Decision guards `[code]` (`agent/guards.py`; never judge the choice)
- Card reward: `reward` this screen + why with `buckets:`, `weakest:`, `numbers:`, `judgment:`.
- Map fork, Neow/ancient: `routes`/`route` this floor.
- Shop buy: `eval`/`rmcalc`/`routes`/`pickplan` this floor. Rest: `routes --hp <after rest>` + `routes` + `eval` upgrade variant this floor. Both: `numbers:` + `judgment:`.
- Elite/boss click below 60% HP: `a <i> !`. Refusal names what is missing: run it, repeat.

## Batch `[code]`
`python -m agent - <<'EOF'` one command per line, literal text, output under `>>> cmd`; stops at `ERR`/`REFUSED`/`[chain stopped` unless `- --keep-going`.

## Tables `[code]`
Variants of one call share fights: a `paired` `vs` line = se of the difference. Draws seeded per screen (re-run repeats; `--seed N` fresh). `eval` prints HP-lost q10/50/90/97.5 (loss = start HP); decide on q90 + death tail. Warm-daemon cost `[hyp]` (Act 2 deck): `reward` ~27 s, `routes` ~19 s (cached), `rmcalc` ~57 s, `eval --smooth --boss` ~11 s/variant, `pickplan` ~145 s; cold daemon +20 s: never `quit` unless code changed.

## Quirks `[code]`
- Option numbers shift after each action: bare `a <i>` after an earlier chain/batch step refused; ambiguous `~text` refused. Exceptions: identical labels; gold (`a ~gold; ~gold; ~card`).
- Never chain map choices. `do` refused. `hold`, `potion allow/deny/keep` retired.
- Crystal Sphere: `a 0 <x> <y>`, tool, proceed (`Decisions.cs` `CrystalSphere`); `sts2-crystal-sphere`.
- `potion aside` survives daemon restart. Solver never throws/discards live.
- Fight-start prediction is before Pantograph's heal: `eval --hp <entry HP>`.
- `combat` returned without playing: `s`; `ERR` says why.
- Relic state synced (saved props each action; counters from display). Not synced: state neither reveals; sim-created cards lack real keywords until played.

## Speed `[hyp]` (test: durations in `review`)
Run ~30 min, fight 1-2 min; `combat` for easy fights; short whys; no re-reading unchanged state; reward screen in one chain.

## Review loop (after each run)
1. `python -m agent.improve review` -> `runs/<run>/review.md` (predicted vs actual, overrides, fidelity, picks vs best, judgments, costly fights).
2. Fidelity first: `python -m agent.fidelity_sweep --mode recorded`; fix before model comparisons; sim changes pass `bash tools/gate.sh`.
3. Costly fights (lost >= 30% max HP, or lost): `python -m agent.hindsight <file> --log` -> luck vs solver gap (large search better by >= 2 HP). Gaps -> corpus; encounter patterns -> `sts2-acts/encounters.md`.
4. Judgments: edit the `sts2-deckbuilding` rules each `judgment:` relied on; `[sim]` once measured else `[hyp]` + test; tally `evals/judgments.jsonl`; `python -m agent.improve lessons` = backlog.
5. Model change only on a 2-3 run pattern: `improve corpus` -> `finetune` -> `gate` (>= +1 point corpus holdout, <= -1 on fixed and 4-7-energy evals) -> `adopt`; `evals/ledger.jsonl`.
