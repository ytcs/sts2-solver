//! Energy / stars commands (`PlayerCmd.GainEnergy` ...), X values, star costs and the "after modifying" dispatch helper.

use super::damage::Mods;
use crate::content;
use crate::dec::Dec;
use crate::engine::HKind;
use crate::hooks::*;
use crate::state::*;
use crate::types::*;

impl Combat {
    /// `Hook.After*Modifying*(…, modifiers)` pattern: re-enumerates the listeners of the hook (fresh snapshot) and
    /// calls `f` only for the models recorded in `mods` — in listener order, once per model even if it was recorded
    /// by several passes (spec 02 §0).
    #[inline(always)]
    pub fn dispatch_modifiers(&mut self, guarded: bool, bit: u32, mods: &Mods, mut f: impl FnMut(&mut Combat, Me, &'static dyn Listener)) {
        if mods.is_empty() || !self.listen.has(bit) || (guarded && !self.hooks_enabled()) {
            return;
        }
        self.dispatch_modifiers_slow(bit, mods, &mut f);
    }

    #[inline(never)]
    fn dispatch_modifiers_slow(&mut self, bit: u32, mods: &Mods, f: &mut dyn FnMut(&mut Combat, Me, &'static dyn Listener)) {
        if !self.pass_enter() {
            return; // runaway-work safeguard tripped (`engine/budget.rs`)
        }
        let mut snap = crate::engine::Snapshot::new();
        self.snapshot_into(Mask::bit(bit), &mut snap);
        for e in snap.iter() {
            if self.still_live(&e.me) && mods.iter().any(|m| m.kind == e.me.kind && m.owner == e.me.owner && m.idx == e.me.idx) {
                f(self, e.me, content::listener(&e.me));
            }
        }
        self.pass_exit();
    }

    // ---- energy ----------------------------------------------------------------------------------------------------

    /// `Hook.ModifyEnergyGain` (guarded, threaded): the models whose `(int)` value changed are recorded.
    fn modify_energy_gain(&self, amount: Dec) -> (Dec, Mods) {
        let mut v = amount;
        let mut mods = Mods::new();
        if self.listen.has(hookbit::modify_energy_gain) && self.hooks_enabled() {
            let mut snap = crate::engine::Snapshot::new();
            self.snapshot_into(Mask::bit(hookbit::modify_energy_gain), &mut snap);
            for e in snap.iter() {
                if self.still_live(&e.me) {
                    let n = content::listener(&e.me).modify_energy_gain(self, e.me, v);
                    if v.trunc() != n.trunc() {
                        mods.push(e.me);
                    }
                    v = n;
                }
            }
        }
        (v, mods)
    }

    /// `PlayerCmd.GainEnergy(amount)`.
    pub fn gain_energy(&mut self, n: i32) {
        self.gain_energy_dec(Dec::int(n as i64));
    }

    pub fn gain_energy_dec(&mut self, amount: Dec) {
        if amount <= Dec::ZERO || self.is_ending() {
            return;
        }
        let (fin, mods) = self.modify_energy_gain(amount);
        self.dispatch_modifiers(true, hookbit::after_modifying_energy_gain, &mods, |cx, me, l| l.after_modifying_energy_gain(cx, me));
        if fin > Dec::ZERO {
            self.player.energy = (self.player.energy as i64 + fin.trunc() as i64).clamp(0, 999_999_999) as i32;
        }
    }

    /// `PlayerCmd.LoseEnergy(amount)`.
    pub fn lose_energy(&mut self, n: i32) {
        if n <= 0 || self.is_ending() {
            return;
        }
        self.player.energy = (self.player.energy - n).clamp(0, 999_999_999);
    }

    /// `PlayerCmd.SetEnergy(amount)`: gain / lose the difference.
    pub fn set_energy(&mut self, n: i32) {
        if self.is_ending() {
            return;
        }
        let e = self.player.energy;
        if e < n {
            self.gain_energy(n - e);
        } else if e > n {
            self.lose_energy(e - n);
        }
    }

    // ---- stars -----------------------------------------------------------------------------------------------------

    /// `PlayerCmd.GainStars` (`ShouldGainStars` AND, then `AfterStarsGained`).
    pub fn gain_stars(&mut self, n: i32) {
        if self.is_ending() {
            return;
        }
        if self.first_veto_g(hookbit::should_gain_stars, |cx, me, l| l.should_gain_stars(cx, me, Dec::int(n as i64))).is_some() {
            return;
        }
        let old = self.player.stars;
        self.player.stars = (old + n).max(0);
        let delta = self.player.stars - old;
        if delta != 0 {
            self.hist_push(HKind::StarsModified, PLAYER, NO, 0, NO, delta, 0, 0, 0);
        }
        self.dispatch_g(hookbit::after_stars_gained, |cx, me, l| l.after_stars_gained(cx, me, n));
    }

    /// `PlayerCmd.LoseStars`.
    pub fn lose_stars(&mut self, n: i32) {
        if n <= 0 || self.is_ending() {
            return;
        }
        let old = self.player.stars;
        self.player.stars = (old - n).max(0);
        let delta = self.player.stars - old;
        if delta != 0 {
            self.hist_push(HKind::StarsModified, PLAYER, NO, 0, NO, delta, 0, 0, 0);
        }
    }

    /// `PlayerCmd.SetStars`.
    pub fn set_stars(&mut self, n: i32) {
        if self.is_ending() {
            return;
        }
        let s = self.player.stars;
        if s < n {
            self.gain_stars(n - s);
        } else if s > n {
            self.lose_stars(s - n);
        }
    }

    // ---- star costs (`CardModel.*StarCost*`) -------------------------------------------------------------------------

    /// `CanonicalStarCost` marker for `HasStarCostX` cards (see `tools/gen_defs.py`).
    pub const STAR_COST_X: i8 = -2;

    #[inline]
    pub fn card_has_star_cost_x(&self, c: CardIdx) -> bool {
        self.card_def(c).star_cost == Self::STAR_COST_X
    }

    /// `CardModel.BaseStarCost` (-1 = none; star-X cards have canonical cost -1): canonical + upgrade delta.
    #[inline]
    pub fn card_base_star_cost(&self, c: CardIdx) -> i32 {
        let d = self.card_def(c);
        if d.star_cost < 0 {
            -1
        } else {
            (d.star_cost as i32 + d.up_star_cost as i32 * self.cards[c as usize].upgrade as i32).max(0)
        }
    }

    /// `CardModel.CurrentStarCost`: the last temporary star cost, except that a temporary 0 never gives a card
    /// without a star cost one.
    pub fn card_current_star_cost(&self, c: CardIdx) -> i32 {
        let base = self.card_base_star_cost(c);
        match self.cards[c as usize].star_mods.last() {
            Some(m) => {
                if m.amount == 0 && base < 0 {
                    base
                } else {
                    m.amount as i32
                }
            }
            None => base,
        }
    }

    /// `CardModel.GetStarCostWithModifiers()`: X cards cost all current stars; otherwise the current cost through
    /// `Hook.ModifyStarCost` (guarded, threaded; skipped for negative costs) while in a combat pile. -1 = no star cost.
    #[inline(always)]
    pub fn card_star_cost(&self, c: CardIdx) -> i32 {
        let d = self.card_def(c);
        if d.star_cost < 0 && d.star_cost != Self::STAR_COST_X && self.cards[c as usize].star_mods.is_empty() {
            return -1;
        }
        self.card_star_cost_slow(c)
    }

    #[inline(never)]
    fn card_star_cost_slow(&self, c: CardIdx) -> i32 {
        if self.card_has_star_cost_x(c) {
            return self.player.stars;
        }
        let base = self.card_current_star_cost(c);
        if base < 0 || !self.card_in_combat_pile(c) || !self.listen.has(hookbit::try_modify_star_cost) || !self.hooks_enabled() {
            return base;
        }
        let mut v = Dec::int(base as i64);
        let mut snap = crate::engine::Snapshot::new();
        self.snapshot_into(Mask::bit(hookbit::try_modify_star_cost), &mut snap);
        for e in snap.iter() {
            if self.still_live(&e.me) {
                if let Some(n) = content::listener(&e.me).try_modify_star_cost(self, e.me, c, v) {
                    v = n;
                }
            }
        }
        v.trunc()
    }

    fn add_temp_star_cost(&mut self, c: CardIdx, cost: i32, expire: u8) {
        let m = CostMod::new(cost as i8, false, false, expire);
        let mods = &mut self.cards[c as usize].star_mods;
        if mods.len() >= 2 {
            mods.remove(0);
        }
        mods.push(m);
    }

    /// `CardModel.SetStarCostUntilPlayed`.
    pub fn set_star_cost_until_played(&mut self, c: CardIdx, cost: i32) {
        self.add_temp_star_cost(c, cost, EXPIRE_WHEN_PLAYED);
    }
    /// `CardModel.SetStarCostThisTurn` (cleared at end of turn and when played).
    pub fn set_star_cost_this_turn(&mut self, c: CardIdx, cost: i32) {
        self.add_temp_star_cost(c, cost, EXPIRE_END_OF_TURN | EXPIRE_WHEN_PLAYED);
    }
    /// `CardModel.SetStarCostThisCombat`.
    pub fn set_star_cost_this_combat(&mut self, c: CardIdx, cost: i32) {
        self.add_temp_star_cost(c, cost, 0);
    }

    /// Drops temporary star costs flagged `flag` (`EXPIRE_END_OF_TURN` / `EXPIRE_WHEN_PLAYED`).
    pub(crate) fn clear_star_mods(&mut self, c: CardIdx, flag: u8) {
        let card = &mut self.cards[c as usize];
        if card.star_mods.is_empty() {
            return;
        }
        let mut kept: crate::util::SmallVec<CostMod, 2> = crate::util::SmallVec::new();
        for m in card.star_mods.iter() {
            if m.expire() & flag == 0 {
                kept.push(*m);
            }
        }
        card.star_mods = kept;
    }

    /// `CardModel.ResolveStarXValue`: the captured star X (all stars at play time).
    pub fn resolve_star_x_value(&self, c: CardIdx) -> i32 {
        if !self.card_has_star_cost_x(c) {
            return 0;
        }
        // `Hook.ModifyXValue(CombatState, this, LastStarsSpent)` (ChemicalX also applies to star X costs)
        self.x_value(c)
    }

    /// Sum of the positive `StarsModifiedEntry` amounts of this turn (Radiate).
    pub fn stars_gained_this_turn(&self) -> i32 {
        self.hist_log.iter().filter(|e| e.kind == HKind::StarsModified && self.hist_this_turn(e) && e.val > 0).map(|e| e.val as i32).sum()
    }

    // ---- X cost -----------------------------------------------------------------------------------------------------

    /// `CardModel.ResolveEnergyXValue()`: the captured X through `Hook.ModifyXValue` (ChemicalX).
    pub fn x_value(&self, c: CardIdx) -> i32 {
        let mut v = self.cards[c as usize].x_value as i32;
        if self.listen.has(hookbit::modify_x_value) && self.hooks_enabled() {
            let mut snap = crate::engine::Snapshot::new();
            self.snapshot_into(Mask::bit(hookbit::modify_x_value), &mut snap);
            for e in snap.iter() {
                if self.still_live(&e.me) {
                    v = content::listener(&e.me).modify_x_value(self, e.me, c, v);
                }
            }
        }
        v
    }
}
