use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn owner_on(cx: &Combat, me: Me, side: Side) -> bool {
    cx.cr(me.owner).side == side
}

listener!(AccelerantPower {});

listener!(AccuracyPower {
    fn modify_damage_additive(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if me.owner != q.dealer || !q.props.is_powered() || q.card == NO {
            return Dec::ZERO;
        }
        if cx.card_def(q.card).tags & tag::SHIV == 0 {
            return Dec::ZERO;
        }
        Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
});

listener!(FanOfKnivesPower {});

listener!(EnvenomPower {
    fn after_damage_given(&self, cx: &mut Combat, me: Me, dealer: Cid, target: Cid, unblocked: i32, props: ValueProp) {
        if dealer == me.owner && props.is_powered() && unblocked > 0 {
            let amt = cx.power_amount(me.owner, me.id);
            cx.apply_power(ids::power::POISON_POWER, target, Dec::int(amt as i64), me.owner, NO);
        }
    }
});

listener!(AfterimagePower {
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let amount = cx.power_amount(me.owner, me.id);
        cx.hist.remember_play(me.idx, play.card, amount);
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if let Some(amount) = cx.hist.take_play(me.idx, play.card) {
            if amount > 0 {
                cx.gain_block(me.owner, Dec::int(amount as i64), ValueProp::UNPOWERED, NO);
            }
        }
    }
});

listener!(AnticipatePower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        cx.apply_power(ids::power::DEXTERITY_POWER, target, amount, applier, card);
    }
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        if ch.power_id == me.id && ch.target == me.owner && ch.amount != cx.power_amount(me.owner, me.id) {
            cx.apply_power(ids::power::DEXTERITY_POWER, me.owner, Dec::int(ch.amount as i64), me.owner, NO);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) {
            let amt = cx.power_amount(me.owner, me.id);
            cx.remove_power(me.owner, me.idx);
            cx.apply_power(ids::power::DEXTERITY_POWER, me.owner, Dec::int(-(amt as i64)), me.owner, NO);
        }
    }
});

listener!(BlurPower {
    fn should_clear_block(&self, _cx: &Combat, me: Me, creature: Cid) -> bool {
        me.owner != creature
    }
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

listener!(BurstPower {
    fn modify_card_play_count(&self, cx: &Combat, me: Me, card: CardIdx, _target: Cid, count: i32) -> i32 {
        if me.owner != PLAYER || cx.card_def(card).ctype != CardType::Skill {
            return count;
        }
        count + 1
    }
    fn after_modifying_card_play_count(&self, cx: &mut Combat, me: Me, _card: CardIdx) {
        cx.decrement_power(me.owner, me.idx);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

listener!(CorrosiveWavePower {
    fn after_card_drawn(&self, cx: &mut Combat, me: Me, _card: CardIdx, _from_hand_draw: bool) {
        if me.owner != PLAYER {
            return;
        }
        let amt = cx.power_amount(me.owner, me.id);
        cx.apply_power_to_hittable_enemies(ids::power::POISON_POWER, Dec::int(amt as i64), me.owner, NO);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

listener!(FlankingPower {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner || !q.props.is_powered() {
            return Dec::ONE;
        }
        let applier = cx.power_idx(me.owner, me.idx).map_or(NO, |i| cx.cr(me.owner).powers[i].applier);
        if q.dealer == applier {
            return Dec::ONE;
        }
        Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});
