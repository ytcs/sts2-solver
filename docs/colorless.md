# Colorless / Curse / Status / Token cards

Code: `content/cards/{colorless_a,colorless_b,curses_pool,status,tokens}.rs` (+ `curses.rs` = Ascender's Bane), powers in `content/powers/colorless_basic.rs`, card helpers in `engine/cardcmds.rs` and `engine/autoplay.rs` (nested auto-play, transform, shuffles), potion generation in `engine/potion_gen.rs`.
Validation templates: `oracle/templates/cc/*.json`, run with `python3 verify/diff_sweep.py TEMPLATE --n 40 --keep target/sw --tag X`.

## Conventions
* Turn-end-in-hand cards (Burn, Decay, Regret, Doubt, ...) implement `on_turn_end_in_hand`. Regret keeps the hand size it saw in `BeforeSideTurnEnd` in `card.counter[0]`; Wither's `FakeUpgrade` level is `card.counter[0]` (`status::wither_fake_upgrade`).
* Per-card persistent state lives in `Card::counter` (`[i16; 2]`):
  * Beat Down: the four picked cards as index + 1, one byte each, packed into `[0]` and `[1]`.
  * The Ball: accumulated damage in `[0]`.
  * Sovereign Blade: forged damage in `[0]` (`Combat::blade_add_damage`), repeat-count override in `[1]`.
  * Bolas and Thrumming Hatchet keep no counter: at `BeforeHandDraw` they return to the hand if `hist_any_last_player_turn(CardPlayStarted, ..)` shows they were played last turn.
* Instanced powers keep private state in `Power::aux`: Automation (cards counted), Panache (low byte = count, bit 8 = `alreadyApplied`), The Bomb (bomb damage), Vigor (command tracking). Calamity's per-card amount is recorded at `BeforeCardPlayed` in the per-play table (`play_amount_add/take`), since attacks nest (Uproar auto-plays an Attack from inside its own play).
* Calculated damage (Rend, Mind Blast, Gold Axe, Gang Up) = `CalcBase + ExtraDamage * mult`, computed before `execute_attack` with the single target (the game evaluates `CalculatedDamageVar.Calculate(target)` per hit; identical for single-target cards). Gold Axe's multiplier is `hist_total(HKind::CardPlayFinished)`.
* Multiplayer-only cards (Beacon of Hope, Believe in You, Coordinate, Gang Up, Huddle Up, Intercept, Knockdown, Lift, Mimic, Rally, Tag Team, The Ball) are ported with their single-player behaviour; the `AnyAlly` ones can never be played with one player.

## Decisions raised from inside a hook (Entropy, Stratagem)
Both use the canonical mechanism (`cx.hook_ctx = Some((me, phase))` + `Listener::resume_hook`; turn start resumed through `turn_cont`), so the agent always sees the real state.
* Entropy (`AfterPlayerTurnStart`): like Tools of the Trade; the transform runs in `resume_hook`, one `transform_cards(&[c], &[None])` per card in click order.
* Stratagem (`AfterShuffle`): pauses in place only during the turn-start hand draw (`Combat::drawing_hand`, `draw_resume`, `turn_cont == 4`) and Foregone Conclusion's own shuffle. In every other draw (a card's draw, a hook's draw, Mayhem's auto-play) the step is re-run with the agent's pick (`engine/replay.rs`, `StratagemPower::after_shuffle` -> `Combat::replay_prompt`).
* Known divergence: listeners after the suspending one in the same turn-start pass still run before the decision is answered (the game awaits). Entropy plus another turn-start effect that changes the board (Rolling Boulder) can diverge.

Hook-driven `AutoPlayFromDrawPile` (Mayhem) and Beat Down / Catastrophe use the canonical `auto_play` / `RunResult::Suspended`.
