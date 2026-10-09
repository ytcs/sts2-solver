# STS2 self-play harness
Claude plays Slay the Spire 2 via `python -m agent` (`README.md`). Only this file is guaranteed loaded; the strategy book is `.claude/skills/` (Skill tool). Skills change between runs: never play from memory of an earlier session.

## Rule 0: no game action before the governing skills are loaded and read
At session start and after any compaction/clear, invoke in order: 1 `sts2`, 2 `sts2-harness`, 3 `sts2-strategy`. Read them before acting.
Enforced: hooks (`.claude/settings.json`) refuse game-acting harness commands (`a`, `turn`, `combat`, `x`, `draw`, `f`, direct bridge access) until all three are loaded this session; the daemon refuses by screen (table). Read-only always works: `s`, `brief`, `m`, `d`, `p`, `eval`, `reward`, `route`, `adv`, `relics`, `status`. Never bypass. `REFUSED: skills not loaded in this session: X` (daemon) or `REFUSED: no game action before the governing skills are loaded. Missing in this session: X` (hook) -> invoke X, read, repeat.

## Progressive loading (harness demands the skill when the screen needs it)
| screen / moment | skill first |
|---|---|
| character chosen (`IRONCLAD` in header) | `sts2-<character>` |
| inside an act (`A1`/`A2`/`A3` in header) | `sts2-<character>-act<N>` (pre-plan at the previous boss) |
| Neow / ancient (floor 1), any map | `sts2-pathing` |
| card reward, rewards, shop, rest, upgrade/removal/transform | `sts2-deckbuilding` (`evidence.md` only to question a rule) |
| event after floor 1 | `sts2-mechanics` |
| not enforced: planning fights (pools, double boss, boss asks) | `sts2-acts` + `encounters.md` |
| not enforced: unfamiliar enemy/power/relic | `sts2-mechanics` |
| not enforced: after a run | `sts2` update protocol, then `python -m agent.improve review` (`sts2-harness` "Review loop") |

More specific skill wins; lessons for every character/act go to `sts2-strategy`.

## Meta rules
- Never use SKILL files as a journal: a skill is mutable rules/mechanics/tests with status tags; no runs, fights, play examples, logs, TODOs. Evidence/history -> `runs/<run>/`, `evals/`, git (details: `sts2` "Rules that never bend").
- Docs and skills are written for AI readers: minimal tokens, current state only, no explanation the code or the rule already conveys.
- Plan: `docs/rebuild.md`. Accuracy gate for simulator/search changes: `bash tools/gate.sh`.
- Burn-off procedure -> skill `burn-off` (after a batch of rebuild stages lands, or when the user asks).

## Humans and tests
Gate off: daemon started with `STS2_SKILL_GATE=off` (own terminal, never an agent session), and scripts building `Harness()` directly (sweeps, benches). Design: `agent/skillgate.py`.
