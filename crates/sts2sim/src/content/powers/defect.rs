//! Defect powers.

use crate::dec::Dec;
use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

// FocusPower.ModifyOrbValue: `max(value + Amount, 0)` for the owner's orbs.
listener!(FocusPower {
    fn modify_orb_value(&self, cx: &Combat, me: Me, _orb: &Orb, value: Dec) -> Dec {
        if me.owner != PLAYER {
            return value;
        }
        (value + Dec::int(cx.power_amount(me.owner, me.id) as i64)).max(Dec::ZERO)
    }
});
