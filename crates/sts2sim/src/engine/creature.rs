use crate::content;
use crate::dec::Dec;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;

const MAX_STAT: i32 = 999_999_999;

#[derive(Clone, Copy, Default, Debug)]
pub struct DamageResult {
    pub receiver: Cid,
    pub unblocked: i32,
    pub overkill: i32,
    pub killed: bool,
    pub blocked: i32,
    pub block_broken: bool,
    pub fully_blocked: bool,
    pub hit: u8,
}

impl DamageResult {
    #[inline]
    pub fn total(&self) -> i32 {
        self.blocked + self.unblocked
    }
}

impl Combat {
    pub fn alloc_creature(&mut self) -> Option<Cid> {
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

    pub fn damage_block_internal(&mut self, c: Cid, amount: Dec, props: ValueProp) -> Dec {
        let cr = self.cr_mut(c);
        let blocked = if props.unblockable() { Dec::ZERO } else { Dec::int(cr.block() as i64).min(amount) };
        cr.set_block(cr.block() - blocked.trunc());
        blocked
    }

    pub fn lose_hp_internal(&mut self, c: Cid, amount: Dec) -> DamageResult {
        let cr = self.cr_mut(c);
        let before = cr.hp();
        let killed = before > 0 && amount >= Dec::int(before as i64);
        let n = amount.max(Dec::ZERO).min(Dec::int(MAX_STAT as i64)).trunc();
        cr.set_hp((before - n).max(0));
        DamageResult {
            receiver: c,
            unblocked: before - cr.hp(),
            overkill: if killed { (n - before).max(0) } else { 0 },
            killed,
            ..Default::default()
        }
    }

    pub fn gain_block_internal(&mut self, c: Cid, amount: Dec) {
        let cr = self.cr_mut(c);
        let v = (Dec::int(cr.block() as i64) + amount).min(Dec::int(MAX_STAT as i64));
        cr.set_block(v.trunc());
    }

    pub fn set_current_hp_internal(&mut self, c: Cid, amount: Dec) {
        let cr = self.cr_mut(c);
        cr.set_hp(amount.min(Dec::int(cr.max_hp as i64)).trunc().max(0));
    }

    pub fn gain_block(&mut self, c: Cid, amount: Dec, props: ValueProp, card: CardIdx) -> Dec {
        if self.is_over_or_ending() || self.cr(c).is_dead() {
            return Dec::ZERO;
        }
        self.dispatch_g(hookbit::before_block_gained, |cx, me, l| l.before_block_gained(cx, me, c, amount, props, card));
        let mut mods = super::Mods::new();
        let v = self.modify_block_into(c, amount, props, card, &mut mods);
        let v = v.max(Dec::ZERO);
        self.dispatch_modifiers(true, hookbit::after_modifying_block_amount, &mods, |cx, me, l| l.after_modifying_block_amount(cx, me, v, card));
        if v > Dec::ZERO {
            self.gain_block_internal(c, v);
            self.hist_push(crate::engine::HKind::BlockGained, c, NO, self.play_serial, card, v.trunc(), (card != NO) as u8, props.0, 0);
        }
        self.dispatch_g(hookbit::after_block_gained, |cx, me, l| l.after_block_gained(cx, me, c, v));
        v
    }

    pub fn modify_block(&self, target: Cid, amount: Dec, props: ValueProp, card: CardIdx) -> Dec {
        let mut mods = super::Mods::new();
        self.modify_block_into(target, amount, props, card, &mut mods)
    }

    pub fn modify_block_ex(&self, target: Cid, amount: Dec, props: ValueProp, card: CardIdx) -> (Dec, super::Mods) {
        let mut mods = super::Mods::new();
        let v = self.modify_block_into(target, amount, props, card, &mut mods);
        (v, mods)
    }

    pub fn modify_block_into(&self, target: Cid, amount: Dec, props: ValueProp, card: CardIdx, mods: &mut super::Mods) -> Dec {
        let m = (Mask::bit(hookbit::modify_block_additive)) | (Mask::bit(hookbit::modify_block_multiplicative));
        let mut v = amount;
        if card != NO && self.cards[card as usize].enchant != 0 {
            let me = self.enchantment_me(card);
            let l = content::listener(&me);
            v += l.enchant_block_additive(self, me, v);
            v *= l.enchant_block_multiplicative(self, me, v);
        }
        let mut snap = crate::engine::Snapshot::new();
        self.snapshot_into(m, &mut snap);
        for e in snap.iter() {
            if self.has_hook(&e.me, hookbit::modify_block_additive) && self.still_live(&e.me) {
                let q = BlockQ { target, card, props, amount: v };
                let d = content::listener(&e.me).modify_block_additive(self, e.me, &q);
                v += d;
                if !d.is_zero() {
                    mods.push(e.me);
                }
            }
        }
        for e in snap.iter() {
            if self.has_hook(&e.me, hookbit::modify_block_multiplicative) && self.still_live(&e.me) {
                let q = BlockQ { target, card, props, amount: v };
                let f = content::listener(&e.me).modify_block_multiplicative(self, e.me, &q);
                v *= f;
                if f != Dec::ONE {
                    mods.push(e.me);
                }
            }
        }
        v.max(Dec::ZERO)
    }

    pub fn lose_block(&mut self, c: Cid, amount: Dec, remover: Cid) {
        if self.is_over_or_ending() || self.cr(c).is_dead() || amount <= Dec::ZERO {
            return;
        }
        let before = self.cr(c).block();
        self.cr_mut(c).set_block((Dec::int(before as i64) - amount).max(Dec::ZERO).trunc());
        if before > 0 && self.cr(c).block() <= 0 {
            self.dispatch_u(hookbit::after_block_broken, |cx, me, l| l.after_block_broken(cx, me, c, remover));
        }
    }

    pub fn heal(&mut self, c: Cid, amount: Dec) {
        if !self.cr(c).is_player && self.is_ending() {
            return;
        }
        let was_dead = self.cr(c).is_dead();
        let cur = Dec::int(self.cr(c).hp() as i64);
        self.set_current_hp_internal(c, cur + amount);
        if was_dead && self.cr(c).is_alive() && c == PLAYER {
            self.player_hooks_active = true;
        }
        if amount > Dec::ZERO && self.cr(c).in_combat {
            let d = amount.trunc();
            self.dispatch_u(hookbit::after_current_hp_changed, |cx, me, l| l.after_current_hp_changed(cx, me, c, d));
        }
    }

    pub fn set_current_hp(&mut self, c: Cid, amount: Dec) {
        let old = self.cr(c).hp();
        self.set_current_hp_internal(c, amount);
        if self.cr(c).hp() != old || amount != Dec::int(old as i64) {
            let d = amount.trunc() - old;
            self.dispatch_u(hookbit::after_current_hp_changed, |cx, me, l| l.after_current_hp_changed(cx, me, c, d));
        }
        if self.cr(c).is_dead() {
            self.kill(&[c]);
        }
    }

    pub fn set_max_hp(&mut self, c: Cid, amount: Dec) -> i32 {
        let old = self.cr(c).max_hp;
        let n = amount.max(Dec::ZERO).min(Dec::int(MAX_STAT as i64)).trunc();
        let cr = self.cr_mut(c);
        cr.max_hp = n;
        cr.set_hp(cr.hp().min(n));
        if self.cr(c).max_hp <= 0 {
            self.kill(&[c]);
        }
        self.cr(c).max_hp - old
    }

    pub fn gain_max_hp(&mut self, c: Cid, amount: Dec) {
        let cur = Dec::int(self.cr(c).max_hp as i64);
        let delta = self.set_max_hp(c, cur + amount);
        self.heal(c, Dec::int(delta as i64));
    }

    pub fn lose_max_hp(&mut self, c: Cid, amount: Dec, is_from_card: bool) {
        let new_max = Dec::int(self.cr(c).max_hp as i64) - amount;
        let hp = Dec::int(self.cr(c).hp() as i64);
        if new_max < hp {
            let mut props = ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED);
            if is_from_card {
                props = props.or(ValueProp::MOVE);
            }
            self.damage(&[c], hp - new_max, props, NO, NO);
        }
        self.set_max_hp(c, new_max.max(Dec::ONE));
    }

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
