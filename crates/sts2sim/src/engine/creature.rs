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
        // BeforeBlockGained (raw amount) — no content yet.
        let v = self.modify_block(c, amount, props, card).max(Dec::ZERO);
        // AfterModifyingBlockAmount(mods) — no content yet.
        if v > Dec::ZERO {
            self.gain_block_internal(c, v);
        }
        self.dispatch_g(hookbit::after_block_gained, |cx, me, l| l.after_block_gained(cx, me, c, v));
        v
    }

    /// `Hook.ModifyBlock` (spec 02 §4.1): additive pass, multiplicative pass, floor at 0. No rounding.
    pub fn modify_block(&self, target: Cid, amount: Dec, props: ValueProp, card: CardIdx) -> Dec {
        let m = (Mask::bit(hookbit::modify_block_additive)) | (Mask::bit(hookbit::modify_block_multiplicative));
        let snap = self.snapshot(m);
        let mut v = amount;
        for e in snap.iter() {
            if e.mask.has(hookbit::modify_block_additive) && self.still_live(&e.me) {
                let q = BlockQ { target, card, props, amount: v };
                v += content::listener(&e.me).modify_block_additive(self, e.me, &q);
            }
        }
        for e in snap.iter() {
            if e.mask.has(hookbit::modify_block_multiplicative) && self.still_live(&e.me) {
                let q = BlockQ { target, card, props, amount: v };
                v *= content::listener(&e.me).modify_block_multiplicative(self, e.me, &q);
            }
        }
        v.max(Dec::ZERO)
    }

    /// `CreatureCmd.Heal` (single-player, combat): `(int)min(hp + amount, max)`; heals revive a dead player.
    pub fn heal(&mut self, c: Cid, amount: Dec) {
        if !self.cr(c).is_player && self.is_ending() {
            return;
        }
        let cur = Dec::int(self.cr(c).hp as i64);
        self.set_current_hp_internal(c, cur + amount);
    }

    // ---- death -----------------------------------------------------------------------------------------------

    /// `CreatureCmd.Kill(creatures)`.
    pub fn kill(&mut self, victims: &[Cid]) {
        for &v in victims {
            self.kill_without_check(v);
        }
        if self.cr(PLAYER).is_dead() && self.in_progress {
            self.pending_loss = true;
        }
    }

    /// `KillWithoutCheckingWinCondition` (spec 02 §5.3), single-player path.
    fn kill_without_check(&mut self, c: Cid) {
        if !self.cr(c).in_combat && c != PLAYER {
            return;
        }
        let hp = self.cr(c).hp;
        if hp > 0 {
            self.lose_hp_internal(c, Dec::int(hp as i64));
        }
        // BeforeDeath(c) — no content yet. ShouldDie preventers (Fairy in a Bottle, Lizard Tail) — none yet.
        self.on_died(c);
    }

    fn on_died(&mut self, c: Cid) {
        let is_enemy = self.cr(c).side == Side::Enemy;
        // Hook.ShouldCreatureBeRemovedFromCombatAfterDeath is evaluated BEFORE AfterDeath (spec 02 §5.3).
        let should_remove_from_combat = self.should_creature_be_removed_after_death(c);
        let remove = is_enemy && !self.cr(c).is_pet && should_remove_from_combat;
        // AfterDeath(c, wasRemovalPrevented=false)
        self.dispatch_u(hookbit::after_death, |cx, me, l| l.after_death(cx, me, c));
        // teammates = alive creatures of the same side (snapshot before removal)
        let teammates: crate::util::ArrayVec<Cid, MAX_CREATURES> = {
            let mut o = crate::util::ArrayVec::new();
            if is_enemy {
                for &t in self.enemies.iter() {
                    if t != c && self.cr(t).is_alive() {
                        o.push(t);
                    }
                }
            }
            o
        };
        if remove && self.enemies.contains(c) {
            self.remove_creature(c);
        }
        let is_primary = self.is_primary_enemy(c);
        // RemoveAllPowersAfterDeath: powers stay iff `!ShouldPowerBeRemovedAfterOwnerDeath || !Hook.ShouldPowerBeRemovedOnDeath`.
        let mut removed = [Power::default(); MAX_POWERS];
        let mut n_removed = 0;
        let mut kept: crate::util::ArrayVec<Power, MAX_POWERS> = crate::util::ArrayVec::new();
        let all: crate::util::ArrayVec<Power, MAX_POWERS> = self.cr(c).powers;
        for p in all.iter() {
            let me = Me { kind: Kind::Power, owner: c, idx: p.uid, id: p.id, amount: p.amount };
            let keep = !content::listener(&me).should_power_be_removed_after_owner_death(self, me)
                || !self.should_power_be_removed_on_death(c, p);
            if keep {
                kept.push(*p);
            } else {
                removed[n_removed] = *p;
                n_removed += 1;
            }
        }
        self.cr_mut(c).powers = kept;
        for p in &removed[..n_removed] {
            let me = Me { kind: Kind::Power, owner: c, idx: p.uid, id: p.id, amount: p.amount };
            content::listener(&me).after_removed(self, me, c);
        }
        if is_enemy && is_primary && !teammates.is_empty() && teammates.iter().all(|&t| !self.is_primary_enemy(t)) {
            // the last primary enemy died: every remaining secondary teammate is killed (CreatureCmd.Kill)
            let mut v = [0u8; MAX_CREATURES];
            for (i, &t) in teammates.iter().enumerate() {
                v[i] = t;
            }
            self.kill(&v[..teammates.len()]);
        }
    }

    /// `Hook.ShouldPowerBeRemovedOnDeath(power)`: AND over every combat listener (unguarded).
    fn should_power_be_removed_on_death(&self, owner: Cid, p: &Power) -> bool {
        if !self.listen.has(hookbit::should_power_be_removed_on_death) {
            return true;
        }
        let is_debuff = content::power_def(p.id).ptype == PowerType::Debuff;
        let snap = self.snapshot(Mask::bit(hookbit::should_power_be_removed_on_death));
        for e in snap.iter() {
            if self.still_live(&e.me) && !content::listener(&e.me).should_power_be_removed_on_death(self, e.me, owner, p.id, is_debuff) {
                return false;
            }
        }
        true
    }

    /// `Hook.ShouldCreatureBeRemovedFromCombatAfterDeath`: AND over every combat listener (unguarded).
    pub fn should_creature_be_removed_after_death(&self, c: Cid) -> bool {
        if !self.listen.has(hookbit::should_creature_be_removed_from_combat_after_death) {
            return true;
        }
        let snap = self.snapshot(Mask::bit(hookbit::should_creature_be_removed_from_combat_after_death));
        for e in snap.iter() {
            if self.still_live(&e.me) && !content::listener(&e.me).should_creature_be_removed_from_combat_after_death(self, e.me, c) {
                return false;
            }
        }
        true
    }

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
