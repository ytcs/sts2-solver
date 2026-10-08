use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::Side;

listener!(BattlewornDummyTimeLimitPower {
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side != side {
            return;
        }
        if cx.power_amount(me.owner, me.id) > 1 {
            cx.decrement_power(me.owner, me.idx);
            return;
        }
        cx.escape(me.owner);
    }
});
