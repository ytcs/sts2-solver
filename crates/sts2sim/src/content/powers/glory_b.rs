use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

#[inline]
fn owner_side(cx: &Combat, me: Me, side: Side) -> bool {
    cx.cr(me.owner).side == side
}
#[inline]
fn aux_of(cx: &Combat, me: &Me) -> i32 {
    cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx).map_or(0, |p| p.aux)
}
#[inline]
fn set_aux(cx: &mut Combat, me: &Me, v: i32) {
    if let Some(i) = cx.power_idx(me.owner, me.idx) {
        cx.cr_mut(me.owner).powers[i].aux = v;
    }
}

listener!(HexPower {
    fn try_modify_keywords_in_combat(&self, cx: &Combat, _me: Me, card: CardIdx, keywords: u8) -> u8 {
        if cx.card_affliction(card) != Some(ids::affliction::HEXED) {
            return keywords;
        }
        keywords | kw::ETHEREAL
    }
    fn after_applied(&self, cx: &mut Combat, me: Me) {
        let amount = cx.cr(me.owner).power(me.id).map_or(1, |p| p.amount);
        let cards = cx.all_combat_cards();
        for &c in cards.iter() {
            if cx.card_affliction(c).is_none() {
                cx.afflict_card(c, ids::affliction::HEXED, amount);
            }
        }
    }
    fn after_card_entered_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {
        if cx.card_affliction(card).is_none() {
            let amount = cx.cr(me.owner).power(me.id).map_or(1, |p| p.amount);
            cx.afflict_card(card, ids::affliction::HEXED, amount);
        }
    }
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, was_removal_prevented: bool) {
        if let Some(i) = cx.power_idx(me.owner, me.idx) {
            if !was_removal_prevented && cx.cr(me.owner).powers[i].applier == creature {
                cx.remove_power(me.owner, me.idx);
            }
        }
    }
    fn after_removed(&self, cx: &mut Combat, _me: Me, _old_owner: Cid) {
        let cards = cx.all_combat_cards();
        for &c in cards.iter() {
            if cx.card_affliction(c) == Some(ids::affliction::HEXED) {
                cx.clear_affliction(c);
            }
        }
    }
});

listener!(DampenPower {
    fn after_applied(&self, cx: &mut Combat, _me: Me) {
        if cx.is_ending() {
            return;
        }
        let cards = cx.all_combat_cards();
        for &c in cards.iter() {
            let up = cx.cards[c as usize].upgrade;
            if up > 0 && cx.card_def(c).max_upgrade > 0 {
                cx.cards[c as usize].dampen_saved = up;
                cx.downgrade_card(c);
            }
        }
    }
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, was_removal_prevented: bool) {
        if was_removal_prevented {
            return;
        }
        let a = aux_of(cx, &me);
        let bit = 1i32 << creature;
        if a & bit != 0 {
            let a = a & !bit;
            set_aux(cx, &me, a);
            if a == 0 {
                cx.remove_power(me.owner, me.idx);
            }
        }
    }
    fn after_removed(&self, cx: &mut Combat, _me: Me, _old_owner: Cid) {
        if cx.is_ending() {
            return;
        }
        for c in 0..cx.n_cards as usize {
            let n = cx.cards[c].dampen_saved;
            if n > 0 {
                cx.cards[c].dampen_saved = 0;
                for _ in 0..n {
                    cx.upgrade_card(c as CardIdx);
                }
            }
        }
    }
});

pub fn dampen_add_caster(cx: &mut Combat, target: Cid, caster: Cid) {
    if let Some(p) = cx.cr(target).power(ids::power::DAMPEN_POWER) {
        let uid = p.uid;
        let a = p.aux | (1i32 << caster);
        set_aux(cx, &Me { kind: Kind::Power, owner: target, idx: uid, id: ids::power::DAMPEN_POWER, amount: 0 }, a);
    }
}

listener!(WitheringPresencePower {
    fn initial_power_aux(&self) -> i32 {
        6
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        let left = aux_of(cx, &me) - 1;
        set_aux(cx, &me, left);
        if left <= 0 {
            cx.add_status_cards_as(ids::card::WITHER, PileType::Hand, 1, CardPilePosition::Bottom, false);
            set_aux(cx, &me, 6);
        }
    }
});

listener!(ChainsOfBindingPower {
    fn after_card_drawn(&self, cx: &mut Combat, me: Me, card: CardIdx, _from_hand_draw: bool) {
        if cx.cr(me.owner).side != cx.side {
            return;
        }
        let bound = ids::affliction::BOUND as i16;
        let n = cx.hist_count_this_turn(crate::engine::HKind::CardAfflicted, |e| e.actor == me.owner && e.val == bound);
        let amount = cx.cr(me.owner).power(me.id).map_or(0, |p| p.amount);
        if (n as i32) < amount {
            cx.afflict_card(card, ids::affliction::BOUND, amount);
        }
    }
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let c = play.card;
        if cx.cards[c as usize].dupe_of != NO {
            return;
        }
        if cx.card_affliction(c) != Some(ids::affliction::BOUND) {
            return;
        }
        set_aux(cx, &me, 1);
    }
    fn should_play(&self, cx: &Combat, me: Me, card: CardIdx) -> bool {
        if cx.card_affliction(card) != Some(ids::affliction::BOUND) {
            return true;
        }
        aux_of(cx, &me) == 0
    }
    fn before_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if !owner_side(cx, me, side) {
            return;
        }
        set_aux(cx, &me, 0);
        let cards = cx.all_combat_cards();
        for &c in cards.iter() {
            if cx.card_affliction(c) == Some(ids::affliction::BOUND) {
                cx.clear_affliction(c);
            }
        }
    }
});

listener!(EnragePower {
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.card_def(play.card).ctype == CardType::Skill {
            let a = cx.cr(me.owner).power(me.id).map_or(0, |p| p.amount);
            cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(a as i64), me.owner, NO);
        }
    }
});

listener!(PainfulStabsPower {
    fn should_power_be_removed_after_owner_death(&self, _cx: &Combat, _me: Me) -> bool {
        false
    }
    fn should_creature_be_removed_from_combat_after_death(&self, _cx: &Combat, me: Me, creature: Cid) -> bool {
        creature != me.owner
    }
    fn after_attack(&self, cx: &mut Combat, me: Me, attack: &crate::engine::Attack) {
        if attack.dealer != me.owner || cx.cr(attack.dealer).side == Side::Player || !attack.props.is_powered() {
            return;
        }
        if cx.attack_unblocked_hits == 0 {
            return;
        }
        let n = cx.attack_player_hits as i32;
        let amount = cx.cr(me.owner).power(me.id).map_or(0, |p| p.amount);
        if n > 0 {
            cx.add_status_cards(ids::card::WOUND, PileType::Discard, amount * n, CardPilePosition::Bottom);
        }
    }
});

listener!(NemesisPower {
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if !owner_side(cx, me, side) {
            return;
        }
        let on = aux_of(cx, &me) == 0;
        set_aux(cx, &me, on as i32);
        if on {
            cx.apply_power(ids::power::INTANGIBLE_POWER, me.owner, Dec::ONE, me.owner, NO);
        } else if let Some(p) = cx.cr(me.owner).power(ids::power::INTANGIBLE_POWER) {
            let uid = p.uid;
            cx.remove_power(me.owner, uid);
        }
    }
});

listener!(AdaptablePower {
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, was_removal_prevented: bool) {
        if was_removal_prevented || creature != me.owner || cx.cr(creature).monster.id != ids::monster::TEST_SUBJECT {
            return;
        }
        set_aux(cx, &me, 1);
        if let Some(node) = crate::content::node_by_name(ids::monster::TEST_SUBJECT, "RESPAWN_MOVE") {
            cx.set_move_immediate(creature, node, true);
        }
    }
    fn should_allow_hitting(&self, cx: &Combat, me: Me, creature: Cid) -> bool {
        if creature != me.owner {
            return true;
        }
        aux_of(cx, &me) == 0
    }
    fn should_stop_combat_from_ending(&self, _cx: &Combat, _me: Me) -> bool {
        true
    }
    fn should_creature_be_removed_from_combat_after_death(&self, _cx: &Combat, me: Me, creature: Cid) -> bool {
        creature != me.owner
    }
    fn should_power_be_removed_after_owner_death(&self, _cx: &Combat, _me: Me) -> bool {
        false
    }
});
