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

// TEMP-DEP: the Ironclad teammates' versions of these helpers.
impl Combat {
    pub fn add_cost_modifier(&mut self, c: CardIdx, amount: i32, expire: u8) {
        if amount == 0 {
            return;
        }
        let card = &mut self.cards[c as usize];
        if let Some(last) = card.mods.as_mut_slice().last_mut() {
            if last.relative && !last.reduce_only && last.expire == expire && (last.amount as i32 + amount).abs() < 100 {
                last.amount = (last.amount as i32 + amount) as i8;
                return;
            }
        }
        card.mods.push(CostMod { amount: amount.clamp(-100, 100) as i8, relative: true, reduce_only: false, expire });
    }
    pub fn resolve_energy_x(&self, c: CardIdx) -> i32 {
        self.cards[c as usize].x_value as i32
    }
    pub fn should_death_trigger_fatal(&self, _c: Cid) -> bool {
        true
    }
}
