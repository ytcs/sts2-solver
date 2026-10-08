---
name: sts2
description: Use at the start of any Slay the Spire 2 run or session: root of the strategy book. Rules that never bend (no cheating), the skill hierarchy, evidence tags, and the update protocol after a run.
---

# STS2 strategy book (root)
Loading enforced by hooks + harness; screen->skill table: `CLAUDE.md`.

## Rules that never bend
- No cheating in a scored run: no dev console/god mode, no hidden state (draw order, RNG streams, pre-rolled encounter/elite order, seed future), no restart/reload. Console only for harness tests.
- Allowed info: the screen, public game knowledge (pools, card text, monster patterns, coded odds), what this run showed.
- Goal: win the run; tiebreak: end HP.
- Skill files = mutable rules/mechanics/tests with status tags. Never a journal: no runs, past fights/picks as examples, logs, TODOs. Edit/delete a rule when it changes. Evidence/history -> `runs/<run>/`, `evals/`. Test: line still a rule if every run were forgotten? Else not in a skill.

## Hierarchy (lower levels only add/deviate)
- 0 `sts2`: rules, hierarchy, tags, update protocol.
- 1 `sts2-harness` commands/calculators/review loop; `sts2-strategy` decision loop + shared targets; `sts2-deckbuilding` pick protocol, buckets, macro, decision record (+`evidence.md`); `sts2-pathing` route + Neow/ancient; `sts2-acts` pools, bag (+`encounters.md`); `sts2-mechanics` verified mechanics; `sts2-crystal-sphere` that event.
- 2 `sts2-<character>`: character deviations.
- 3 `sts2-<character>-act<N>`: character x act deviations, each with test + number.
- Below level 1 state only the deviation + evidence. A lesson valid for any character/act -> `sts2-strategy`. New characters get skills when first played.

## Evidence tags (every claim)
- `[code]` decomp source (cite file). `[sim]` solver/simulator measurement (cite script, n, margin). `[expert]` recorded expert run (cite); a choice, not proof. `[hyp]` untested; state its test. `python -m agent.improve lessons` lists every `[hyp]`.

## Update protocol
1. After each run / surprising decision: add or revise at the most specific level where it holds.
2. Evidence = `[code]`/`[sim]`/`[expert]`; else `[hyp]` + test. A run outcome is not evidence. Leave `[hyp]` only when a test supports it; delete/rewrite a contradicted claim.
3. Numbers (win rate, HP, n, margin) over adjectives.
4. Long tables -> sibling file. Run records -> `runs/`.
