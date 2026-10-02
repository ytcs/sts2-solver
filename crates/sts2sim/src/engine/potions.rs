//! Potion use (`UsePotionAction` / `PotionModel.OnUseWrapper`).

use crate::content;
use crate::defs::*;
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

    /// `PlayerCmd` discard: the potion leaves the slot (no effect).
    pub fn discard_potion(&mut self, slot: usize) -> bool {
        if self.player.phase != Phase::Play || slot >= MAX_POTIONS || self.player.potions[slot].is_none() {
            return false;
        }
        let id = self.player.potions[slot].unwrap().id;
        self.player.potions[slot] = None;
        // PotionCmd.Discard -> Hook.AfterPotionDiscarded (run-level, unguarded).
        self.dispatch_u(hookbit::after_potion_discarded, |cx, me, l| l.after_potion_discarded(cx, me, id));
        true
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

    /// Runs / resumes the in-flight potion effect.
    pub fn run_potion(&mut self) {
        let Some(ctx) = self.potion_ctx else { return };
        let l = content::potion_listener(ctx.potion);
        match l.on_use_potion(self, ctx.potion, ctx.target, ctx.phase) {
            Flow::Suspend(next) => {
                self.potion_ctx = Some(PotionCtx { phase: next, ..ctx });
                self.stage = Stage::AwaitChoice;
            }
            Flow::Done => {
                self.potion_ctx = None;
                self.player.effect_depth = self.player.effect_depth.saturating_sub(1);
                let (pid, tgt) = (ctx.potion, ctx.target);
                if !self.cr(PLAYER).is_dead() {
                    self.dispatch_u(hookbit::after_potion_used, |cx, me, lst| lst.after_potion_used(cx, me, pid, tgt));
                }
            }
        }
    }
}
