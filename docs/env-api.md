# Environment contract: choice space and information

The RL environment must expose exactly what a human player can do and see. This file is the contract; the tests in
`crates/sts2sim/tests/{cards,observe}.rs` enforce parts of it.

## Choice space (`Combat::legal_actions`, `Action`)

Dense action space of `ACTION_SPACE` indices (`Action::index` / `from_index`, mask via `Combat::action_mask`; 252 with `MAX_CREATURES = 12`,
always read the constant, never hard-code it):

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

Layout (sections in vector order; sizes are constants in `observe.rs`, `OBS_SIZE` is their sum, asserted in `observe()`):

| # | section | floats |
|---|---|---|
| 1 | global (round, side, phase flags, ...) | `GLOBAL_F` |
| 2 | player (HP, block, energy, stars, powers `(id+1, amount)` x `OBS_POWERS`) | `PLAYER_F` |
| 3 | relics `(id+1, counter)` | `RELIC_F` |
| 4 | potions | `POTION_F` |
| 5 | hand, ordered, `CARD_F` per slot | `MAX_HAND * CARD_F` |
| 6 | draw multiset, discard, exhaust `(id+1, upgrade)` x `OBS_MAX_PILE`, then the three pile sizes | `OBS_MAX_PILE*2*3 + 3` |
| 7 | enemies in list order | `OBS_MAX_ENEMIES * ENEMY_F` |
| 8 | pending decision (header + candidates, `CARD_F + selected` each) | `DECISION_F` |
| 9 | **Regent** (appended): current star cost of each hand card and of each decision candidate | `REGENT_F` |
| 10 | **Necrobinder** (appended): Osty present / alive / HP / max HP / powers, then the Osty-damage preview of each hand card | `OSTY_F` |
| 11 | **Defect** (appended): `MAX_ORBS` orbs `(kind+1, passive, evoke)` front first (empty = zeros; the slot count is `orb_slots` in the player block), then the number of Lightning orbs channeled this combat (Voltaic's text) | `ORBS_F` |

`CARD_F` = 12 per card: `id+1, upgrade, energy cost (-1 = X), playable, keyword bitset, enchantment id, damage preview,
block preview, counter[0], counter[1], enchantment amount, affliction id`.

Appended sections never move earlier ones: new characters/mechanics add a section at the END and a row here (keep `OBS_SIZE`
and the `debug_assert_eq!(w.i, OBS_SIZE)` consistent; `tests/observe.rs` checks hidden-information safety).

Regent block (`REGENT_F` floats): the current star cost of each hand card (`-1` none, `-2` X = all stars, otherwise the cost
with temporary / `Hook.ModifyStarCost` modifiers) and of each decision candidate. The player's stars are in the player block;
a Sovereign Blade's / Kingly Punch's grown damage is already in the per-card damage preview.

Necrobinder block (`OSTY_F` floats): Osty is a visible ally. Present / alive flags, HP / max HP and powers, followed by the
damage preview of each hand card with an Osty-damage variable (what the card text shows: Osty's own damage modifiers, not the
player's Strength / Weak). The block that absorbs damage aimed at Osty is the player's (see the player block). The diff
snapshot exposes the same data as the oracle's `pets` array.

Hidden (never in the observation): draw-pile order, all RNG stream states, monster-internal AI state beyond the
displayed intent. `observe::hidden_state_does_not_leak` perturbs these and asserts the vector is unchanged.

Assumptions to verify against the real UI via the oracle: discard/exhaust piles are shown in pile order; "known top
card" information (after put-on-top effects) is not tracked yet (a human would remember it).

## Episode outcomes and aborted episodes (`sts2env`, `sts2.VecEnv`)

`step` returns `done[i] = 1` when an episode ended; `outcome[i]` tells how (`sts2env::OUTCOME_*`, mirrored in `sts2.OUTCOME_*`):

| code | name | reward | meaning |
|---|---|---|---|
| 1 | `OUTCOME_WIN` | `win + hp_bonus * hp / max_hp` | victory |
| -1 | `OUTCOME_LOSS` | `loss` | defeat |
| 2 | `OUTCOME_TRUNCATED` | `step` only | hit `max_steps` |
| 3 | `OUTCOME_UNIMPLEMENTED` | `step` only | the fight touched content that is not ported (`Combat::missing`) |
| 4 | `OUTCOME_OVERFLOW` | `step` only | a fixed capacity of the simulator was exceeded and data was dropped (`Combat::overflow`) |

Codes 2-4 are **truncations**: bootstrap from the value of the last state, never treat them as win/loss. 3 and 4 mean the fight can no longer be
guaranteed faithful to the real game.

### Capacities (what can overflow, and what happens)
Nothing in the simulator panics or silently drops data when a fixed-capacity container is full. A full `ArrayVec` ignores the push and raises a
thread-local flag (`util::raise_overflow`); `Combat::step` (and `sync_overflow`, called by the env after `observe`) folds it into the sticky
`Combat::overflow` bitset (`state::ov::*`: `CONTAINER`, `CARDS`, `CREATURES`, `HISTORY`, `COUNTER`, `SCENARIO`). `sts2diff` reports it as a
simulator error, `BatchEnv` as `OUTCOME_OVERFLOW`. Invalid scenarios are errors (`Combat::try_new`, `ScenarioError`), never panics.

| resource | capacity | exceeded |
|---|---|---|
| card arena (deck + every generated card; cards are never recycled) | `MAX_CARDS` = 160 | `ov::CARDS` |
| deck at combat start | `MAX_DECK` = 80 | `ScenarioError::DeckTooLarge` |
| each pile | 160 (= arena) | cannot exceed the arena |
| hand | 10 (game rule; extra cards go to the discard pile) | - |
| creatures (player + Osty + enemies, incl. summons) | `MAX_CREATURES` = 12 (largest encounter starts with 4 enemies) | `ov::CREATURES` |
| powers per creature | `MAX_POWERS` = 16 | `ov::CONTAINER` |
| relics / potions / orbs | 24 / 4 / 10 | `ScenarioError` |
| hook listeners of one dispatch (snapshot) | 256 | `ov::CONTAINER` |
| per-attack results | 64 (`after_attack` listeners see the first result of every receiver, plus exact hit counters) | `ov::CONTAINER` |
| decision candidates | 64 (= `MAX_PICK`, the action space addresses `Pick{0..64}`) | `ov::CONTAINER` |
| selected cards of a decision / choice | 16 | `ov::CONTAINER` |
| history ring (this-turn / last-turn queries) | 128 entries of the queried kinds | `ov::HISTORY` when an entry of the current or previous player turn is overwritten |
| whole-combat counters (`hist_total` ...) | 65535 | `ov::COUNTER` |

Observation limits (the observation is a fixed-size window, not a state copy): 8 enemies, 16 powers per creature, 64 cards per pile list, 16
decision candidates (the decision header carries the true candidate count; `Pick{i}` can address up to 64; a Phrog Parasite prompt in the corpus
has 39). Beyond that the extra entries are simply not visible to the agent (the simulation itself is unaffected).

## Resetting in place
`Combat::reset(&Scenario)` / `reset_with` / `reset_validated` re-initialise an existing combat (no 17 KB construct-and-copy); `BatchEnv` resets finished
episodes through `reset_validated` with an allocation-free `ScenarioSource::pick`. A reset combat is bit-identical to `Combat::new` (tested, and
replayed against the oracle with `STS2DIFF_REUSE=1`).

## Throughput (release, random policy, observation + legal actions every step, `cargo run --release -p sts2env --example bench`)
~0.17M env-steps/s per core (1 thread), scaling linearly: 2.4M on an idle 14-core machine (0.95M measured while ~10 foreign cores were busy).
The observation (~40% of an env step) and the engine step (~30%) dominate; see `docs/design.md` "Hardening phase" for the breakdown.
