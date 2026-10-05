# Colorless / Curse / Status / Token cards

Code: `content/cards/{colorless_a,colorless_b,curses_pool,status,tokens}.rs` (+ `curses.rs` = Ascender's Bane),
powers in `content/powers/colorless_basic.rs`, helpers in `engine/cardcmds.rs`, engine extensions in `engine/ext.rs`.
Validation templates: `oracle/templates/cc/*.json` (run `verify/diff_sweep.py TEMPLATE --n 40 --keep target/sw --tag X`;
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

## Decisions raised from inside a hook (Entropy, Stratagem)

Both use the canonical mechanism (`cx.hook_ctx = Some((me, phase))` + `Listener::resume_hook`, turn start resumed through
`turn_cont`), so the agent always sees the real state.

* Entropy (`AfterPlayerTurnStart`): like Tools of the Trade; the transform runs in `resume_hook` via `transform_cards(&[c], &[None])`.
* Stratagem (`AfterShuffle`): paused in place only during the turn-start hand draw (`Combat::drawing_hand`, `draw_resume`, `turn_cont == 4`
  in `engine/turn.rs`/`piles.rs`) and Foregone Conclusion's own shuffle. In every other draw (a card's draw, a hook's draw, Mayhem's
  auto-play) the step is re-run with the agent's pick (`engine/replay.rs`, `StratagemPower::after_shuffle` -> `Combat::replay_prompt`).
* Inherited engine-core limitation: the listeners after the suspending one in the same turn-start pass still run before the
  decision is answered (the game awaits). Entropy + another turn-start effect that changes the board (Rolling Boulder) can diverge.

Hook-driven `AutoPlayFromDrawPile` (Mayhem) and Beat Down / Catastrophe use the canonical `auto_play` / `RunResult::Suspended`.
