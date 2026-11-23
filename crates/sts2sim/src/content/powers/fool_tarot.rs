// Powers of The Fool's relics (Conductor's Baton).
use crate::dec::Dec;
use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

// `DamageDecrease` = 0.75m (a plain `DynamicVar`, not in the generated tables).
listener!(ConductorsBatonPower {
    fn modify_damage_multiplicative(&self, _cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner || !q.props.is_powered() {
            return Dec::ONE;
        }
        Dec::frac(75, 2)
    }
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            cx.remove_power(me.owner, me.idx);
        }
    }
});
