---
name: sts2
description: Use at the start of any Slay the Spire 2 run or session: root of the strategy book. Rules that never bend (no cheating), the skill hierarchy, evidence tags, and the update protocol after a run.
---

# STS2 strategy book (root)

Skill loading is enforced by hooks and the harness; the screen-to-skill table is in `CLAUDE.md`.

## Rules that never bend
- No cheating in a scored run: no dev console or god mode, no hidden state (draw order, RNG streams, the pre-rolled encounter and elite order, the seed's future), no restarts or reloads. Dev console only for harness tests. Never look for or reconstruct SPIRECAST2 / NaveGreed material.
- Allowed information: the screen, public game knowledge (pools, card text, monster patterns), what I observed this run.
- Goal: win the run; second, finish with the most HP.

## Hierarchy (general first; lower levels only add or deviate)
- 0 `sts2`: rules, hierarchy, tags, update protocol.
- 1 `sts2-harness`: commands, batching, solver and `eval` use, quirks, review loop.
- 1 `sts2-strategy`: decision loop and targets shared by every character and act.
- 1 `sts2-deckbuilding`: pick protocol (`reward` / `eval --smooth`, then the blind-spot pass), five buckets, plan and switch, macro choices, override log; `evidence.md`.
- 1 `sts2-pathing`: map route and Neow / ancient as one decision; floors as a budget; HP gates.
- 1 `sts2-acts`: act structure, encounter pools, bag mechanics; `encounters.md`: what each boss / elite asks, target decks, potion numbers.
- 1 `sts2-mechanics`: verified monster / power / relic / event mechanics.
- 2 `sts2-<character>`: only what differs for that character.
- 3 `sts2-<character>-act<N>`: only what differs for that character in that act, each with its test and number.

Below level 1 state the deviation and its evidence, not the general rule; a lesson that holds for any character or act moves to `sts2-strategy`. Other characters (Silent, Defect, Necrobinder, Regent) get their skills when first played.

## Evidence tags (on every claim)
- `[code]` read from the decompiled game source (cite file).
- `[sim]` measured with the solver / simulator (cite script, sample size, margin).
- `[expert]` observed in a recorded expert run (cite it); a choice, not proof.
- `[played]` happened in one of my runs (cite the run).
- `[hyp]` untested; write its test. `python -m agent.improve lessons` lists every `[hyp]` line.

## Update protocol
1. After every run and every surprising decision, add or revise the entry at the most specific level where it holds (act, else character, else `sts2-strategy`).
2. A claim leaves `[hyp]` only when a test supports it; say which. Delete or rewrite a claim a test contradicts and note it under "Revised".
3. Prefer measured numbers (win rate, HP lost, sample size, margin) over adjectives.
4. Long tables go in a sibling file. Run records live in the repo's `runs/` directory; the book holds distilled lessons only.
