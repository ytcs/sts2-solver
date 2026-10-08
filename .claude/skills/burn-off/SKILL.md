---
name: burn-off
description: Repo burn-off procedure. Behaviour-preserving purge of everything not load-bearing, to minimise codebase entropy and token spend. Use when the user asks for a burn-off, or after a batch of rebuild stages lands.
---

# burn-off

Goal: lowest practical entropy. Keep only what is load-bearing; performance and output accuracy must not change.

## Preconditions
- All subagents and running jobs are finished. Plan nothing before that.
- Consolidate every branch and worktree into `master`; delete merged branches (local and remote) and worktrees.
- `bash tools/gate.sh` passes on master. That is the baseline.

## Manifest (read-only, before any change)
- Classify every tracked file as KEEP / GATE / COMPRESS / DELETE / UNCERTAIN, from its references (imports, executed scripts, data reads, skills, CLAUDE.md, hooks, Cargo, models/current.json) and from the plan's target layout.
- Load-bearing:
  - live harness reachable from `python -m agent` (including skill-referenced commands), the game bridge mod, the skill gate and hooks;
  - simulator, search and bindings;
  - code for in-progress or planned studies;
  - data the kept code reads;
  - the plan, the evidence record, the game-bug tracker;
  - the gate.
- Also list: dead code in kept files (file:line), harness commands or skill sections that break if a module goes, Python->Rust candidates.
- Send the user one message: the counts, the UNCERTAIN questions, a recommended answer for each. Wait for the reply.

## Phases (one commit and push each; gate after each)
1. Delete files that need no code change. Then build, import-smoke every kept module, run the harness tests, run the gate.
2. Simplify code: dead functions, flags and branches; the features the user ablated; fold one-use helpers. No numeric changes, no Python->Rust ports.
3. Compress docs and skills to a current-state snapshot (details below). Keep every name and string the code or hooks check.
4. Comments: extremely scarce; code is self-documenting; the readers are AI. Keep one terse line only for a non-obvious invariant whose violation would silently break correctness (SAFETY, the information contract, cache-key exactness, ordering requirements). Prefer renaming.
5. Last: run the harness tests one final time, then delete the remaining tests outside the gate. The user's active work (e.g. tools/dashboard and its test) stays untouched.

## Docs and skills style
- Skills: terse imperative rules, each with its tag ([code]/[sim]/[expert]/[hyp] plus test) and number. No why unless the why is a rule. No history, no examples from play. Aim for about a third of the size.
- Plan: one file, current state, each stage as goal / status / gate.
- Evidence: each E# in 1-3 lines, finding plus numbers.
- No reference may point at a deleted file, command or option (grep for each).

## Parallelism
- One manifest agent (read-only).
- Then non-overlapping worktree agents: code (all of crates + agent/rl/tools; Rust and Python change together) and docs/skills.
- Merge in phase order, running the gate after each merge.

## After
- Performance pass on a quiet GPU: profile, then port to Rust only what is hot. It must stay bit-identical under the gate, or show statistical equivalence for numerics changes.
- Then resume the studies.
