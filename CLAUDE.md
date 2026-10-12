# STS2 self-play harness
@SPEC.md

Claude plays Slay the Spire 2 via `python -m agent` (`README.md`). Only this file (and `SPEC.md`) is guaranteed loaded; the operator book is `.claude/skills/` (Skill tool). Skills change between runs: never play from memory of an earlier session.

## Rule 0: no game action before the governing skills are loaded and read
At session start and after any compaction/clear, invoke in order: 1 `sts2`, 2 `sts2-harness`, 3 `sts2-plan`. Read them before acting.
Enforced: hooks (`.claude/settings.json`) refuse game-acting harness commands (`a`, `loot`, `potion`, `turn`, `combat`, `x`, `draw`, direct bridge access) until all three are loaded this session; the daemon refuses by screen (table). Read-only always works (`agent/skillgate.py` READ_ONLY). `REFUSED: skills not loaded in this session: X` (daemon) or `REFUSED: no game action before the governing skills are loaded. Missing in this session: X` (hook) -> invoke X, read, repeat.

## Progressive loading (harness demands the skill when the screen needs it)
| screen / moment | skill first |
|---|---|
| character chosen (`IRONCLAD` in header) | `sts2-<character>` |
| inside an act (`A1`/`A2`/`A3` in header) | `sts2-<character>-act<N>` (pre-plan at the previous boss) |
| Neow / ancient (floor 1), any map | `sts2-pathing` |
| card reward, rewards, shop, rest, upgrade/removal/transform | `sts2-deckbuilding` |
| event after floor 1 | `sts2-mechanics` |
| combat | `sts2-acts` (+ `encounters.md`) |
| after a run (the harness refuses a new run until reviewed) | `sts2-improve` |
| not enforced: unfamiliar enemy/power/relic | `sts2-mechanics` |

More specific skill wins; lessons for every character/act go to level 1 (`sts2` hierarchy).

## Meta rules
- Skills are rules with IDs, tags and tests (`sts2`), never a journal. Evidence -> `docs/research/evidence.md`, per-rule stats -> `evals/rules/ledger.jsonl`, history -> git.
- Docs and skills are written for AI readers: minimal tokens, current state only.
- Plan and operations: `docs/plan.md`. Accuracy gate for simulator/search changes: `bash tools/gate.sh`.
- Burn-off procedure -> skill `burn-off`.

## Humans and tests
Gate off: daemon started with `STS2_SKILL_GATE=off` (own terminal, never an agent session), and scripts building `Harness()` directly (sweeps, benches). Design: `agent/skillgate.py`.
