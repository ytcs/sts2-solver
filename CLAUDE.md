# STS2 self-play harness

This repo lets Claude play Slay the Spire 2 through `python -m agent` (reference: `README.md`). This file is the only thing every session is guaranteed to see. The strategy book is in `.claude/skills/` and loads through the Skill tool, in the order below. Do not play from memory of an earlier session: skills change between runs.

## Rule 0: no game action before the governing skills are loaded and read
Before the first game action of a session (and again after any context compaction or clear, which forgets what you loaded), invoke with the Skill tool, in this order:
1. `sts2` (governing: rules that never bend, the hierarchy, evidence tags, update protocol)
2. `sts2-harness` (the commands, batching, what `REFUSED` means)
3. `sts2-strategy` (principles and the decision loop)

Then read what they say before acting. This is enforced, not advisory: hooks (`.claude/settings.json`) refuse any harness command that acts on the game (`a`, `turn`, `combat`, `x`, `draw`, `f`, direct bridge access) until all three are loaded in this session, and the harness daemon refuses by screen (below). Looking is always allowed: `s`, `brief`, `m`, `d`, `p`, `eval`, `reward`, `route`, `adv`, `relics`, `status`. Never try to bypass the gate (the hooks refuse commands that touch it); when a command comes back `REFUSED: skills not loaded: X`, invoke X, read it, repeat the command.

## Progressive loading: the harness demands each skill when its screen needs it
| screen / moment | skill the harness requires first |
|---|---|
| character chosen (`IRONCLAD` in the header) | `sts2-<character>` (e.g. `sts2-ironclad`) |
| inside an act (`A1`, `A2`, `A3` in the header) | `sts2-<character>-act<N>` (pre-plan it at the previous boss) |
| Neow / ancient choice (floor 1), any map screen | `sts2-pathing` |
| card reward, rewards, shop, rest, upgrade / removal / transform | `sts2-deckbuilding` (then `evidence.md` only to question a rule) |
| an event after floor 1 | `sts2-mechanics` |
| not enforced, load when it applies: planning fights ahead (pools, ancients, the double boss, what each boss asks) | `sts2-acts` and its `encounters.md` |
| not enforced: an unfamiliar enemy, power or relic | `sts2-mechanics` |
| not enforced: after a run | the `sts2` update protocol, then `python -m agent.improve review` (`sts2-harness`, "Review loop") |

Lower levels only add to or deviate from the general ones; if two skills disagree, the more specific one wins, and a lesson that holds for every character or act belongs in `sts2-strategy`.

## For humans and tests
The gate is off for a daemon started with `STS2_SKILL_GATE=off` (set it yourself in your terminal, never from an agent session) and for any script that builds `Harness()` directly (sweeps, calibration, benches). Design and rationale: `agent/skillgate.py`.
