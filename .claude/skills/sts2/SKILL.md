---
name: sts2
description: Use at the start of any Slay the Spire 2 run or session: root of the strategy book. Rules that never bend (no cheating), the skill hierarchy with when to load each, evidence tags, and the update protocol after a run.
---

# STS2 strategy book (root)

## Rules that never bend
- No cheating in a scored run: no dev console or god mode, no hidden state (draw order, RNG streams, the pre-rolled encounter and elite order, the seed's future), no restarts or reloads. Dev console only for harness tests. Never look for or reconstruct SPIRECAST2 / NaveGreed material.
- Allowed information: the screen, public game knowledge (pools, card text, monster patterns), what I observed this run.
- Goal: win the run; second, finish with the most HP.

## Hierarchy (general first, specific only adds or deviates)
| level | skill | holds | load |
|---|---|---|---|
| 0 | `sts2` | this file: rules, hierarchy, tags, update protocol | always, first |
| 1 | `sts2-harness` | commands, solver and `eval` use, speed, review loop | at run start |
| 1 | `sts2-strategy` | principles shared by every character and act: combat, macro, route, cards, potions, boss-first | at run start |
| 1 | `sts2-deckbuilding` | pick protocol (`reward` / `eval --smooth` numbers, then the judgment pass for what the numbers are blind to), five-bucket audit, what each fight asks, override log | at every card reward, shop, rest, relic |
| 1 | `sts2-pathing` | map route and Neow / ancient as one co-optimized decision; floors as a budget | at every Neow / ancient and before every map click |
| 1 | `sts2-acts` | act structure, encounter pools, ancients; `encounters.md`: what each boss/elite asks (shared reference) | when planning routes or fights |
| 1 | `sts2-mechanics` | verified monster / power / relic mechanics (shared reference) | before an unfamiliar enemy or power |
| 2 | `sts2-<character>` | only what differs for that character: archetypes, key cards, starting-deck facts | at character select |
| 3 | `sts2-<character>-act<N>` | only what differs for that character in that act: act-specific picks, elites, boss prep, distilled evidence (`runs.md` holds the run logs) | on entering the act (pre-plan at the previous boss) |

Rule for every file below level 1: do not restate a general rule; state the deviation ("here, X instead of the general rule because ..."), the additions, and the evidence. If a lesson holds for any character or act, promote it to `sts2-strategy` and delete it below. Other characters (Silent, Defect, Necrobinder, Regent) get `sts2-<character>` and `sts2-<character>-act<N>` when first played.

## Evidence tags (use on every claim)
- `[code]` read from the decompiled game source (cite file).
- `[sim]` measured with the solver / simulator (cite script, sample size, margin).
- `[expert]` observed in a recorded expert run (cite it); an expert's choice, not proof.
- `[played]` happened in one of my own runs (cite the run log).
- `[hyp]` hypothesis, not yet tested.

## Update protocol
1. After every run, and after every surprising decision, add or revise the entry at the most specific level where it holds (act, else character, else `sts2-strategy`).
2. A claim moves from `[hyp]` to a firmer tag only when a test supports it; say what the test was. Delete or rewrite a claim a test contradicts, and note it under "Revised".
3. Prefer measured numbers (win rate, HP lost, sample size, margin) over adjectives.
4. Keep each file short enough to read at the start of a run; long tables go in a sibling file in the same directory. Run logs go in `runs/`; skills hold distilled lessons.
