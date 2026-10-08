use crate::dec::Dec;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

pub enum Ask {
    Resolved(ArrayVec<CardIdx, 16>),
    Pending,
}

impl Combat {
    pub fn flag_missing(&mut self, kind: Kind, id: u16) {
        if self.missing.is_none() {
            self.missing = Some((kind, id));
        }
    }

    pub fn character_pool(&self) -> &'static [u16] {
        use crate::content::gen_pools as p;
        match self.character {
            0 => &p::IRONCLAD,
            1 => &p::SILENT,
            2 => &p::DEFECT,
            3 => &p::NECROBINDER,
            _ => &p::REGENT,
        }
    }

    pub fn get_distinct_for_combat(&mut self, pool: &[u16], count: usize, extra: impl Fn(&crate::defs::CardDef) -> bool) -> ArrayVec<CardIdx, 16> {
        let mut list: ArrayVec<u16, 256> = ArrayVec::new();
        for &id in pool {
            let d = crate::content::card_def(id);
            if !d.multiplayer_only && extra(d) && d.can_be_generated_in_combat
                && !matches!(d.rarity, CardRarity::Basic | CardRarity::Ancient | CardRarity::Event)
            {
                list.push(id);
            }
        }
        self.rng.combat_card_generation.shuffle(list.as_mut_slice());
        let mut out = ArrayVec::new();
        for &id in list.iter().take(count) {
            if let Some(c) = self.new_card(id, 0) {
                out.push(c);
            }
        }
        out
    }

    pub fn unstable_shuffle_cards(&mut self, list: &mut [CardIdx], stream: crate::state::RngStream) {
        self.rng_stream_mut(stream).shuffle(list);
    }

    pub fn stable_shuffle_cards(&mut self, list: &mut [CardIdx], stream: crate::state::RngStream) {
        let cards = &self.cards;
        crate::sort::intro_sort(list, |a, b| Combat::card_cmp(cards, a, b));
        self.rng_stream_mut(stream).shuffle(list);
    }

    pub fn get_for_combat(&mut self, pool: &[u16], count: usize) -> ArrayVec<CardIdx, 16> {
        self.get_for_combat_where(pool, count, |_| true)
    }

    pub fn get_for_combat_where(&mut self, pool: &[u16], count: usize, extra: impl Fn(&crate::defs::CardDef) -> bool) -> ArrayVec<CardIdx, 16> {
        let mut list: ArrayVec<u16, 256> = ArrayVec::new();
        for &id in pool {
            let d = crate::content::card_def(id);
            if !d.multiplayer_only && extra(d) && d.can_be_generated_in_combat && !matches!(d.rarity, CardRarity::Basic | CardRarity::Ancient | CardRarity::Event) {
                list.push(id);
            }
        }
        let mut out = ArrayVec::new();
        if list.is_empty() {
            return out;
        }
        for _ in 0..count {
            let i = self.rng.combat_card_generation.next_int_range(0, list.len() as i32) as usize;
            if let Some(c) = self.new_card(list[i], 0) {
                out.push(c);
            }
        }
        out
    }

    pub fn set_to_free_this_turn(&mut self, c: CardIdx) {
        let canonical = self.card_def(c).cost;
        if canonical >= 0 {
            let card = &mut self.cards[c as usize];
            card.mods.push(CostMod::new(0, false, false, EXPIRE_END_OF_TURN | EXPIRE_WHEN_PLAYED));
        }
        self.set_star_cost_this_turn(c, 0);
    }

    pub fn gain_gold(&mut self, n: i32) {
        let mut v = Dec::int(n as i64);
        if self.listen.has(hookbit::modify_gold_gained) {
            let mut snap = crate::engine::Snapshot::new();
            self.snapshot_into(Mask::bit(hookbit::modify_gold_gained), &mut snap);
            for e in snap.iter() {
                if self.still_live(&e.me) {
                    v = crate::content::listener(&e.me).modify_gold_gained(self, e.me, v);
                }
            }
        }
        if v > Dec::ZERO {
            self.gold = self.gold.saturating_add(v.trunc());
            self.dispatch_u(hookbit::after_gold_gained, |cx, me, l| l.after_gold_gained(cx, me));
        }
    }

    pub fn lose_gold(&mut self, n: i32) -> i32 {
        let l = n.clamp(0, self.gold);
        self.gold -= l;
        l
    }

    pub fn hittable_enemies(&self) -> ArrayVec<Cid, MAX_CREATURES> {
        let mut o = ArrayVec::new();
        for &e in self.enemies.iter() {
            if self.cr(e).is_alive() && self.cr(e).in_combat && self.should_allow_hitting(e) {
                o.push(e);
            }
        }
        o
    }

    pub fn apply_power_to_hittable_enemies(&mut self, id: u16, amount: Dec, applier: Cid, card: CardIdx) {
        let targets = self.hittable_enemies();
        for &t in targets.iter() {
            self.apply_power(id, t, amount, applier, card);
        }
    }

    pub fn random_hand_card(&mut self) -> Option<CardIdx> {
        let n = self.player.hand.len();
        if n == 0 {
            return None;
        }
        let i = self.rng.combat_card_selection.next_int_range(0, n as i32) as usize;
        Some(self.player.hand[i])
    }

    pub fn upgrade_in_combat(&mut self, c: CardIdx) {
        if self.cards[c as usize].upgrade < self.card_def(c).max_upgrade {
            self.upgrade_card(c);
        }
    }

    pub fn deck_upgradable_count(&self) -> usize {
        (0..self.deck_len as usize).filter(|&i| self.deck_upgrade[i] < crate::content::card_def(self.cards[i].id).max_upgrade).count()
    }

    pub fn is_upgradable(&self, c: CardIdx) -> bool {
        self.cards[c as usize].upgrade < self.card_def(c).max_upgrade
    }

    pub fn clone_card(&mut self, c: CardIdx) -> Option<CardIdx> {
        if self.n_cards as usize >= MAX_CARDS {
            return None;
        }
        let idx = self.n_cards as CardIdx;
        self.n_cards += 1;
        let mut copy = self.cards[c as usize];
        copy.pile = PileType::None as u8;
        copy.flags &= !(cflag::EXHAUST_ON_NEXT_PLAY | cflag::REMOVED);
        copy.flags |= cflag::IS_CLONE;
        copy.dampen_saved = 0;
        copy.deck_idx = NO;
        copy.dampen_saved = 0;
        self.cards[idx as usize] = copy;
        self.listen |= crate::content::card_mask(copy.id);
        self.listen_cards |= crate::content::card_mask(copy.id);
        Some(idx)
    }

    pub fn add_generated_card(&mut self, c: CardIdx, pile: PileType, pos: CardPilePosition) -> bool {
        let by_player = self.side == Side::Player;
        self.add_generated_card_by(c, pile, pos, by_player)
    }

    pub fn play_amount_add(&mut self, uid: u16, card: CardIdx, amount: i32) {
        if self.hist.play_amounts.len() < 32 {
            self.hist.play_amounts.push(PlayAmount { uid, card, amount });
        }
    }

    pub fn play_amount_take(&mut self, uid: u16, card: CardIdx) -> Option<i32> {
        let pos = self.hist.play_amounts.as_slice().iter().rposition(|e| e.uid == uid && e.card == card)?;
        Some(self.hist.play_amounts.remove(pos).amount)
    }

    pub fn add_generated_card_by(&mut self, c: CardIdx, pile: PileType, pos: CardPilePosition, by_player: bool) -> bool {
        self.hist_card_generated(c, by_player);
        let ok = self.move_card(c, pile, pos);
        if ok {
            self.dispatch_g(hookbit::after_card_generated_for_combat, |cx, me, l| l.after_card_generated_for_combat(cx, me, c, by_player));
        }
        ok
    }

    pub fn card_named_var(&self, c: CardIdx, name: u16) -> i32 {
        let card = &self.cards[c as usize];
        for v in crate::content::card_def(card.id).vars {
            if v.kind == crate::defs::VarKind::Named && v.arg == name {
                return v.base as i32 + v.up as i32 * card.upgrade as i32;
            }
        }
        0
    }

    pub fn card_damage_dec(&self, c: CardIdx) -> Dec {
        let card = &self.cards[c as usize];
        let mut v = Dec::int(0);
        for d in crate::content::card_def(card.id).vars {
            if d.kind == crate::defs::VarKind::Damage {
                v = Dec::int(d.base as i64 + d.up as i64 * card.upgrade as i64);
            }
        }
        v + Dec::frac(card.dmg_bonus as i64, 4)
    }

    pub fn add_card_damage(&mut self, c: CardIdx, amount: Dec) {
        let milli = (amount * Dec::int(10_000)).trunc();
        let card = &mut self.cards[c as usize];
        card.dmg_bonus = card.dmg_bonus.saturating_add(milli);
    }

    pub fn draw_one(&mut self) -> Option<CardIdx> {
        self.draw_cards_list_nosuspend(1, false).first()
    }

    pub fn ask_hand(&mut self, purpose: u16, min: u8, max: u8, filter: impl Fn(&Combat, CardIdx) -> bool) -> Ask {
        let mut cands: ArrayVec<CardIdx, MAX_CARDS> = ArrayVec::new();
        for &c in self.player.hand.iter() {
            if filter(self, c) {
                cands.push(c);
            }
        }
        self.raise(DecisionSource::Hand, purpose, min, max, cands, false)
    }

    pub fn ask_pile(&mut self, purpose: u16, pile: PileType, min: u8, max: u8, filter: impl Fn(&Combat, CardIdx) -> bool) -> Ask {
        let mut cands: ArrayVec<CardIdx, MAX_CARDS> = ArrayVec::new();
        for &c in self.pile(pile).iter() {
            if filter(self, c) {
                cands.push(c);
            }
        }
        let forced = min == max && cands.len() <= min as usize;
        if pile == PileType::Draw && !forced {
            let cards = &self.cards;
            let key = |c: &CardIdx| {
                let id = cards[*c as usize].id;
                (crate::content::card_def(id).rarity, id)
            };
            let sl = cands.as_mut_slice();
            for i in 1..sl.len() {
                let x = sl[i];
                let kx = key(&x);
                let mut j = i;
                while j > 0 && key(&sl[j - 1]) > kx {
                    sl[j] = sl[j - 1];
                    j -= 1;
                }
                sl[j] = x;
            }
        }
        self.raise(DecisionSource::Pile(pile), purpose, min, max, cands, false)
    }

    pub fn ask_options(&mut self, purpose: u16, options: &[CardIdx], can_skip: bool) -> Ask {
        let mut cands: ArrayVec<CardIdx, MAX_CARDS> = ArrayVec::new();
        for &c in options {
            cands.push(c);
        }
        if self.is_over_or_ending() || cands.is_empty() {
            return Ask::Resolved(ArrayVec::new());
        }
        if self.auto_select {
            return Ask::Resolved(self.auto_selected(&cands, 1));
        }
        self.begin_decision(DecisionSource::Options, purpose, if can_skip { 0 } else { 1 }, 1, cands, false, can_skip);
        Ask::Pending
    }

    fn auto_selected(&self, cands: &ArrayVec<CardIdx, MAX_CARDS>, max: usize) -> ArrayVec<CardIdx, 16> {
        let mut v = ArrayVec::new();
        for &c in cands.iter().take(max) {
            v.push(c);
        }
        v
    }

    fn raise(&mut self, source: DecisionSource, purpose: u16, min: u8, max: u8, cands: ArrayVec<CardIdx, MAX_CARDS>, can_skip: bool) -> Ask {
        if self.is_over_or_ending() || cands.is_empty() {
            return Ask::Resolved(ArrayVec::new());
        }
        let manual = min != max;
        if !manual && cands.len() <= min as usize {
            let mut all = ArrayVec::new();
            for &c in cands.iter() {
                all.push(c);
            }
            return Ask::Resolved(all);
        }
        if self.auto_select {
            return Ask::Resolved(self.auto_selected(&cands, max as usize));
        }
        self.begin_decision(source, purpose, min, max, cands, manual, can_skip);
        Ask::Pending
    }

    fn begin_decision(&mut self, source: DecisionSource, purpose: u16, min: u8, max: u8, cands: ArrayVec<CardIdx, MAX_CARDS>, confirm_required: bool, can_skip: bool) {
        self.decision_seq += 1;
        self.decision = Some(Decision { source, min, max, cands, selected: ArrayVec::new(), confirm_required, can_skip, purpose });
    }

    pub fn decision_view(&self, d: &Decision) -> ArrayVec<u8, MAX_CARDS> {
        let mut view: ArrayVec<u8, MAX_CARDS> = ArrayVec::new();
        for k in 0..d.cands.len() {
            view.push(k as u8);
        }
        if matches!(d.source, DecisionSource::Pile(_)) {
            let mut keys: ArrayVec<u64, MAX_CARDS> = ArrayVec::new();
            for &c in d.cands.iter() {
                keys.push(self.visible_key(c));
            }
            let sl = view.as_mut_slice();
            for i in 1..sl.len() {
                let x = sl[i];
                let mut j = i;
                while j > 0 && (keys[sl[j - 1] as usize], sl[j - 1]) > (keys[x as usize], x) {
                    sl[j] = sl[j - 1];
                    j -= 1;
                }
                sl[j] = x;
            }
        }
        view
    }

    pub fn visible_key(&self, c: CardIdx) -> u64 {
        let card = &self.cards[c as usize];
        let d = crate::content::card_def(card.id);
        (d.rarity as u64) << 56
            | (card.id as u64) << 40
            | (card.upgrade as u64) << 32
            | (self.card_cost(c, true).clamp(-1, 254) as u64 & 0xFF) << 24
            | (card.enchant as u64) << 16
            | (card.affliction as u64) << 8
    }

    pub(crate) fn decision_pick(&mut self, i: u8) -> bool {
        let Some(d) = self.decision.as_ref() else { return false };
        let view = self.decision_view(d);
        let Some(&true_idx) = view.as_slice().get(i as usize) else { return false };
        self.decision_pick_game(true_idx)
    }

    pub(crate) fn decision_pick_game(&mut self, i: u8) -> bool {
        let Some(d) = self.decision.as_mut() else { return false };
        if i as usize >= d.cands.len() {
            return false;
        }
        if let Some(pos) = d.selected.position(i) {
            d.selected.remove(pos);
        } else {
            if d.selected.len() >= d.max as usize {
                d.selected.truncate(d.selected.len().saturating_sub(1));
            }
            d.selected.push(i);
        }
        let done = !d.confirm_required && d.selected.len() >= d.max as usize;
        if done {
            self.finish_decision();
        }
        true
    }

    pub(crate) fn decision_confirm(&mut self) -> bool {
        let Some(d) = self.decision.as_ref() else { return false };
        let n = d.selected.len();
        let ok = (d.confirm_required && n >= d.min as usize && n <= d.max as usize) || (d.can_skip && n == 0);
        if ok {
            self.finish_decision();
        }
        ok
    }

    fn finish_decision(&mut self) {
        if let Some(rp) = self.replay.as_mut().filter(|r| r.at_prompt) {
            rp.done = true;
            return;
        }
        let d = self.decision.take().unwrap();
        let mut ch = Choice::default();
        for &i in d.selected.iter() {
            ch.cards.push(d.cands[i as usize]);
        }
        self.choice = ch;
        self.stage = Stage::AwaitAction;
        self.resume_after_decision();
    }
}

impl Combat {
    pub fn apply_single_turn_sly(&mut self, c: CardIdx) {
        self.cards[c as usize].flags |= cflag::SINGLE_TURN_SLY;
    }

    pub fn apply_single_turn_retain(&mut self, c: CardIdx) {
        self.cards[c as usize].flags |= cflag::SINGLE_TURN_RETAIN;
    }

    pub fn remove_power_by_id(&mut self, c: Cid, id: u16) {
        if let Some(uid) = self.cr(c).power(id).map(|p| p.uid) {
            self.remove_power(c, uid);
        }
    }

    pub fn create_shivs_in_hand(&mut self, count: i32) -> ArrayVec<CardIdx, MAX_HAND> {
        let mut shivs: ArrayVec<CardIdx, MAX_HAND> = ArrayVec::new();
        if count <= 0 || self.is_over_or_ending() {
            return shivs;
        }
        for _ in 0..count.min(MAX_HAND as i32) {
            if let Some(c) = self.new_card(crate::ids::card::SHIV, 0) {
                shivs.push(c);
            }
        }
        for i in 0..shivs.len() {
            let c = shivs[i];
            self.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        shivs
    }
}

impl Combat {
}
