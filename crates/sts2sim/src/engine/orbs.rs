//! Defect orb subsystem (spec 05 §5): `OrbQueue`, `OrbCmd`, the five orbs' passive/evoke effects.
//!
//! Orbs live in `PlayerState::orbs` (front = next to evoke) with a slot capacity. They carry identity (`Orb::uid`) because the
//! game works by object reference (`Remove(orb)`, `orb == Orbs[0]`, TeslaCoil's snapshot of Lightning orbs).
//! Orb values go through `Hook.ModifyOrbValue` (guarded iterator: once the combat is over/ending nothing modifies them).

use crate::content;
use crate::dec::Dec;
use crate::engine::HKind;
use crate::hooks::*;
use crate::ids;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

pub const MAX_ORB_SLOTS: i32 = 10;

/// `OrbModel._validOrbs` order (NOT the id order): Lightning, Frost, Dark, Plasma, Glass.
pub const VALID_ORBS: [u16; 5] = [ids::orb::LIGHTNING_ORB, ids::orb::FROST_ORB, ids::orb::DARK_ORB, ids::orb::PLASMA_ORB, ids::orb::GLASS_ORB];

type Targets = ArrayVec<Cid, MAX_CREATURES>;

impl Combat {
    // ---- values ----------------------------------------------------------------------------------------------------

    /// `Hook.ModifyOrbValue(orb, v)`: threaded fold over the guarded listener list.
    pub fn modify_orb_value(&self, orb: &Orb, v: Dec) -> Dec {
        let mut v = v;
        if self.hooks_enabled() {
            let snap = self.snapshot(Mask::bit(hookbit::modify_orb_value));
            for e in snap.iter() {
                if self.still_live(&e.me) {
                    v = content::listener(&e.me).modify_orb_value(self, e.me, orb, v);
                }
            }
        }
        v
    }

    /// `OrbModel.PassiveVal`.
    pub fn orb_passive_val(&self, orb: &Orb) -> Dec {
        match orb.kind {
            ids::orb::LIGHTNING_ORB => self.modify_orb_value(orb, Dec::int(3)),
            ids::orb::FROST_ORB => self.modify_orb_value(orb, Dec::int(2)),
            ids::orb::DARK_ORB => self.modify_orb_value(orb, Dec::int(6)),
            ids::orb::GLASS_ORB => self.modify_orb_value(orb, Dec::int(orb.val as i64)),
            _ => Dec::ONE, // Plasma
        }
    }

    /// `OrbModel.EvokeVal`.
    pub fn orb_evoke_val(&self, orb: &Orb) -> Dec {
        match orb.kind {
            ids::orb::LIGHTNING_ORB => self.modify_orb_value(orb, Dec::int(8)),
            ids::orb::FROST_ORB => self.modify_orb_value(orb, Dec::int(5)),
            ids::orb::DARK_ORB => Dec::int(orb.val as i64),
            ids::orb::GLASS_ORB => self.orb_passive_val(orb) * Dec::int(2),
            _ => Dec::int(2), // Plasma
        }
    }

    /// A fresh mutable orb (`ModelDb.Orb<T>().ToMutable()`).
    pub fn new_orb(&mut self, kind: u16) -> Orb {
        let uid = self.player.next_orb_uid;
        self.player.next_orb_uid = uid.wrapping_add(1);
        let val = match kind {
            ids::orb::DARK_ORB => 6,
            ids::orb::GLASS_ORB => 4,
            _ => 0,
        };
        Orb { kind, uid, val }
    }

    /// Number of distinct orb types in the queue (`group by orb.Id`).
    pub fn distinct_orb_types(&self) -> i32 {
        let mut seen = 0u32;
        for o in self.player.orbs.iter() {
            seen |= 1 << o.kind;
        }
        seen.count_ones() as i32
    }

    fn orb_index(&self, uid: u16) -> Option<usize> {
        self.player.orbs.iter().position(|o| o.uid == uid)
    }

    /// The live version of an orb (its state may have changed since the caller copied it).
    fn live_orb(&self, orb: &Orb) -> Orb {
        self.orb_index(orb.uid).map_or(*orb, |i| self.player.orbs[i])
    }

    fn set_orb_val(&mut self, uid: u16, val: i32) {
        if let Some(i) = self.orb_index(uid) {
            self.player.orbs[i].val = val;
        }
    }

    // ---- slots -----------------------------------------------------------------------------------------------------

    /// `OrbCmd.AddSlots`.
    pub fn add_orb_slots(&mut self, n: i32) {
        if self.is_over_or_ending() {
            return;
        }
        let n = n.min(MAX_ORB_SLOTS - self.player.orb_slots as i32);
        self.player.orb_slots = (self.player.orb_slots as i32 + n).max(0) as u8;
    }

    /// `OrbCmd.RemoveSlots`: slots go from the back; orbs beyond the capacity are dropped silently (no evoke, no hook).
    pub fn remove_orb_slots(&mut self, n: i32) {
        if self.is_over_or_ending() {
            return;
        }
        let n = n.min(self.player.orb_slots as i32);
        let cap = (self.player.orb_slots as i32 - n).max(0);
        self.player.orb_slots = cap as u8;
        while self.player.orbs.len() as i32 > cap {
            let last = self.player.orbs.len() - 1;
            self.player.orbs.remove(last);
        }
    }

    /// `Player.Character.BaseOrbSlotCount` (a character constant, unlike the scenario's `BaseOrbSlotCount`).
    fn character_base_orb_slots(&self) -> i32 {
        if self.character == 2 { 3 } else { 0 }
    }

    // ---- channel / evoke -------------------------------------------------------------------------------------------

    /// `OrbCmd.Channel<T>`.
    pub fn channel_orb(&mut self, kind: u16) {
        let orb = self.new_orb(kind);
        self.channel(orb);
    }

    /// `OrbCmd.Channel(orb)`.
    pub fn channel(&mut self, orb: Orb) {
        if self.is_over_or_ending() {
            return;
        }
        if self.character_base_orb_slots() == 0 && self.player.orb_slots == 0 {
            self.add_orb_slots(1);
        }
        if self.player.orbs.len() >= self.player.orb_slots as usize {
            self.evoke_next(true);
        }
        // OrbQueue.TryEnqueue: false iff capacity == 0 (the orb is lost); "full" can only happen through re-entrancy.
        if self.player.orb_slots == 0 || self.player.orbs.len() >= self.player.orb_slots as usize {
            return;
        }
        self.player.orbs.push(orb);
        self.hist_push(HKind::OrbChanneled, PLAYER, NO, orb.kind, NO, 0, 0, 0, 0); // CombatHistory.OrbChanneled
        if orb.kind == ids::orb::LIGHTNING_ORB {
            self.hist_log.lightning_channeled = self.hist_log.lightning_channeled.saturating_add(1);
        }
        self.dispatch_g(hookbit::after_orb_channeled, |cx, me, l| l.after_orb_channeled(cx, me, &orb));
    }

    /// `OrbCmd.EvokeNext`.
    pub fn evoke_next(&mut self, dequeue: bool) {
        if let Some(orb) = self.player.orbs.first() {
            self.evoke_orb(orb, dequeue);
        }
    }

    /// `OrbCmd.EvokeLast`.
    pub fn evoke_last(&mut self, dequeue: bool) {
        if let Some(orb) = self.player.orbs.last() {
            self.evoke_orb(orb, dequeue);
        }
    }

    /// `OrbCmd.Evoke` (private in the game).
    fn evoke_orb(&mut self, orb: Orb, dequeue: bool) {
        if self.is_over_or_ending() || self.player.orbs.is_empty() {
            return;
        }
        if dequeue {
            if let Some(i) = self.orb_index(orb.uid) {
                self.player.orbs.remove(i); // removal happens BEFORE the effect
            }
        }
        let targets = self.orb_evoke_effect(&orb);
        if self.cr(PLAYER).in_combat {
            self.dispatch_g(hookbit::after_orb_evoked, |cx, me, l| l.after_orb_evoked(cx, me, &orb, targets.as_slice()));
        }
    }

    /// `OrbModel.Evoke`: the effect; returns the creatures the orb reports as targets.
    fn orb_evoke_effect(&mut self, orb: &Orb) -> Targets {
        let mut out = Targets::new();
        match orb.kind {
            ids::orb::LIGHTNING_ORB => {
                let v = self.orb_evoke_val(orb);
                return self.lightning_damage(v, NO);
            }
            ids::orb::FROST_ORB => {
                let v = self.orb_evoke_val(orb);
                self.gain_block(PLAYER, v, ValueProp::UNPOWERED, NO);
                out.push(PLAYER);
            }
            ids::orb::DARK_ORB => {
                let hittable = self.hittable_enemies();
                if hittable.is_empty() {
                    return out;
                }
                // MinBy(CurrentHp): first minimal in list order.
                let mut weakest = hittable[0];
                for &e in hittable.iter() {
                    if self.cr(e).hp < self.cr(weakest).hp {
                        weakest = e;
                    }
                }
                let v = self.orb_evoke_val(orb);
                self.damage(&[weakest], v, ValueProp::UNPOWERED, PLAYER, NO);
                out.push(weakest);
            }
            ids::orb::PLASMA_ORB => {
                let v = self.orb_evoke_val(orb);
                self.gain_energy(v.trunc());
                out.push(PLAYER);
            }
            _ => {
                // Glass
                let enemies = self.hittable_enemies();
                let v = self.orb_evoke_val(orb);
                if v <= Dec::ZERO {
                    return out;
                }
                self.damage(enemies.as_slice(), v, ValueProp::UNPOWERED, PLAYER, NO);
                return enemies;
            }
        }
        out
    }

    /// `LightningOrb.ApplyLightningDamage`: one random hittable opponent (one `combat_targets` draw even for a single
    /// enemy) unless `target` is given.
    fn lightning_damage(&mut self, value: Dec, target: Cid) -> Targets {
        let mut out = Targets::new();
        let opps = self.hittable_enemies();
        if opps.is_empty() {
            return out;
        }
        let t = if target == NO { opps[self.rng.combat_targets.next_int_range(0, opps.len() as i32) as usize] } else { target };
        self.damage(&[t], value, ValueProp::UNPOWERED, PLAYER, NO);
        out.push(t);
        out
    }

    // ---- passives --------------------------------------------------------------------------------------------------

    /// `OrbCmd.Passive(orb, target, countAffectedByHooks)`.
    pub fn orb_passive(&mut self, orb: Orb, target: Cid, count_affected_by_hooks: bool) {
        if self.is_over_or_ending() {
            return;
        }
        if count_affected_by_hooks {
            self.trigger_orb_passive(orb, target);
        } else {
            self.orb_passive_once(orb, target);
        }
    }

    /// `OrbModel.TriggerPassive`: trigger count through `ModifyOrbPassiveTriggerCount`, then that many passives.
    pub fn trigger_orb_passive(&mut self, orb: Orb, target: Cid) {
        let mut count = 1;
        let mut mods: ArrayVec<Me, 24> = ArrayVec::new();
        if self.hooks_enabled() {
            let snap = self.snapshot(Mask::bit(hookbit::modify_orb_passive_trigger_counts));
            for e in snap.iter() {
                if self.still_live(&e.me) {
                    let n = content::listener(&e.me).modify_orb_passive_trigger_counts(self, e.me, &orb, count);
                    if n != count {
                        mods.push(e.me);
                    }
                    count = n;
                }
            }
        }
        if !mods.is_empty() && self.hooks_enabled() {
            let snap = self.snapshot(Mask::bit(hookbit::after_modifying_orb_passive_trigger_count));
            for e in snap.iter() {
                if self.still_live(&e.me) && mods.iter().any(|m| m.kind == e.me.kind && m.idx == e.me.idx && m.owner == e.me.owner && m.id == e.me.id) {
                    content::listener(&e.me).after_modifying_orb_passive_trigger_count(self, e.me, &orb);
                }
            }
        }
        for _ in 0..count {
            self.orb_passive_once(orb, target);
        }
    }

    /// `OrbModel.Passive` (skips the trigger-count hooks).
    fn orb_passive_once(&mut self, orb: Orb, target: Cid) {
        let orb = self.live_orb(&orb);
        match orb.kind {
            ids::orb::LIGHTNING_ORB => {
                let v = self.orb_passive_val(&orb);
                self.lightning_damage(v, target);
            }
            ids::orb::FROST_ORB => {
                let v = self.orb_passive_val(&orb);
                self.gain_block(PLAYER, v, ValueProp::UNPOWERED, NO);
            }
            ids::orb::DARK_ORB => {
                let v = self.orb_passive_val(&orb);
                self.set_orb_val(orb.uid, orb.val + v.trunc());
            }
            ids::orb::PLASMA_ORB => {
                let v = self.orb_passive_val(&orb);
                self.gain_energy(v.trunc());
            }
            _ => {
                // Glass: targets are computed before the value; the base decrements before the damage is dealt.
                let targets = self.hittable_enemies();
                let v = self.orb_passive_val(&orb);
                if v > Dec::ZERO {
                    self.set_orb_val(orb.uid, (orb.val - 1).max(0));
                    self.damage(targets.as_slice(), v, ValueProp::UNPOWERED, PLAYER, NO);
                }
            }
        }
    }

    // ---- turn triggers ---------------------------------------------------------------------------------------------

    /// `OrbQueue.AfterTurnStart`: Plasma triggers (front to back over a snapshot of the queue).
    pub fn orbs_after_turn_start(&mut self) {
        let snapshot = self.player.orbs;
        for o in snapshot.iter() {
            if !self.cr(PLAYER).in_combat {
                return;
            }
            if o.kind == ids::orb::PLASMA_ORB {
                self.trigger_orb_passive(*o, NO);
            }
        }
    }

    /// `OrbQueue.BeforeTurnEnd`: Lightning / Frost / Dark / Glass trigger (front to back over a snapshot).
    pub fn orbs_before_turn_end(&mut self) {
        let snapshot = self.player.orbs;
        for o in snapshot.iter() {
            if !self.cr(PLAYER).in_combat {
                return;
            }
            if o.kind != ids::orb::PLASMA_ORB {
                self.trigger_orb_passive(*o, NO);
            }
        }
    }
}

// ---- small helpers shared by the Defect cards -----------------------------------------------------------------------
impl Combat {
    #[inline]
    pub fn orb_count(&self) -> i32 {
        self.player.orbs.len() as i32
    }

    /// `combatState.CreateCard<T>(owner)` + `CardPileCmd.AddGeneratedCardToCombat(card, pile, Owner)`.
    pub fn create_card_for_player(&mut self, id: u16, upgrade: u8, pile: PileType, pos: CardPilePosition) -> Option<CardIdx> {
        let c = self.new_card(id, upgrade)?;
        self.add_generated_card(c, pile, pos);
        Some(c)
    }

    /// Cards in the combat piles in `PlayerCombatState.AllCards` order (hand, draw, discard, exhaust, play).
    pub fn combat_cards_in_pile_order(&self) -> ArrayVec<CardIdx, MAX_CARDS> {
        let mut v = ArrayVec::new();
        for pile in [&self.player.hand, &self.player.draw, &self.player.discard, &self.player.exhaust, &self.player.play] {
            for &c in pile.iter() {
                v.push(c);
            }
        }
        v
    }

    /// `Monster.IntendsToAttack`: the monster's pending move has an attack intent.
    pub fn intends_to_attack(&self, e: Cid) -> bool {
        use crate::defs::Intent;
        // (`move_view` also covers the synthetic STUNNED move, whose intent is Stun)
        match self.move_view(e) {
            Some((_, intents)) => intents.iter().any(|i| matches!(i, Intent::Attack { .. } | Intent::DeathBlow | Intent::DeathBlowAttack { .. })),
            None => false,
        }
    }

    /// Per-instance growth the card's logic adds to a dynamic var's base value (`DynamicVars.X.BaseValue += ...`):
    /// Claw's accumulated damage and Genetic Algorithm's block (`CurrentBlock = 1 + IncreasedBlock`), both in `counter[0]`.
    #[inline]
    pub fn card_var_extra(&self, c: CardIdx, kind: crate::defs::VarKind) -> i32 {
        use crate::defs::VarKind;
        let card = &self.cards[c as usize];
        match (card.id, kind) {
            (ids::card::CLAW, VarKind::Damage) => card.counter[0] as i32,
            (ids::card::GENETIC_ALGORITHM, VarKind::Block) => 1 + card.counter[0] as i32,
            _ => 0,
        }
    }
}
