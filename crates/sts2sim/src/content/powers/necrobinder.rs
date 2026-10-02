//! Necrobinder powers (Osty / Doom / Souls).

use crate::dec::Dec;
use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

// Osty's permanent power: the owner's powered attacks hit Osty first. It survives Osty's death (and revival), and a dead
// Osty (hp 0) is not hittable / cannot receive powers.
listener!(DieForYouPower {
    fn modify_unblocked_damage_target(&self, cx: &Combat, me: Me, target: Cid, _amount: Dec, props: ValueProp, _dealer: Cid) -> Cid {
        if target != cx.cr(me.owner).owner {
            return target;
        }
        if cx.cr(me.owner).is_dead() {
            return target;
        }
        if !props.is_powered() {
            return target;
        }
        me.owner
    }
    fn should_allow_hitting(&self, cx: &Combat, _me: Me, creature: Cid) -> bool {
        cx.cr(creature).is_alive()
    }
    fn should_power_be_removed_after_owner_death(&self) -> bool {
        false
    }
});
