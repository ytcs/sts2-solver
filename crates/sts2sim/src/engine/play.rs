use crate::content;
use crate::engine::HKind;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunResult {
    Finished,
    Suspended,
}

impl Combat {
    pub fn card_target_type(&self, c: CardIdx) -> TargetType {
        let d = self.card_def(c);
        if d.id == crate::ids::card::SHIV && self.has_power(PLAYER, crate::ids::power::FAN_OF_KNIVES_POWER) {
            TargetType::AllEnemies
        } else if d.id == crate::ids::card::SOVEREIGN_BLADE && self.has_power(PLAYER, crate::ids::power::SEEKING_EDGE_POWER) {
            TargetType::AllEnemies
        } else {
            d.target
        }
    }

    pub fn is_valid_target(&self, c: CardIdx, t: Cid) -> bool {
        let tt = self.card_target_type(c);
        if t == NO {
            return tt != TargetType::AnyEnemy && tt != TargetType::AnyAlly;
        }
        let cr = self.cr(t);
        if !cr.in_combat || cr.is_dead() {
            return false;
        }
        match tt {
            TargetType::AnyEnemy => cr.side == Side::Enemy,
            TargetType::AnyAlly => cr.side == Side::Player,
            _ => false,
        }
    }

    #[inline]
    pub fn should_play_preventer(&self, c: CardIdx, kind: AutoPlayType) -> Option<Me> {
        if !self.listen.intersects(Mask::bit(hookbit::should_play) | Mask::bit(hookbit::should_play_kind)) {
            return None;
        }
        self.should_play_preventer_slow(c, kind)
    }

    #[inline(never)]
    fn should_play_preventer_slow(&self, c: CardIdx, kind: AutoPlayType) -> Option<Me> {
        if !self.hooks_enabled() {
            return None;
        }
        let mut snap = crate::engine::Snapshot::new();
        self.snapshot_into(Mask::bit(hookbit::should_play) | Mask::bit(hookbit::should_play_kind), &mut snap);
        for e in snap.iter() {
            if !self.still_live(&e.me) {
                continue;
            }
            let l = content::listener(&e.me);
            let ok = (!self.has_hook(&e.me, hookbit::should_play) || l.should_play(self, e.me, c)) && (!self.has_hook(&e.me, hookbit::should_play_kind) || l.should_play_kind(self, e.me, c, kind));
            if !ok {
                return Some(e.me);
            }
        }
        None
    }

    pub fn has_enough_resources_for(&self, c: CardIdx) -> bool {
        let mut e = self.card_cost(c, true).max(0);
        let mut s = self.card_star_cost(c).max(0);
        if e > self.player.energy && self.any_true_g(hookbit::should_pay_excess_energy_cost_with_stars, |cx, me, l| l.should_pay_excess_energy_cost_with_stars(cx, me)) {
            s += (e - self.player.energy) * 2;
            e = self.player.energy;
        }
        !(e > self.player.energy || s > self.player.stars)
    }

    pub fn can_play(&self, c: CardIdx) -> bool {
        let kws = self.card_keywords(c);
        if kws & kw::UNPLAYABLE != 0 {
            return false;
        }
        let d = self.card_def(c);
        if !d.x_cost && !self.has_enough_resources_for(c) {
            return false;
        }
        if d.x_cost && self.card_star_cost(c) > self.player.stars {
            return false;
        }
        if self.card_target_type(c) == TargetType::AnyAlly {
            return false;
        }
        if self.should_play_preventer(c, AutoPlayType::None).is_some() {
            return false;
        }
        content::listener(&Me { kind: Kind::Card, owner: PLAYER, idx: c as u16, id: self.cards[c as usize].id, amount: 0 }).is_playable(self, c)
    }

    pub fn spend_resources(&mut self, c: CardIdx) -> (i32, i32) {
        let d = self.card_def(c);
        let energy_to_spend = if d.x_cost { self.player.energy } else { self.card_cost(c, true).max(0) };
        let stars_to_spend = self.card_star_cost(c).max(0);
        if d.x_cost {
            self.cards[c as usize].x_value = energy_to_spend as i16;
            self.cards[c as usize].flags |= cflag::X_CAPTURED;
        }
        if self.card_has_star_cost_x(c) {
            self.cards[c as usize].x_value = stars_to_spend as i16;
            self.cards[c as usize].flags |= cflag::X_CAPTURED;
        }
        if energy_to_spend > 0 {
            self.hist_push(HKind::EnergySpent, PLAYER, NO, self.cards[c as usize].id, c, energy_to_spend, 0, 0, 0);
            self.lose_energy(energy_to_spend);
        }
        self.dispatch_g(hookbit::after_energy_spent, |cx, me, l| l.after_energy_spent(cx, me, c, energy_to_spend));
        if stars_to_spend > 0 {
            self.lose_stars(stars_to_spend);
            self.dispatch_g(hookbit::after_stars_spent, |cx, me, l| l.after_stars_spent(cx, me, stars_to_spend));
        }
        (energy_to_spend, stars_to_spend)
    }

    pub fn play_card(&mut self, hand_pos: usize, target: Cid) -> bool {
        if self.stage != Stage::AwaitAction || self.player.phase != Phase::Play {
            return false;
        }
        let Some(c) = self.player.hand.get(hand_pos) else { return false };
        if !self.can_play(c) || !self.is_valid_target(c, target) {
            return false;
        }
        let (energy_to_spend, stars_to_spend) = self.spend_resources(c);
        let play = CardPlay {
            card: c,
            target,
            is_auto: false,
            play_index: 0,
            play_count: 1,
            result_pile: PileType::Discard,
            energy_spent: energy_to_spend,
            stars_spent: stars_to_spend,
            energy_value: energy_to_spend,
        };
        self.begin_play(play);
        true
    }

    fn default_result_location(&mut self, c: CardIdx) -> CardLocation {
        let kws = self.card_keywords(c);
        let flags = self.cards[c as usize].flags;
        let base = if flags & cflag::IS_DUPE != 0 || self.card_def(c).ctype == CardType::Power {
            CardLocation::new(PileType::None, CardPilePosition::Bottom)
        } else if flags & cflag::EXHAUST_ON_NEXT_PLAY != 0 || kws & kw::EXHAUST != 0 {
            self.cards[c as usize].flags &= !cflag::EXHAUST_ON_NEXT_PLAY;
            CardLocation::new(PileType::Exhaust, CardPilePosition::Bottom)
        } else {
            CardLocation::new(PileType::Discard, CardPilePosition::Bottom)
        };
        let me = Me { kind: Kind::Card, owner: PLAYER, idx: c as u16, id: self.cards[c as usize].id, amount: 0 };
        content::listener(&me).get_result_location_for_card_play(self, me, c, base)
    }

    fn modify_result_location(&mut self, c: CardIdx, is_auto: bool, energy_value: i32, mut loc: CardLocation) -> CardLocation {
        if !self.listen.has(hookbit::modify_card_play_result_location) || !self.hooks_enabled() {
            return loc;
        }
        let mut snap = crate::engine::Snapshot::new();
        self.snapshot_into(Mask::bit(hookbit::modify_card_play_result_location), &mut snap);
        let mut mods = super::Mods::new();
        for e in snap.iter() {
            if self.still_live(&e.me) {
                let n = content::listener(&e.me).modify_card_play_result_location(self, e.me, c, is_auto, energy_value, loc);
                if n != loc {
                    mods.push(e.me);
                }
                loc = n;
            }
        }
        let l = loc;
        self.dispatch_modifiers(true, hookbit::after_modifying_card_play_result_location, &mods, |cx, me, li| li.after_modifying_card_play_result_location(cx, me, c, l));
        loc
    }

    pub fn enchanted_replay_count(&self, c: CardIdx) -> i32 {
        let base = self.cards[c as usize].base_replay as i32;
        if self.cards[c as usize].enchant != 0 {
            let me = self.enchantment_me(c);
            content::listener(&me).enchant_play_count(self, me, base)
        } else {
            base
        }
    }

    pub fn generate_play_count(&mut self, c: CardIdx, target: Cid) -> i32 {
        let mut count = self.enchanted_replay_count(c) + 1;
        if self.listen.has(hookbit::modify_card_play_count) && self.hooks_enabled() {
            let mut snap = crate::engine::Snapshot::new();
            self.snapshot_into(Mask::bit(hookbit::modify_card_play_count), &mut snap);
            let mut mods = super::Mods::new();
            for e in snap.iter() {
                if self.still_live(&e.me) {
                    let n = content::listener(&e.me).modify_card_play_count(self, e.me, c, target, count);
                    if n != count {
                        mods.push(e.me);
                    }
                    count = n;
                }
            }
            self.dispatch_modifiers(true, hookbit::after_modifying_card_play_count, &mods, |cx, me, li| li.after_modifying_card_play_count(cx, me, c));
        }
        count
    }

    pub(crate) fn begin_play(&mut self, mut play: CardPlay) -> RunResult {
        let c = play.card;
        if !play.is_auto {
            self.player.hand.remove_value(c);
            self.player.play.push(c);
            self.cards[c as usize].pile = PileType::Play as u8;
            self.fire_card_changed_piles(c, PileType::Hand);
        } else {
            self.move_card(c, PileType::Play, CardPilePosition::Bottom);
        }
        if self.cards[c as usize].flags & cflag::REMOVED != 0 || !self.in_progress {
            return RunResult::Finished;
        }
        let loc = self.default_result_location(c);
        let loc = self.modify_result_location(c, play.is_auto, play.energy_value, loc);
        let count = self.generate_play_count(c, play.target);
        if self.cr(PLAYER).is_dead() {
            return RunResult::Finished;
        }
        play.result_pile = loc.pile;
        play.play_count = count.clamp(0, 255) as u8;
        if self.play_stack.len() >= 6 {
            crate::util::raise_overflow(crate::util::OV_CONTAINER);
            return RunResult::Finished;
        }
        self.player.effect_depth += 1;
        self.play_stack.push(PlayCtx { play, step: PlayStep::Before, count: count.clamp(0, 255) as u8, result: loc });
        let idx = self.play_stack.len() - 1;
        self.run_play_at(idx)
    }

    pub fn run_play_stack(&mut self) {
        while !self.play_stack.is_empty() {
            let idx = self.play_stack.len() - 1;
            if self.run_play_at(idx) == RunResult::Suspended {
                return;
            }
            if self.resume_queues() == RunResult::Suspended {
                return;
            }
        }
    }

    fn run_play_at(&mut self, idx: usize) -> RunResult {
        loop {
            let mut ctx = self.play_stack[idx];
            let c = ctx.play.card;
            match ctx.step {
                PlayStep::Before => {
                    if self.is_over_or_ending() || !self.tick() {
                        return self.finish_play(idx);
                    }
                    let p = ctx.play;
                    let (next, wrapped) = self.play_serial.overflowing_add(1);
                    if wrapped {
                        crate::util::raise_overflow(ov::COUNTER as u32);
                    }
                    self.play_serial = next;
                    self.dispatch_g(hookbit::before_card_played, |cx, me, l| l.before_card_played(cx, me, &p));
                    self.hist_card_play_started(&p);
                    self.hist.cards_played_this_turn += 1;
                    match self.card_def(c).ctype {
                        CardType::Attack => self.hist.attacks_played_this_turn += 1,
                        CardType::Skill => self.hist.skills_played_this_turn += 1,
                        _ => {}
                    }
                    ctx.step = PlayStep::OnPlay(0);
                    self.play_stack[idx] = ctx;
                }
                PlayStep::OnPlay(phase) => {
                    let me = Me { kind: Kind::Card, owner: PLAYER, idx: c as u16, id: self.cards[c as usize].id, amount: 0 };
                    let p = ctx.play;
                    match content::listener(&me).on_play(self, &p, phase) {
                        Flow::Done => {
                            self.play_stack[idx].step = PlayStep::After;
                            if self.play_stack.len() > idx + 1 {
                                return RunResult::Suspended;
                            }
                        }
                        Flow::Suspend(next) => {
                            self.play_stack[idx].step = PlayStep::OnPlay(next);
                            if self.stage != Stage::AwaitChoice {
                                self.stage = Stage::AwaitChoice;
                            }
                            return RunResult::Suspended;
                        }
                    }
                }
                PlayStep::After => {
                    if self.cr(PLAYER).is_dead() {
                        return self.finish_play(idx);
                    }
                    let p = ctx.play;
                    if self.cards[c as usize].enchant != 0 {
                        let me = self.enchantment_me(c);
                        content::listener(&me).on_play_enchantment(self, me, &p);
                        if self.cr(PLAYER).is_dead() {
                            return self.finish_play(idx);
                        }
                    }
                    if self.cards[c as usize].affliction != 0 {
                        let me = self.affliction_me(c);
                        content::listener(&me).on_play_affliction(self, me, &p);
                        if self.cr(PLAYER).is_dead() {
                            return self.finish_play(idx);
                        }
                    }
                    let ethereal = (self.card_keywords(c) & kw::ETHEREAL != 0) as u8;
                    crate::engine::history::bump(&mut self.hist_log.total[HKind::CardPlayFinished as usize]);
                    self.hist.set_finished(c);
                    self.hist.cards_finished_this_turn += 1;
                    match self.card_def(c).ctype {
                        CardType::Attack => self.hist.attacks_finished_this_turn += 1,
                        CardType::Skill => self.hist.skills_finished_this_turn += 1,
                        _ => {}
                    }
                    if self.card_def(c).tags & tag::SHIV != 0 {
                        self.hist.shivs_finished_this_turn += 1;
                    }
                    if ethereal != 0 {
                        crate::engine::history::bump(&mut self.hist_log.ethereal_finished);
                    }
                    if self.in_progress {
                        self.dispatch_u(hookbit::after_card_played, |cx, me, l| l.after_card_played(cx, me, &p));
                        self.dispatch_u(hookbit::after_card_played_late, |cx, me, l| l.after_card_played_late(cx, me, &p));
                        if self.cr(PLAYER).is_dead() {
                            return self.finish_play(idx);
                        }
                    }
                    let mut ctx = self.play_stack[idx];
                    ctx.play.play_index += 1;
                    if ctx.play.play_index < ctx.count {
                        ctx.step = PlayStep::Before;
                        self.play_stack[idx] = ctx;
                    } else {
                        return self.finish_play(idx);
                    }
                }
            }
        }
    }

    fn finish_play(&mut self, idx: usize) -> RunResult {
        debug_assert_eq!(idx + 1, self.play_stack.len());
        let ctx = self.play_stack.pop().unwrap();
        let c = ctx.play.card;
        self.player.effect_depth = self.player.effect_depth.saturating_sub(1);
        if self.cr(PLAYER).is_dead() {
            return RunResult::Finished;
        }
        if self.card_pile_type(c) == PileType::Play && self.cards[c as usize].flags & cflag::REMOVED == 0 {
            match ctx.result.pile {
                PileType::None => self.remove_card_from_combat(c),
                PileType::Exhaust => self.exhaust_card(c, false),
                p => {
                    self.move_card(c, p, ctx.result.pos);
                }
            }
        }
        self.check_for_empty_hand();
        let card = &mut self.cards[c as usize];
        let mut kept: crate::engine::CostMods = crate::util::SmallVec::new();
        for m in card.mods.iter() {
            if m.expire() & EXPIRE_WHEN_PLAYED == 0 {
                kept.push(*m);
            }
        }
        card.mods = kept;
        card.flags &= !cflag::X_CAPTURED;
        self.clear_star_mods(c, EXPIRE_WHEN_PLAYED);
        RunResult::Finished
    }
}
