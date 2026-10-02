# Colorless / Curse / Status / Token cards

Code: `content/cards/{colorless_a,colorless_b,curses_pool,status,tokens}.rs` (+ `curses.rs` = Ascender's Bane),
powers in `content/powers/colorless_basic.rs`, helpers in `engine/cardcmds.rs`, engine extensions in `engine/ext.rs`.
Validation templates: `oracle/templates/cc/*.json` (run `tools/diff_sweep.py TEMPLATE --n 40 --keep target/sw --tag X`;
`tools/play_cover.py TEMPLATE` shows which cards the random policy actually played).

Conventions worth knowing:

* Turn-end-in-hand cards (Burn, Decay, Regret, Doubt, ...) implement `on_turn_end_in_hand`; Regret keeps the hand size it
  saw in `BeforeSideTurnEnd` in `card.counter[0]`; Wither's `FakeUpgrade` level is `card.counter[0]` (`status::wither_fake_upgrade`).
* Per-card persistent state lives in `Card::counter` (`[i16; 2]`): Bolas / Thrumming Hatchet `[0]` = player turn of their last
  finished play (they return to the hand at the next `BeforeHandDraw`), Calamity `[1]` = amount recorded at `BeforeCardPlayed`,
  Beat Down `[0..2]` = the four picked cards (index + 1), The Ball `[0]` = accumulated damage, Sovereign Blade `[0]`/`[1]` =
  forged damage / repeat count (`tokens::sovereign_blade_add_damage` / `_set_repeats`).
* Instanced powers keep private state in `Power::aux` (Automation: cards counted, Panache: bit 8 `alreadyApplied` + count,
  The Bomb: the bomb damage, Vigor: command tracking).
* Calculated damage (Rend, Mind Blast, Gold Axe, Gang Up) = `CalcBase + ExtraDamage * mult`, computed before `execute_attack`
  with the single target (the game evaluates `CalculatedDamageVar.Calculate(target)` per hit; identical for single-target cards).
  `History::cards_finished_total` counts `CardPlayFinished` entries for Gold Axe.
* Multiplayer-only cards (Beacon of Hope, Believe in You, Coordinate, Gang Up, Huddle Up, Intercept, Knockdown, Lift, Mimic,
  Rally, Tag Team, The Ball) are ported with their single-player behaviour; the `AnyAlly` ones can never be played with one player.

## Decisions raised from inside a hook (Entropy, Stratagem) — `engine/ext.rs`

The game awaits the choice in the middle of a hook dispatch (`EntropyPower.AfterPlayerTurnStart`, `StratagemPower.AfterShuffle`
inside a draw). Hooks are plain calls here, so the whole *agent step* is rolled back and replayed instead:

1. `Combat::step` snapshots the state (cheap `Copy`; only when `ExtState::unwind_enabled`, set by `new_card` for Entropy /
   Stratagem) and runs the step.
2. A hook calls `cx.hook_decision(cx.ask_*(..), kind, id)`. If the decision is real and no recorded answer exists the step is
   marked for rollback, the hook sees an empty selection and the step finishes "blind".
3. `step` restores the snapshot, shows the decision (`stage = AwaitChoice`), and when the agent has answered re-executes the same
   action with the answer in `ExtState::replay_answers`; every hook-decision site consumes its answer in order, so the
   re-execution is bit-identical (RNG streams included) and may raise the next hook decision the same way.
Between the rollback and the answer the observation shows the state *before* the action plus the decision's candidates.
A hook decision in a combat that did not announce such cards is flagged `missing` (never silently wrong).
The relics branch has a different mechanism for turn-start hooks (`suspend_hook_for_decision`); both can coexist.

Hook-driven `AutoPlayFromDrawPile` (Mayhem) keeps its remaining cards in `ExtState::autoplay_queue`; `resume_after_decision`
continues it after a nested card that asked for a decision. Card effects (Beat Down, Catastrophe) use `auto_play` /
`Flow::Suspend` as documented in `play.rs`.
