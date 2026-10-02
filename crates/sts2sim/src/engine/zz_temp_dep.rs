//! TEMP-DEP: copy of the potion teammate's temp-power helpers (engine/potion_gen.rs). Delete before merge.
use crate::hooks::*;
use crate::state::*;
use crate::types::*;

impl Combat {
    pub fn temp_before_applied(&mut self, inner: u16, sign: i32, target: Cid, amount: crate::dec::Dec, applier: Cid, card: CardIdx) {
        self.apply_power(inner, target, crate::dec::Dec::int(sign as i64) * amount, applier, card);
    }
    pub fn temp_after_amount_changed(&mut self, me: Me, inner: u16, sign: i32, ch: &PowerChange) {
        if ch.target != me.owner || ch.uid != me.idx {
            return;
        }
        if ch.amount == self.power_amount(me.owner, me.id) {
            return;
        }
        self.apply_power(inner, me.owner, crate::dec::Dec::int((sign * ch.amount) as i64), ch.applier, ch.card);
    }
    pub fn temp_after_side_turn_end(&mut self, me: Me, inner: u16, sign: i32, side: Side) {
        if self.cr(me.owner).side != side {
            return;
        }
        let amount = self.power_amount(me.owner, me.id);
        self.remove_power(me.owner, me.idx);
        self.apply_power(inner, me.owner, crate::dec::Dec::int((-sign * amount) as i64), me.owner, NO);
    }
}
