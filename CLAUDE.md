# STS2 self-play harness

This repo lets Claude play Slay the Spire 2 through `python -m agent` (reference: `README.md`). The strategy book is in `.claude/skills/`; this file only tells you when to open which part, so invoke each skill (Skill tool) at the moment it applies instead of reading them all up front.

**At the start of any run:** invoke `sts2` (rules that never bend, the hierarchy, evidence tags), then `sts2-harness` (commands) and `sts2-strategy` (principles and the decision loop).

| moment | invoke |
|---|---|
| Neow / ancient choice, any map click, planning a route | `sts2-pathing` |
| card reward, shop, rest site, relic, upgrade, removal, transform | `sts2-deckbuilding` |
| the character is chosen | `sts2-<character>` (e.g. `sts2-ironclad`) |
| entering an act (and pre-planning it at the previous boss) | `sts2-<character>-act<N>` |
| planning fights ahead: pools, ancients, the double boss, what each boss asks | `sts2-acts` (and its `encounters.md`) |
| an unfamiliar enemy, power or relic | `sts2-mechanics` |
| after a run | the `sts2` update protocol, then `python -m agent.improve review` |

Lower levels only add to or deviate from the general ones; if two skills disagree, the more specific one wins, and a lesson that holds for every character or act belongs in `sts2-strategy`.
