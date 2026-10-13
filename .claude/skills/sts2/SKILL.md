---
name: sts2
description: Root of the STS2 operator book. Load first every session: rules that never bend, the information contract, the decision loop every macro choice follows, rule IDs and citations, tags and test kinds, the skill hierarchy, the self-improvement loop.
---

# STS2 operator book (root)
The search plays fights; you own the macro (plan, draft, route, budgets). Screen -> skill table: `CLAUDE.md`.

## Rules that never bend
- Scored run: no console / god mode, no restart / reload, no hidden state (draw order, RNG, pre-rolled encounters, the seed's future, unrevealed maps or act variants). Never read forks, snapshots (`Fork.cs`, `--history`), logged runs or community data of this seed, or models evaluated on their own training seeds (BASE0001-0040 are training-only).
- Allowed: the screen, public game knowledge (pools, card text, move patterns, coded odds), what this run showed, the harness tools.
- Goal: win the run; end HP breaks ties. Every death is an avoidable error until the review says otherwise.
- Skills hold current rules only: no runs, fights, logs, TODOs. Evidence -> `docs/research/evidence.md` (E#), per-rule stats -> `evals/rules/ledger.jsonl`, history -> git.

## Decision loop (every macro decision)
1. Plan (`sts2-plan`): say what this decision changes in the run plan.
2. Numbers (`sts2-harness`): `price` (card reward, shop, event), `eval --boss` / `--v` (deck vs a named boss), `routes` (map, rest), `restcalc`.
3. Rules: the IDs that apply; on conflict the more specific level wins.
4. Act: `a <i> -- rules: <IDs or new: claim> ; numbers: <tool and margin> ; judgment: <what decided it>`. Cards by name (`a ~Card`). `new:` = reasoning no rule covers; it becomes a candidate rule in the review.

## Rule format
`ID [tag] rule. Test: <kind>: <check>.` IDs: P plan, D deckbuilding, R pathing, F fights / acts, M mechanics, IC Ironclad, A1- / A2- / A3- Ironclad acts, I improve. IDs are never reused.
- Tags: `[code]` decomp (file:line); `[sim]` measurement (E#); `[expert]` recorded expert reasoning; `[community]` community run statistics: a correlational prior, never evidence for promotion; `[hyp]` claim under test.
- Test kinds: instrument (named tool + threshold, scored every review), outcome (n runs, ledger), expert-attested (none yet).

## Hierarchy (lower levels only add or deviate)
0 `sts2`. 1 `sts2-harness`, `sts2-plan`, `sts2-deckbuilding`, `sts2-pathing`, `sts2-acts`, `sts2-mechanics`, `sts2-crystal-sphere`. 2 `sts2-<character>`. 3 `sts2-<character>-act<N>`. Any-character lesson -> level 1.

## Improvement
After every run `sts2-improve` (`newrun` is refused until reviewed): score cited decisions, update the ledger, change rules on evidence, one change per rule per run.
