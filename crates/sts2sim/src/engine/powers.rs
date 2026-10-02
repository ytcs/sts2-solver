//! Power application / stacking / removal (spec 02 §6).

use super::damage::Mods;
use crate::content;
use crate::dec::Dec;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;

const MAX_POWER_AMOUNT: i32 = 999_999_999;

impl Combat {
    pub fn power_idx(&self, c: Cid, uid: u16) -> Option<usize> {
        self.cr(c).powers.iter().position(|p| p.uid == uid)
    }

    /// Mutable access to a live power instance by uid (private `aux` state).
    pub fn power_mut(&mut self, c: Cid, uid: u16) -> Option<&mut Power> {
        self.cr_mut(c).powers.as_mut_slice().iter_mut().find(|p| p.uid == uid)
    }

    /// Current amount of the creature's power `id` (0 if absent).
    #[inline]
    pub fn power_amount(&self, c: Cid, id: u16) -> i32 {
        self.cr(c).power_amount(id)
    }

    #[inline]
    pub fn has_power(&self, c: Cid, id: u16) -> bool {
        self.cr(c).power(id).is_some()
    }

    /// `Creature.CanReceivePowers`: attached to the combat and `ShouldAllowHitting` (AND over listeners).
    pub fn can_receive_powers(&self, c: Cid) -> bool {
        self.cr(c).in_combat && self.should_allow_hitting(c)
    }

    /// `Hook.ShouldAllowHitting` — AND over guarded listeners (every creature is hittable once combat is ending).
    pub fn should_allow_hitting(&self, c: Cid) -> bool {
        if !self.hooks_enabled() {
            return true;
        }
        let snap = self.snapshot(Mask::bit(hookbit::should_allow_hitting));
        for e in snap.iter() {
            if self.still_live(&e.me) && !content::listener(&e.me).should_allow_hitting(self, e.me, c) {
                return false;
            }
        }
        true
    }

    /// `PowerModel.GetTypeForAmount`.
    pub fn power_type_for_amount(id: u16, amount: i32) -> PowerType {
        let d = content::power_def(id);
        if d.counter && d.allow_negative && amount < 0 {
            PowerType::Debuff
        } else if !d.allow_negative && d.ptype == PowerType::Debuff && amount < 0 {
            PowerType::Buff
        } else {
            d.ptype
        }
    }

    fn find_stacking_instance(&self, id: u16, target: Cid, applier: Cid) -> Option<usize> {
        let d = content::power_def(id);
        match d.instance {
            InstanceType::Instanced => None,
            InstanceType::Single => self.cr(target).powers.iter().position(|p| p.id == id),
            InstanceType::PerApplier => self.cr(target).powers.iter().position(|p| p.id == id && p.applier == applier),
        }
    }

    /// `PowerCmd.Apply<T>`. Returns the power uid if the power exists afterwards (stacked or new).
    pub fn apply_power(&mut self, id: u16, target: Cid, amount: Dec, applier: Cid, card: CardIdx) -> Option<u16> {
        if self.is_ending() {
            return None;
        }
        if !self.can_receive_powers(target) {
            return None;
        }
        if let Some(i) = self.find_stacking_instance(id, target, applier) {
            let uid = self.cr(target).powers[i].uid;
            let new_amount = self.modify_power_amount(target, uid, amount, applier, card);
            return if new_amount == 0 { None } else { Some(uid) };
        }
        self.apply_new_power(id, target, amount, applier, card)
    }

    fn apply_new_power(&mut self, id: u16, target: Cid, amount: Dec, applier: Cid, card: CardIdx) -> Option<u16> {
        if self.is_ending() || amount.is_zero() || !self.can_receive_powers(target) {
            return None;
        }
        if let Some(i) = self.find_stacking_instance(id, target, applier) {
            let uid = self.cr(target).powers[i].uid;
            self.modify_power_amount(target, uid, amount, applier, card);
            return Some(uid);
        }
        let uid = self.next_power_uid;
        self.next_power_uid = self.next_power_uid.wrapping_add(1);
        self.listen |= content::power_mask(id);
        if !content::power_implemented(id) {
            self.flag_missing(Kind::Power, id);
        }
        // Not yet attached: a stand-in `Me` for the not-yet-existing power.
        let me = Me { kind: Kind::Power, owner: target, idx: uid, id, amount: 0 };
        self.cur_power_card = card;
        self.dispatch_g(hookbit::before_power_amount_changed, |cx, m, l| l.before_power_amount_changed(cx, m, id, amount, target, applier));
        let (mut v, given_mods) = if applier != NO && self.cr(applier).in_combat {
            self.modify_power_amount_given(id, applier, amount, target, card)
        } else {
            (amount, Mods::new())
        };
        let (v2, recv_mods) = self.modify_power_amount_received(id, target, v, applier);
        v = v2;
        content::listener(&me).before_applied(self, me, target, v, applier, card);
        if self.can_receive_powers(target) {
            let d = content::power_def(id);
            let mut attached = false;
            if !v.is_zero() {
                let amt = v.trunc().clamp(-MAX_POWER_AMOUNT, MAX_POWER_AMOUNT);
                let p = Power { id, uid, amount: amt, amount_on_turn_start: 0 /* set by the next turn start (PowerModel._amountOnTurnStart default) */, aux: 0, applier, skip_next_tick: false };
                self.cr_mut(target).powers.push(p);
                attached = true;
            }
            if attached && self.cr(target).side == Side::Player && d.ptype == PowerType::Debuff {
                if let Some(i) = self.power_idx(target, uid) {
                    self.cr_mut(target).powers[i].skip_next_tick = true;
                }
            }
            for m in given_mods.iter() {
                if self.still_live(m) {
                    content::listener(m).after_modifying_power_amount_given(self, *m, id);
                }
            }
            for m in recv_mods.iter() {
                if self.still_live(m) {
                    content::listener(m).after_modifying_power_amount_received(self, *m, id);
                }
            }
            if !v.is_zero() {
                let amt = self.power_idx(target, uid).map_or(v.trunc(), |i| self.cr(target).powers[i].amount);
                let me = Me { kind: Kind::Power, owner: target, idx: uid, id, amount: amt };
                if self.power_idx(target, uid).is_some() {
                    content::listener(&me).after_applied(self, me);
                }
                let vi = v.trunc();
                self.dispatch_g(hookbit::after_power_amount_changed, |cx, m, l| l.after_power_amount_changed(cx, m, id, vi));
                let ch = PowerChange { power_id: id, target, uid, amount: vi, applier, card };
                self.dispatch_g(hookbit::after_power_amount_changed_full, |cx, m, l| l.after_power_amount_changed_full(cx, m, &ch));
            }
            return if attached { Some(uid) } else { None };
        }
        None
    }

    /// `ModifyPowerAmountGiven`: additive pass then multiplicative pass (SneckoSkull / UnsettlingLamp).
    fn modify_power_amount_given(&self, id: u16, giver: Cid, amount: Dec, target: Cid, card: CardIdx) -> (Dec, Mods) {
        let m = (Mask::bit(hookbit::modify_power_amount_given_additive)) | (Mask::bit(hookbit::modify_power_amount_given_multiplicative));
        let snap = self.snapshot(m);
        let mut v = amount;
        let mut mods = Mods::new();
        for e in snap.iter() {
            if e.mask.has(hookbit::modify_power_amount_given_additive) && self.still_live(&e.me) {
                let d = content::listener(&e.me).modify_power_amount_given_additive(self, e.me, id, giver, v, target, card);
                v += d;
                if !d.is_zero() {
                    mods.push(e.me);
                }
            }
        }
        for e in snap.iter() {
            if e.mask.has(hookbit::modify_power_amount_given_multiplicative) && self.still_live(&e.me) {
                let f = content::listener(&e.me).modify_power_amount_given_multiplicative(self, e.me, id, giver, v, target, card);
                v *= f;
                if f != Dec::ONE {
                    mods.push(e.me);
                }
            }
        }
        (v, mods)
    }

    /// `ModifyPowerAmountReceived`: threaded TRY hooks (Artifact, RuinedHelmet).
    fn modify_power_amount_received(&self, id: u16, target: Cid, amount: Dec, applier: Cid) -> (Dec, Mods) {
        let snap = self.snapshot(Mask::bit(hookbit::try_modify_power_amount_received));
        let mut v = amount;
        let mut mods = Mods::new();
        if !self.hooks_enabled() {
            return (v, mods);
        }
        for e in snap.iter() {
            if self.still_live(&e.me) {
                if let Some(nv) = content::listener(&e.me).try_modify_power_amount_received(self, e.me, id, target, v, applier) {
                    v = nv;
                    mods.push(e.me);
                }
            }
        }
        (v, mods)
    }

    /// `PowerCmd.ModifyAmount` — the stacking / decrement path. Returns the new amount.
    pub fn modify_power_amount(&mut self, c: Cid, uid: u16, offset: Dec, applier: Cid, card: CardIdx) -> i32 {
        if self.is_ending() {
            return 0;
        }
        if !self.cr(c).in_combat {
            return 0;
        }
        let Some(i) = self.power_idx(c, uid) else { return 0 };
        let id = self.cr(c).powers[i].id;
        self.cur_power_card = card;
        self.dispatch_g(hookbit::before_power_amount_changed, |cx, m, l| l.before_power_amount_changed(cx, m, id, offset, c, applier));
        let (mut v, given_mods) = if applier != NO && self.cr(applier).in_combat {
            self.modify_power_amount_given(id, applier, offset, c, card)
        } else {
            (offset, Mods::new())
        };
        let (v2, recv_mods) = self.modify_power_amount_received(id, c, v, applier);
        v = v2;
        let Some(i) = self.power_idx(c, uid) else { return 0 };
        let new_amount = (self.cr(c).powers[i].amount as i64 + v.trunc() as i64)
            .clamp(-(MAX_POWER_AMOUNT as i64), MAX_POWER_AMOUNT as i64) as i32;
        self.cr_mut(c).powers[i].amount = new_amount;
        for m in given_mods.iter() {
            if self.still_live(m) {
                content::listener(m).after_modifying_power_amount_given(self, *m, id);
            }
        }
        for m in recv_mods.iter() {
            if self.still_live(m) {
                content::listener(m).after_modifying_power_amount_received(self, *m, id);
            }
        }
        let vi = v.trunc();
        if vi != 0 {
            self.dispatch_g(hookbit::after_power_amount_changed, |cx, m, l| l.after_power_amount_changed(cx, m, id, vi));
            let ch = PowerChange { power_id: id, target: c, uid, amount: vi, applier, card };
            self.dispatch_g(hookbit::after_power_amount_changed_full, |cx, m, l| l.after_power_amount_changed_full(cx, m, &ch));
        }
        if let Some(i) = self.power_idx(c, uid) {
            let d = content::power_def(id);
            let a = self.cr(c).powers[i].amount;
            let remove = if d.allow_negative { a == 0 } else { a <= 0 };
            if remove {
                self.remove_power(c, uid);
            }
        }
        new_amount
    }

    /// `PowerCmd.Remove`: list removal then `AfterRemoved` (no amount hooks).
    pub fn remove_power(&mut self, c: Cid, uid: u16) {
        if let Some(i) = self.power_idx(c, uid) {
            let p = self.cr_mut(c).powers.remove(i);
            let me = Me { kind: Kind::Power, owner: c, idx: p.uid, id: p.id, amount: p.amount };
            content::listener(&me).after_removed(self, me, c);
        }
    }

    /// `PowerCmd.Decrement(power)` = `ModifyAmount(-1, null, null)`.
    pub fn decrement_power(&mut self, c: Cid, uid: u16) {
        self.modify_power_amount(c, uid, Dec::int(-1), NO, NO);
    }

    /// `PowerCmd.TickDownDuration`.
    pub fn tick_down_power(&mut self, c: Cid, uid: u16) {
        if let Some(i) = self.power_idx(c, uid) {
            if self.cr(c).powers[i].skip_next_tick {
                self.cr_mut(c).powers[i].skip_next_tick = false;
            } else {
                self.decrement_power(c, uid);
            }
        }
    }
}
