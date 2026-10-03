//! Potion use (`UsePotionAction` / `PotionModel.OnUseWrapper`), discard and procure (`PotionCmd`).

use crate::content;
use crate::defs::*;
use crate::engine::HKind;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;

impl Combat {
    /// `PotionModel` dynamic var by kind.
    pub fn potion_var(&self, potion: u16, kind: VarKind) -> i32 {
        for v in content::potion_def(potion).vars {
            if v.kind == kind && v.kind != VarKind::Power {
                return v.base as i32;
            }
        }
        0
    }

    /// Generic named potion var (`DynamicVar("Name", v)`), by `gen_cards::var_name::*`.
    pub fn potion_named_var(&self, potion: u16, name: u16) -> i32 {
        for v in content::potion_def(potion).vars {
            if v.kind == VarKind::Named && v.arg == name {
                return v.base as i32;
            }
        }
        0
    }

    pub fn potion_power_var(&self, potion: u16, power: u16) -> i32 {
        for v in content::potion_def(potion).vars {
            if v.kind == VarKind::Power && v.arg == power {
                return v.base as i32;
            }
        }
        0
    }

    /// `PotionCmd.Discard`: the potion leaves the slot (no effect), then `Hook.AfterPotionDiscarded`.
    pub fn discard_potion(&mut self, slot: usize) -> bool {
        if self.player.phase != Phase::Play || slot >= MAX_POTIONS || self.player.potions[slot].is_none() {
            return false;
        }
        let id = self.player.potions[slot].unwrap().id;
        self.player.potions[slot] = None;
        self.dispatch_u(hookbit::after_potion_discarded, |cx, me, l| l.after_potion_discarded(cx, me, id));
        true
    }

    /// `PotionCmd.TryToProcure`: `ShouldProcurePotion` (AND) veto, first free slot, `AfterPotionProcured`.
    /// Returns the slot of the new potion.
    pub fn try_to_procure_potion(&mut self, potion: u16) -> Option<usize> {
        if self.first_veto(hookbit::should_procure_potion, |cx, me, l| l.should_procure_potion(cx, me, potion)).is_some() {
            return None;
        }
        let slot = (0..self.player.potion_slots as usize).find(|&i| self.player.potions[i].is_none())?;
        if !content::potion_implemented(potion) {
            self.flag_missing(Kind::Potion, potion);
        }
        self.listen |= content::potion_mask(potion);
        self.player.potions[slot] = Some(Potion { id: potion });
        self.dispatch_u(hookbit::after_potion_procured, |cx, me, l| l.after_potion_procured(cx, me, potion));
        Some(slot)
    }

    /// Manual use of the potion in `slot`.
    pub fn use_potion(&mut self, slot: usize, mut target: Cid) -> bool {
        if self.stage != Stage::AwaitAction || self.player.phase != Phase::Play || slot >= MAX_POTIONS {
            return false;
        }
        let Some(p) = self.player.potions[slot] else { return false };
        let d = content::potion_def(p.id);
        if d.usage == PotionUsage::Automatic || d.usage == PotionUsage::None {
            return false;
        }
        // EnqueueManualUse: a null target for a potion that may target the owner defaults to the owner.
        match d.target {
            TargetType::AnyEnemy => {
                if target == NO || !self.cr(target).in_combat || self.cr(target).is_dead() || self.cr(target).side != Side::Enemy {
                    return false;
                }
            }
            TargetType::AnyPlayer | TargetType::Self_ => target = PLAYER,
            _ => target = NO,
        }
        // OnUseWrapper: RemoveBeforeUse; BeforePotionUsed; effect (depth++); AfterPotionUsed.
        self.player.potions[slot] = None;
        let pid = p.id;
        self.dispatch_u(hookbit::before_potion_used, |cx, me, l| l.before_potion_used(cx, me, pid, target));
        self.player.effect_depth += 1;
        self.potion_ctx = Some(PotionCtx { potion: pid, target, phase: 0 });
        self.run_potion();
        true
    }

    /// `PotionModel.OnUseWrapper` run to completion without ever suspending (potions triggered by hooks, e.g. Fairy in
    /// a Bottle from `AfterPreventingDeath`): the potion leaves its slot first, then BeforePotionUsed, OnUse,
    /// AfterPotionUsed. Does not touch the in-flight potion context.
    pub fn use_potion_now(&mut self, slot: usize, target: Cid) {
        let Some(p) = self.player.potions[slot] else { return };
        self.player.potions[slot] = None;
        let pid = p.id;
        self.dispatch_u(hookbit::before_potion_used, |cx, me, l| l.before_potion_used(cx, me, pid, target));
        self.player.effect_depth += 1;
        let r = content::potion_listener(pid).on_use_potion(self, pid, target, 0);
        debug_assert!(r == Flow::Done, "a hook-triggered potion must not ask for a decision");
        self.player.effect_depth = self.player.effect_depth.saturating_sub(1);
        if !self.cr(PLAYER).is_dead() {
            self.hist_push(HKind::PotionUsed, PLAYER, target, pid, NO, 0, 0, 0, 0);
            self.dispatch_u(hookbit::after_potion_used, |cx, me, l| l.after_potion_used(cx, me, pid, target));
            self.check_for_empty_hand();
        }
    }

    /// Runs / resumes the in-flight potion effect.
    pub fn run_potion(&mut self) {
        let Some(ctx) = self.potion_ctx else { return };
        let l = content::potion_listener(ctx.potion);
        let mut phase = ctx.phase;
        let mut r = None;
        if phase == PH_DRAW_TAIL {
            // The potion's draw was interrupted by a decision (`draw_cards_s`): finish it, then continue the effect.
            if self.resume_effect_draw() {
                r = Some(Flow::Suspend(PH_DRAW_TAIL));
            } else if self.draw_next == DRAW_DONE {
                r = Some(Flow::Done);
            } else {
                phase = self.draw_next;
            }
        }
        let flow = match r {
            Some(f) => f,
            None => l.on_use_potion(self, ctx.potion, ctx.target, phase),
        };
        match flow {
            Flow::Suspend(next) => {
                self.potion_ctx = Some(PotionCtx { phase: next, ..ctx });
                self.stage = Stage::AwaitChoice;
            }
            Flow::Done => {
                self.potion_ctx = None;
                self.player.effect_depth = self.player.effect_depth.saturating_sub(1);
                let (pid, tgt) = (ctx.potion, ctx.target);
                if !self.cr(PLAYER).is_dead() {
                    self.hist_push(HKind::PotionUsed, PLAYER, tgt, pid, NO, 0, 0, 0, 0);
                    self.dispatch_u(hookbit::after_potion_used, |cx, me, lst| lst.after_potion_used(cx, me, pid, tgt));
                    self.check_for_empty_hand();
                }
            }
        }
    }
}
