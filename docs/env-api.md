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
* Hand selections list candidates in hand order (the hand is displayed in order); **every pile selection (draw, discard,
  exhaust) is presented in a canonical order sorted by what a player can see of each card** (rarity, id, upgrade, cost,
  enchantment, affliction; ties by game order), so the hidden pile order never leaks through a selection screen.
  `Action::Pick{idx}` indexes this DISPLAYED list (`Combat::decision_view`). The engine keeps the game's own order
  internally (`Decision::cands`); the differential harness clicks in game order via `Combat::step_pick_game_order`.
  "Choose a card" (Discovery-style) screens list the generated cards.
* Forced choices auto-resolve with no decision (`|candidates| <= min` when `min == max`; empty list).
* `Pick` toggles a candidate; when `max` are selected the most recent selection is replaced (hand-UI behaviour).
  The decision completes by itself once `max` are selected unless `confirm_required` (`min != max`); `Confirm`
  finishes the selection (or skips when nothing is selected and the screen is skippable).
* Result order = click order (this is what the real hand UI returns; matters for put-back-on-top effects).

## Information (`Combat::observe`, flat `f32`, `OBS_SIZE`)
Visible to the agent: HP / max HP / block / energy / stars / powers, relics + counters, potions, the hand in order (it is
displayed in order; hand positions are the play actions) with current cost, playability, keywords and damage/block previews,
**the draw, discard and exhaust piles as unordered multisets** (a player knows what is in a pile, never its order), every
enemy's HP/block/powers, current intent(s) (type, per-hit damage computed with the same modifiers the UI uses, hit count) and
its last four performed moves, **expert pattern knowledge of the enemy's upcoming turns** (below), per-turn play counters, and
any pending decision with its candidates.

**Upcoming enemy turns (`Combat::lookahead`, layout rows 12-13).** The game shows only the current intent, but experienced players
know each monster's pattern by heart, so the pattern is treated as known information: for each enemy and each of the next
`LOOK_H` = 4 turns after the one shown, the observation carries the probability of each move node (`LOOK_NODES` = 16 slots; node
indices are per monster, the monster id is in the enemy block) and the expected total attack damage. It is computed by playing the
next turns forward on a projected copy of the combat (`engine/monster.rs`, `look_turn` in `engine/turn.rs`):
* the player passes and is inert (no powers, relics, cards or block act; it cannot die), so the rows are the pattern under the
  status quo; branches the player can force (damage thresholds, kills, wake-ups by damage, stuns) are not anticipated;
* the engine runs each enemy turn: moves are performed, powers tick (Asleep / Slumber count down, debuffs expire, poison), monsters
  buff themselves, summon, die; at every roll the monster's state machine branches into each possible move with the game's odds,
  conditions and weights reading the projected combat;
* projections are per monster: the monster looked at branches, and so do its joint peers (`LOOK_JOINT`: Two-Tailed Rats read
  each other's pending summon and share a call count); every other enemy takes its most likely move at each roll. Enemies roll in
  list order, so a roll sees the moves rolled before it;
* the expected damage is what the intent would show in the projected combat (the monster's projected Strength, Weak ...) against
  the player's current modifiers;
* paths reaching the same state are merged; at most 8 are kept per turn (the least likely are dropped, and a row then sums to less
  than 1, as it does when the projected monster dies or the projected fight ends).

**No RNG is consumed and no realized random outcome is read:** the copy's RNG streams are replaced by a fixed seed, so a random
effect inside a projected move gets an arbitrary fixed outcome (`lookahead_consumes_no_rng...`, the hidden-state test). Calibration
(`tests/lookahead.rs`): with a passive player (what the projection assumes) every observed move count matches its predicted count
within 3 sigma over 162k predictions, and no move predicted impossible happens; under random play 1.2% of the moves happen with
predicted probability 0 (all forced by the player). `LOOK_LEGACY` (`sts2.set_look_legacy`) restores the look-ahead from before S1
(each machine walked alone over 3 turns, conditions reading the current combat) for networks trained before it.

Section 13 (`enemy_moves`, `MOVE_STATE_F` = 2 per enemy slot) says where each pattern stands: the pending move node + 1 (the intent
block shows only its intent types; 255 = stunned) and the node the monster resumes after a stun (or its stored follow-up) + 1, 0 when
none, in the encoding of the performed-move history.

Layout (sections in vector order; sizes are constants in `observe.rs`, `OBS_SIZE` is their sum, asserted in `observe()`):

| # | section | floats |
|---|---|---|
| 1 | global (round, side, phase flags, ...) | `GLOBAL_F` |
| 2 | player (HP, block, energy, stars, powers `(id+1, amount)` x `OBS_POWERS`) | `PLAYER_F` |
| 3 | relics `(id+1, counter)` | `RELIC_F` |
| 4 | potions | `POTION_F` |
| 5 | hand, ordered, `CARD_F` per slot | `MAX_HAND * CARD_F` |
| 6 | draw, discard, exhaust as sorted multisets `(id+1, upgrade)` x `OBS_MAX_PILE`, then the three pile sizes | `OBS_MAX_PILE*2*3 + 3` |
| 7 | enemies in list order | `OBS_MAX_ENEMIES * ENEMY_F` |
| 8 | pending decision (header + candidates, `CARD_F + selected` each) | `DECISION_F` |
| 9 | **Regent** (appended): current star cost of each hand card and of each decision candidate | `REGENT_F` |
| 10 | **Necrobinder** (appended): Osty present / alive / HP / max HP / powers, then the Osty-damage preview of each hand card | `OSTY_F` |
| 11 | **Defect** (appended): `MAX_ORBS` orbs `(kind+1, passive, evoke)` front first (empty = zeros; the slot count is `orb_slots` in the player block), then the number of Lightning orbs channeled this combat (Voltaic's text) | `ORBS_F` |
| 12 | **Expert pattern knowledge** (appended): per enemy slot and per future turn `LOOK_H`: probability of each of `LOOK_NODES` move nodes + expected attack damage | `LOOK_F` = `OBS_MAX_ENEMIES * LOOK_H * (LOOK_NODES + 1)` |
| 13 | **Enemy moves** (appended, S1): per enemy slot, pending move node + 1 and the stored follow-up + 1 | `ENEMY_MOVES_F` = `OBS_MAX_ENEMIES * MOVE_STATE_F` |

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

Hidden (never in the observation): the order of the draw, discard and exhaust piles, all RNG stream states, the realized
outcome of random enemy branches, monster-internal AI state beyond the move pattern.
`observe::hidden_state_does_not_leak` perturbs these (permuting all three piles, rewriting every RNG stream, scrambling
monster logs) and asserts the vector is unchanged; `pile_selection_screen_does_not_reveal_pile_order` does the same for
selection screens.

Not tracked: "known top card" information (after a put-on-top effect a human remembers which card is on top of the draw pile).

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
| hook listeners of one dispatch (snapshot) | 256 (`SNAPSHOT_CAP`) | `ov::CONTAINER` |
| per-attack results | 16 (`after_attack` listeners see the first 16 per-hit results, plus exact hit counters) | `ov::CONTAINER` |
| decision candidates | 64 (= `MAX_PICK`, the action space addresses `Pick{0..64}`) | `ov::CONTAINER` |
| selected cards of a decision / choice | 16 | `ov::CONTAINER` |
| history ring (this-turn / last-turn queries) | 160 entries (`HIST_CAP`) of the queried kinds | `ov::HISTORY` when an entry of the current or previous player turn is overwritten |
| whole-combat counters (`hist_total` ...) | 65535 | `ov::COUNTER` |

Observation limits (the observation is a fixed-size window, not a state copy): 8 enemies, 16 powers per creature, 64 cards per pile list, 16
decision candidates (the decision header carries the true candidate count; `Pick{i}` can address up to 64; a Phrog Parasite prompt in the corpus
has 39). Beyond that the extra entries are simply not visible to the agent (the simulation itself is unaffected).

## Resetting in place
`Combat::reset(&Scenario)` / `reset_with` / `reset_validated` re-initialise an existing combat (no 18.8 KB construct-and-copy); `BatchEnv` resets finished
episodes through `reset_validated` with an allocation-free `ScenarioSource::pick`. A reset combat is bit-identical to `Combat::new` (tested, and
replayed against the oracle with `STS2DIFF_REUSE=1`).

## Throughput (release, random policy, observation + legal actions every step, `cargo run --release -p sts2env --example bench`)
~0.17M env-steps/s per core (1 thread), scaling linearly: 2.4M on an idle 14-core machine (0.95M measured while ~10 foreign cores were busy).
The observation (~40% of an env step) and the engine step (~30%) dominate; see `docs/design.md` "Performance". These figures predate S1:
the projected look-ahead costs about 8 us per uncached enemy (`examples/lookprof.rs`). On the training mix (`BENCH_SCENARIOS=data/train/eval.json`,
1 thread, random play, 83% of look-aheads uncached) an env step takes 6.3 us before S1, 11.5 / 14.2 / 16.9 us with `LOOK_H` = 3 / 4 / 5.
