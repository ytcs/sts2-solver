# Spec
- Goal: win Slay the Spire 2 at Ascension 10 with every character at expert-human level (~90%+ of runs; every death is an avoidable error until shown otherwise). Ironclad first.
- Player: the harness (simulator, search, predictors, calculators) operated by an LLM agent. The search plays fights; the operator owns macro decisions and commits potions. Scripted policies are instruments, not the player.
- Fair play: a scored run uses only what a human sees: the screen, public game knowledge (game code, pools, move patterns, odds) and what the run has revealed. The random state and everything derived from it but not yet shown is hidden. No console, reload, forks or logs of the seed. Learned components are judged on seeds disjoint from their training seeds.
- Currency: every decision is priced by P(win the run).
- Self-improvement: rules (skills), models and tools change only on evidence through gates, recursively, including the skills themselves.
- Claims: say exactly what was measured; our solver's results are lower bounds, not verdicts on plans; no blanket bans.
- Records: current state only in files (skills = rules, `docs/plan.md` = plan and operations, `docs/research/evidence.md` = evidence, `docs/game-bugs.md` = game bugs for its developer); history in git.
- Code: one branch, lowest practical entropy, written for AI readers, comments only for silent-correctness invariants, Rust where it pays. Game binaries are never committed or published.
- Autonomy: the agent drives the plan; the user steers.
