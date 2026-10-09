---
name: sts2
description: Use at the start of any Slay the Spire 2 run or session: root of the strategy book. Rules that never bend (no cheating), the skill hierarchy, evidence tags, and the update protocol after a run.
---

# STS2 strategy book (root)
Loading: hooks + harness; screen->skill table `CLAUDE.md`.

## Rules that never bend
- Scored run: no console/god mode, no hidden state (draw order, RNG, pre-rolled encounters, seed future), no restart/reload. Console only for harness tests.
- Allowed: the screen, public game knowledge (pools, card text, patterns, coded odds), what this run showed.
- Goal: win the run; tiebreak end HP.
- Skills = mutable rules/mechanics/tests with tags. No runs, past fights/picks, logs, TODOs; evidence -> `runs/<run>/`, `evals/`. Test: still a rule if every run were forgotten?

## Hierarchy (lower levels only add/deviate)
0 `sts2`. 1 `sts2-harness`, `sts2-strategy`, `sts2-deckbuilding`, `sts2-pathing`, `sts2-acts`, `sts2-mechanics`, `sts2-crystal-sphere`. 2 `sts2-<character>`. 3 `sts2-<character>-act<N>`. Any-character lesson -> `sts2-strategy`.

## Tags (every claim)
`[code]` decomp (cite). `[sim]` measurement (script, n, margin). `[expert]` recorded expert choice, not proof. `[hyp]` + its test (`python -m agent.improve lessons` lists them).

## Update protocol
1. After a run: revise at the most specific level that holds.
2. A run outcome is not evidence; contradicted -> rewrite/delete.
3. Numbers over adjectives. Long tables -> sibling file.
