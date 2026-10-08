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

listener!(PersonalHivePower {
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, _unblocked: i32, props: ValueProp, dealer: Cid) {
        if target != me.owner || dealer == NO || !props.is_powered() {
            return;
        }
        let n = cx.power_amount(me.owner, me.id);
        cx.add_status_cards_as(ids::card::DAZED, PileType::Draw, n, CardPilePosition::Random, false);
    }
});

fn other_segments_all_dead(cx: &Combat, owner: Cid) -> bool {
    cx.enemies.iter().all(|&e| e == owner || !cx.has_power(e, ids::power::REATTACH_POWER) || cx.cr(e).is_dead())
}

listener!(ReattachPower {
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, was_removal_prevented: bool) {
        if was_removal_prevented || creature != me.owner {
            return;
        }
        if !other_segments_all_dead(cx, me.owner) || !cx.cr(me.owner).is_dead() {
            cx.set_power_aux(me.owner, me.idx, 1);
            let mid = cx.cr(me.owner).monster.id;
            if let Some(node) = crate::content::node_by_name(mid, "DEAD_MOVE") {
                cx.set_move_immediate(me.owner, node, false);
            }
        }
    }
    fn should_allow_hitting(&self, cx: &Combat, me: Me, creature: Cid) -> bool {
        if creature != me.owner {
            return true;
        }
        cx.power_aux(me.owner, me.idx) == 0
    }
    fn should_creature_be_removed_from_combat_after_death(&self, _cx: &Combat, me: Me, creature: Cid) -> bool {
        creature != me.owner
    }
    fn should_power_be_removed_after_owner_death(&self, _cx: &Combat, _me: Me) -> bool {
        false
    }
    fn should_owner_death_trigger_fatal(&self, cx: &Combat, me: Me) -> bool {
        other_segments_all_dead(cx, me.owner)
    }
});

pub fn do_reattach(cx: &mut Combat, owner: Cid) {
    let Some(p) = cx.cr(owner).power(ids::power::REATTACH_POWER) else { return };
    let (uid, amount) = (p.uid, p.amount);
    if !other_segments_all_dead(cx, owner) {
        cx.set_power_aux(owner, uid, 0);
        cx.heal(owner, Dec::int(amount as i64));
    }
}

fn tainted_cards_matching(cx: &Combat, tainted: bool) -> crate::util::ArrayVec<CardIdx, MAX_CARDS> {
    let mut o = crate::util::ArrayVec::new();
    for c in cx.all_combat_cards().iter().copied() {
        let is_t = cx.card_affliction(c) == Some(ids::affliction::TAINTED);
        if tainted && is_t || !tainted && cx.card_def(c).ctype == CardType::Skill {
            o.push(c);
        }
    }
    o
}

listener!(VitalSparkPower {
    fn before_combat_start(&self, cx: &mut Combat, me: Me) {
        let amount = cx.power_amount(me.owner, me.id);
        let cards = tainted_cards_matching(cx, false);
        for &c in cards.iter() {
            cx.afflict_card(c, ids::affliction::TAINTED, amount);
        }
    }
    fn after_card_entered_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {
        if cx.card_affliction(card).is_none() && cx.card_def(card).ctype == CardType::Skill {
            let amount = cx.power_amount(me.owner, me.id);
            cx.afflict_card(card, ids::affliction::TAINTED, amount);
        }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.card_affliction(play.card) == Some(ids::affliction::TAINTED) {
            let amount = cx.power_amount(me.owner, me.id);
            cx.apply_power(ids::power::TAINTED_POWER, PLAYER, Dec::int(amount as i64), NO, NO);
        }
    }
    fn after_removed(&self, cx: &mut Combat, _me: Me, _old_owner: Cid) {
        let cards = tainted_cards_matching(cx, true);
        for &c in cards.iter() {
            cx.clear_affliction(c);
        }
    }
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        if ch.uid != me.idx {
            return;
        }
        let amount = cx.power_amount(me.owner, me.id);
        let cards = tainted_cards_matching(cx, true);
        for &c in cards.iter() {
            cx.cards[c as usize].affliction_amount = amount as i16;
        }
    }
});

listener!(TaintedPower {
    fn modify_damage_additive(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner || !q.props.is_powered() {
            return Dec::ZERO;
        }
        Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Enemy {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

listener!(BackAttackLeftPower {});
listener!(BackAttackRightPower {});

listener!(CrabRagePower {
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, _was_removal_prevented: bool) {
        if creature != me.owner && cx.cr(creature).side == cx.cr(me.owner).side {
            cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(6), me.owner, NO);
            cx.gain_block(me.owner, Dec::int(99), ValueProp::UNPOWERED, NO);
            cx.remove_power(me.owner, me.idx);
        }
    }
});

listener!(SurroundedPower {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.dealer == NO || q.target != me.owner {
            return Dec::ONE;
        }
        let behind = if cx.power_aux(me.owner, me.idx) == 0 { ids::power::BACK_ATTACK_LEFT_POWER } else { ids::power::BACK_ATTACK_RIGHT_POWER };
        if !cx.has_power(q.dealer, behind) {
            return Dec::ONE;
        }
        Dec::frac(15, 1)
    }
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if play.target != NO && me.owner == PLAYER {
            surrounded_update_direction(cx, me, play.target);
        }
    }
    fn before_potion_used(&self, cx: &mut Combat, me: Me, _potion: u16, target: Cid) {
        if cx.in_progress && target != NO && me.owner == PLAYER {
            surrounded_update_direction(cx, me, target);
        }
    }
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, was_removal_prevented: bool) {
        if was_removal_prevented || cx.cr(creature).side == cx.cr(me.owner).side {
            return;
        }
        let hittable = cx.hittable_enemies();
        if hittable.is_empty() {
            return;
        }
        let all_left = hittable.iter().all(|&e| cx.has_power(e, ids::power::BACK_ATTACK_LEFT_POWER));
        let all_right = hittable.iter().all(|&e| cx.has_power(e, ids::power::BACK_ATTACK_RIGHT_POWER));
        if all_left || all_right {
            surrounded_update_direction(cx, me, hittable[0]);
        }
    }
});

fn surrounded_update_direction(cx: &mut Combat, me: Me, target: Cid) {
    let facing = cx.power_aux(me.owner, me.idx);
    if facing == 0 && cx.has_power(target, ids::power::BACK_ATTACK_LEFT_POWER) {
        cx.set_power_aux(me.owner, me.idx, 1);
    } else if facing == 1 && cx.has_power(target, ids::power::BACK_ATTACK_RIGHT_POWER) {
        cx.set_power_aux(me.owner, me.idx, 0);
    }
}

listener!(SandpitPower {
    fn after_side_turn_start_late(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Enemy {
            cx.decrement_power(me.owner, me.idx);
        }
    }
    fn after_removed(&self, cx: &mut Combat, _me: Me, old_owner: Cid) {
        if cx.cr(old_owner).is_dead() || cx.cr(PLAYER).is_dead() {
            return;
        }
        let mut victims: crate::util::ArrayVec<Cid, 2> = crate::util::ArrayVec::new();
        victims.push(PLAYER);
        if let Some(o) = cx.osty() {
            victims.push(o);
        }
        for &v in victims.iter() {
            cx.kill_ex(&[v], true);
        }
    }
});

listener!(DisintegrationPower {
    fn after_side_turn_end_late(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_side(cx, me, side) {
            let amount = cx.power_amount(me.owner, me.id);
            cx.damage(&[me.owner], Dec::int(amount as i64), ValueProp::UNPOWERED, me.owner, NO);
        }
    }
});

listener!(MindRotPower {
    fn modify_hand_draw(&self, cx: &Combat, me: Me, count: Dec) -> Dec {
        if me.owner != PLAYER {
            return count;
        }
        (count - Dec::int(cx.power_amount(me.owner, me.id) as i64)).max(Dec::ZERO)
    }
});

listener!(SlothPower {
    fn should_play(&self, cx: &Combat, me: Me, _card: CardIdx) -> bool {
        cx.power_aux(me.owner, me.idx) < cx.power_amount(me.owner, me.id)
    }
    fn before_card_played(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        if me.owner == PLAYER {
            let a = cx.power_aux(me.owner, me.idx);
            cx.set_power_aux(me.owner, me.idx, a + 1);
        }
    }
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_side(cx, me, side) {
            cx.set_power_aux(me.owner, me.idx, 0);
        }
    }
});

listener!(WasteAwayPower {
    fn modify_max_energy(&self, cx: &Combat, me: Me, amount: Dec) -> Dec {
        if me.owner != PLAYER {
            return amount;
        }
        amount - Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
});
