//! Turn loop (spec 01): combat start, player turn start / end, enemy turn, side switching, win/loss.

use super::action::Action;
use crate::content;
use crate::dec::Dec;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;

pub const BASE_HAND_DRAW: i32 = 5;

impl Combat {
    // ---- combat start -----------------------------------------------------------------------------------------------

    /// `StartCombatInternal` (spec 01 §4). Leaves the combat awaiting the first player action (or over).
    pub(crate) fn start_combat(&mut self) {
        // Hook.AfterRoomEntered (run-level iterator, before the enemies' AfterAddedToRoom / initial RollMove).
        self.dispatch_u(hookbit::after_room_entered, |cx, me, l| l.after_room_entered(cx, me));
        // AfterAddedToRoom + first RollMove, per enemy in list order, interleaved (spec 04 §1.7).
        let order: crate::util::ArrayVec<Cid, MAX_CREATURES> = {
            let mut o = crate::util::ArrayVec::new();
            for &c in self.allies.iter().chain(self.enemies.iter()) {
                o.push(c);
            }
            o
        };
        for &c in order.iter() {
            if self.cr(c).side == Side::Enemy && !self.cr(c).is_pet {
                let def = content::monster_def(self.cr(c).monster.id);
                if let Some(f) = def.on_spawn {
                    f(self, c);
                }
                if self.side == Side::Player {
                    self.roll_move(c);
                }
            }
        }
        self.in_progress = true;
        self.is_starting = false;
        self.dispatch_u(hookbit::before_combat_start, |cx, me, l| l.before_combat_start(cx, me));
        self.dispatch_u(hookbit::before_combat_start_late, |cx, me, l| l.before_combat_start_late(cx, me));
        self.start_player_turn();
    }

    // ---- player turn start ----------------------------------------------------------------------------------------

    fn before_turn_start(&mut self, side: Side) {
        let list: crate::util::ArrayVec<Cid, MAX_CREATURES> = self.creatures_on(side);
        self.before_turn_start_for(&list);
    }

    fn before_turn_start_for(&mut self, list: &crate::util::ArrayVec<Cid, MAX_CREATURES>) {
        for &c in list.iter() {
            for p in self.cr_mut(c).powers.as_mut_slice() {
                p.amount_on_turn_start = p.amount;
            }
        }
    }

    pub fn creatures_on(&self, side: Side) -> crate::util::ArrayVec<Cid, MAX_CREATURES> {
        let mut o = crate::util::ArrayVec::new();
        let src = if side == Side::Player { self.allies.as_slice() } else { self.enemies.as_slice() };
        for &c in src {
            o.push(c);
        }
        o
    }

    /// `Creature.ClearBlock`.
    fn clear_block(&mut self, c: Cid) {
        let mut preventer: Option<Me> = None;
        if self.hooks_enabled() {
            let snap = self.snapshot(Mask::bit(hookbit::should_clear_block));
            for e in snap.iter() {
                if self.still_live(&e.me) && !content::listener(&e.me).should_clear_block(self, e.me, c) {
                    preventer = Some(e.me);
                    break;
                }
            }
        }
        match preventer {
            None => self.cr_mut(c).block = 0,
            Some(m) => {
                if self.still_live(&m) {
                    content::listener(&m).after_preventing_block_clear(self, m, c);
                }
            }
        }
    }

    /// `StartTurn(Player)` (spec 01 §6.1).
    pub(crate) fn start_player_turn(&mut self) {
        self.player.phase = Phase::None;
        let extra = self.extra_turn;
        // Extra turn: only the extra-turn players (the player creature) start the turn; pets do not.
        let list = if extra {
            let mut l = crate::util::ArrayVec::new();
            l.push(PLAYER);
            l
        } else {
            self.creatures_on(Side::Player)
        };
        self.before_turn_start_for(&list);
        self.dispatch_g(hookbit::before_side_turn_start, |cx, me, l| l.before_side_turn_start(cx, me, Side::Player));
        self.player.phase = Phase::Start;
        // PrepareForNextTurn: enemies roll intents before the player draws (not on extra turns).
        if !extra {
            let enemies = self.creatures_on(Side::Enemy);
            for &e in enemies.iter() {
                self.prepare_for_next_turn(e);
            }
        }
        for &c in list.iter() {
            // Creature.AfterTurnStart: block clear, skipped on the player's first turn.
            let skip = c == PLAYER && self.player.turn_number == 1;
            if !skip {
                self.clear_block(c);
            }
        }
        for &c in list.iter() {
            self.dispatch_g(hookbit::after_block_cleared, |cx, me, l| l.after_block_cleared(cx, me, c));
        }
        if self.cr(PLAYER).is_alive() && self.setup_player_turn(0) {
            return; // suspended on a decision raised by a turn-start hook; `resume_turn_start` continues
        }
        self.finish_player_turn_start();
    }

    /// Rest of `StartTurn(Player)` after `SetupPlayerTurn` (spec 01 §6.1).
    fn finish_player_turn_start(&mut self) {
        self.dispatch_g(hookbit::after_side_turn_start, |cx, me, l| l.after_side_turn_start(cx, me, Side::Player));
        self.dispatch_g(hookbit::after_side_turn_start_late, |cx, me, l| l.after_side_turn_start_late(cx, me, Side::Player));
        // OrbQueue.AfterTurnStart (Plasma), after the whole Hook.AfterSideTurnStart (incl. the Late pass).
        if self.cr(PLAYER).is_alive() {
            self.orbs_after_turn_start();
        }
        if self.cr(PLAYER).is_dead() {
            // StartTurn step 10b: a dead player is marked ready to end the turn, which (single player) immediately runs
            // phase one of the turn end; its `CheckWinCondition` then processes the pending loss (phase ends as `End`).
            if self.in_progress {
                self.end_player_turn();
            }
            return;
        }
        // RunAutoPrePlayPhase
        self.player.phase = Phase::AutoPrePlay;
        self.check_for_empty_hand();
        self.dispatch_g(hookbit::after_auto_pre_play_phase_entered_early, |cx, me, l| l.after_auto_pre_play_phase_entered_early(cx, me));
        self.dispatch_g(hookbit::after_auto_pre_play_phase_entered, |cx, me, l| l.after_auto_pre_play_phase_entered(cx, me));
        self.dispatch_g(hookbit::after_auto_pre_play_phase_entered_late, |cx, me, l| l.after_auto_pre_play_phase_entered_late(cx, me));
        self.player.phase = Phase::Play;
        if !self.check_win_condition() && self.stage != Stage::AwaitChoice {
            self.stage = Stage::AwaitAction;
            // An end-turn requested by the turn-start effects (Void Form ...) is held until `StartTurn` returns.
            self.consume_end_turn_request();
        }
    }

    /// Continues a turn start that was suspended by a decision raised inside a turn-start hook (`turn_cont`: 1 = in
    /// `BeforeHandDraw`, 2 = in `AfterPlayerTurnStart`; the listeners after the suspended one are not re-run).
    pub(crate) fn resume_turn_start(&mut self, cont: u8) {
        match cont {
            1 => {
                if self.setup_player_turn(1) {
                    return;
                }
                self.finish_player_turn_start();
            }
            2 => {
                self.dispatch_g(hookbit::after_player_turn_start_late, |cx, me, l| l.after_player_turn_start_late(cx, me));
                self.finish_player_turn_start();
            }
            _ => {}
        }
    }

    /// `Hook.ModifyMaxEnergy` (threaded) applied to the base max energy.
    pub fn max_energy(&self) -> i32 {
        let mut v = Dec::int(self.player.max_energy as i64);
        if self.hooks_enabled() {
            let snap = self.snapshot(Mask::bit(hookbit::modify_max_energy));
            for e in snap.iter() {
                if self.still_live(&e.me) {
                    v = content::listener(&e.me).modify_max_energy(self, e.me, v);
                }
            }
        }
        v.trunc()
    }

    fn should_player_reset_energy(&self) -> bool {
        if self.hooks_enabled() {
            let snap = self.snapshot(Mask::bit(hookbit::should_player_reset_energy));
            for e in snap.iter() {
                if self.still_live(&e.me) && !content::listener(&e.me).should_player_reset_energy(self, e.me) {
                    return false;
                }
            }
        }
        true
    }

    /// `SetupPlayerTurn` (spec 01 §6.2).
    /// Returns true if it suspended on a decision raised by a hook (`turn_cont` says where to resume).
    fn setup_player_turn(&mut self, from: u8) -> bool {
        if from == 0 {
            if self.should_player_reset_energy() {
                self.player.energy = self.max_energy();
            } else {
                self.player.energy += self.max_energy();
            }
            self.dispatch_g(hookbit::after_energy_reset, |cx, me, l| l.after_energy_reset(cx, me));
            self.dispatch_g(hookbit::after_energy_reset_late, |cx, me, l| l.after_energy_reset_late(cx, me));
            self.dispatch_g(hookbit::before_hand_draw, |cx, me, l| l.before_hand_draw(cx, me));
            self.dispatch_g(hookbit::before_hand_draw_late, |cx, me, l| l.before_hand_draw_late(cx, me));
            if self.stage == Stage::AwaitChoice {
                self.turn_cont = 1;
                return true;
            }
        }
        // Hook.ModifyHandDraw: pass 1 ModifyHandDraw, pass 2 ModifyHandDrawLate (threaded decimals); a listener is a
        // "modifier" iff the (int) value changed; only modifiers get AfterModifyingHandDraw.
        let mut draw = Dec::int(BASE_HAND_DRAW as i64);
        let mut mods = super::Mods::new();
        if self.hooks_enabled() {
            for bit in [hookbit::modify_hand_draw, hookbit::modify_hand_draw_late] {
                if !self.listen.has(bit) {
                    continue;
                }
                let snap = self.snapshot(Mask::bit(bit));
                for e in snap.iter() {
                    if self.still_live(&e.me) {
                        let l = content::listener(&e.me);
                        let nv = if bit == hookbit::modify_hand_draw { l.modify_hand_draw(self, e.me, draw) } else { l.modify_hand_draw_late(self, e.me, draw) };
                        if draw.trunc() != nv.trunc() {
                            mods.push(e.me);
                        }
                        draw = nv;
                    }
                }
            }
        }
        self.dispatch_modifiers(true, hookbit::after_modifying_hand_draw, &mods, |cx, me, l| l.after_modifying_hand_draw(cx, me));
        let mut hand_draw = draw.trunc();
        if self.player.turn_number == 1 {
            // Cards whose enchantment starts at the bottom (Imbued) move to the bottom first (pile order), then Innate
            // cards (excluding those) move to the top one by one => their on-top order is the REVERSE of pile order.
            let mut bottom: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
            for &c in self.player.draw.iter() {
                if self.cards[c as usize].enchant != 0 {
                    let me = self.enchantment_me(c);
                    if content::listener(&me).should_start_at_bottom_of_draw_pile(self, me) {
                        bottom.push(c);
                    }
                }
            }
            for &c in bottom.iter() {
                self.player.draw.remove_value(c);
                self.player.draw.push(c);
            }
            let innate: crate::util::ArrayVec<CardIdx, MAX_CARDS> = {
                let mut v = crate::util::ArrayVec::new();
                for &c in self.player.draw.iter() {
                    if self.card_keywords(c) & kw::INNATE != 0 && !bottom.contains(c) {
                        v.push(c);
                    }
                }
                v
            };
            for &c in innate.iter() {
                self.player.draw.remove_value(c);
                self.player.draw.insert(0, c);
            }
            hand_draw = hand_draw.max(innate.len() as i32).min(MAX_HAND as i32);
        }
        self.draw_cards(hand_draw, true);
        self.dispatch_g(hookbit::after_player_turn_start_early, |cx, me, l| l.after_player_turn_start_early(cx, me));
        self.dispatch_g(hookbit::after_player_turn_start, |cx, me, l| l.after_player_turn_start(cx, me));
        if self.stage == Stage::AwaitChoice {
            self.turn_cont = 2;
            return true;
        }
        self.dispatch_g(hookbit::after_player_turn_start_late, |cx, me, l| l.after_player_turn_start_late(cx, me));
        false
    }

    /// `PlayerCmd.EndTurn(player)`: marks the player ready to end the turn. The signal is consumed when the effect (or the
    /// turn start) that raised it has returned (spec 01 §6.3, §7). Ignored once the turn is already ending.
    pub fn request_end_turn(&mut self) {
        if self.side == Side::Player && matches!(self.player.phase, Phase::Start | Phase::AutoPrePlay | Phase::Play) && self.cr(PLAYER).is_alive() {
            self.end_turn_requested = true;
        }
    }

    fn consume_end_turn_request(&mut self) {
        if self.end_turn_requested && self.in_progress && self.stage == Stage::AwaitAction && self.player.phase == Phase::Play {
            self.end_turn_requested = false;
            self.end_player_turn();
        }
    }

    /// `CheckForEmptyHand` -> `Hook.AfterHandEmptied`.
    pub fn check_for_empty_hand(&mut self) {
        if self.in_progress && self.player.effect_depth == 0 && self.player.hand.is_empty() {
            self.dispatch_g(hookbit::after_hand_emptied, |cx, me, l| l.after_hand_emptied(cx, me));
        }
    }

    // ---- player turn end -----------------------------------------------------------------------------------------------

    /// Player ends the turn: phase one, phase two, side switch, enemy turn, next player turn (until the next decision).
    fn end_player_turn(&mut self) {
        self.stage = Stage::AwaitAction; // not accepting actions while resolving
        // ---- phase one ----
        self.player.phase = Phase::AutoPostPlay;
        if !self.run_post_play_hooks(None) {
            return; // an auto-played card (Stampede) asked for a decision; `resume_after_decision` continues the turn end
        }
        self.end_player_turn_rest();
    }

    /// `Hook.AfterAutoPostPlayPhaseEntered` pass. Returns false if a listener suspended on a decision (the listener is
    /// remembered in `end_turn_resume` and re-entered when the decision is done; it must keep its own loop progress).
    fn run_post_play_hooks(&mut self, resume: Option<Me>) -> bool {
        if !(self.listen.has(hookbit::after_auto_post_play_phase_entered) && self.hooks_enabled()) {
            return true;
        }
        let snap = self.snapshot(Mask::bit(hookbit::after_auto_post_play_phase_entered));
        let mut started = resume.is_none();
        for e in snap.iter() {
            if !started {
                match resume {
                    Some(r) if r.kind == e.me.kind && r.owner == e.me.owner && r.idx == e.me.idx => started = true,
                    _ => continue,
                }
            }
            if self.still_live(&e.me) {
                content::listener(&e.me).after_auto_post_play_phase_entered(self, e.me);
                if self.stage == Stage::AwaitChoice {
                    self.end_turn_resume = Some(e.me);
                    return false;
                }
            }
        }
        true
    }

    /// Continues the end of the player's turn after the post-play phase hooks were interrupted by a decision.
    fn resume_end_turn(&mut self, me: Me) {
        if !self.run_post_play_hooks(Some(me)) {
            return;
        }
        self.end_player_turn_rest();
    }

    /// Rest of phase one + phase two of the turn end (from `Phase::End` on).
    fn end_player_turn_rest(&mut self) {
        self.player.phase = Phase::End;
        self.dispatch_g(hookbit::before_side_turn_end_very_early, |cx, me, l| l.before_side_turn_end_very_early(cx, me, Side::Player));
        self.dispatch_g(hookbit::before_side_turn_end_early, |cx, me, l| l.before_side_turn_end_early(cx, me, Side::Player));
        self.dispatch_g(hookbit::before_side_turn_end, |cx, me, l| l.before_side_turn_end(cx, me, Side::Player));
        if self.check_win_condition() {
            return;
        }
        self.do_turn_end();
        if self.check_win_condition() {
            return;
        }
        self.dispatch_g(hookbit::before_flush, |cx, me, l| l.before_flush(cx, me));
        self.dispatch_g(hookbit::before_flush_late, |cx, me, l| l.before_flush_late(cx, me));
        self.check_win_condition();
        if !self.in_progress {
            return;
        }
        // ---- phase two ----
        self.flush_player_hand();
        self.dispatch_g(hookbit::after_side_turn_end, |cx, me, l| l.after_side_turn_end(cx, me, Side::Player));
        self.dispatch_g(hookbit::after_side_turn_end_late, |cx, me, l| l.after_side_turn_end_late(cx, me, Side::Player));
        // SwitchFromPlayerToEnemySide (spec 01 §9.2): PlayersTakingExtraTurn is recomputed at every player turn end.
        self.extra_turn = self.any_true_g(hookbit::should_take_extra_turn, |cx, me, l| l.should_take_extra_turn(cx, me));
        let extra = self.extra_turn;
        self.flip_sides();
        if extra {
            self.dispatch_g(hookbit::after_taking_extra_turn, |cx, me, l| l.after_taking_extra_turn(cx, me));
        }
        self.continue_after_switch();
    }

    /// `DoTurnEnd`: ethereal cards exhaust (hand order), turn-end-in-hand cards resolve.
    fn do_turn_end(&mut self) {
        self.orbs_before_turn_end();
        if !self.in_progress || self.is_ending() {
            return;
        }
        let hand = self.player.hand;
        let mut ethereal: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
        let mut turn_end: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
        for &c in hand.iter() {
            if self.card_def(c).turn_end_in_hand {
                turn_end.push(c);
            } else if self.card_keywords(c) & kw::ETHEREAL != 0 && self.first_veto_g(hookbit::should_ethereal_trigger, |cx, me, l| l.should_ethereal_trigger(cx, me, c)).is_none() {
                ethereal.push(c);
            }
        }
        for &c in ethereal.iter() {
            self.exhaust_card(c, true);
        }
        for &c in turn_end.iter() {
            self.move_card(c, PileType::Play, CardPilePosition::Bottom);
            let me = Me { kind: Kind::Card, owner: PLAYER, idx: c as u16, id: self.cards[c as usize].id, amount: 0 };
            content::listener(&me).on_turn_end_in_hand(self, c);
            if self.card_keywords(c) & kw::ETHEREAL != 0 {
                self.exhaust_card(c, true);
            } else {
                self.move_card(c, PileType::Discard, CardPilePosition::Bottom);
            }
        }
    }

    /// `FlushPlayerHand` (spec 01 §8.5).
    fn flush_player_hand(&mut self) {
        let mut flush = true;
        if self.hooks_enabled() {
            let snap = self.snapshot(Mask::bit(hookbit::should_flush));
            for e in snap.iter() {
                if self.still_live(&e.me) && !content::listener(&e.me).should_flush(self, e.me) {
                    flush = false;
                    break;
                }
            }
        }
        let hand = self.player.hand;
        let mut flushed: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
        for &c in hand.iter() {
            let retain = !flush || self.should_retain_this_turn(c);
            if !retain {
                flushed.push(c);
            }
        }
        for &c in flushed.iter() {
            self.move_card(c, PileType::Discard, CardPilePosition::Bottom);
        }
        self.dispatch_g(hookbit::after_flush, |cx, me, l| l.after_flush(cx, me));
        self.end_of_turn_cleanup();
    }

    /// `PlayerCombatState.EndOfTurnCleanup`.
    pub fn end_of_turn_cleanup(&mut self) {
        for i in 0..self.n_cards as usize {
            let card = &mut self.cards[i];
            if card.pile == 0 || card.pile > 5 {
                continue;
            }
            card.flags &= !(cflag::EXHAUST_ON_NEXT_PLAY | cflag::SINGLE_TURN_RETAIN | cflag::SINGLE_TURN_SLY);
            self.clear_star_mods(i as CardIdx, EXPIRE_END_OF_TURN);
            let card = &mut self.cards[i];
            if !card.mods.is_empty() {
                let mut kept: crate::engine::CostMods = crate::util::ArrayVec::new();
                for m in card.mods.iter() {
                    if m.expire & EXPIRE_END_OF_TURN == 0 {
                        kept.push(*m);
                    }
                }
                card.mods = kept;
            }
        }
    }

    // ---- sides ---------------------------------------------------------------------------------------------------------

    /// `SwitchSides` state change (spec 01 §9.1): Player && !extra -> Enemy; otherwise (Enemy -> Player, or an extra
    /// player turn) -> Player with the player's turn number incremented and, unless it is an extra turn, the round.
    fn flip_sides(&mut self) {
        let extra = self.extra_turn;
        if self.side == Side::Player && !extra {
            self.side = Side::Enemy;
        } else {
            self.side = Side::Player;
            if !extra {
                self.round += 1;
            }
            self.player.turn_number += 1;
        }
        // Creature.OnSideSwitch: monsters lose SpawnedThisTurn.
        for i in 0..MAX_CREATURES {
            if self.creatures[i].in_combat && !self.creatures[i].is_player {
                self.creatures[i].monster.spawned_this_turn = false;
            }
        }
        // player-turn bookkeeping that is per-turn
        self.hist = History::default();
    }

    /// What the turn loop does after a side switch: the enemy turn, or the next player turn.
    fn continue_after_switch(&mut self) {
        if self.side == Side::Enemy {
            self.run_enemy_turn();
        } else if self.in_progress {
            self.start_player_turn();
        }
    }

    /// `SwitchSides` + the turn loop continuing (enemy -> player).
    fn switch_sides(&mut self) {
        self.flip_sides();
        self.continue_after_switch();
    }

    /// `StartTurn(Enemy)` + `ExecuteEnemyTurn` + `EndEnemyTurn` (spec 01 §10).
    fn run_enemy_turn(&mut self) {
        self.player.phase = Phase::None;
        let list = self.creatures_on(Side::Enemy);
        self.before_turn_start(Side::Enemy);
        self.dispatch_g(hookbit::before_side_turn_start, |cx, me, l| l.before_side_turn_start(cx, me, Side::Enemy));
        for &c in list.iter() {
            self.clear_block(c);
        }
        for &c in list.iter() {
            self.dispatch_g(hookbit::after_block_cleared, |cx, me, l| l.after_block_cleared(cx, me, c));
        }
        self.dispatch_g(hookbit::after_side_turn_start, |cx, me, l| l.after_side_turn_start(cx, me, Side::Enemy));
        self.dispatch_g(hookbit::after_side_turn_start_late, |cx, me, l| l.after_side_turn_start_late(cx, me, Side::Enemy));
        if self.check_win_condition() {
            return;
        }
        // ExecuteEnemyTurn: snapshot of Enemies at turn start.
        let snapshot = self.creatures_on(Side::Enemy);
        for &e in snapshot.iter() {
            if !self.enemies.contains(e) {
                continue;
            }
            if !self.cr(e).monster.spawned_this_turn {
                self.perform_move(e);
            }
            if self.check_win_condition() {
                return;
            }
        }
        // EndEnemyTurnInternal
        self.dispatch_g(hookbit::before_side_turn_end_very_early, |cx, me, l| l.before_side_turn_end_very_early(cx, me, Side::Enemy));
        self.dispatch_g(hookbit::before_side_turn_end_early, |cx, me, l| l.before_side_turn_end_early(cx, me, Side::Enemy));
        self.dispatch_g(hookbit::before_side_turn_end, |cx, me, l| l.before_side_turn_end(cx, me, Side::Enemy));
        self.end_of_turn_cleanup();
        self.dispatch_g(hookbit::after_side_turn_end, |cx, me, l| l.after_side_turn_end(cx, me, Side::Enemy));
        self.dispatch_g(hookbit::after_side_turn_end_late, |cx, me, l| l.after_side_turn_end_late(cx, me, Side::Enemy));
        if self.check_win_condition() {
            // Quirk (spec 01 §10.3): `IsCombatEnding` is false once the combat is no longer in progress, so the side
            // switch (round / turn counters) still happens after a win or loss detected right here.
            self.flip_sides();
            return;
        }
        self.switch_sides();
    }

    // ---- win / loss -------------------------------------------------------------------------------------------------

    /// `CheckWinCondition`. Returns true if the combat ended (win or loss).
    pub fn check_win_condition(&mut self) -> bool {
        if self.pending_loss {
            self.pending_loss = false;
            self.in_progress = false;
            self.outcome = Outcome::Defeat;
            self.stage = Stage::Over;
            return true;
        }
        if self.in_progress && self.is_ending() {
            self.end_combat_victory();
            return true;
        }
        false
    }

    /// `EndCombatInternal` (spec 01 §13.3).
    fn end_combat_victory(&mut self) {
        self.in_progress = false;
        self.extra_turn = false;
        self.player.phase = Phase::None;
        self.dispatch_u(hookbit::after_combat_end, |cx, me, l| l.after_combat_end(cx, me));
        // Player.AfterCombatEnd: powers (no hooks), combat piles, block.
        self.cr_mut(PLAYER).powers.clear();
        self.cr_mut(PLAYER).block = 0;
        self.player.hand.clear();
        self.player.draw.clear();
        self.player.discard.clear();
        self.player.exhaust.clear();
        self.player.play.clear();
        self.dispatch_u(hookbit::after_combat_victory_early, |cx, me, l| l.after_combat_victory_early(cx, me));
        self.dispatch_u(hookbit::after_combat_victory, |cx, me, l| l.after_combat_victory(cx, me));
        self.hist_log = Default::default(); // History.Clear()
        self.outcome = Outcome::Victory;
        self.stage = Stage::Over;
    }

    // ---- agent interface ------------------------------------------------------------------------------------------

    /// Applies an action. Returns false if it was illegal (state unchanged).
    pub fn step(&mut self, a: Action) -> bool {
        match (self.stage, a) {
            (Stage::AwaitAction, Action::PlayCard { hand_pos, target }) => {
                if !self.play_card(hand_pos as usize, target) {
                    return false;
                }
                if self.stage != Stage::AwaitChoice {
                    self.after_action();
                }
                true
            }
            (Stage::AwaitAction, Action::UsePotion { slot, target }) => {
                if !self.use_potion(slot as usize, target) {
                    return false;
                }
                if self.stage != Stage::AwaitChoice {
                    self.after_action();
                }
                true
            }
            (Stage::AwaitAction, Action::DiscardPotion { slot }) => self.discard_potion(slot as usize),
            (Stage::AwaitAction, Action::EndTurn) => {
                if self.player.phase != Phase::Play {
                    return false;
                }
                self.end_player_turn();
                true
            }
            (Stage::AwaitChoice, Action::Pick { idx }) => {
                if !self.decision_pick(idx) {
                    return false;
                }
                if self.stage != Stage::AwaitChoice {
                    self.after_action();
                }
                true
            }
            (Stage::AwaitChoice, Action::Confirm) => {
                if !self.decision_confirm() {
                    return false;
                }
                if self.stage != Stage::AwaitChoice {
                    self.after_action();
                }
                true
            }
            _ => false,
        }
    }

    /// Continues whichever effect raised the decision that just finished.
    pub(crate) fn resume_after_decision(&mut self) {
        if let Some((me, phase)) = self.hook_ctx.take() {
            content::listener(&me).resume_hook(self, me, phase);
            if self.stage == Stage::AwaitChoice {
                return; // the hook's effect (e.g. a Sly auto-play) raised its own decision: that play resumes later
            }
        }
        if !self.play_stack.is_empty() {
            self.run_play_stack();
            if self.stage == Stage::AwaitChoice {
                return;
            }
            // The turn end was interrupted by an auto-played card of an `AfterAutoPostPlayPhaseEntered` listener.
            if let Some(me) = self.end_turn_resume.take() {
                self.resume_end_turn(me);
                return;
            }
        }
        if self.potion_ctx.is_some() {
            self.run_potion();
        }
        // A turn start suspended by a hook decision (Tools of the Trade, ...) continues once the hook's own effects
        // (including a nested Sly auto-play it triggered) are fully resolved.
        if self.turn_cont != 0 && self.stage != Stage::AwaitChoice && self.play_stack.is_empty() {
            let t = self.turn_cont;
            self.turn_cont = 0;
            self.resume_turn_start(t);
        }
    }

    /// `ActionExecutor`: win/loss check after every executed game action (the hand-empty check already ran at the end
    /// of the card play / potion use).
    fn after_action(&mut self) {
        self.check_win_condition();
        self.consume_end_turn_request();
    }
}
