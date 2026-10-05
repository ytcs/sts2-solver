---
name: sts2-index
description: Index of the Slay the Spire 2 strategy book (skills by topic, character and act) and the protocol for updating it. Load first when planning or playing any STS2 run, or when asked what the book contains.
---

# STS2 strategy book: index

Goal: win the run first, then win it with the most HP left. The book is the persistent memory of what has been learned about playing STS2 well. It is a collection of skills, one directory each under `.claude/skills/`, indexed here by topic, character and act.

| skill | covers | status |
|---|---|---|
| `sts2-core` | objective, rules, commands, solver budget, eval limits, route and card-pick guidance, review loop | played one shakedown run |
| `sts2-mechanics` | verified monster / power / relic mechanics that change how turns are played | seeded from code and play |
| `sts2-acts` | act structure, encounter pools, weak/regular/elite lists, bosses, ancients, hidden vs visible information | from the game code and harness tests |
| `sts2-ironclad` | Ironclad principles, card/relic notes, archetypes | empty framework |
| `sts2-ironclad-act1` | Ironclad Act 1 (Overgrowth / Underdocks) | empty framework |
| `sts2-ironclad-act2` | Ironclad Act 2 (Hive) | empty framework |
| `sts2-ironclad-act3` | Ironclad Act 3 (Glory) incl. the second boss at A10 | empty framework |

Other characters (Silent, Defect, Necrobinder, Regent) get `sts2-<character>` and `sts2-<character>-act<N>` when first played.

## Evidence tags (use on every claim)
- `[code]` read from the decompiled game source (cite file).
- `[sim]` measured with the solver / simulator (cite script, sample size, margin).
- `[expert]` observed in a recorded expert run (cite it); an expert's choice, not proof.
- `[played]` happened in one of my own runs (cite run log).
- `[hyp]` hypothesis, not yet tested.

## Update protocol
1. After every run, and after every non-trivial decision that turned out surprising, add or revise an entry in the most specific skill (character + act, else character, else core).
2. A claim only moves from `[hyp]` to a firmer tag when a test supports it. Say what the test was. Delete or rewrite a claim a test contradicts, and note the contradiction under "Revised".
3. Prefer measured numbers (win rate, HP lost, sample size, margin) over adjectives.
4. Keep each SKILL.md short enough to read at the start of a run. Move long tables to a sibling file in the same directory and link it.
5. Run logs go in `runs/` (not in skills); skills hold the distilled lessons.
