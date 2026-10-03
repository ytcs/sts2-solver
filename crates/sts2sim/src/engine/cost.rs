//! Local energy-cost modifiers (`CardEnergyCost.Set*/Add*`, spec 03 §2.2).

use crate::state::*;

/// Capacity of a card's local modifier list.
pub type CostMods = crate::util::SmallVec<CostMod, 4>;

impl Combat {
    /// Appends a local cost modifier. The game's list is unbounded; here an absolute, non-reduce-only modifier that
    /// lasts the whole combat makes every earlier modifier irrelevant (it overwrites the running value), so those are
    /// dropped — this keeps Slither (a new `SetThisCombat` on every draw) bounded without changing any result.
    pub fn push_cost_mod(&mut self, c: CardIdx, m: CostMod) {
        let card = &mut self.cards[c as usize];
        if !m.relative() && !m.reduce_only() && m.expire() == 0 {
            card.mods.clear();
        }
        // Adjacent relative, non-reduce-only modifiers with the same expiry commute: merge them (keeps Stampede-style
        // "-1 per attack" stacks inside the 4 slots).
        if m.relative() && !m.reduce_only() {
            if let Some(last) = card.mods.as_mut_slice().last_mut() {
                if last.relative() && !last.reduce_only() && last.expire() == m.expire() && (last.amount as i32 + m.amount as i32).abs() < 100 {
                    last.amount += m.amount;
                    return;
                }
            }
        }
        if card.mods.len() >= 4 {
            // Full: drop the oldest modifier that an absolute later one dominates, else the oldest (never expected).
            card.mods.remove(0);
        }
        card.mods.push(m);
    }

    fn cost_guard_set(&self, c: CardIdx, amount: i32) -> bool {
        // `c != 0 || Canonical >= 0`: "set to 0" on a card whose canonical cost is negative is ignored.
        amount != 0 || self.card_def(c).cost >= 0
    }

    /// `SetUntilPlayed(amount)` — absolute, expires when played.
    pub fn set_cost_until_played(&mut self, c: CardIdx, amount: i32, reduce_only: bool) {
        if self.cost_guard_set(c, amount) {
            self.push_cost_mod(c, CostMod::new(amount as i8, false, reduce_only, EXPIRE_WHEN_PLAYED));
        }
    }

    /// `SetThisTurnOrUntilPlayed(amount)` — absolute, expires at end of turn or when played.
    pub fn set_cost_this_turn_or_until_played(&mut self, c: CardIdx, amount: i32, reduce_only: bool) {
        if self.cost_guard_set(c, amount) {
            self.push_cost_mod(c, CostMod::new(amount as i8, false, reduce_only, EXPIRE_END_OF_TURN | EXPIRE_WHEN_PLAYED));
        }
    }

    /// `SetThisTurn(amount)` — absolute, expires at end of turn.
    pub fn set_cost_this_turn(&mut self, c: CardIdx, amount: i32, reduce_only: bool) {
        if self.cost_guard_set(c, amount) {
            self.push_cost_mod(c, CostMod::new(amount as i8, false, reduce_only, EXPIRE_END_OF_TURN));
        }
    }

    /// `SetThisCombat(amount)` — absolute, lasts the combat.
    pub fn set_cost_this_combat(&mut self, c: CardIdx, amount: i32, reduce_only: bool) {
        if self.cost_guard_set(c, amount) {
            self.push_cost_mod(c, CostMod::new(amount as i8, false, reduce_only, 0));
        }
    }

    /// `AddUntilPlayed(delta)` / `AddThisTurnOrUntilPlayed` / `AddThisTurn` / `AddThisCombat` — relative (no-op for 0).
    pub fn add_cost_until_played(&mut self, c: CardIdx, delta: i32, reduce_only: bool) {
        if delta != 0 {
            self.push_cost_mod(c, CostMod::new(delta as i8, true, reduce_only, EXPIRE_WHEN_PLAYED));
        }
    }
    pub fn add_cost_this_turn_or_until_played(&mut self, c: CardIdx, delta: i32, reduce_only: bool) {
        if delta != 0 {
            self.push_cost_mod(c, CostMod::new(delta as i8, true, reduce_only, EXPIRE_END_OF_TURN | EXPIRE_WHEN_PLAYED));
        }
    }
    pub fn add_cost_this_turn(&mut self, c: CardIdx, delta: i32, reduce_only: bool) {
        if delta != 0 {
            self.push_cost_mod(c, CostMod::new(delta as i8, true, reduce_only, EXPIRE_END_OF_TURN));
        }
    }
    pub fn add_cost_this_combat(&mut self, c: CardIdx, delta: i32, reduce_only: bool) {
        if delta != 0 {
            self.push_cost_mod(c, CostMod::new(delta as i8, true, reduce_only, 0));
        }
    }

    /// `CardModel.AddKeyword` (local keyword set).
    pub fn add_keyword(&mut self, c: CardIdx, k: u8) {
        let card = &mut self.cards[c as usize];
        card.kw_add |= k;
        card.kw_remove &= !k;
    }

    /// `CardModel.RemoveKeyword` (local keyword set).
    pub fn remove_keyword(&mut self, c: CardIdx, k: u8) {
        let card = &mut self.cards[c as usize];
        card.kw_remove |= k;
        card.kw_add &= !k;
    }

    /// `CardEnergyCost.UpgradeBy(addend)` (spec 03 §2.6): changes the base cost (never below 0); absolute local modifiers
    /// above the new base are lowered to it.
    pub fn upgrade_cost_by(&mut self, c: CardIdx, a: i32) {
        if self.card_def(c).x_cost || a == 0 {
            return;
        }
        let card = &mut self.cards[c as usize];
        let old = card.cost_base as i32;
        let new = (old + a).max(0);
        if new < old {
            for m in card.mods.as_mut_slice() {
                if !m.relative() && m.amount as i32 > new {
                    m.amount = new as i8;
                }
            }
        }
        card.cost_base = new as i8;
    }
}
