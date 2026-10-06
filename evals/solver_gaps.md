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

## Relic-input sensitivity (the network plays differently with an irrelevant relic)
- Observed: vs Knowledge Demon at 75 HP (192 attempts, se ~0.036), adding Lava Lamp (+0.29 to +0.32) or Planisphere (+0.28), relics with no combat effect, raised the win from ~0.55-0.58 to ~0.85-0.88; Meal Ticket and Golden Idol moved nothing. The rules are unchanged: the combat start snapshot is identical and fixed random action sequences play out identically with and without them, so the change is in the networks' response to the relic input.
- Consequences: relic prices from `eval` are unreliable for this deck (Sturdy Clamp read +0.40 raw, +0.11 against a no-op relic control); the network's own play in this fight is far below what the same deck can do.
- Tests: (1) for a set of decks and fights, the win with each non-combat relic added; a spread beyond 2 se means relic embeddings leak into play; (2) check which relic ids the training mix covers (`tools/gen_train.py`), and whether these two ids share features with a combat relic; (3) a control relic in every relic eval until fixed.

## FIX: Knowledge Demon curse choice can be skipped (cheating; human must pick)
- The bridge exposes the Curse of Knowledge screen as `SELECT 0-1` (`a -` = none) and the sim port calls `ask_options(..., can_skip = true)` (`crates/sts2sim/src/content/monsters/hive_b.rs`, `curse_of_knowledge`). A human must take Disintegration or the partner curse. Fix both: the sim (no skip) and the bridge's minimum for this screen (`mods/AgentBridge/src/Decisions.cs`), and re-check every `ask_options(..., true)` against the game's `CardSelectCmd` call for a can-skip flag.
- Effect: every eval / fight prediction vs Knowledge Demon assumed no curse (predicted 1.00 at 68 HP).

## FIX: fake merchant event not handled by the bridge
- Act 3 unknown room "The Merchant???" showed `Placeholder` with only Proceed: the bridge has no case for this event (a fake merchant shop), so its shop was skipped entirely. Add the event's screen to `mods/AgentBridge/src/Decisions.cs` (and its rules to the sim / `sts2-mechanics` once read from the game code).

## Missed lethal (Turret Operator, 16 HP, Mangle in hand)
- Observed: hand Mangle (3 cost, 20 damage) with 6 energy vs a lone Turret Operator at 16 HP about to attack 6x5; the solver (via `turn !`) played other cards, took 4 HP and needed another turn.
- Test: from logged fights, every state where some single card or 2-card line kills the last enemy; how often the solver's first choice reaches lethal this turn. Expected ~100%.
