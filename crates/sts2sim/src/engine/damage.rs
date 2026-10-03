//! Damage pipeline (spec 02 §3): `Hook.ModifyDamage` → `CreatureCmd.Damage` → `AttackCommand`.

use super::creature::DamageResult;
use crate::content;
use crate::engine::HKind;
use crate::dec::Dec;
use crate::defs::VarKind;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

pub type Mods = ArrayVec<Me, 24>;
// 64: multi-hit attacks keep one result per hit, two when Osty absorbs a hit (DieForYou redirect).
pub type Results = ArrayVec<DamageResult, 64>;

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
    /// `CalculatedDamageVar` multiplier (`WithMultiplier`), evaluated per hit as `CalcBase + ExtraDamage * f(card, singleTarget)`;
    /// the target is `NO` when the hit has several targets. When set, `damage` is ignored.
    pub calc_mult: Option<fn(&Combat, CardIdx, Cid) -> i32>,
}

impl Attack {
    /// Monster move attack: `DamageCmd.Attack(n).FromMonster(m)` targeting the player(s).
    pub fn from_monster(dealer: Cid, damage: i32) -> Attack {
        Attack { dealer, card: NO, damage: Dec::int(damage as i64), hits: 1, props: ValueProp::MOVE, targeting: Targeting::AllOpponents, calc_mult: None }
    }
    /// Card attack: `DamageCmd.Attack(n).FromCard(card, play).Targeting(t)`.
    pub fn from_card(dealer: Cid, card: CardIdx, damage: i32, targeting: Targeting) -> Attack {
        Attack { dealer, card, damage: Dec::int(damage as i64), hits: 1, props: ValueProp::MOVE, targeting, calc_mult: None }
    }
    /// Card attack with a `CalculatedDamageVar` (Ashen Strike, Body Slam, Bully, Perfected Strike, ...).
    pub fn from_card_calc(dealer: Cid, card: CardIdx, targeting: Targeting, mult: fn(&Combat, CardIdx, Cid) -> i32) -> Attack {
        Attack { dealer, card, damage: Dec::ZERO, hits: 1, props: ValueProp::MOVE, targeting, calc_mult: Some(mult) }
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

/// `CalculatedVar.Calculate`: `CalculationBase + ExtraDamage * multiplier` (the multiplier only while the combat is in progress).
pub fn calc_with(cx: &Combat, card: CardIdx, mult: i32) -> Dec {
    let m = if cx.in_progress { mult } else { 0 };
    Dec::int(cx.card_var(card, VarKind::CalcBase) as i64 + cx.card_var(card, VarKind::ExtraDamage) as i64 * m as i64)
}

impl Combat {
    /// `Hook.ModifyDamage` (spec 02 §3.2): additive pass, multiplicative pass, cap pass, floor at 0. No rounding.
    pub fn modify_damage(&self, target: Cid, dealer: Cid, amount: Dec, props: ValueProp, card: CardIdx) -> (Dec, Mods) {
        let mut mods = Mods::new();
        let v = self.modify_damage_into(target, dealer, amount, props, card, &mut mods);
        (v, mods)
    }

    /// `modify_damage` when the modifier list is not needed (previews).
    #[inline]
    pub fn modify_damage_value(&self, target: Cid, dealer: Cid, amount: Dec, props: ValueProp, card: CardIdx) -> Dec {
        let mut mods = Mods::new();
        self.modify_damage_into(target, dealer, amount, props, card, &mut mods)
    }

    /// [`Combat::modify_damage`] with the list of modifiers appended to a caller-owned `mods` (no 300-byte copy per call).
    pub fn modify_damage_into(&self, target: Cid, dealer: Cid, amount: Dec, props: ValueProp, card: CardIdx, mods: &mut Mods) -> Dec {
        let m = (Mask::bit(hookbit::modify_damage_additive))
            | (Mask::bit(hookbit::modify_damage_multiplicative))
            | (Mask::bit(hookbit::modify_damage_cap));
        let mut snap = crate::engine::Snapshot::new();
        self.snapshot_into(m, &mut snap);
        let mut v = amount;
        // card.Enchantment.EnchantDamageAdditive then ...Multiplicative (before every listener pass; each enchantment
        // checks the damage props itself).
        if card != NO && self.cards[card as usize].enchant != 0 {
            let me = self.enchantment_me(card);
            let l = content::listener(&me);
            v += l.enchant_damage_additive(self, me, v, props);
            v *= l.enchant_damage_multiplicative(self, me, v, props);
        }
        for e in snap.iter() {
            if self.has_hook(&e.me, hookbit::modify_damage_additive) && self.still_live(&e.me) {
                let q = DmgQ { target, dealer, card, props, amount: v };
                let d = content::listener(&e.me).modify_damage_additive(self, e.me, &q);
                v += d;
                if !d.is_zero() {
                    mods.push(e.me);
                }
            }
        }
        for e in snap.iter() {
            if self.has_hook(&e.me, hookbit::modify_damage_multiplicative) && self.still_live(&e.me) {
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
            if self.has_hook(&e.me, hookbit::modify_damage_cap) && self.still_live(&e.me) {
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
        v.max(Dec::ZERO)
    }

    /// `Hook.ModifyHpLost` for one phase pair (spec 02 §3.4). `after_osty=false` runs the BeforeOsty passes.
    fn modify_hp_lost(&self, target: Cid, amount: Dec, props: ValueProp, dealer: Cid, card: CardIdx, after_osty: bool, mods: &mut Mods) -> Dec {
        let (b1, b2) = if after_osty {
            (hookbit::modify_hp_lost_after_osty, hookbit::modify_hp_lost_after_osty_late)
        } else {
            (hookbit::modify_hp_lost_before_osty, hookbit::modify_hp_lost_before_osty_late)
        };
        let mut snap = crate::engine::Snapshot::new();
        self.snapshot_into((Mask::bit(b1)) | (Mask::bit(b2)), &mut snap);
        let mut v = amount;
        for bit in [b1, b2] {
            for e in snap.iter() {
                if self.has_hook(&e.me, bit) && self.still_live(&e.me) {
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
        v
    }

    fn after_modifying_hp_lost(&mut self, mods: &Mods, after_osty: bool) {
        if after_osty {
            self.dispatch_modifiers(false, hookbit::after_modifying_hp_lost_after_osty, mods, |cx, me, l| l.after_modifying_hp_lost_after_osty(cx, me));
        } else {
            self.dispatch_modifiers(false, hookbit::after_modifying_hp_lost_before_osty, mods, |cx, me, l| l.after_modifying_hp_lost_before_osty(cx, me));
        }
    }

    /// `Hook.ModifyUnblockedDamageTarget` (unguarded, threaded): DieForYou redirects the HP loss to Osty.
    fn modify_unblocked_damage_target(&self, target: Cid, amount: Dec, props: ValueProp, dealer: Cid) -> Cid {
        if !self.listen.has(hookbit::modify_unblocked_damage_target) {
            return target;
        }
        let mut snap = crate::engine::Snapshot::new();
        self.snapshot_into(Mask::bit(hookbit::modify_unblocked_damage_target), &mut snap);
        let mut t = target;
        for e in snap.iter() {
            if self.still_live(&e.me) {
                t = content::listener(&e.me).modify_unblocked_damage_target(self, e.me, t, amount, props, dealer);
            }
        }
        t
    }

    /// `CreatureCmd.Damage` (spec 02 §3.3).
    #[inline(always)]
    pub fn damage(&mut self, targets: &[Cid], amount: Dec, props: ValueProp, dealer: Cid, card: CardIdx) -> Results {
        let mut results = Results::new();
        self.damage_into(targets, amount, props, dealer, card, &mut results);
        results
    }

    /// [`Combat::damage`] writing into a caller-owned (empty) result list: the 1.3 KB list is not copied on return.
    pub fn damage_into(&mut self, targets: &[Cid], amount: Dec, props: ValueProp, dealer: Cid, card: CardIdx, results: &mut Results) {
        if dealer != NO && self.cr(dealer).is_dead() {
            return;
        }
        for &t in targets {
            if self.cr(t).is_dead() {
                continue;
            }
            let mut mods = Mods::new();
            let modified = self.modify_damage_into(t, dealer, amount, props, card, &mut mods);
            self.dispatch_modifiers(false, hookbit::after_modifying_damage_amount, &mods, |cx, me, l| l.after_modifying_damage_amount(cx, me, card));
            // (every listener call re-sets the side channel: a listener's own nested damage call overwrites it)
            self.dispatch_u(hookbit::before_damage_received, |cx, me, l| {
                cx.dmg_card = card;
                l.before_damage_received(cx, me, t, modified, props, dealer)
            });
            // Pet quirk: damage to Osty is absorbed by its owner's block.
            let block_owner = if self.cr(t).is_pet && self.cr(t).owner != NO { self.cr(t).owner } else { t };
            let blocked = self.damage_block_internal(block_owner, modified, props);
            mods.clear();
            let unblocked = self.modify_hp_lost(t, (modified - blocked).max(Dec::ZERO), props, dealer, card, false, &mut mods);
            self.after_modifying_hp_lost(&mods, false);
            let hp_target = self.modify_unblocked_damage_target(t, unblocked, props, dealer);
            mods.clear();
            let unblocked = self.modify_hp_lost(hp_target, unblocked, props, dealer, card, true, &mut mods);
            self.after_modifying_hp_lost(&mods, true);
            let mut res = self.lose_hp_internal(hp_target, unblocked);
            let block_left = self.cr(t).block;
            let was_block_broken = block_left <= 0 && blocked > Dec::ZERO;
            let was_fully_blocked = !props.unblockable() && (blocked > Dec::ZERO || block_left > 0) && unblocked.trunc() == 0;
            if hp_target == t {
                res.blocked = blocked.trunc();
                res.block_broken = was_block_broken;
                res.fully_blocked = was_fully_blocked;
                results.push(res);
                self.hist_damage_received(res, dealer, card, props);
            } else {
                // Redirected (Osty took the hit): the overkill is re-run through ModifyHpLost(AfterOsty) against the
                // original target.
                results.push(res);
                self.hist_damage_received(res, dealer, card, props);
                mods.clear();
                let over = self.modify_hp_lost(t, Dec::int(res.overkill as i64), props, dealer, card, true, &mut mods);
                self.after_modifying_hp_lost(&mods, true);
                let mut r2 = if over > Dec::ZERO { self.lose_hp_internal(t, over) } else { DamageResult { receiver: t, ..Default::default() } };
                r2.blocked = blocked.trunc();
                r2.block_broken = was_block_broken;
                r2.fully_blocked = was_fully_blocked;
                results.push(r2);
                self.hist_damage_received(r2, dealer, card, props);
            }
        }

        // ---- post-hooks run after ALL targets resolved ----
        let mut killed: ArrayVec<Cid, 32> = ArrayVec::new();
        for i in 0..results.len() {
            let r = results[i];
            let t = r.receiver;
            // (the side channel `dmg_card` / `dmg_result` is rewritten before each listener call: a nested damage call made
            // by a listener would overwrite it for the listeners after it)
            if r.block_broken && self.listen.has(hookbit::after_block_broken) {
                self.dispatch_u(hookbit::after_block_broken, |cx, me, l| {
                    cx.dmg_card = card;
                    cx.dmg_result = r;
                    l.after_block_broken(cx, me, t, dealer)
                });
            }
            if r.unblocked > 0 && self.listen.has(hookbit::after_current_hp_changed) {
                let d = -r.unblocked;
                self.dispatch_u(hookbit::after_current_hp_changed, |cx, me, l| l.after_current_hp_changed(cx, me, t, d));
            }
            if self.listen.has(hookbit::after_damage_given) {
                self.dispatch_u(hookbit::after_damage_given, |cx, me, l| {
                    cx.dmg_card = card;
                    cx.dmg_result = r;
                    l.after_damage_given(cx, me, dealer, t, r.unblocked, props)
                });
            }
            if !r.killed || !self.cr(t).is_dead() {
                if self.listen.has(hookbit::after_damage_received) {
                    self.dispatch_u(hookbit::after_damage_received, |cx, me, l| {
                        cx.dmg_card = card;
                        cx.dmg_result = r;
                        l.after_damage_received(cx, me, t, r.unblocked, props, dealer)
                    });
                }
                if self.listen.has(hookbit::after_damage_received_late) {
                    self.dispatch_u(hookbit::after_damage_received_late, |cx, me, l| {
                        cx.dmg_card = card;
                        cx.dmg_result = r;
                        l.after_damage_received_late(cx, me, t, r.unblocked, props, dealer)
                    });
                }
            } else {
                killed.push(t);
            }
        }
        if !killed.is_empty() {
            let mut v = [0u8; 32];
            let n = killed.len();
            v[..n].copy_from_slice(killed.as_slice());
            self.kill(&v[..n]);
        }
    }

    /// `History.DamageReceived` — only while the combat is live (in progress and not ending).
    fn hist_damage_received(&mut self, r: DamageResult, dealer: Cid, card: CardIdx, props: ValueProp) {
        if self.in_progress && !self.is_ending() {
            let flags = r.fully_blocked as u8 | (r.block_broken as u8) << 1 | (r.killed as u8) << 2;
            self.hist_push(HKind::DamageReceived, r.receiver, dealer, 0, card, r.unblocked, flags, props.0, 0);
            if r.receiver == PLAYER && r.unblocked > 0 {
                crate::engine::history::bump(&mut self.hist_log.player_hits_taken);
            }
        }
    }

    /// `Hook.ModifyAttackHitCount` (guarded, threaded int).
    fn modify_attack_hit_count(&self, a: &Attack) -> i32 {
        let mut hits = a.hits;
        if self.listen.has(hookbit::modify_attack_hit_count) && self.hooks_enabled() {
            let mut snap = crate::engine::Snapshot::new();
            self.snapshot_into(Mask::bit(hookbit::modify_attack_hit_count), &mut snap);
            for e in snap.iter() {
                if self.still_live(&e.me) {
                    hits = content::listener(&e.me).modify_attack_hit_count(self, e.me, a, hits);
                }
            }
        }
        hits
    }

    /// `Hook.AfterAttack` for an attack whose per-hit results are `all`: publishes them in `attack_results` (what
    /// `AttackCommand.Results` gives the listeners) and dispatches. Also the `AttackContext` disposal of cards that drive
    /// their own context (Echoing Slash, Omnislice), which skips the `CreatureAttacked` history entry.
    pub fn dispatch_after_attack(&mut self, a: &Attack, all: &Results) {
        if self.listen.has(hookbit::after_attack) {
            self.attack_results.clear();
            for x in all.iter().take(16) {
                self.attack_results.push(*x);
            }
            self.attack_unblocked_hits = all.iter().filter(|r| r.unblocked > 0).count() as u8;
            self.attack_player_hits = all.iter().filter(|r| r.unblocked > 0 && r.receiver == PLAYER).count() as u8;
        }
        self.dispatch_g(hookbit::after_attack, |cx, me, l| l.after_attack(cx, me, a));
    }

    /// `AttackCommand.Execute` (spec 02 §3.1).
    #[inline(always)]
    pub fn execute_attack(&mut self, a: &Attack) -> Results {
        let mut all = Results::new();
        self.execute_attack_into(a, &mut all);
        all
    }

    /// [`Combat::execute_attack`] writing into a caller-owned (empty) result list (most callers ignore the results: no copy).
    pub fn execute_attack_into(&mut self, a: &Attack, all: &mut Results) {
        if self.is_over_or_ending() || a.dealer == NO || self.cr(a.dealer).is_dead() {
            return;
        }
        self.dispatch_g(hookbit::before_attack, |cx, me, l| l.before_attack(cx, me, a));
        let hits = self.modify_attack_hit_count(a);
        let dealer_side = self.cr(a.dealer).side;
        let mut hit_sizes: ArrayVec<u8, 16> = ArrayVec::new();
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
            let amount = match a.calc_mult {
                None => a.damage,
                Some(f) => {
                    // `Calculate(singleTarget)`: base + extra * multiplier (0 when combat is not in progress).
                    let single = if hit.len() == 1 { hit[0] } else { NO };
                    let m = if self.in_progress { f(self, a.card, single) } else { 0 };
                    Dec::int(self.card_var(a.card, VarKind::CalcBase) as i64 + self.card_var(a.card, VarKind::ExtraDamage) as i64 * m as i64)
                }
            };
            let mut r = Results::new();
            self.damage_into(hit.as_slice(), amount, a.props, a.dealer, a.card, &mut r);
            for x in r.iter() {
                let mut x = *x;
                x.hit = i.min(255) as u8;
                all.push(x);
            }
            if hit_sizes.len() < 16 {
                hit_sizes.push(r.len() as u8);
            }
            i += 1;
        }
        self.hist_push(HKind::CreatureAttacked, a.dealer, NO, 0, a.card, all.len() as i32, 0, a.props.0, 0);
        if self.listen.has(hookbit::after_attack) {
            self.set_attack_results(all.as_slice(), hit_sizes.as_slice());
        }
        self.dispatch_g(hookbit::after_attack, |cx, me, l| l.after_attack(cx, me, a));
    }

    /// Fills the `AfterAttack` side channel (C# `command.Results`): `all` = the results of every hit in order, `sizes` =
    /// how many of them belong to each hit. Cards that drive an `AttackContext` by hand (Omnislice, Echoing Slash) call
    /// this right before dispatching `after_attack` themselves.
    pub fn set_attack_results(&mut self, all: &[DamageResult], sizes: &[u8]) {
        self.attack_results.clear();
        // Only the first 16 results / hits are kept (a Tear Asunder with 50 hits against one target: the listeners that read the list
        // look for the receiver or count; the counters below always cover the whole attack). Not an overflow.
        for x in all.iter().take(16) {
            self.attack_results.push(*x);
        }
        self.attack_hit_sizes.clear();
        for &s in sizes.iter().take(16) {
            self.attack_hit_sizes.push(s);
        }
        self.attack_unblocked_hits = all.iter().filter(|r| r.unblocked > 0).count() as u8;
        self.attack_player_hits = all.iter().filter(|r| r.unblocked > 0 && r.receiver == PLAYER).count() as u8;
    }
}
