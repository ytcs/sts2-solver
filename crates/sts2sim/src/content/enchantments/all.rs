//! The 23 enchantments (`Models/Enchantments/*.cs`, spec 03 §12.2). Every listener's `me.idx` is the enchanted card;
//! `me.amount` is the enchantment's `Amount` at snapshot time. Per-instance state: `Card::enchant_status` (0 Normal,
//! 1 Disabled) and `Card::enchant_aux` (Glam used / Momentum extra damage).

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

// Adroit: OnPlay gains `Amount` block through the block hooks (BlockVar(0, Move), BaseValue = Amount).
listener!(Adroit {
    fn on_play_enchantment(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        cx.gain_block(PLAYER, Dec::int(me.amount as i64), ValueProp::MOVE, play.card);
    }
});

// Clone: marker only.
listener!(Clone {});

// Corrupted (Attack): damage x1.5 for powered attacks; OnPlay the owner takes 2 unblockable unpowered damage.
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

// Glam: one extra replay (Times = 1) until the first completed play this combat (`UsedThisCombat` -> Status Disabled).
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

// Goopy (Defend-tagged): adds Exhaust; every completed play grows `Amount` (also on the deck card); block +(Amount - 1).
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
            cx.deck_enchant_inc[di as usize] = cx.deck_enchant_inc[di as usize].saturating_add(1);
        }
    }
    fn enchant_block_additive(&self, _cx: &Combat, me: Me, _original: Dec) -> Dec {
        Dec::int(me.amount as i64 - 1)
    }
});

// Imbued (Skill): starts at the bottom of the draw pile; auto-played on turn 1 (from wherever it is).
listener!(Imbued {
    fn can_enchant_card_type(&self, t: CardType) -> bool {
        t == CardType::Skill
    }
    fn should_start_at_bottom_of_draw_pile(&self, _cx: &Combat, _me: Me) -> bool {
        true
    }
    fn after_auto_pre_play_phase_entered(&self, cx: &mut Combat, me: Me) {
        if cx.player.turn_number <= 1 {
            // (a decision raised by the auto-played card cannot be resumed from a turn-start hook; see `run_play_stack`)
            cx.auto_play(me.idx as CardIdx, NO, AutoPlayType::Default, false);
        }
    }
});

// Inky: OnPlay applies Weak (1) to the target / every hittable enemy for AllEnemies cards.
listener!(Inky {
    fn on_play_enchantment(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let card = me.idx as CardIdx;
        // `Card.TargetType` (a Shiv is AllEnemies under Fan of Knives)
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

// Instinct (Attack): damage x2 for powered attacks.
listener!(Instinct {
    fn can_enchant_card_type(&self, t: CardType) -> bool {
        t == CardType::Attack
    }
    fn enchant_damage_multiplicative(&self, _cx: &Combat, _me: Me, _original: Dec, props: ValueProp) -> Dec {
        if powered(props) { Dec::int(2) } else { Dec::ONE }
    }
});

// Momentum (Attack): every play (replays included) adds `Amount` to the card's extra damage (combat only).
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

// Nimble (cards that gain block): block +Amount. (`GainsBlock` approximated by "has a Block variable".)
listener!(Nimble {
    fn can_enchant(&self, cx: &Combat, _me: Me, card: CardIdx) -> bool {
        cx.base_can_enchant(card, true) && cx.card_def(card).vars.iter().any(|v| v.kind == crate::defs::VarKind::Block)
    }
    fn enchant_block_additive(&self, _cx: &Combat, me: Me, _original: Dec) -> Dec {
        Dec::int(me.amount as i64)
    }
});

// Perfect Fit: on a RESHUFFLE (not the initial shuffle) its card moves to the top of the shuffled list.
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

// Royally Approved (Attack / Skill): Innate + Retain.
listener!(RoyallyApproved {
    fn can_enchant_card_type(&self, t: CardType) -> bool {
        matches!(t, CardType::Attack | CardType::Skill)
    }
    fn on_enchant(&self, cx: &mut Combat, _me: Me, card: CardIdx) {
        cx.add_keyword(card, kw::INNATE);
        cx.add_keyword(card, kw::RETAIN);
    }
});

// Sharp (Attack): damage +Amount for powered attacks.
listener!(Sharp {
    fn can_enchant_card_type(&self, t: CardType) -> bool {
        t == CardType::Attack
    }
    fn enchant_damage_additive(&self, _cx: &Combat, me: Me, _original: Dec, props: ValueProp) -> Dec {
        if powered(props) { Dec::int(me.amount as i64) } else { Dec::ZERO }
    }
});

// Slither (playable, non-X): when its card is drawn into the hand its cost becomes a random 0-3 for the combat.
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

// Slumbering Essence: before the hand flush, a card still in hand gets 1 cheaper (until played).
listener!(SlumberingEssence {
    fn before_flush(&self, cx: &mut Combat, me: Me) {
        let card = me.idx as CardIdx;
        if cx.card_pile_type(card) == PileType::Hand {
            cx.add_cost_until_played(card, -1, false);
        }
    }
});

// Souls (cards with a LOCAL Exhaust keyword): removes Exhaust.
listener!(SoulsPower {
    fn can_enchant(&self, cx: &Combat, _me: Me, card: CardIdx) -> bool {
        cx.base_can_enchant(card, true) && cx.card_keywords_local(card) & kw::EXHAUST != 0
    }
    fn on_enchant(&self, cx: &mut Combat, _me: Me, card: CardIdx) {
        cx.remove_keyword(card, kw::EXHAUST);
    }
});

// Sown: once, gain `Amount` energy when its card is played.
listener!(Sown {
    fn on_play_enchantment(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        if cx.cards[me.idx as usize].enchant_status == 0 {
            cx.cards[me.idx as usize].enchant_status = 1;
            cx.gain_energy(me.amount);
        }
    }
});

// Spiral (basic Strike / Defend): permanently one extra replay (Times = 1).
listener!(Spiral {
    fn can_enchant(&self, cx: &Combat, _me: Me, card: CardIdx) -> bool {
        let d = cx.card_def(card);
        cx.base_can_enchant(card, true) && d.rarity == CardRarity::Basic && d.tags & (tag::STRIKE | tag::DEFEND) != 0
    }
    fn enchant_play_count(&self, _cx: &Combat, _me: Me, base: i32) -> i32 {
        base + 1
    }
});

// Steady: Retain.
listener!(Steady {
    fn on_enchant(&self, cx: &mut Combat, _me: Me, card: CardIdx) {
        cx.add_keyword(card, kw::RETAIN);
    }
});

// Swift: once, draw `Amount` cards when its card is played.
listener!(Swift {
    fn on_play_enchantment(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        if cx.cards[me.idx as usize].enchant_status == 0 {
            cx.cards[me.idx as usize].enchant_status = 1;
            cx.draw_cards(me.amount, false);
        }
    }
});

// Tezcatara's Ember: base cost 0, Eternal, damage +3 for powered attacks (DamageVar(3)).
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

// Vigorous (Attack): damage +Amount while Normal; Disabled after the first completed play iteration.
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
