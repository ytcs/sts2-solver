use super::play::RunResult;
use crate::content;
use crate::engine::HKind;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

impl Combat {
    pub fn move_to_result_pile_without_playing(&mut self, c: CardIdx) {
        self.move_card(c, PileType::Play, CardPilePosition::Bottom);
        if self.card_pile_type(c) != PileType::Play {
            return;
        }
        let flags = self.cards[c as usize].flags;
        if flags & cflag::IS_DUPE != 0 {
            self.remove_card_from_combat(c);
        } else if flags & cflag::EXHAUST_ON_NEXT_PLAY != 0 || self.card_keywords(c) & kw::EXHAUST != 0 {
            self.exhaust_card(c, false);
        } else {
            self.move_card(c, PileType::Discard, CardPilePosition::Bottom);
        }
    }

    pub fn auto_play(&mut self, c: CardIdx, target: Cid, kind: AutoPlayType, skip_x_capture: bool) -> RunResult {
        if self.is_over_or_ending() || self.cr(PLAYER).is_dead() {
            return RunResult::Finished;
        }
        if self.card_keywords(c) & kw::UNPLAYABLE != 0 {
            self.move_to_result_pile_without_playing(c);
            return RunResult::Finished;
        }
        if self.should_play_preventer(c, kind).is_some() {
            self.move_to_result_pile_without_playing(c);
            return RunResult::Finished;
        }
        let mut target = target;
        match self.card_target_type(c) {
            TargetType::AnyEnemy => {
                if target == NO {
                    let h = self.hittable_enemies();
                    if !h.is_empty() {
                        target = h[self.rng.combat_targets.next_int_range(0, h.len() as i32) as usize];
                    }
                }
                if target == NO {
                    self.move_to_result_pile_without_playing(c);
                    return RunResult::Finished;
                }
            }
            TargetType::AnyAlly => {
                self.move_to_result_pile_without_playing(c);
                return RunResult::Finished;
            }
            _ => {}
        }
        let d = self.card_def(c);
        if d.x_cost && !skip_x_capture {
            self.cards[c as usize].x_value = self.player.energy as i16;
            self.cards[c as usize].flags |= cflag::X_CAPTURED;
        }
        if !skip_x_capture && self.card_has_star_cost_x(c) {
            self.cards[c as usize].x_value = self.player.stars as i16;
            self.cards[c as usize].flags |= cflag::X_CAPTURED;
        }
        if self.card_pile_type(c) == PileType::None {
            self.move_card(c, PileType::Play, CardPilePosition::Bottom);
        }
        self.dispatch_g(hookbit::before_card_auto_played, |cx, me, l| l.before_card_auto_played(cx, me, c, target, kind));
        let energy_value = if d.x_cost { self.player.energy } else { self.card_cost(c, true).max(0) };
        let play = CardPlay {
            card: c,
            target,
            is_auto: true,
            play_index: 0,
            play_count: 1,
            result_pile: PileType::Discard,
            energy_spent: 0,
            stars_spent: 0,
            energy_value,
        };
        self.begin_play(play)
    }

    pub fn auto_play_from_draw_pile(&mut self, count: i32, pos: CardPilePosition, force_exhaust: bool) -> RunResult {
        if self.is_over_or_ending() {
            return RunResult::Finished;
        }
        let mut cards: ArrayVec<CardIdx, AUTOPLAY_MAX> = ArrayVec::new();
        if count > AUTOPLAY_MAX as i32 {
            crate::util::raise_overflow(crate::util::OV_CONTAINER);
        }
        for _ in 0..count.min(AUTOPLAY_MAX as i32) {
            self.shuffle_if_necessary();
            let n = self.player.draw.len();
            let c = match pos {
                CardPilePosition::Bottom => self.player.draw.last(),
                CardPilePosition::Top => self.player.draw.first(),
                CardPilePosition::Random => {
                    if n == 0 {
                        None
                    } else {
                        self.player.draw.get(self.rng.combat_card_selection.next_int_range(0, n as i32) as usize)
                    }
                }
            };
            let Some(c) = c else { break };
            cards.push(c);
            self.move_card(c, PileType::Play, CardPilePosition::Bottom);
        }
        let owner = self.play_stack.len() as i8 - 1;
        self.autoplay_stack.push(AutoQueue { cards, force_exhaust, sly: false, plain: false, owner });
        self.drain_top_queue()
    }

    pub fn auto_play_list(&mut self, cards: &[CardIdx]) -> RunResult {
        let mut q: ArrayVec<CardIdx, AUTOPLAY_MAX> = ArrayVec::new();
        if cards.len() > AUTOPLAY_MAX {
            crate::util::raise_overflow(crate::util::OV_CONTAINER);
        }
        for &c in cards.iter().take(AUTOPLAY_MAX) {
            q.push(c);
        }
        let owner = self.play_stack.len() as i8 - 1;
        self.autoplay_stack.push(AutoQueue { cards: q, force_exhaust: false, sly: false, plain: true, owner });
        self.drain_top_queue()
    }

    pub(crate) fn drain_top_queue(&mut self) -> RunResult {
        loop {
            let Some(q) = self.autoplay_stack.as_mut_slice().last_mut() else { return RunResult::Finished };
            if q.cards.is_empty() {
                self.autoplay_stack.pop();
                return RunResult::Finished;
            }
            let c = q.cards.remove(0);
            let (sly, fe, plain) = (q.sly, q.force_exhaust, q.plain);
            if !sly && self.cr(PLAYER).is_dead() {
                self.autoplay_stack.pop();
                return RunResult::Finished;
            }
            if !sly {
                if self.cards[c as usize].flags & cflag::REMOVED != 0 {
                    continue;
                }
                if plain {
                } else if fe {
                    self.cards[c as usize].flags |= cflag::EXHAUST_ON_NEXT_PLAY;
                } else {
                    self.cards[c as usize].flags &= !cflag::EXHAUST_ON_NEXT_PLAY;
                }
            }
            let kind = if sly { AutoPlayType::SlyDiscard } else { AutoPlayType::Default };
            if self.auto_play(c, NO, kind, false) == RunResult::Suspended {
                return RunResult::Suspended;
            }
        }
    }

    pub(crate) fn resume_queues(&mut self) -> RunResult {
        loop {
            let Some(q) = self.autoplay_stack.last() else { return RunResult::Finished };
            let len = self.play_stack.len() as i8;
            if q.owner >= len {
                self.autoplay_stack.pop();
            } else if q.owner == len - 1 {
                if self.drain_top_queue() == RunResult::Suspended {
                    return RunResult::Suspended;
                }
            } else {
                return RunResult::Finished;
            }
        }
    }

    pub fn discard_cards(&mut self, cards: &[CardIdx], cards_to_draw: i32) -> RunResult {
        if self.is_over_or_ending() || cards.is_empty() {
            return RunResult::Finished;
        }
        let mut sly: ArrayVec<CardIdx, MAX_HAND> = ArrayVec::new();
        for &c in cards {
            if self.is_sly_this_turn(c) {
                sly.push(c);
            }
            self.move_card(c, PileType::Discard, CardPilePosition::Bottom);
            let id = self.cards[c as usize].id;
            self.hist_push(HKind::CardDiscarded, PLAYER, NO, id, c, 0, 0, 0, 0);
            self.dispatch_g(hookbit::after_card_discarded, |cx, me, l| l.after_card_discarded(cx, me, c));
        }
        if cards_to_draw > 0 {
            self.draw_cards(cards_to_draw, false);
        }
        let owner = self.play_stack.len() as i8 - 1;
        let mut q: ArrayVec<CardIdx, AUTOPLAY_MAX> = ArrayVec::new();
        for &c in sly.iter() {
            q.push(c);
        }
        self.autoplay_stack.push(AutoQueue { cards: q, force_exhaust: false, sly: true, plain: false, owner });
        self.drain_top_queue()
    }

    pub fn is_sly_this_turn(&self, c: CardIdx) -> bool {
        self.card_keywords(c) & kw::SLY != 0 || self.cards[c as usize].flags & cflag::SINGLE_TURN_SLY != 0
    }

    pub fn should_retain_this_turn(&self, c: CardIdx) -> bool {
        self.card_keywords(c) & kw::RETAIN != 0 || self.cards[c as usize].flags & cflag::SINGLE_TURN_RETAIN != 0
    }

    pub fn create_dupe(&mut self, c: CardIdx) -> Option<CardIdx> {
        if self.cards[c as usize].flags & cflag::IS_DUPE != 0 {
            let orig = self.cards[c as usize].dupe_of;
            if orig != NO {
                return self.create_dupe(orig);
            }
        }
        let n = self.clone_card(c)?;
        let card = &mut self.cards[n as usize];
        card.flags |= cflag::IS_DUPE;
        card.dupe_of = c;
        card.kw_add &= !kw::EXHAUST;
        card.kw_remove |= kw::EXHAUST;
        Some(n)
    }

    pub fn pool_of(card_id: u16) -> &'static [u16] {
        use crate::content::gen_pools as p;
        for pool in [&p::IRONCLAD[..], &p::SILENT[..], &p::DEFECT[..], &p::NECROBINDER[..], &p::REGENT[..], &p::COLORLESS[..], &p::CURSE[..], &p::STATUS[..], &p::TOKEN[..], &p::EVENT[..], &p::QUEST[..]] {
            if pool.contains(&card_id) {
                return pool;
            }
        }
        &[]
    }

    pub fn transform_options(&self, original: CardIdx) -> ArrayVec<u16, 128> {
        let d = self.card_def(original);
        let pool: &[u16] = if d.ctype == CardType::Quest || matches!(d.rarity, CardRarity::Event | CardRarity::Ancient | CardRarity::Token) {
            &crate::content::gen_pools::COLORLESS[..]
        } else {
            Self::pool_of(d.id)
        };
        let mut out: ArrayVec<u16, 128> = ArrayVec::new();
        for &id in pool {
            let cd = content::card_def(id);
            let rar_ok = matches!(d.rarity, CardRarity::Status | CardRarity::Curse) || matches!(cd.rarity, CardRarity::Common | CardRarity::Uncommon | CardRarity::Rare);
            if rar_ok && cd.can_be_generated_in_combat && id != d.id && !cd.multiplayer_only {
                out.push(id);
            }
        }
        out
    }

    pub fn transform_cards(&mut self, originals: &[CardIdx], replacements: &[Option<(u16, u8)>]) -> ArrayVec<CardIdx, 10> {
        let mut out: ArrayVec<CardIdx, 10> = ArrayVec::new();
        if self.is_ending() || originals.is_empty() {
            return out;
        }
        let mut work: ArrayVec<(u8, u8, CardIdx, u16, u8), 10> = ArrayVec::new();
        for (i, &o) in originals.iter().enumerate().take(10) {
            let pile = self.card_pile_type(o);
            let idx = self.pile(pile).iter().position(|&x| x == o).unwrap_or(0);
            let (rid, rup) = match replacements.get(i).copied().flatten() {
                Some(r) => r,
                None => {
                    let opts = self.transform_options(o);
                    assert!(!opts.is_empty(), "All transformation options provided are invalid!");
                    (opts[self.rng.combat_card_selection.next_int_range(0, opts.len() as i32) as usize], 0)
                }
            };
            self.pile_mut(pile).remove_value(o);
            work.push((pile as u8, idx as u8, o, rid, rup));
        }
        let sl = work.as_mut_slice();
        for i in 1..sl.len() {
            let x = sl[i];
            let mut j = i;
            while j > 0 && (sl[j - 1].0, sl[j - 1].1) > (x.0, x.1) {
                sl[j] = sl[j - 1];
                j -= 1;
            }
            sl[j] = x;
        }
        let work2 = work;
        for &(pile, idx, _o, rid, rup) in work2.iter() {
            let Some(n) = self.new_card(rid, rup) else { continue };
            let pt = match pile {
                1 => PileType::Draw,
                2 => PileType::Hand,
                3 => PileType::Discard,
                4 => PileType::Exhaust,
                _ => PileType::Play,
            };
            let at = (idx as usize).min(self.pile(pt).len());
            self.pile_mut(pt).insert(at, n);
            self.cards[n as usize].pile = pt as u8;
            self.hist_card_generated(n, true);
            self.dispatch_g(hookbit::after_card_entered_combat, |cx, me, l| l.after_card_entered_combat(cx, me, n));
            self.fire_card_changed_piles(n, pt);
            out.push(n);
        }
        for &n in out.iter() {
            self.dispatch_g(hookbit::after_card_generated_for_combat, |cx, me, l| l.after_card_generated_for_combat(cx, me, n, true));
        }
        for &(_, _, o, _, _) in work2.iter() {
            self.cards[o as usize].pile = PileType::None as u8;
            self.cards[o as usize].flags |= cflag::REMOVED;
        }
        out
    }
}
