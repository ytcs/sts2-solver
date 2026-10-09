use super::action::Action;
use crate::content;
use crate::dec::Dec;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;

pub const BASE_HAND_DRAW: i32 = 5;

impl Combat {
    pub(crate) fn start_combat(&mut self) {
        self.dispatch_u(hookbit::after_room_entered, |cx, me, l| l.after_room_entered(cx, me));
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

    fn clear_block(&mut self, c: Cid) {
        let mut preventer: Option<Me> = None;
        if self.hooks_enabled() {
            let mut snap = crate::engine::Snapshot::new();
            self.snapshot_into(Mask::bit(hookbit::should_clear_block), &mut snap);
            for e in snap.iter() {
                if self.still_live(&e.me) && !content::listener(&e.me).should_clear_block(self, e.me, c) {
                    preventer = Some(e.me);
                    break;
                }
            }
        }
        match preventer {
            None => self.cr_mut(c).set_block(0),
            Some(m) => {
                if self.still_live(&m) {
                    content::listener(&m).after_preventing_block_clear(self, m, c);
                }
            }
        }
    }

    pub(crate) fn start_player_turn(&mut self) {
        if !self.turn_enter() {
            return;
        }
        self.player.phase = Phase::None;
        let extra = self.extra_turn;
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
        if !extra {
            let enemies = self.creatures_on(Side::Enemy);
            for &e in enemies.iter() {
                self.prepare_for_next_turn(e);
            }
        }
        for &c in list.iter() {
            let skip = c == PLAYER && self.player.turn_number == 1;
            if !skip {
                self.clear_block(c);
            }
        }
        for &c in list.iter() {
            self.dispatch_g(hookbit::after_block_cleared, |cx, me, l| l.after_block_cleared(cx, me, c));
        }
        if self.cr(PLAYER).is_alive() && self.setup_player_turn(0) {
            return;
        }
        self.finish_player_turn_start(0);
    }

    fn finish_player_turn_start(&mut self, from: u8) {
        if from == 0 && self.finish_turn_start_before_auto_pre_play() {
            return;
        }
        if from <= 5 && self.dispatch_resumable(hookbit::after_auto_pre_play_phase_entered_early, |cx, me, l| l.after_auto_pre_play_phase_entered_early(cx, me)) {
            self.turn_cont = 5;
            return;
        }
        if from <= 6 && self.dispatch_resumable(hookbit::after_auto_pre_play_phase_entered, |cx, me, l| l.after_auto_pre_play_phase_entered(cx, me)) {
            self.turn_cont = 6;
            return;
        }
        if from <= 7 && self.dispatch_resumable(hookbit::after_auto_pre_play_phase_entered_late, |cx, me, l| l.after_auto_pre_play_phase_entered_late(cx, me)) {
            self.turn_cont = 7;
            return;
        }
        self.player.phase = Phase::Play;
        if !self.check_win_condition() && self.stage != Stage::AwaitChoice {
            self.stage = Stage::AwaitAction;
            self.consume_end_turn_request();
        }
    }

    fn finish_turn_start_before_auto_pre_play(&mut self) -> bool {
        self.dispatch_g(hookbit::after_side_turn_start, |cx, me, l| l.after_side_turn_start(cx, me, Side::Player));
        self.dispatch_g(hookbit::after_side_turn_start_late, |cx, me, l| l.after_side_turn_start_late(cx, me, Side::Player));
        if self.cr(PLAYER).is_alive() {
            self.orbs_after_turn_start();
        }
        if self.cr(PLAYER).is_dead() {
            if self.in_progress {
                self.end_player_turn();
            }
            return true;
        }
        self.player.phase = Phase::AutoPrePlay;
        self.check_for_empty_hand();
        false
    }

    pub(crate) fn resume_turn_start(&mut self, cont: u8) {
        if (1..=4).contains(&cont) {
            if self.setup_player_turn(cont) {
                return;
            }
            self.finish_player_turn_start(0);
        } else if (5..=7).contains(&cont) {
            self.finish_player_turn_start(cont);
        }
    }

    pub fn max_energy(&self) -> i32 {
        let mut v = Dec::int(self.player.max_energy as i64);
        if self.hooks_enabled() {
            let mut snap = crate::engine::Snapshot::new();
            self.snapshot_into(Mask::bit(hookbit::modify_max_energy), &mut snap);
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
            let mut snap = crate::engine::Snapshot::new();
            self.snapshot_into(Mask::bit(hookbit::should_player_reset_energy), &mut snap);
            for e in snap.iter() {
                if self.still_live(&e.me) && !content::listener(&e.me).should_player_reset_energy(self, e.me) {
                    return false;
                }
            }
        }
        true
    }

    fn setup_player_turn(&mut self, from: u8) -> bool {
        if from == 0 {
            if self.should_player_reset_energy() {
                self.player.energy = self.max_energy();
            } else {
                self.player.energy += self.max_energy();
            }
            self.dispatch_g(hookbit::after_energy_reset, |cx, me, l| l.after_energy_reset(cx, me));
            self.dispatch_g(hookbit::after_energy_reset_late, |cx, me, l| l.after_energy_reset_late(cx, me));
        }
        if from <= 1 && self.dispatch_resumable(hookbit::before_hand_draw, |cx, me, l| l.before_hand_draw(cx, me)) {
            self.turn_cont = 1;
            return true;
        }
        if from <= 2 && self.dispatch_resumable(hookbit::before_hand_draw_late, |cx, me, l| l.before_hand_draw_late(cx, me)) {
            self.turn_cont = 2;
            return true;
        }
        if from == 4 {
            if let Some((n, from_hand)) = self.draw_resume.take() {
                self.drawing_hand = true;
                if let Some((card, phase)) = self.draw_pass.take() {
                    let suspended = if phase == 2 {
                        self.dispatch_resumable(hookbit::after_shuffle, |cx, me, l| l.after_shuffle(cx, me))
                    } else {
                        self.drawn_hooks(card, from_hand, phase)
                    };
                    if suspended {
                        if phase == 2 {
                            self.draw_pass = Some((NO, 2));
                        }
                        self.drawing_hand = false;
                        self.draw_resume = Some((n, from_hand));
                        self.turn_cont = 4;
                        return true;
                    }
                }
                self.draw_cards(n, from_hand);
                self.drawing_hand = false;
                if self.stage == Stage::AwaitChoice {
                    self.turn_cont = 4;
                    return true;
                }
            }
            self.dispatch_g(hookbit::after_player_turn_start_early, |cx, me, l| l.after_player_turn_start_early(cx, me));
        } else if from <= 2 && self.draw_opening_hand() {
            self.turn_cont = 4;
            return true;
        }
        if from <= 4 && self.dispatch_resumable(hookbit::after_player_turn_start, |cx, me, l| l.after_player_turn_start(cx, me)) {
            self.turn_cont = 3;
            return true;
        }
        self.dispatch_g(hookbit::after_player_turn_start_late, |cx, me, l| l.after_player_turn_start_late(cx, me));
        false
    }

    fn draw_opening_hand(&mut self) -> bool {
        let mut draw = Dec::int(BASE_HAND_DRAW as i64);
        let mut mods = super::Mods::new();
        if self.hooks_enabled() {
            for bit in [hookbit::modify_hand_draw, hookbit::modify_hand_draw_late] {
                if !self.listen.has(bit) {
                    continue;
                }
                let mut snap = crate::engine::Snapshot::new();
                self.snapshot_into(Mask::bit(bit), &mut snap);
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
        self.drawing_hand = true;
        self.draw_cards(hand_draw, true);
        self.drawing_hand = false;
        if self.stage == Stage::AwaitChoice && self.draw_resume.is_some() {
            return true;
        }
        self.dispatch_g(hookbit::after_player_turn_start_early, |cx, me, l| l.after_player_turn_start_early(cx, me));
        false
    }

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

    pub fn check_for_empty_hand(&mut self) {
        if self.in_progress && self.player.effect_depth == 0 && self.player.hand.is_empty() {
            self.dispatch_g(hookbit::after_hand_emptied, |cx, me, l| l.after_hand_emptied(cx, me));
        }
    }

    fn end_player_turn(&mut self) {
        self.stage = Stage::AwaitAction;
        self.player.phase = Phase::AutoPostPlay;
        if !self.run_post_play_hooks(None) {
            return;
        }
        self.end_player_turn_rest();
    }

    fn run_post_play_hooks(&mut self, resume: Option<Me>) -> bool {
        if !(self.listen.has(hookbit::after_auto_post_play_phase_entered) && self.hooks_enabled()) {
            return true;
        }
        if !self.tick() {
            return true;
        }
        let mut snap = crate::engine::Snapshot::new();
        self.snapshot_into(Mask::bit(hookbit::after_auto_post_play_phase_entered), &mut snap);
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

    fn resume_end_turn(&mut self, me: Me) {
        if !self.run_post_play_hooks(Some(me)) {
            return;
        }
        self.end_player_turn_rest();
    }

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
        self.flush_player_hand();
        self.dispatch_g(hookbit::after_side_turn_end, |cx, me, l| l.after_side_turn_end(cx, me, Side::Player));
        self.dispatch_g(hookbit::after_side_turn_end_late, |cx, me, l| l.after_side_turn_end_late(cx, me, Side::Player));
        self.extra_turn = self.any_true_g(hookbit::should_take_extra_turn, |cx, me, l| l.should_take_extra_turn(cx, me));
        let extra = self.extra_turn;
        self.flip_sides();
        if extra {
            self.dispatch_g(hookbit::after_taking_extra_turn, |cx, me, l| l.after_taking_extra_turn(cx, me));
        }
        self.continue_after_switch();
    }

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

    fn flush_player_hand(&mut self) {
        let mut flush = true;
        if self.hooks_enabled() {
            let mut snap = crate::engine::Snapshot::new();
            self.snapshot_into(Mask::bit(hookbit::should_flush), &mut snap);
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
                let mut kept: crate::engine::CostMods = crate::util::SmallVec::new();
                for m in card.mods.iter() {
                    if m.expire() & EXPIRE_END_OF_TURN == 0 {
                        kept.push(*m);
                    }
                }
                card.mods = kept;
            }
        }
    }

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
        for i in 0..MAX_CREATURES {
            if self.creatures[i].in_combat && !self.creatures[i].is_player {
                self.creatures[i].monster.spawned_this_turn = false;
            }
        }
        self.hist = History::default();
    }

    fn continue_after_switch(&mut self) {
        if self.side == Side::Enemy {
            self.run_enemy_turn();
        } else if self.in_progress {
            self.start_player_turn();
        }
    }

    fn switch_sides(&mut self) {
        self.flip_sides();
        self.continue_after_switch();
    }

    fn run_enemy_turn(&mut self) {
        if self.start_enemy_turn() {
            return;
        }
        let snapshot = self.creatures_on(Side::Enemy);
        self.enemy_turn_from(snapshot, 0);
    }

    fn start_enemy_turn(&mut self) -> bool {
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
        self.check_win_condition()
    }

    fn enemy_turn_from(&mut self, snapshot: crate::util::ArrayVec<Cid, MAX_CREATURES>, from: usize) {
        for i in from..snapshot.len() {
            let e = snapshot[i];
            if !self.enemies.contains(e) {
                continue;
            }
            if !self.cr(e).monster.spawned_this_turn {
                if let Some(nm) = self.perform_move(e) {
                    self.enemy_cont = Some((snapshot, i as u8, nm));
                    return;
                }
            }
            if self.check_win_condition() {
                return;
            }
        }
        if self.end_enemy_turn(true) {
            self.flip_sides();
            return;
        }
        self.switch_sides();
    }

    fn end_enemy_turn(&mut self, cleanup: bool) -> bool {
        self.dispatch_g(hookbit::before_side_turn_end_very_early, |cx, me, l| l.before_side_turn_end_very_early(cx, me, Side::Enemy));
        self.dispatch_g(hookbit::before_side_turn_end_early, |cx, me, l| l.before_side_turn_end_early(cx, me, Side::Enemy));
        self.dispatch_g(hookbit::before_side_turn_end, |cx, me, l| l.before_side_turn_end(cx, me, Side::Enemy));
        if cleanup {
            self.end_of_turn_cleanup();
        }
        self.dispatch_g(hookbit::after_side_turn_end, |cx, me, l| l.after_side_turn_end(cx, me, Side::Enemy));
        self.dispatch_g(hookbit::after_side_turn_end_late, |cx, me, l| l.after_side_turn_end_late(cx, me, Side::Enemy));
        self.check_win_condition()
    }

    pub(crate) fn look_turn(&mut self) -> bool {
        #[cfg(feature = "obs_prof")]
        #[allow(unused_assignments)]
        let mut _t = unsafe { core::arch::x86_64::_rdtsc() };
        self.budget_reset();
        self.look_drop_decision();
        let mut snapshot = self.creatures_on(Side::Enemy);
        let mut from = 0;
        if let Some((snap, i, nm)) = self.enemy_cont.take() {
            self.finish_move(snap[i as usize], nm);
            snapshot = snap;
            from = i as usize + 1;
        } else if self.side == Side::Player {
            self.player.phase = Phase::End;
            self.dispatch_g(hookbit::before_side_turn_end_very_early, |cx, me, l| l.before_side_turn_end_very_early(cx, me, Side::Player));
            self.dispatch_g(hookbit::before_side_turn_end_early, |cx, me, l| l.before_side_turn_end_early(cx, me, Side::Player));
            self.dispatch_g(hookbit::before_side_turn_end, |cx, me, l| l.before_side_turn_end(cx, me, Side::Player));
            self.dispatch_g(hookbit::after_side_turn_end, |cx, me, l| l.after_side_turn_end(cx, me, Side::Player));
            self.dispatch_g(hookbit::after_side_turn_end_late, |cx, me, l| l.after_side_turn_end_late(cx, me, Side::Player));
            self.look_drop_decision();
            self.flip_sides();
            if self.check_win_condition() || self.start_enemy_turn() {
                return false;
            }
            snapshot = self.creatures_on(Side::Enemy);
        }
        #[cfg(feature = "obs_prof")]
        unsafe {
            let n = core::arch::x86_64::_rdtsc();
            crate::observe::OBS_PROF[22] += n - _t;
            _t = n;
        }
        for &e in snapshot.iter().skip(from) {
            if !self.enemies.contains(e) || self.cr(e).monster.spawned_this_turn || self.cr(e).monster.next_move == NO {
                continue;
            }
            if let Some(nm) = self.perform_move(e) {
                self.look_drop_decision();
                self.finish_move(e, nm);
            }
            if self.check_win_condition() {
                return false;
            }
        }
        #[cfg(feature = "obs_prof")]
        unsafe {
            let n = core::arch::x86_64::_rdtsc();
            crate::observe::OBS_PROF[23] += n - _t;
            _t = n;
        }
        if self.end_enemy_turn(false) {
            return false;
        }
        #[cfg(feature = "obs_prof")]
        unsafe {
            let n = core::arch::x86_64::_rdtsc();
            crate::observe::OBS_PROF[24] += n - _t;
            _t = n;
        }

        self.look_drop_decision();
        self.flip_sides();
        let list = self.creatures_on(Side::Player);
        self.before_turn_start_for(&list);
        self.dispatch_g(hookbit::before_side_turn_start, |cx, me, l| l.before_side_turn_start(cx, me, Side::Player));
        self.player.phase = Phase::Start;
        for &c in list.iter() {
            self.clear_block(c);
        }
        self.look_drop_decision();
        #[cfg(feature = "obs_prof")]
        unsafe {
            let n = core::arch::x86_64::_rdtsc();
            crate::observe::OBS_PROF[25] += n - _t;
            _t = n;
        }
        self.in_progress && !self.is_ending()
    }

    fn look_drop_decision(&mut self) {
        for _ in 0..4 {
            if self.stage != Stage::AwaitChoice {
                break;
            }
            self.decision = None;
            self.choice.cards.clear();
            self.stage = Stage::AwaitAction;
            if let Some((me, phase)) = self.hook_ctx.take() {
                if me.kind == Kind::Monster {
                    content::listener(&me).resume_hook(self, me, phase);
                }
            }
        }
        self.play_stack.clear();
        self.potion_ctx = None;
        self.hook_after = None;
        self.draw_pass = None;
        self.turn_cont = 0;
        self.end_turn_resume = None;
        self.susp.clear();
    }

    fn resume_enemy_turn(&mut self) {
        let Some((snapshot, i, nm)) = self.enemy_cont.take() else { return };
        self.finish_move(snapshot[i as usize], nm);
        if self.check_win_condition() {
            return;
        }
        self.enemy_turn_from(snapshot, i as usize + 1);
    }

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

    fn end_combat_victory(&mut self) {
        self.in_progress = false;
        self.extra_turn = false;
        self.player.phase = Phase::None;
        self.dispatch_u(hookbit::after_combat_end, |cx, me, l| l.after_combat_end(cx, me));
        self.cr_mut(PLAYER).powers.clear();
        self.sync_secondary(PLAYER);
        self.cr_mut(PLAYER).set_block(0);
        self.player.hand.clear();
        self.player.draw.clear();
        self.player.discard.clear();
        self.player.exhaust.clear();
        self.player.play.clear();
        self.dispatch_u(hookbit::after_combat_victory_early, |cx, me, l| l.after_combat_victory_early(cx, me));
        self.dispatch_u(hookbit::after_combat_victory, |cx, me, l| l.after_combat_victory(cx, me));
        self.hist_log.clear();
        self.outcome = Outcome::Victory;
        self.stage = Stage::Over;
    }

    /// Upper bound on the HP the player can still gain before a won fight ends, victory heals included; None when a source
    /// (Feed, Not Yet, Book Repair Knife, Dragon Fruit) has no small bound. Cards and potions created later in the fight are not counted.
    pub fn hp_gain_bound(&self) -> Option<i32> {
        use crate::ids::{card, potion, power, relic};
        let p = &self.player;
        if p.relics.iter().any(|r| matches!(r.id, relic::BOOK_REPAIR_KNIFE | relic::DRAGON_FRUIT)) {
            return None;
        }
        let in_play = [&p.hand, &p.draw, &p.discard, &p.play];
        if in_play.iter().any(|pile| pile.iter().any(|&c| matches!(self.cards[c as usize].id, card::FEED | card::NOT_YET))) {
            return None;
        }
        let max = self.cr(PLAYER).max_hp;
        let mut regen = self.power_amount(PLAYER, power::REGEN_POWER);
        let mut gain = 0;
        for pot in p.potions.iter().flatten() {
            match pot.id {
                potion::BLOOD_POTION | potion::AMBERGRIS => gain += max * self.potion_named_var(pot.id, content::gen_cards::var_name::HEAL_PERCENT) / 100 + 1,
                potion::FAIRY_IN_A_BOTTLE => gain += max * 3 / 10 + 1,
                potion::FRUIT_JUICE => gain += self.potion_var(pot.id, crate::defs::VarKind::MaxHp),
                potion::REGEN_POTION => regen += self.potion_power_var(pot.id, power::REGEN_POWER),
                _ => {}
            }
        }
        gain += regen * (regen + 1) / 2;
        if p.relics.iter().any(|r| r.id == relic::LIZARD_TAIL && !r.flag(0)) {
            gain += max / 2 + 1;
        }
        let mut won = self.clone();
        won.cr_mut(PLAYER).set_hp(1);
        won.end_combat_victory();
        Some(gain + won.cr(PLAYER).hp() - 1)
    }

    pub fn step(&mut self, a: Action) -> bool {
        self.sync_overflow();
        self.budget_reset();
        let ok = if self.strat_possible { self.step_replayed(a) } else { self.step_inner(a) };
        self.sync_overflow();
        if self.overflow & ov::LOOP != 0 {
            self.stage = Stage::Over;
        }
        ok
    }

    #[inline]
    pub fn sync_overflow(&mut self) {
        let o = crate::util::take_overflow();
        if o != 0 {
            self.overflow |= o as u16;
        }
    }

    pub(crate) fn step_inner(&mut self, a: Action) -> bool {
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

    pub fn step_pick_game_order(&mut self, idx: u8) -> bool {
        let Some(d) = self.decision.as_ref() else { return false };
        let Some(pos) = self.decision_view(d).iter().position(|&g| g == idx) else { return false };
        self.step(Action::Pick { idx: pos as u8 })
    }

    pub(crate) fn resume_after_decision(&mut self) {
        if let Some((me, phase)) = self.hook_ctx.take() {
            content::listener(&me).resume_hook(self, me, phase);
            if self.stage == Stage::AwaitChoice {
                return;
            }
        }
        if self.hook_after.is_some() {
            if let Some((_, 2)) = self.draw_pass {
                self.draw_pass = None;
                if self.dispatch_resumable(hookbit::after_shuffle, |cx, me, l| l.after_shuffle(cx, me)) {
                    self.draw_pass = Some((NO, 2));
                    return;
                }
            }
            let (me, phase) = self.hook_after.take().unwrap();
            content::listener(&me).resume_hook(self, me, phase);
            if self.stage == Stage::AwaitChoice {
                return;
            }
        }
        if self.enemy_cont.is_some() {
            self.resume_enemy_turn();
            return;
        }
        if !self.play_stack.is_empty() {
            self.run_play_stack();
            if self.stage == Stage::AwaitChoice {
                return;
            }
            if let Some(me) = self.end_turn_resume.take() {
                self.resume_end_turn(me);
                return;
            }
        }
        if self.potion_ctx.is_some() {
            self.run_potion();
        }
        if self.turn_cont != 0 && self.stage != Stage::AwaitChoice && self.play_stack.is_empty() {
            let t = self.turn_cont;
            self.turn_cont = 0;
            self.resume_turn_start(t);
        }
    }

    fn after_action(&mut self) {
        self.check_win_condition();
        self.consume_end_turn_request();
    }
}
