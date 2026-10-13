---
name: sts2-improve
description: Use after every run (the harness refuses newrun until the previous run is reviewed) and every 5 runs for the meta-review: score cited decisions against the instruments, update the rule ledger, promote / narrow / delete rules on evidence, turn new: claims into candidate rules, and review this loop itself.
---

# Self-improvement loop (operator book)
Rules change on decision-level evidence; one run cannot move a win rate. Forks (`Fork.cs`) continue with the scripted policy, not yours: offline, rare, budgeted, confound stated; never during a scored run.

## After every run
1. `python -m agent.improve review` -> `runs/<run>/review.md`: outcome, death attribution, and per cited macro decision its rule IDs, the instrument logged on that screen (`price`, `routes`, `restcalc`, `eval`), its best option vs yours and the margin; `new:` claims listed.
2. The review appends one row per decision to `evals/rules/ledger.jsonl`; `python -m agent.improve rules` prints per rule: citations, runs, scored, agreed, mean margin when you disagreed. Margins: `price` its ranked rung; `routes` P(win boss) (an elite's margin is its boss-win cost); `restcalc` the rest gain.
3. Death attribution: combat (`python -m agent.hindsight <fight> --log`: luck or a search gap) vs macro (the decisions that set it up: pair strength, arrival HP, potions); encounter gimmicks -> `sts2-acts/encounters.md`.
4. Change rules (I1-I4), each edit citing the ledger or an E#; commit skill + ledger together. Skills get the rule, never the run story; numbers go to `docs/research/evidence.md`.

## Meta-rules
- I1 One change per rule per run; a rule changes only on its own test's evidence. A lapse that recurs after its rule was rewritten goes to a harness guard or readout (`docs/plan.md`), not another rewrite (E145).
- I2 Promote `[hyp]` -> `[sim]` when its instrument test agrees on >= 5 citations with no disagreement beyond 2 se; narrow or rewrite when it disagrees on >= 2 decisions beyond 2 se; a rule contradicted by an E# measurement is rewritten that run. A row counts for a rule only when the scored instrument is its test's (`routes` models no shops or events; `price` sees no gold, M12); a choice that breaks the rule it cites is a lapse, not evidence.
- I3 A `[hyp]` rule with no checkable test after K = 10 citations is deleted; a rule never cited in 10 runs is deleted unless it is `[code]` or a fight rule (F#: the ledger holds macro whys only).
- I4 A `new:` claim the instrument agreed with becomes a `[hyp]` rule with a test in the most specific skill; one the instrument contradicted is recorded in the ledger only.
- I5 Every 5 runs (meta-review): decision agreement and margin trend, rules cited vs unused, decisions with no rule; whether this loop changed anything that later held. Propose architecture changes (skills, IDs, tests, this skill) in `docs/plan.md` and apply them: this skill is under the same rules.
- I6 Instrument confidence decides how hard a disagreement counts: deck strength vs a named boss (E70) > `routes` > `price` far rungs (E86) > the learned value heads (E95).
- I7 Migration: a rule the instrument agrees with on >= 10 citations becomes a harness default (`docs/plan.md`) and leaves the skill; operator overrides of an instrument that hold (outcome, or `eval` on the realised deck) go to the instrument (calibration, horizon), not to the rule.
