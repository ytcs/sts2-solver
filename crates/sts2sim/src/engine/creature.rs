//! Creature lifecycle, HP / block primitives and death handling (spec 02 §4-5, spec 01 §13).

use crate::content;
use crate::dec::Dec;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;

const MAX_STAT: i32 = 999_999_999;

/// `DamageResult`.
#[derive(Clone, Copy, Default, Debug)]
pub struct DamageResult {
    pub receiver: Cid,
    pub unblocked: i32,
    pub overkill: i32,
    pub killed: bool,
    pub blocked: i32,
    pub block_broken: bool,
    pub fully_blocked: bool,
}

impl Combat {
    pub fn alloc_creature(&mut self) -> Option<Cid> {
        // Slot 0 is the player; recycle freed slots.
        (1..MAX_CREATURES).find(|&i| !self.creatures[i].active).map(|i| i as Cid)
    }

    #[inline(always)]
    pub fn cr(&self, c: Cid) -> &Creature {
        &self.creatures[c as usize]
    }
    #[inline(always)]
    pub fn cr_mut(&mut self, c: Cid) -> &mut Creature {
        &mut self.creatures[c as usize]
    }

    // ---- primitives (Creature.*Internal) -------------------------------------------------------------------

    /// `Creature.DamageBlockInternal`: returns the (fractional) blocked amount; `Block -= (int)blocked`.
    pub fn damage_block_internal(&mut self, c: Cid, amount: Dec, props: ValueProp) -> Dec {
        let cr = self.cr_mut(c);
        let blocked = if props.unblockable() { Dec::ZERO } else { Dec::int(cr.block as i64).min(amount) };
        cr.block -= blocked.trunc();
        blocked
    }

    /// `Creature.LoseHpInternal`.
    pub fn lose_hp_internal(&mut self, c: Cid, amount: Dec) -> DamageResult {
        let cr = self.cr_mut(c);
        let killed = cr.hp > 0 && amount >= Dec::int(cr.hp as i64);
        let before = cr.hp;
        let n = amount.max(Dec::ZERO).min(Dec::int(MAX_STAT as i64)).trunc();
        cr.hp = (cr.hp - n).max(0);
        DamageResult {
            receiver: c,
            unblocked: before - cr.hp,
            overkill: if killed { (n - before).max(0) } else { 0 },
            killed,
            ..Default::default()
        }
    }

    /// `Creature.GainBlockInternal`: `Block = (int)min(Block + amount, 999_999_999)`.
    pub fn gain_block_internal(&mut self, c: Cid, amount: Dec) {
        let cr = self.cr_mut(c);
        let v = (Dec::int(cr.block as i64) + amount).min(Dec::int(MAX_STAT as i64));
        cr.block = v.trunc();
    }

    /// `Creature.SetCurrentHpInternal`: `CurrentHp = (int)min(amount, MaxHp)`.
    pub fn set_current_hp_internal(&mut self, c: Cid, amount: Dec) {
        let cr = self.cr_mut(c);
        cr.hp = amount.min(Dec::int(cr.max_hp as i64)).trunc().max(0);
    }

    // ---- commands --------------------------------------------------------------------------------------------

    /// `CreatureCmd.GainBlock` (spec 02 §4.1). Returns the (modified) amount applied.
    pub fn gain_block(&mut self, c: Cid, amount: Dec, props: ValueProp, card: CardIdx) -> Dec {
        if self.is_over_or_ending() || self.cr(c).is_dead() {
            return Dec::ZERO;
        }
        self.dispatch_g(hookbit::before_block_gained, |cx, me, l| l.before_block_gained(cx, me, c, amount, props, card));
        let (v, mods) = self.modify_block_ex(c, amount, props, card);
        let v = v.max(Dec::ZERO);
        if self.hooks_enabled() {
            for me in mods.iter() {
                if self.still_live(me) {
                    content::listener(me).after_modifying_block_amount(self, *me, v, card);
                }
            }
        }
        if v > Dec::ZERO {
            self.gain_block_internal(c, v);
        }
        self.dispatch_g(hookbit::after_block_gained, |cx, me, l| l.after_block_gained(cx, me, c, v));
        v
    }

    /// `Hook.ModifyBlock` (spec 02 §4.1): enchantment add/mul, additive pass, multiplicative pass, floor at 0.
    pub fn modify_block(&self, target: Cid, amount: Dec, props: ValueProp, card: CardIdx) -> Dec {
        self.modify_block_ex(target, amount, props, card).0
    }

    /// `Hook.ModifyBlock` with the list of models that changed the value (non-zero adders, non-1 multipliers).
    pub fn modify_block_ex(&self, target: Cid, amount: Dec, props: ValueProp, card: CardIdx) -> (Dec, super::Mods) {
        let m = (Mask::bit(hookbit::modify_block_additive)) | (Mask::bit(hookbit::modify_block_multiplicative));
        let mut mods = super::Mods::new();
        let mut v = amount;
        if card != NO && self.cards[card as usize].enchant != 0 {
            let me = self.enchantment_me(card);
            let l = content::listener(&me);
            v += l.enchant_block_additive(self, me, v);
            v *= l.enchant_block_multiplicative(self, me, v);
        }
        let snap = self.snapshot(m);
        for e in snap.iter() {
            if e.mask.has(hookbit::modify_block_additive) && self.still_live(&e.me) {
                let q = BlockQ { target, card, props, amount: v };
                let d = content::listener(&e.me).modify_block_additive(self, e.me, &q);
                v += d;
                if !d.is_zero() {
                    mods.push(e.me);
                }
            }
        }
        for e in snap.iter() {
            if e.mask.has(hookbit::modify_block_multiplicative) && self.still_live(&e.me) {
                let q = BlockQ { target, card, props, amount: v };
                let f = content::listener(&e.me).modify_block_multiplicative(self, e.me, &q);
                v *= f;
                if f != Dec::ONE {
                    mods.push(e.me);
                }
            }
        }
        (v.max(Dec::ZERO), mods)
    }

    /// `CreatureCmd.Heal(creature, amount)`: no-op for non-players once combat is ending; `(int)min(hp + amount, max)`;
    /// healing a dead player revives it (hooks re-activate); `AfterCurrentHpChanged(amount)` fires with the REQUESTED
    /// amount if `amount > 0` (spec 02 §5.2).
    pub fn heal(&mut self, c: Cid, amount: Dec) {
        if !self.cr(c).is_player && self.is_ending() {
            return;
        }
        let was_dead = self.cr(c).is_dead();
        let cur = Dec::int(self.cr(c).hp as i64);
        self.set_current_hp_internal(c, cur + amount);
        if was_dead && self.cr(c).is_alive() && c == PLAYER {
            self.player_hooks_active = true;
        }
        if amount > Dec::ZERO && self.cr(c).in_combat {
            let d = amount.trunc();
            self.dispatch_u(hookbit::after_current_hp_changed, |cx, me, l| l.after_current_hp_changed(cx, me, c, d));
        }
    }

    /// `CreatureCmd.SetCurrentHp`: hook delta if the value changed; kills if the creature ends up dead.
    pub fn set_current_hp(&mut self, c: Cid, amount: Dec) {
        let old = self.cr(c).hp;
        self.set_current_hp_internal(c, amount);
        if self.cr(c).hp != old || amount != Dec::int(old as i64) {
            let d = amount.trunc() - old;
            self.dispatch_u(hookbit::after_current_hp_changed, |cx, me, l| l.after_current_hp_changed(cx, me, c, d));
        }
        if self.cr(c).is_dead() {
            self.kill(&[c]);
        }
    }

    /// `CreatureCmd.SetMaxHp`: returns the change; max HP <= 0 kills.
    pub fn set_max_hp(&mut self, c: Cid, amount: Dec) -> i32 {
        let old = self.cr(c).max_hp;
        let n = amount.max(Dec::ZERO).min(Dec::int(MAX_STAT as i64)).trunc();
        let cr = self.cr_mut(c);
        cr.max_hp = n;
        cr.hp = cr.hp.min(n);
        if self.cr(c).max_hp <= 0 {
            self.kill(&[c]);
        }
        self.cr(c).max_hp - old
    }

    /// `CreatureCmd.GainMaxHp`: raise max HP, then heal by the change.
    pub fn gain_max_hp(&mut self, c: Cid, amount: Dec) {
        let cur = Dec::int(self.cr(c).max_hp as i64);
        let delta = self.set_max_hp(c, cur + amount);
        self.heal(c, Dec::int(delta as i64));
    }

    /// `CreatureCmd.LoseMaxHp`: if the new max is below the current HP, a full `Damage` call (Unblockable | Unpowered
    /// [| Move]) removes the excess first; then max HP becomes `max(1, new)`.
    pub fn lose_max_hp(&mut self, c: Cid, amount: Dec, is_from_card: bool) {
        let new_max = Dec::int(self.cr(c).max_hp as i64) - amount;
        let hp = Dec::int(self.cr(c).hp as i64);
        if new_max < hp {
            let mut props = ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED);
            if is_from_card {
                props = props.or(ValueProp::MOVE);
            }
            self.damage(&[c], hp - new_max, props, NO, NO);
        }
        self.set_max_hp(c, new_max.max(Dec::ONE));
    }

    // ---- death: see `death.rs` ----

    /// `CombatManager.RemoveCreature` + `CombatState.RemoveCreature`: leaves hooks / the enemy list. A monster that
    /// dies during its own move stays attached until the move finishes (handled by `perform_move`).
    pub fn remove_creature(&mut self, c: Cid) {
        if self.cr(c).monster.is_performing {
            return;
        }
        self.detach_creature(c);
    }

    pub fn detach_creature(&mut self, c: Cid) {
        self.enemies.remove_value(c);
        self.allies.remove_value(c);
        let cr = self.cr_mut(c);
        cr.in_combat = false;
    }
}
