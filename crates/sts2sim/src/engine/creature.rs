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
        let remove = is_enemy && !self.cr(c).is_pet;
        // AfterDeath(c, wasRemovalPrevented=false)
        self.dispatch_u(hookbit::after_death, |cx, me, l| l.after_death(cx, me, c));
        if remove && self.enemies.contains(c) {
            self.remove_creature(c);
        }
        // RemoveAllPowersAfterDeath: strip powers (AfterRemoved callbacks, no amount hooks).
        let mut removed = [Power::default(); MAX_POWERS];
        let n = self.cr(c).powers.len();
        removed[..n].copy_from_slice(self.cr(c).powers.as_slice());
        self.cr_mut(c).powers.clear();
        for p in &removed[..n] {
            let me = Me { kind: Kind::Power, owner: c, idx: p.uid, id: p.id, amount: p.amount };
            content::listener(&me).after_removed(self, me, c);
        }
        if c == PLAYER {
            self.creatures[PLAYER as usize].block = self.creatures[PLAYER as usize].block;
            // CreatureCmd.Kill (player branch): OrbQueue.Clear() — orbs gone and capacity zeroed.
            self.player.orbs.clear();
            self.player.orb_slots = 0;
        }
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
