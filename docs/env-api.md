# Environment contract: choice space and information

The RL environment must expose exactly what a human player can do and see. This file is the contract; the tests in
`crates/sts2sim/tests/{cards,observe}.rs` enforce parts of it.

## Choice space (`Combat::legal_actions`, `Action`)

Dense action space of `ACTION_SPACE` indices (`Action::index` / `from_index`, mask via `Combat::action_mask`):

| Action | Legal when | Notes |
|---|---|---|
| `PlayCard{hand_pos, target}` | play phase, `can_play` (energy/star cost incl. cost modifiers, `ShouldPlay` vetoes, card logic), valid target | one action per living enemy for single-target cards; `target = NO` otherwise |
| `UsePotion{slot, target}` | play phase, potion usable (`CombatOnly`/`AnyTime`; `Automatic` ones never) | self-targeted potions take no target (the game auto-targets the player); enemy-targeted potions need an enemy |
| `DiscardPotion{slot}` | play phase, slot filled | |
| `EndTurn` | play phase | |
| `Pick{idx}` / `Confirm` | a decision is pending | mirrors the UI, see below |

Cards that are unplayable are **not** listed (a human cannot click them).

### Decisions (card select screens)
Any effect may suspend for a `Decision` (`Combat::decision`): choose `min..=max` of an ordered candidate list.
* Hand selections list candidates in hand order; **draw-pile selections are presented sorted by (rarity, id)** so the
  hidden order does not leak (matches the game's pile screen / `CardSelectCmd`); discard/exhaust in pile order;
  "choose a card" (Discovery-style) screens list the generated cards.
* Forced choices auto-resolve with no decision (`|candidates| <= min` when `min == max`; empty list).
* `Pick` toggles a candidate; when `max` are selected the most recent selection is replaced (hand-UI behaviour).
  The decision completes by itself once `max` are selected unless `confirm_required` (`min != max`); `Confirm`
  finishes the selection (or skips when nothing is selected and the screen is skippable).
* Result order = click order (this is what the real hand UI returns; matters for put-back-on-top effects).

## Information (`Combat::observe`, flat `f32`, `OBS_SIZE`)
Visible to the agent: HP / max HP / block / energy / stars / powers, relics + counters, potions, hand in order with
current cost, playability, keywords and damage/block previews, draw pile **as an unordered multiset** (sorted by
rarity/id like the pile screen), discard and exhaust in pile order, every enemy's HP/block/powers, current intent(s)
(type, per-hit damage computed with the same modifiers the UI uses, hit count) and its last four performed moves
(the pattern history a player has seen), per-turn play counters, and any pending decision with its candidates.

Hidden (never in the observation): draw-pile order, all RNG stream states, monster-internal AI state beyond the
displayed intent. `observe::hidden_state_does_not_leak` perturbs these and asserts the vector is unchanged.

Assumptions to verify against the real UI via the oracle: discard/exhaust piles are shown in pile order; "known top
card" information (after put-on-top effects) is not tracked yet (a human would remember it).

## Throughput (this machine, release, random policy, observation + legal actions every step)
~0.26M steps/s/thread, ~2M steps/s on 14 threads (observation encoding is the main cost; room to optimise).
