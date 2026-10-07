# Operator protocol (draft for the S7 skills)

Status: draft, 2026-10-06. It becomes the operator skill once the predictor passes S3. Until then the old skills govern live play.

## Principles
1. **One currency.** Decisions maximise P(win the run). When that is flat for every option (a deck far from what lies ahead), use the next horizon: P(clear this act), then floors reached. Then use a measured game plan (S6). Never use the operator's own unmeasured intuition.
2. **The predictor is the authority on fights.** Question it only with a reason the model can't see (a mechanic it lacks, a desync, a state outside its training mix), and log that reason as a gap.
3. **Only the exact random state is hidden.** Use everything that follows from the game source and what this run showed: move patterns, pool narrowing, potion and rarity odds (the `public odds` line), shop prices.
4. **Every disagreement is a gap.** When the operator overrides a number, or a real outcome lands in the predicted tail, log it typed (fidelity / calibration slice / missing model / tool). Gaps are the learning loop's input.

## Per screen

| Screen | Do | Tool |
|---|---|---|
| Neow / ancient | Choose the run's or act's plan: `plans` for this character and the known boss. Price the options; for relics without a modelled macro effect, add their effect by judgment and log the gap. | `plans`, `price` |
| Map | `price` at every fork; take the best option at the longest non-flat horizon. Re-price when HP, gold or the deck changed. | `price` |
| Card reward | `price`; if every option is flat, take the card the chosen plan names, else skip. | `price`, `plans` |
| Shop | `price` (singles); price the top two or three as a basket by buying one and re-pricing. | `price` |
| Rest | `price` (rest vs each smith). | `price` |
| Event | Read the options in the catalog (`data/events.json`); `price` once events are wired. An unmodelled option: decide by judgment and log the gap. | `price` |
| Combat | `combat` / `turn`. The solver plays and stops at a potion proposal (use now / keep / save, each priced). Commit a potion by hand (`a <i>`) or continue. `adv` on a pivotal turn. | `combat`, `turn`, `adv` |

## After a run
`python -m agent.improve review` (S7 replaces it with the gap review):
- predicted vs real fights (calibration, the tail share);
- `price` decisions vs what happened;
- plan choices vs their tables;
- the gap list.

Each gap becomes a fix (fidelity), training data (calibration slice), a model addition (events, relics) or a tool change, adopted only through its stage's gate.
