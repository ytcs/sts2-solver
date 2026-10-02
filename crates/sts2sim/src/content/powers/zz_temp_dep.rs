//! TEMP-DEP: powers other teammates own, copied only so this branch can be swept (drop before merge).
use crate::dec::Dec;
use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn owner_on(cx: &Combat, me: Me, side: Side) -> bool {
    cx.cr(me.owner).side == side
}

fn amount_on_turn_start(cx: &Combat, me: Me) -> i32 {
    cx.power_idx(me.owner, me.idx).map_or(0, |i| cx.cr(me.owner).powers[i].amount_on_turn_start)
}

// Draw `Amount` extra cards at the start of the next turn (only if it was already present when this turn began).
listener!(DrawCardsNextTurnPower {
    fn modify_hand_draw(&self, cx: &Combat, me: Me, count: Dec) -> Dec {
        if amount_on_turn_start(cx, me) == 0 {
            return count;
        }
        count + Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) && amount_on_turn_start(cx, me) != 0 {
            cx.remove_power(me.owner, me.idx);
        }
    }
});


listener!(IntangiblePower {
    fn modify_hp_lost_after_osty(&self, cx: &Combat, me: Me, target: Cid, amount: Dec, _props: ValueProp, _dealer: Cid, _card: CardIdx) -> Dec {
        if !cx.in_progress || target != me.owner || amount < Dec::ONE {
            return amount;
        }
        Dec::ONE
    }
    fn modify_damage_cap(&self, _cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner {
            return Dec::MAX;
        }
        Dec::ONE
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Enemy {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});
