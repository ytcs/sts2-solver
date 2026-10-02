//! Powers that only event combatants use (spec 04 §3.5).

use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::Side;

// ---- BattlewornDummyTimeLimitPower: counts down at the end of the dummy's own side turn, then the dummy escapes ----------------
// (`encounter.RanOutOfTime = true` only feeds the event's post-combat text/rewards; not combat state.)
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
