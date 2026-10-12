---
name: sts2-improve
description: Use after every run (the harness refuses newrun until the previous run is reviewed) and every 5 runs for the meta-review: score cited decisions against the instruments, update the rule ledger, promote / narrow / delete rules on evidence, turn new: claims into candidate rules, and review this loop itself.
---

# Self-improvement loop (operator book)
Rules are improved on decision-level evidence; one run cannot move a win rate. Forks (`Fork.cs`) continue with the scripted policy, not yours: offline, rare, budgeted, confound stated; never during a scored run.

## After every run
1. `python -m agent.improve review` -> `runs/<run>/review.md`: outcome, death attribution, and per cited macro decision its rule IDs, the instrument logged on that screen (`price`, `routes`, `restcalc`, `eval`), the instrument's best option vs yours and the margin; `new:` claims listed.
2. The review appends one row per decision to `evals/rules/ledger.jsonl` (rules, `new:`, instrument, best vs chosen, agreement, margin, run outcome); `python -m agent.improve rules` prints per rule: citations, runs, scored, agreed, mean margin when you disagreed. Margins: `price` its ranked rung; `routes` P(win boss), so an elite's margin is its boss-win cost; `restcalc` the rest gain.
3. Death attribution: combat (`python -m agent.hindsight <fight> --log`: luck or a search gap) vs macro (the decisions that set the death up: pair strength, arrival HP, potions); encounter patterns -> `sts2-acts/encounters.md`.
4. Change rules (I1-I4), each edit citing the ledger or an E#; commit skill + ledger together.

## Meta-rules
- I1 One change per rule per run; a rule changes only on its own test's evidence.
- I2 Promote `[hyp]` -> `[sim]` when its instrument test agrees on >= 5 citations with no disagreement beyond 2 se; narrow or rewrite when it disagrees on >= 2 decisions beyond 2 se; a rule contradicted by an E# measurement is rewritten that run.
- I3 A `[hyp]` rule with no checkable test after K = 10 citations is deleted; a rule never cited in 10 runs is deleted unless it is `[code]`.
- I4 A `new:` claim the instrument agreed with becomes a `[hyp]` rule with a test in the most specific skill; one the instrument contradicted is recorded in the ledger only.
- I5 Every 5 runs (meta-review): decision agreement and margin trend, rules cited vs unused, decisions with no rule; whether this loop changed anything that later held. Propose changes to the architecture (skills, IDs, tests, this skill) in `docs/plan.md` and apply them: this skill is under the same rules.
- I7 Migration: a rule the instrument agrees with on >= 10 citations becomes a harness default (`docs/plan.md` split) and leaves the skill; operator overrides of an instrument that hold (outcome, or `eval` on the realised deck) go to the instrument (calibration, horizon), not to the rule.
- I6 Instrument confidence decides how hard a disagreement counts: deck strength vs a named boss (validated, Spearman 0.73 vs live boss wins) > `routes` > `price` far rungs (wrong sign from act-1 decks, E86) > the learned value heads (no skill on unseen seeds, E95).
