use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

#[inline]
fn powered(props: ValueProp) -> bool {
    props.is_powered()
}

listener!(Adroit {
    fn on_play_enchantment(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        cx.gain_block(PLAYER, Dec::int(me.amount as i64), ValueProp::MOVE, play.card);
    }
});

listener!(Clone {});

listener!(Corrupted {
    fn can_enchant_card_type(&self, t: CardType) -> bool {
        t == CardType::Attack
    }
    fn enchant_damage_multiplicative(&self, _cx: &Combat, _me: Me, _original: Dec, props: ValueProp) -> Dec {
        if powered(props) { Dec::frac(15, 1) } else { Dec::ONE }
    }
    fn on_play_enchantment(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        let props = ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED).or(ValueProp::MOVE);
        cx.damage(&[PLAYER], Dec::int(2), props, PLAYER, me.idx as CardIdx);
    }
});

listener!(Glam {
    fn enchant_play_count(&self, cx: &Combat, me: Me, base: i32) -> i32 {
        if cx.cards[me.idx as usize].enchant_aux != 0 { base } else { base + 1 }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let c = &mut cx.cards[me.idx as usize];
        if c.enchant_aux != 0 || play.card as u16 != me.idx {
            return;
        }
        c.enchant_aux = 1;
        c.enchant_status = 1;
    }
});

listener!(Goopy {
    fn can_enchant(&self, cx: &Combat, me: Me, card: CardIdx) -> bool {
        cx.base_can_enchant(card, true) && cx.card_def(card).tags & tag::DEFEND != 0 && me.id == ids::enchantment::GOOPY
    }
    fn on_enchant(&self, cx: &mut Combat, _me: Me, card: CardIdx) {
        cx.add_keyword(card, kw::EXHAUST);
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if play.card as u16 != me.idx {
            return;
        }
        let c = &mut cx.cards[me.idx as usize];
        c.enchant_amount += 1;
        let di = c.deck_idx;
        if (di as usize) < cx.deck_enchant_inc.len() {
            if cx.deck_enchant_inc[di as usize] == u8::MAX {
                crate::util::raise_overflow(crate::state::ov::COUNTER as u32);
            }
            cx.deck_enchant_inc[di as usize] = cx.deck_enchant_inc[di as usize].saturating_add(1);
        }
    }
    fn enchant_block_additive(&self, _cx: &Combat, me: Me, _original: Dec) -> Dec {
        Dec::int(me.amount as i64 - 1)
    }
});

listener!(Imbued {
    fn can_enchant_card_type(&self, t: CardType) -> bool {
        t == CardType::Skill
    }
    fn should_start_at_bottom_of_draw_pile(&self, _cx: &Combat, _me: Me) -> bool {
        true
    }
    fn after_auto_pre_play_phase_entered(&self, cx: &mut Combat, me: Me) {
        if cx.player.turn_number <= 1 {
            cx.auto_play(me.idx as CardIdx, NO, AutoPlayType::Default, false);
        }
    }
});

listener!(Inky {
    fn on_play_enchantment(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let card = me.idx as CardIdx;
        if cx.card_target_type(card) != TargetType::AllEnemies {
            cx.apply_power(ids::power::WEAK_POWER, play.target, Dec::ONE, PLAYER, card);
        } else {
            let t = cx.hittable_enemies();
            for &e in t.iter() {
                cx.apply_power(ids::power::WEAK_POWER, e, Dec::ONE, PLAYER, card);
            }
        }
    }
});

listener!(Instinct {
    fn can_enchant_card_type(&self, t: CardType) -> bool {
        t == CardType::Attack
    }
    fn enchant_damage_multiplicative(&self, _cx: &Combat, _me: Me, _original: Dec, props: ValueProp) -> Dec {
        if powered(props) { Dec::int(2) } else { Dec::ONE }
    }
});

listener!(Momentum {
    fn can_enchant_card_type(&self, t: CardType) -> bool {
        t == CardType::Attack
    }
    fn on_play_enchantment(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        cx.cards[me.idx as usize].enchant_aux += me.amount as i16;
    }
    fn enchant_damage_additive(&self, cx: &Combat, me: Me, _original: Dec, props: ValueProp) -> Dec {
        if !powered(props) {
            return Dec::ZERO;
        }
        Dec::int(cx.cards[me.idx as usize].enchant_aux as i64)
    }
});

listener!(Nimble {
    fn can_enchant(&self, cx: &Combat, _me: Me, card: CardIdx) -> bool {
        cx.base_can_enchant(card, true) && cx.card_def(card).vars.iter().any(|v| v.kind == crate::defs::VarKind::Block)
    }
    fn enchant_block_additive(&self, _cx: &Combat, me: Me, _original: Dec) -> Dec {
        Dec::int(me.amount as i64)
    }
});

listener!(PerfectFit {
    fn modify_shuffle_order(&self, _cx: &Combat, me: Me, cards: &mut [CardIdx], is_initial_shuffle: bool) {
        if is_initial_shuffle {
            return;
        }
        if let Some(i) = cards.iter().position(|&c| c as u16 == me.idx) {
            let c = cards[i];
            cards.copy_within(0..i, 1);
            cards[0] = c;
        }
    }
});

listener!(RoyallyApproved {
    fn can_enchant_card_type(&self, t: CardType) -> bool {
        matches!(t, CardType::Attack | CardType::Skill)
    }
    fn on_enchant(&self, cx: &mut Combat, _me: Me, card: CardIdx) {
        cx.add_keyword(card, kw::INNATE);
        cx.add_keyword(card, kw::RETAIN);
    }
});

listener!(Sharp {
    fn can_enchant_card_type(&self, t: CardType) -> bool {
        t == CardType::Attack
    }
    fn enchant_damage_additive(&self, _cx: &Combat, me: Me, _original: Dec, props: ValueProp) -> Dec {
        if powered(props) { Dec::int(me.amount as i64) } else { Dec::ZERO }
    }
});

listener!(Slither {
    fn can_enchant(&self, cx: &Combat, _me: Me, card: CardIdx) -> bool {
        cx.base_can_enchant(card, true) && cx.card_keywords(card) & kw::UNPLAYABLE == 0 && !cx.card_def(card).x_cost
    }
    fn after_card_drawn(&self, cx: &mut Combat, me: Me, card: CardIdx, _from_hand_draw: bool) {
        if card as u16 != me.idx || cx.card_pile_type(card) != PileType::Hand {
            return;
        }
        let n = cx.rng.combat_energy_costs.next_int(4);
        cx.set_cost_this_combat(card, n, false);
    }
});

listener!(SlumberingEssence {
    fn before_flush(&self, cx: &mut Combat, me: Me) {
        let card = me.idx as CardIdx;
        if cx.card_pile_type(card) == PileType::Hand {
            cx.add_cost_until_played(card, -1, false);
        }
    }
});

listener!(SoulsPower {
    fn can_enchant(&self, cx: &Combat, _me: Me, card: CardIdx) -> bool {
        cx.base_can_enchant(card, true) && cx.card_keywords_local(card) & kw::EXHAUST != 0
    }
    fn on_enchant(&self, cx: &mut Combat, _me: Me, card: CardIdx) {
        cx.remove_keyword(card, kw::EXHAUST);
    }
});

listener!(Sown {
    fn on_play_enchantment(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        if cx.cards[me.idx as usize].enchant_status == 0 {
            cx.cards[me.idx as usize].enchant_status = 1;
            cx.gain_energy(me.amount);
        }
    }
});

listener!(Spiral {
    fn can_enchant(&self, cx: &Combat, _me: Me, card: CardIdx) -> bool {
        let d = cx.card_def(card);
        cx.base_can_enchant(card, true) && d.rarity == CardRarity::Basic && d.tags & (tag::STRIKE | tag::DEFEND) != 0
    }
    fn enchant_play_count(&self, _cx: &Combat, _me: Me, base: i32) -> i32 {
        base + 1
    }
});

listener!(Steady {
    fn on_enchant(&self, cx: &mut Combat, _me: Me, card: CardIdx) {
        cx.add_keyword(card, kw::RETAIN);
    }
});

listener!(Swift {
    fn on_play_enchantment(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        if cx.cards[me.idx as usize].enchant_status == 0 {
            cx.cards[me.idx as usize].enchant_status = 1;
            cx.draw_cards(me.amount, false);
        }
    }
});

listener!(TezcatarasEmber {
    fn on_enchant(&self, cx: &mut Combat, _me: Me, card: CardIdx) {
        let base = cx.cards[card as usize].cost_base as i32;
        cx.upgrade_cost_by(card, -base);
        cx.add_keyword(card, kw::ETERNAL);
    }
    fn enchant_damage_additive(&self, _cx: &Combat, _me: Me, _original: Dec, props: ValueProp) -> Dec {
        if powered(props) { Dec::int(3) } else { Dec::ZERO }
    }
});

listener!(Vigorous {
    fn can_enchant_card_type(&self, t: CardType) -> bool {
        t == CardType::Attack
    }
    fn enchant_damage_additive(&self, cx: &Combat, me: Me, _original: Dec, props: ValueProp) -> Dec {
        if cx.cards[me.idx as usize].enchant_status != 0 || !powered(props) {
            return Dec::ZERO;
        }
        Dec::int(me.amount as i64)
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if play.card as u16 == me.idx {
            cx.cards[me.idx as usize].enchant_status = 1;
        }
    }
});

listener!(DeprecatedEnchantment {});
