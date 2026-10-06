# Post-run review backlog (open items to work through after the current run)

- **Upgrade debt.** Some cards gain far more from an upgrade than others (cost drop, a keyword, a doubled effect); a deck holding several of them un-upgraded carries debt that smith decisions should price. Study: per card in the deck, `eval --smooth --boss --next --v "up|upgrade=ID"` divided by its draw frequency; rank by gain per smith; check whether rest-vs-smith (`sts2-deckbuilding` section 6) should compare the rest against the summed debt rather than one upgrade.
- **RL curriculum design for the solver** (discussion with the user): training-set coverage (potion x scheduled-big-hit encounters, potion timing), encounter and deck mix, HP-worth curves, hold rules.
- Potion and solver findings of this run: `evals/solver_gaps.md`.
