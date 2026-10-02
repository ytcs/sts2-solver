//! Enchantments and afflictions on card instances (spec 03 §12).
//!
//! A card carries at most one enchantment (`Card::enchant` = id + 1, `enchant_amount`, `enchant_status`, private
//! `enchant_aux`) and one affliction (`Card::affliction` = id + 1, `affliction_amount`). Both are hook listeners while
//! their card sits in a combat pile (listener order: card, affliction, enchantment — see `snapshot`). The engine calls the
//! enchantment virtuals (`enchant_damage_additive` ...) directly from `modify_damage` / `modify_block_ex` /
//! `generate_play_count`, and `on_play_enchantment` / `on_play_affliction` from the replay loop.

use crate::content;
use crate::engine::HKind;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;

impl Combat {
    /// The `Me` of a card's enchantment (`card.enchant != 0`; ids are stored `+1`, `0` = none).
    #[inline]
    pub fn enchantment_me(&self, card: CardIdx) -> Me {
        let c = &self.cards[card as usize];
        Me { kind: Kind::Enchantment, owner: PLAYER, idx: card as u16, id: (c.enchant.wrapping_sub(1)) as u16, amount: c.enchant_amount as i32 }
    }

    /// The `Me` of a card's affliction.
    #[inline]
    pub fn affliction_me(&self, card: CardIdx) -> Me {
        let c = &self.cards[card as usize];
        Me { kind: Kind::Affliction, owner: PLAYER, idx: card as u16, id: (c.affliction.wrapping_sub(1)) as u16, amount: c.affliction_amount as i32 }
    }

    /// `EnchantmentModel.CanEnchant` base rule: not a Status / Curse / Quest card, the enchantment accepts the card type,
    /// no Unplayable card in the deck pile, and no enchantment already present (none is stackable).
    pub fn base_can_enchant(&self, card: CardIdx, type_ok: bool) -> bool {
        let d = self.card_def(card);
        if matches!(d.ctype, CardType::Status | CardType::Curse | CardType::Quest) || !type_ok {
            return false;
        }
        if self.card_pile_type(card) == PileType::Deck && self.card_keywords(card) & kw::UNPLAYABLE != 0 {
            return false;
        }
        self.cards[card as usize].enchant == 0
    }

    /// `CardCmd.Enchant`: applies enchantment `id` (amount `amount`) to `card`: `EnchantInternal` then `ModifyCard`
    /// (`OnEnchant`). Returns false if the card cannot be enchanted (the game throws).
    pub fn enchant_card(&mut self, card: CardIdx, id: u16, amount: i32) -> bool {
        let probe = Me { kind: Kind::Enchantment, owner: PLAYER, idx: card as u16, id, amount };
        let l = content::listener(&probe);
        if self.cards[card as usize].enchant != 0 {
            // stacking onto the same enchantment is not possible (nothing is stackable); different one throws
            return false;
        }
        if !l.can_enchant(self, probe, card) {
            return false;
        }
        self.enchant_unchecked(card, id, amount);
        true
    }

    /// `EnchantInternal` + `ModifyCard` without the `CanEnchant` check (what loading a saved card does).
    pub fn enchant_unchecked(&mut self, card: CardIdx, id: u16, amount: i32) {
        self.listen |= content::enchantment_mask(id);
        self.listen_cards |= content::enchantment_mask(id);
        {
            let c = &mut self.cards[card as usize];
            c.enchant = (id + 1) as u8;
            c.enchant_amount = amount as i16;
            c.enchant_status = 0;
            c.enchant_aux = 0;
        }
        let me = self.enchantment_me(card);
        content::listener(&me).on_enchant(self, me, card);
    }

    /// `CardCmd.ClearEnchantment`.
    pub fn clear_enchantment(&mut self, card: CardIdx) {
        let c = &mut self.cards[card as usize];
        c.enchant = 0;
        c.enchant_amount = 0;
        c.enchant_status = 0;
        c.enchant_aux = 0;
    }

    /// `AfflictionModel.CanAfflict` base rule.
    pub fn base_can_afflict(&self, me: Me, card: CardIdx) -> bool {
        let l = content::listener(&me);
        if !l.can_afflict_card_type(self.card_def(card).ctype) {
            return false;
        }
        if self.card_keywords(card) & kw::UNPLAYABLE != 0 && !l.can_afflict_unplayable_cards() {
            return false;
        }
        let c = &self.cards[card as usize];
        !(c.affliction != 0 && (!l.affliction_is_stackable() || c.affliction != me.id as u8 + 1))
    }

    /// `CardCmd.Afflict`: refused once combat is ending (for a combat card), when `ShouldAfflict` vetoes or `CanAfflict`
    /// fails; stacks onto the same stackable affliction. Returns whether the card is afflicted afterwards.
    pub fn afflict_card(&mut self, card: CardIdx, id: u16, amount: i32) -> bool {
        if self.is_over_or_ending() && self.card_in_combat_pile(card) {
            return false;
        }
        if self.first_veto_g(hookbit::should_afflict, |cx, me, l| l.should_afflict(cx, me, card, id as u8)).is_some() {
            return false;
        }
        self.listen |= content::affliction_mask(id);
        self.listen_cards |= content::affliction_mask(id);
        let probe = Me { kind: Kind::Affliction, owner: PLAYER, idx: card as u16, id, amount };
        if !content::listener(&probe).can_afflict(self, probe, card) {
            return false;
        }
        if self.cards[card as usize].affliction == 0 {
            let c = &mut self.cards[card as usize];
            c.affliction = (id + 1) as u8;
            c.affliction_amount = amount as i16;
            let me = self.affliction_me(card);
            content::listener(&me).after_applied(self, me);
        } else {
            self.cards[card as usize].affliction_amount += amount as i16;
        }
        let cid = self.cards[card as usize].id;
        self.hist_push(HKind::CardAfflicted, PLAYER, NO, cid, card, id as i32, 0, 0, 0);
        true
    }

    /// `CardCmd.ClearAffliction`.
    pub fn clear_affliction(&mut self, card: CardIdx) {
        let c = &mut self.cards[card as usize];
        c.affliction = 0;
        c.affliction_amount = 0;
    }

    /// `CardModel.EnchantedReplayCount` helper for content: whether the enchantment on `card` is `Disabled`.
    #[inline]
    pub fn enchant_disabled(&self, card: CardIdx) -> bool {
        self.cards[card as usize].enchant_status != 0
    }
}
