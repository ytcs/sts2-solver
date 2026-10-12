---
name: sts2
description: Root of the STS2 operator book. Load first every session: rules that never bend, the information contract, the decision loop every macro choice follows, rule IDs and citations, tags and test kinds, the skill hierarchy, the self-improvement loop.
---

# STS2 operator book (root)
The player is the harness plus you, the operator: the search plays fights; you own the macro (plan, draft, route, budgets). Loading: hooks + harness; screen -> skill table in `CLAUDE.md`.

## Rules that never bend
- Scored run: no console / god mode, no hidden state (draw order, RNG, pre-rolled encounters, the seed's future, unrevealed maps or act variants), no restart / reload. Never read forks, snapshots (`Fork.cs`, `--history`), logged runs of this seed, or models evaluated on their own training seeds (BASE0001-0040 are training-only).
- Allowed: the screen, public game knowledge (pools, card text, move patterns, coded odds), what this run showed, the harness tools.
- Goal: win the run; end HP breaks ties. Every death is an avoidable error until the review says otherwise.
- Skills are the current rules, nothing else: no runs, fights, logs, TODOs. Evidence -> `docs/research/evidence.md` (E#), per-rule stats -> `evals/rules/ledger.jsonl`, history -> git.

## Decision loop (every macro decision)
1. Plan: the run plan from `sts2-plan` (win condition, target deck vs the final pair, budgets, phase); say what this decision changes in it.
2. Numbers: the tool for this decision (`sts2-harness`): `price` (card reward, shop, event), `eval --boss` / `--v` (deck strength vs a named boss), `routes` (map, rest), `restcalc`.
3. Rules: the rule IDs that apply; where they conflict, the higher level's more specific rule wins (hierarchy below).
4. Act: `a <i> -- rules: <IDs or new: claim> ; numbers: <tool and margin> ; judgment: <what decided it>`. Pick cards by name (`a ~Card`). `new:` marks reasoning no rule covers: it becomes a candidate rule in the review.

## Rule format
`ID [tag] rule. Test: <kind>: <check>.` IDs by skill: P plan, D deckbuilding, R pathing, F fights / acts, M mechanics, IC Ironclad, A1- / A2- / A3- Ironclad acts, I improve. IDs are never reused.
- Tags: `[code]` decomp (cite file:line); `[sim]` measurement (E#, n, margin); `[expert]` recorded expert reasoning; `[hyp]` claim under test.
- Test kinds: instrument (a named tool and threshold, scored in every review), outcome (needs n runs; the ledger counts), expert-attested (none yet; stays `[expert]` until another kind applies).

## Hierarchy (lower levels only add or deviate)
0 `sts2`. 1 `sts2-harness`, `sts2-plan`, `sts2-deckbuilding`, `sts2-pathing`, `sts2-acts`, `sts2-mechanics`, `sts2-crystal-sphere`. 2 `sts2-<character>`. 3 `sts2-<character>-act<N>`. Any-character lesson -> level 1.

## Improvement
After every run `sts2-improve` (the harness refuses `newrun` until the previous run is reviewed): score the cited decisions, update the ledger, change rules on evidence, one change per rule per run.
