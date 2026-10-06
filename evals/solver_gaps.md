# Solver gaps seen in live play (open)

Each entry: the observation, the hypothesis, the test that would confirm it. Close an entry with the test's result (or move it to the ledger).

## Potion timing (Speed Potion, seed SEEDSTUDY1 attempt 2, Vantom)
- Observed: the search scored "Speed Potion now" level with the best card on turns with no block card in hand and no attack coming, while a 30-damage Dismember was two turns away; on the Dismember turn itself it gave the same potion no extra preference.
- Hypothesis: the networks do not know when a potion is worth most (coverage: few training fights pair a given potion with a scheduled big-hit turn), so using it now and holding it look alike to the leaf evaluator.
- Tests: (1) count potion x encounter pairs in `target/train/train.json` (`tools/gen_train.py` mix) for monsters with a scheduled big hit (Vantom Dismember, Kaiser Laser, Byrdonis); (2) probe: for N states two turns before such a hit, compare the value net's V(after using the potion now) vs V(holding it) with a long-budget search's ground truth on the same states.

## Early-stop on ties
- Observed: `adv` stops after 4 rounds / 0.3 s when the top options are within the regret tolerance, also in manual (elite / boss) fights; the tie is often between lines a multi-turn plan separates (block now vs strip Slippery).
- Test: `agent.hindsight` with a large budget on the manual fights of this run: how often does the long search break the tie the same way as the plan written in the `-- why`?

## Self-damaging potions (Foul Potion)
- Observed: with two Foul Potions (12 damage to every creature, the player included) in the belt, the simulated Knowledge Demon fight won 0.19 vs 0.35 with an empty belt (192 attempts, se ~0.03): the search throws them and hurts itself. Held potions were also left in the boss fight of every table; fixed in `Harness._deck_raw` (held potions leave the priced snapshot).
- Test: per potion id, a boss fight with only that potion vs none; any potion that scores below "none" is misused by the policy / search.

## Unreproduced boss estimate (Knowledge Demon, floor 27 vs 28)
- Observed: `eval --boss --hp 66` gave 0.664 (Power + Vulnerable belt) and 0.652 with `potions=` on floor 27; on floor 28 the same deck gave 0.36-0.40 without potions and 0.58-0.60 with Power + Vulnerable, and floor number did not matter (direct `Engine.solve`). The floor-27 "no potions" figure is the outlier.
- Test: rerun a recorded `eval` event's spec from `runs/<run>/events.jsonl` with the logged deck; if it differs, find what in the scenario changed (relic props, hold, enchantments).
