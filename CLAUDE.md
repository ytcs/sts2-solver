# STS2 self-play harness

Claude plays Slay the Spire 2 through `python -m agent` (reference: `README.md`). This file is the only one every session is guaranteed to see; the strategy book is `.claude/skills/`, loaded with the Skill tool. Skills change between runs: never play from memory of an earlier session.

## Rule 0: no game action before the governing skills are loaded and read
Before the first game action of a session, and again after any compaction or clear, invoke with the Skill tool, in order:
1. `sts2` (rules that never bend, hierarchy, evidence tags, update protocol)
2. `sts2-harness` (commands, batching, what `REFUSED` means)
3. `sts2-strategy` (principles and the decision loop)

Read them before acting. Enforced: hooks (`.claude/settings.json`) refuse any harness command that acts on the game (`a`, `turn`, `combat`, `x`, `draw`, `f`, direct bridge access) until all three are loaded in this session, and the harness daemon refuses by screen (table below). Read-only commands always work: `s`, `brief`, `m`, `d`, `p`, `eval`, `reward`, `route`, `adv`, `relics`, `status`. Never bypass the gate. `REFUSED: skills not loaded: X` means: invoke X, read it, repeat the command.

## Progressive loading: the harness demands each skill when its screen needs it
| screen / moment | skill required first |
|---|---|
| character chosen (`IRONCLAD` in the header) | `sts2-<character>` (e.g. `sts2-ironclad`) |
| inside an act (`A1`, `A2`, `A3` in the header) | `sts2-<character>-act<N>` (pre-plan it at the previous boss) |
| Neow / ancient choice (floor 1), any map screen | `sts2-pathing` |
| card reward, rewards, shop, rest, upgrade / removal / transform | `sts2-deckbuilding` (`evidence.md` only to question a rule) |
| an event after floor 1 | `sts2-mechanics` |
| not enforced: planning fights ahead (pools, the double boss, what each boss asks) | `sts2-acts` and its `encounters.md` |
| not enforced: an unfamiliar enemy, power or relic | `sts2-mechanics` |
| not enforced: after a run | the `sts2` update protocol, then `python -m agent.improve review` (`sts2-harness`, "Review loop") |

Lower levels only add to or deviate from the general ones; the more specific skill wins. A lesson for every character or act belongs in `sts2-strategy`.

**Meta rule: never use SKILL files as a journal.** A skill is a mutable set of rules (rules, mechanics, tests, status tags) and carries no history: no runs, past fights, examples from play, logs or TODOs. Evidence and history go to `runs/<run>/` and `evals/` (details: `sts2`, "Rules that never bend").

## For humans and tests
The gate is off for a daemon started with `STS2_SKILL_GATE=off` (your own terminal, never an agent session) and for scripts that build `Harness()` directly (sweeps, calibration, benches). Design: `agent/skillgate.py`.
