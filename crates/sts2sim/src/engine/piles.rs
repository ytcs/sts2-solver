//! Card instances, costs, piles, draw, shuffle (spec 03 §1-2, §6-7).

use crate::content;
use crate::defs::*;
use crate::dec::Dec;
use crate::engine::HKind;
use crate::hooks::*;
use crate::rng::Rng;
use crate::sort::intro_sort;
use crate::state::*;
use crate::types::*;

impl Combat {
    // ---- card instances ----------------------------------------------------------------------------------------

    /// `CombatState.CreateCard` / `CloneCard`: allocates a card instance in the arena (in no pile).
    pub fn new_card(&mut self, id: u16, upgrade: u8) -> Option<CardIdx> {
        self.new_card_ex(id, upgrade, 0, 0)
    }

    /// Like `new_card` for a saved card (`CardModel.FromSerializable`): the enchantment (`enchant` = id + 1, 0 = none) is
    /// applied FIRST (`OnEnchant` runs on the un-upgraded card), then the upgrades.
    pub fn new_card_ex(&mut self, id: u16, upgrade: u8, enchant: u8, enchant_amount: i16) -> Option<CardIdx> {
        if self.n_cards as usize >= MAX_CARDS {
            // The arena is full: the combat can no longer be faithful (the env treats `missing` as an error).
            self.flag_missing(Kind::Card, u16::MAX);
            return None;
        }
        if !content::card_implemented(id) {
            self.flag_missing(Kind::Card, id);
        }
        let d = content::card_def(id);
        self.listen |= content::card_mask(id);
        self.listen_cards |= content::card_mask(id);
        let idx = self.n_cards as CardIdx;
        self.n_cards += 1;
        self.cards[idx as usize] = Card {
            id,
            pile: PileType::None as u8,
            cost_base: if d.x_cost { 0 } else { d.cost },
            deck_idx: NO,
            dupe_of: NO,
            ..Default::default()
        };
        if enchant != 0 {
            let eid = (enchant - 1) as u16;
            if !content::enchantment_implemented(eid) {
                self.flag_missing(Kind::Enchantment, eid);
            }
            self.enchant_unchecked(idx, eid, enchant_amount as i32);
        }
        for _ in 0..upgrade {
            self.upgrade_card(idx);
        }
        Some(idx)
    }

    #[inline(always)]
    pub fn card_def(&self, c: CardIdx) -> &'static CardDef {
        let card = &self.cards[c as usize];
        // Mad Science's type / target are per-instance saved properties (`TinkerTimeType`): every other card is static.
        if card.id == crate::ids::card::MAD_SCIENCE {
            return content::cards::mad_science::variant(card.counter);
        }
        content::card_def(card.id)
    }

    /// `CardModel.UpgradeInternal` (stats only): +1 level, `UpgradeBy` on cost, keyword edits. Var deltas are
    /// applied lazily from `VarDef::up * level`.
    pub fn upgrade_card(&mut self, c: CardIdx) {
        let d = self.card_def(c);
        let card = &mut self.cards[c as usize];
        if card.upgrade >= d.max_upgrade {
            return;
        }
        card.upgrade += 1;
        if d.up_cost != 0 && !d.x_cost {
            let old = card.cost_base;
            let new = (old + d.up_cost).max(0);
            if new < old {
                for m in card.mods.as_mut_slice() {
                    if !m.relative && m.amount > new {
                        m.amount = new;
                    }
                }
            }
            card.cost_base = new;
        }
        card.kw_add |= d.up_add_kw;
        card.kw_remove |= d.up_remove_kw;
        card.kw_add &= !d.up_remove_kw;
        card.kw_remove &= !d.up_add_kw;
    }

    /// `CardModel.DowngradeInternal`: back to the canonical (un-upgraded) form — upgrade level 0, base energy cost reset,
    /// local keyword edits dropped. The dynamic vars are re-cloned from the canonical model, but every card whose Damage
    /// var grows during combat (Rampage, Thrash, Claw, Maul, Kingly Punch, The Ball) re-applies its accumulated growth in
    /// `AfterDowngraded`, so `dmg_bonus` is kept. (Cost modifiers, enchantment and affliction are kept; their
    /// `ModifyCard` / `AfterApplied` re-runs are no-ops for every ported entity.)
    pub fn downgrade_card(&mut self, c: CardIdx) {
        let d = self.card_def(c);
        let card = &mut self.cards[c as usize];
        card.upgrade = 0;
        card.cost_base = if d.x_cost { 0 } else { d.cost };
        card.kw_add = 0;
        card.kw_remove = 0;
        // `DowngradeInternal`: `Enchantment?.ModifyCard()` (= `OnEnchant` again: Tezcatara's Ember re-zeroes the cost and re-adds
        // Eternal, ...) then `Affliction?.AfterApplied()`.
        if self.cards[c as usize].enchant != 0 {
            let me = self.enchantment_me(c);
            content::listener(&me).on_enchant(self, me, c);
        }
        if self.cards[c as usize].affliction != 0 {
            let me = self.affliction_me(c);
            content::listener(&me).after_applied(self, me);
        }
    }

    /// Value of the card's dynamic var of `kind` (`DynamicVars.X.BaseValue` as int).
    pub fn card_var(&self, c: CardIdx, kind: VarKind) -> i32 {
        let card = &self.cards[c as usize];
        for v in content::card_def(card.id).vars {
            if v.kind == kind && v.kind != VarKind::Power {
                let mut x = v.base as i32 + v.up as i32 * card.upgrade as i32;
                if kind == VarKind::Damage {
                    x += card.dmg_bonus / 10_000; // Rampage / Thrash growth
                }
                return x + self.card_var_extra(c, kind);
            }
        }
        0
    }

    /// `PowerVar<T>` value.
    pub fn card_power_var(&self, c: CardIdx, power: u16) -> i32 {
        let card = &self.cards[c as usize];
        for v in content::card_def(card.id).vars {
            if v.kind == VarKind::Power && v.arg == power {
                return v.base as i32 + v.up as i32 * card.upgrade as i32;
            }
        }
        0
    }

    /// Local keywords only (`CardModel.GetKeywordsWithSources(Local)`).
    #[inline]
    pub fn card_keywords_local(&self, c: CardIdx) -> u8 {
        let card = &self.cards[c as usize];
        (self.card_def(c).keywords | card.kw_add) & !card.kw_remove
    }

    /// `CardModel.Keywords`: the local keyword set plus the global keywords of `Hook.ModifyKeywordsInCombat`
    /// (unguarded; only for cards in a combat pile; HexPower adds Ethereal).
    #[inline(always)]
    pub fn card_keywords(&self, c: CardIdx) -> u8 {
        let local = self.card_keywords_local(c);
        if !self.listen.has(hookbit::try_modify_keywords_in_combat) {
            return local;
        }
        self.card_keywords_global(c, local)
    }

    #[inline(never)]
    fn card_keywords_global(&self, c: CardIdx, local: u8) -> u8 {
        if !self.card_in_combat_pile(c) {
            return local;
        }
        let snap = self.snapshot(Mask::bit(hookbit::try_modify_keywords_in_combat));
        let mut k = local;
        for e in snap.iter() {
            if self.still_live(&e.me) {
                k = content::listener(&e.me).try_modify_keywords_in_combat(self, e.me, c, k);
            }
        }
        k
    }

    /// `CardEnergyCost.GetWithModifiers` (spec 03 §2.3).
    pub fn card_cost(&self, c: CardIdx, global: bool) -> i32 {
        let card = &self.cards[c as usize];
        let d = self.card_def(c);
        let mut n = card.cost_base as i32;
        if card.cost_base < 0 || d.x_cost {
            return n;
        }
        for m in card.mods.iter() {
            n = if m.relative {
                if m.reduce_only { n.min(n + m.amount as i32) } else { n + m.amount as i32 }
            } else if m.reduce_only {
                n.min(m.amount as i32)
            } else {
                m.amount as i32
            };
        }
        if global && self.card_in_combat_pile(c) {
            n = self.modify_energy_cost_in_combat(c, n);
        }
        n.max(0)
    }

    /// `Hook.ModifyEnergyCostInCombat`: pass 1 then pass 2 ("Late" = free-cost effects); skipped if cost < 0.
    fn modify_energy_cost_in_combat(&self, c: CardIdx, cost: i32) -> i32 {
        if cost < 0 || !self.hooks_enabled() {
            return cost;
        }
        let mut v = Dec::int(cost as i64);
        for bit in [hookbit::try_modify_energy_cost_in_combat, hookbit::try_modify_energy_cost_in_combat_late] {
            let snap = self.snapshot(Mask::bit(bit));
            for e in snap.iter() {
                if self.still_live(&e.me) {
                    let l = content::listener(&e.me);
                    let r = if bit == hookbit::try_modify_energy_cost_in_combat {
                        l.try_modify_energy_cost_in_combat(self, e.me, c, v)
                    } else {
                        l.try_modify_energy_cost_in_combat_late(self, e.me, c, v)
                    };
                    if let Some(nv) = r {
                        v = nv;
                    }
                }
            }
        }
        v.trunc()
    }

    pub fn card_in_combat_pile(&self, c: CardIdx) -> bool {
        let p = self.cards[c as usize].pile;
        (1..=5).contains(&p)
    }

    // ---- piles ----------------------------------------------------------------------------------------------------

    pub fn pile(&self, p: PileType) -> &Pile {
        match p {
            PileType::Draw => &self.player.draw,
            PileType::Hand => &self.player.hand,
            PileType::Discard => &self.player.discard,
            PileType::Exhaust => &self.player.exhaust,
            PileType::Play => &self.player.play,
            _ => panic!("not a combat pile"),
        }
    }

    pub fn pile_mut(&mut self, p: PileType) -> &mut Pile {
        match p {
            PileType::Draw => &mut self.player.draw,
            PileType::Hand => &mut self.player.hand,
            PileType::Discard => &mut self.player.discard,
            PileType::Exhaust => &mut self.player.exhaust,
            PileType::Play => &mut self.player.play,
            _ => panic!("not a combat pile"),
        }
    }

    #[inline(always)]
    pub fn card_pile_type(&self, c: CardIdx) -> PileType {
        match self.cards[c as usize].pile {
            1 => PileType::Draw,
            2 => PileType::Hand,
            3 => PileType::Discard,
            4 => PileType::Exhaust,
            5 => PileType::Play,
            6 => PileType::Deck,
            _ => PileType::None,
        }
    }

    /// `CardPileCmd.Add` for one card (spec 03 §6.2): hand-full redirect, removal from the old pile, positioned
    /// insert, `AfterCardEnteredCombat` for brand-new cards, `AfterCardChangedPiles` when the pile *type* changes.
    pub fn move_card(&mut self, c: CardIdx, to: PileType, pos: CardPilePosition) -> bool {
        if self.cards[c as usize].flags & cflag::REMOVED != 0 {
            return false;
        }
        if self.is_ending() {
            return false;
        }
        if !self.in_progress && !self.is_starting {
            return false;
        }
        let old = self.card_pile_type(c);
        let mut target = to;
        if to == PileType::Hand && self.player.hand.len() >= MAX_HAND {
            target = PileType::Discard;
        }
        if old != PileType::None {
            self.pile_mut(old).remove_value(c);
        }
        let n = self.pile(target).len();
        let index = match pos {
            CardPilePosition::Bottom => n,
            CardPilePosition::Top => 0,
            CardPilePosition::Random => self.rng.shuffle.next_int((n + 1) as i32) as usize,
        };
        self.pile_mut(target).insert(index, c);
        self.cards[c as usize].pile = target as u8;
        if old == PileType::None {
            self.dispatch_g(hookbit::after_card_entered_combat, |cx, me, l| l.after_card_entered_combat(cx, me, c));
        }
        if old != target {
            self.fire_card_changed_piles(c, old);
        }
        true
    }

    /// `Hook.AfterCardChangedPiles`: two full passes (`AfterCardChangedPiles`, then `...Late`) over the run-level iterator.
    #[inline]
    pub fn fire_card_changed_piles(&mut self, c: CardIdx, old: PileType) {
        if !self.listen.has(hookbit::after_card_changed_piles) && !self.listen.has(hookbit::after_card_changed_piles_late) {
            return;
        }
        self.dispatch_u(hookbit::after_card_changed_piles, |cx, me, l| l.after_card_changed_piles(cx, me, c, old));
        self.dispatch_u(hookbit::after_card_changed_piles_late, |cx, me, l| l.after_card_changed_piles_late(cx, me, c, old));
    }

    /// `CardPileCmd.RemoveFromCombat`: the card leaves combat for good.
    pub fn remove_card_from_combat(&mut self, c: CardIdx) {
        let old = self.card_pile_type(c);
        if old != PileType::None {
            self.pile_mut(old).remove_value(c);
        }
        self.cards[c as usize].pile = PileType::None as u8;
        // Hook.AfterCardChangedPiles(card, oldPile, newPile = None) — before `RemoveFromState`
        if old != PileType::None {
            self.fire_card_changed_piles(c, old);
        }
        self.cards[c as usize].flags |= cflag::REMOVED;
    }

    // ---- shuffle / draw -----------------------------------------------------------------------------------------

    /// Total order the game sorts cards by: ModelId ordinal (== our dense id) then upgrade level.
    #[inline]
    pub(crate) fn card_cmp(cards: &[Card; MAX_CARDS], a: &CardIdx, b: &CardIdx) -> i32 {
        let (ca, cb) = (&cards[*a as usize], &cards[*b as usize]);
        if ca.id != cb.id {
            return if ca.id < cb.id { -1 } else { 1 };
        }
        (ca.upgrade as i32 - cb.upgrade as i32).signum()
    }

    /// Initial combat shuffle: `UnstableShuffle` of the (deck-ordered) draw pile — NO sort (spec 03 §7.2).
    pub fn initial_shuffle(&mut self) {
        let rng: &mut Rng = &mut self.rng.shuffle;
        rng.shuffle(self.player.draw.as_mut_slice());
        // Hook.ModifyShuffleOrder(isInitial = true) (exempt from the combat-ending guard: the combat is starting).
        let mut list = self.player.draw;
        self.modify_shuffle_order(list.as_mut_slice(), true);
        self.player.draw = list;
    }

    /// `Hook.ModifyShuffleOrder` (guarded, exempt while starting): every listener may reorder the list in place.
    pub fn modify_shuffle_order(&self, list: &mut [CardIdx], is_initial: bool) {
        if !self.listen.has(hookbit::modify_shuffle_order) || !self.hooks_enabled() {
            return;
        }
        let snap = self.snapshot(Mask::bit(hookbit::modify_shuffle_order));
        for e in snap.iter() {
            if self.still_live(&e.me) {
                content::listener(&e.me).modify_shuffle_order(self, e.me, list, is_initial);
            }
        }
    }

    /// `CardPileCmd.Shuffle` (spec 03 §7.3): discard ++ draw → `StableShuffle` → draw pile.
    pub fn shuffle_discard_into_draw(&mut self) {
        if self.is_over_or_ending() {
            return;
        }
        let mut list: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
        for &c in self.player.discard.iter() {
            list.push(c);
        }
        for &c in self.player.draw.iter() {
            list.push(c);
        }
        let cards = &self.cards;
        intro_sort(list.as_mut_slice(), |a, b| Self::card_cmp(cards, a, b));
        self.rng.shuffle.shuffle(list.as_mut_slice());
        self.modify_shuffle_order(list.as_mut_slice(), false);
        let from_discard = self.player.discard;
        self.player.discard.clear();
        self.player.draw.clear();
        for &c in list.iter() {
            self.player.draw.push(c);
            self.cards[c as usize].pile = PileType::Draw as u8;
        }
        for &c in from_discard.iter() {
            self.fire_card_changed_piles(c, PileType::Discard);
        }
        self.run_after_shuffle();
    }

    /// `Hook.AfterShuffle` pass. A listener that raises a decision (Stratagem) stops the pass; it continues with the listeners that
    /// follow it (`dispatch_resumable`) once the decision is answered (`setup_player_turn(4)`).
    pub(crate) fn run_after_shuffle(&mut self) -> bool {
        self.dispatch_resumable(hookbit::after_shuffle, |cx, me, l| l.after_shuffle(cx, me))
    }

    #[inline]
    pub fn shuffle_if_necessary(&mut self) {
        if self.player.draw.is_empty() && !self.player.discard.is_empty() {
            self.shuffle_discard_into_draw();
        }
    }

    /// `CardPileCmd.Draw` (spec 03 §6.3). Returns the number of cards drawn.
    pub fn draw_cards(&mut self, count: i32, from_hand_draw: bool) -> usize {
        self.draw_cards_list(count, from_hand_draw).len()
    }

    /// `CardPileCmd.Draw` returning the drawn cards in draw order (Expertise, Escape Plan, ...).
    pub fn draw_cards_list(&mut self, count: i32, from_hand_draw: bool) -> crate::util::ArrayVec<CardIdx, MAX_HAND> {
        self.draw_depth = self.draw_depth.saturating_add(1);
        let out = self.draw_cards_list_inner(count, from_hand_draw);
        self.draw_depth = self.draw_depth.saturating_sub(1);
        out
    }

    /// Whether an `AfterShuffle` decision raised right now can be resumed: only inside the outermost turn-start hand draw.
    pub fn shuffle_decision_resumable(&self) -> bool {
        self.drawing_hand && self.draw_depth <= 1
    }

    fn draw_cards_list_inner(&mut self, count: i32, from_hand_draw: bool) -> crate::util::ArrayVec<CardIdx, MAX_HAND> {
        let mut out = crate::util::ArrayVec::new();
        if self.is_over_or_ending() {
            return out;
        }
        // Hook.ShouldDraw (guarded AND): the vetoing model (NoDraw) is told via AfterPreventingDraw.
        if let Some(m) = self.first_veto_g(hookbit::should_draw, |cx, me, l| l.should_draw(cx, me, from_hand_draw)) {
            if self.hooks_enabled() {
                self.notify_one(m, |cx, me, l| l.after_preventing_draw(cx, me));
            }
            return out;
        }
        if count <= 0 {
            return out;
        }
        let mut room = (MAX_HAND as i32 - self.player.hand.len() as i32).max(0);
        if room == 0 {
            return out;
        }
        for i in 0..count {
            if room <= 0 || self.is_over_or_ending() {
                break;
            }
            if self.player.draw.len() + self.player.discard.len() == 0 || self.player.hand.len() >= MAX_HAND {
                break;
            }
            self.shuffle_if_necessary();
            if self.stage == Stage::AwaitChoice && self.hook_ctx.is_some() && self.shuffle_decision_resumable() {
                // An `AfterShuffle` listener (Stratagem) asked for a decision: the turn-start draw resumes afterwards.
                self.draw_resume = Some((count - i, from_hand_draw));
                break;
            }
            if self.player.draw.len() + self.player.discard.len() == 0 {
                break;
            }
            let Some(card) = self.player.draw.first() else { break };
            if self.player.hand.len() >= MAX_HAND {
                break;
            }
            self.move_card(card, PileType::Hand, CardPilePosition::Bottom);
            out.push(card);
            let id = self.cards[card as usize].id;
            self.hist_push(HKind::CardDrawn, PLAYER, NO, id, card, 0, from_hand_draw as u8, 0, 0);
            self.dispatch_g(hookbit::after_card_drawn_early, |cx, me, l| l.after_card_drawn_early(cx, me, card, from_hand_draw));
            self.dispatch_g(hookbit::after_card_drawn, |cx, me, l| l.after_card_drawn(cx, me, card, from_hand_draw));
            room = (MAX_HAND as i32 - self.player.hand.len() as i32).max(0);
        }
        out
    }

    /// `CardCmd.Exhaust`.
    pub fn exhaust_card(&mut self, c: CardIdx, caused_by_ethereal: bool) {
        if self.is_over_or_ending() {
            return;
        }
        self.move_card(c, PileType::Exhaust, CardPilePosition::Bottom);
        let id = self.cards[c as usize].id;
        self.hist_push(HKind::CardExhausted, PLAYER, NO, id, c, 0, caused_by_ethereal as u8, 0, 0);
        self.dispatch_g(hookbit::after_card_exhausted, |cx, me, l| l.after_card_exhausted(cx, me, c, caused_by_ethereal));
    }
}
