//! Damage pipeline (spec 02 §3): `Hook.ModifyDamage` → `CreatureCmd.Damage` → `AttackCommand`.

use super::creature::DamageResult;
use crate::content;
use crate::dec::Dec;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

pub type Mods = ArrayVec<Me, 24>;
pub type Results = ArrayVec<DamageResult, 16>;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Targeting {
    Single(Cid),
    AllOpponents,
    /// Random opponent per hit (`Rng.CombatTargets`).
    Random,
}

/// `DamageCmd.Attack(...)` builder state.
#[derive(Clone, Copy, Debug)]
pub struct Attack {
    pub dealer: Cid,
    pub card: CardIdx,
    pub damage: Dec,
    pub hits: i32,
    pub props: ValueProp,
    pub targeting: Targeting,
}

impl Attack {
    /// Monster move attack: `DamageCmd.Attack(n).FromMonster(m)` targeting the player(s).
    pub fn from_monster(dealer: Cid, damage: i32) -> Attack {
        Attack { dealer, card: NO, damage: Dec::int(damage as i64), hits: 1, props: ValueProp::MOVE, targeting: Targeting::AllOpponents }
    }
    /// Card attack: `DamageCmd.Attack(n).FromCard(card, play).Targeting(t)`.
    pub fn from_card(dealer: Cid, card: CardIdx, damage: i32, targeting: Targeting) -> Attack {
        Attack { dealer, card, damage: Dec::int(damage as i64), hits: 1, props: ValueProp::MOVE, targeting }
    }
    pub fn hits(mut self, n: i32) -> Attack {
        self.hits = n;
        self
    }
    pub fn unpowered(mut self) -> Attack {
        self.props = self.props.or(ValueProp::UNPOWERED);
        self
    }
    pub fn targeting(mut self, t: Targeting) -> Attack {
        self.targeting = t;
        self
    }
}

impl Combat {
    /// `Hook.ModifyDamage` (spec 02 §3.2): additive pass, multiplicative pass, cap pass, floor at 0. No rounding.
    pub fn modify_damage(&self, target: Cid, dealer: Cid, amount: Dec, props: ValueProp, card: CardIdx) -> (Dec, Mods) {
        let m = (Mask::bit(hookbit::modify_damage_additive))
            | (Mask::bit(hookbit::modify_damage_multiplicative))
            | (Mask::bit(hookbit::modify_damage_cap));
        let snap = self.snapshot(m);
        let mut mods = Mods::new();
        let mut v = amount;
        // (card enchantment: EnchantDamageAdditive / Multiplicative — not implemented yet)
        for e in snap.iter() {
            if e.mask.has(hookbit::modify_damage_additive) && self.still_live(&e.me) {
                let q = DmgQ { target, dealer, card, props, amount: v };
                let d = content::listener(&e.me).modify_damage_additive(self, e.me, &q);
                v += d;
                if !d.is_zero() {
                    mods.push(e.me);
                }
            }
        }
        for e in snap.iter() {
            if e.mask.has(hookbit::modify_damage_multiplicative) && self.still_live(&e.me) {
                let q = DmgQ { target, dealer, card, props, amount: v };
                let f = content::listener(&e.me).modify_damage_multiplicative(self, e.me, &q);
                v *= f;
                if f != Dec::ONE {
                    mods.push(e.me);
                }
            }
        }
        let mut best = Dec::MAX;
        for e in snap.iter() {
            if e.mask.has(hookbit::modify_damage_cap) && self.still_live(&e.me) {
                let q = DmgQ { target, dealer, card, props, amount: v };
                let c = content::listener(&e.me).modify_damage_cap(self, e.me, &q);
                if c < best {
                    best = c;
                    if v > c {
                        v = c;
                        mods.push(e.me);
                    }
                }
            }
        }
        (v.max(Dec::ZERO), mods)
    }

    /// `Hook.ModifyHpLost` for one phase pair (spec 02 §3.4). `after_osty=false` runs the BeforeOsty passes.
    fn modify_hp_lost(&self, target: Cid, amount: Dec, props: ValueProp, dealer: Cid, card: CardIdx, after_osty: bool) -> (Dec, Mods) {
        let (b1, b2) = if after_osty {
            (hookbit::modify_hp_lost_after_osty, hookbit::modify_hp_lost_after_osty_late)
        } else {
            (hookbit::modify_hp_lost_before_osty, hookbit::modify_hp_lost_before_osty_late)
        };
        let snap = self.snapshot((Mask::bit(b1)) | (Mask::bit(b2)));
        let mut v = amount;
        let mut mods = Mods::new();
        for bit in [b1, b2] {
            for e in snap.iter() {
                if e.mask.has(bit) && self.still_live(&e.me) {
                    let l = content::listener(&e.me);
                    let nv = match bit {
                        x if x == hookbit::modify_hp_lost_before_osty => l.modify_hp_lost_before_osty(self, e.me, target, v, props, dealer, card),
                        x if x == hookbit::modify_hp_lost_before_osty_late => l.modify_hp_lost_before_osty_late(self, e.me, target, v, props, dealer, card),
                        x if x == hookbit::modify_hp_lost_after_osty => l.modify_hp_lost_after_osty(self, e.me, target, v, props, dealer, card),
                        _ => l.modify_hp_lost_after_osty_late(self, e.me, target, v, props, dealer, card),
                    };
                    if v.truncate() != nv.truncate() {
                        mods.push(e.me);
                    }
                    v = nv;
                }
            }
        }
        (v, mods)
    }

    fn after_modifying_hp_lost(&mut self, mods: &Mods, after_osty: bool) {
        for me in mods.iter() {
            if self.still_live(me) {
                let l = content::listener(me);
                if after_osty {
                    l.after_modifying_hp_lost_after_osty(self, *me);
                } else {
                    l.after_modifying_hp_lost_before_osty(self, *me);
                }
            }
        }
    }

    /// `CreatureCmd.Damage` (spec 02 §3.3).
    pub fn damage(&mut self, targets: &[Cid], amount: Dec, props: ValueProp, dealer: Cid, card: CardIdx) -> Results {
        let mut results = Results::new();
        if dealer != NO && self.cr(dealer).is_dead() {
            return results;
        }
        for &t in targets {
            if self.cr(t).is_dead() {
                continue;
            }
            let (modified, mods) = self.modify_damage(t, dealer, amount, props, card);
            for me in mods.iter() {
                if self.still_live(me) {
                    content::listener(me).after_modifying_damage_amount(self, *me, card);
                }
            }
            self.dispatch_u(hookbit::before_damage_received, |cx, me, l| l.before_damage_received(cx, me, t, modified, props, dealer));
            // Pet quirk: damage to Osty is absorbed by its owner's block.
            let block_owner = if self.cr(t).is_pet && self.cr(t).owner != NO { self.cr(t).owner } else { t };
            let blocked = self.damage_block_internal(block_owner, modified, props);
            let (unblocked, mods) = self.modify_hp_lost(t, (modified - blocked).max(Dec::ZERO), props, dealer, card, false);
            self.after_modifying_hp_lost(&mods, false);
            // (ModifyUnblockedDamageTarget — Osty's DieForYou redirect — not implemented yet.)
            let hp_target = t;
            let (unblocked, mods) = self.modify_hp_lost(hp_target, unblocked, props, dealer, card, true);
            self.after_modifying_hp_lost(&mods, true);
            let mut res = self.lose_hp_internal(hp_target, unblocked);
            let block_left = self.cr(block_owner).block;
            res.block_broken = block_left <= 0 && blocked > Dec::ZERO;
            res.fully_blocked = !props.unblockable() && (blocked > Dec::ZERO || block_left > 0) && unblocked.trunc() == 0;
            res.blocked = blocked.trunc();
            results.push(res);
        }

        // ---- post-hooks run after ALL targets resolved ----
        let mut killed: ArrayVec<Cid, 16> = ArrayVec::new();
        for i in 0..results.len() {
            let r = results[i];
            let t = r.receiver;
            if t == PLAYER && !r.fully_blocked && self.in_progress && !self.is_ending() {
                self.player.damaged_turn = self.player.turn_number; // History.DamageReceived (Emotion Chip: `!WasFullyBlocked`)
            }
            if r.block_broken {
                self.dispatch_u(hookbit::after_block_broken, |cx, me, l| l.after_block_broken(cx, me, t, dealer));
            }
            if r.unblocked > 0 {
                let d = -r.unblocked;
                self.dispatch_u(hookbit::after_current_hp_changed, |cx, me, l| l.after_current_hp_changed(cx, me, t, d));
            }
            self.dispatch_u(hookbit::after_damage_given, |cx, me, l| l.after_damage_given(cx, me, dealer, t, r.unblocked, props));
            if !r.killed || !self.cr(t).is_dead() {
                self.dispatch_u(hookbit::after_damage_received, |cx, me, l| l.after_damage_received(cx, me, t, r.unblocked, props, dealer));
            } else {
                killed.push(t);
            }
        }
        if !killed.is_empty() {
            let mut v = [0u8; 16];
            let n = killed.len();
            v[..n].copy_from_slice(killed.as_slice());
            self.kill(&v[..n]);
        }
        results
    }

    /// `AttackCommand.Execute` (spec 02 §3.1).
    pub fn execute_attack(&mut self, a: &Attack) -> Results {
        let mut all = Results::new();
        if self.is_over_or_ending() || a.dealer == NO || self.cr(a.dealer).is_dead() {
            return all;
        }
        self.dispatch_g(hookbit::before_attack, |cx, me, l| l.before_attack(cx, me, a));
        let hits = a.hits; // ModifyAttackHitCount: no content overrides it.
        let dealer_side = self.cr(a.dealer).side;
        let mut i = 0;
        while i < hits {
            if self.cr(a.dealer).is_dead() {
                break;
            }
            // possibleTargets, recomputed every hit; IsAlive (not IsHittable).
            let mut valid: ArrayVec<Cid, MAX_CREATURES> = ArrayVec::new();
            match a.targeting {
                Targeting::Single(t) => {
                    if self.cr(t).in_combat && self.cr(t).is_alive() {
                        valid.push(t);
                    }
                }
                Targeting::AllOpponents | Targeting::Random => {
                    let pool: &[Cid] = if dealer_side == Side::Player { self.enemies.as_slice() } else { &[PLAYER] };
                    for &c in pool {
                        if self.cr(c).is_alive() {
                            valid.push(c);
                        }
                    }
                }
            }
            if valid.is_empty() {
                break;
            }
            let hit: ArrayVec<Cid, MAX_CREATURES> = if a.targeting == Targeting::Random {
                let k = self.rng.combat_targets.next_int_range(0, valid.len() as i32) as usize;
                let mut one = ArrayVec::new();
                one.push(valid[k]);
                one
            } else {
                valid
            };
            let r = self.damage(hit.as_slice(), a.damage, a.props, a.dealer, a.card);
            for x in r.iter() {
                all.push(*x);
            }
            i += 1;
        }
        self.dispatch_g(hookbit::after_attack, |cx, me, l| l.after_attack(cx, me, a));
        all
    }
}
