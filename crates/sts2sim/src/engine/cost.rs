use crate::state::*;

pub type CostMods = crate::util::SmallVec<CostMod, 4>;

impl Combat {
    pub fn push_cost_mod(&mut self, c: CardIdx, m: CostMod) {
        let card = &mut self.cards[c as usize];
        if !m.relative() && !m.reduce_only() && m.expire() == 0 {
            card.mods.clear();
        }
        if m.relative() && !m.reduce_only() {
            for e in card.mods.as_mut_slice().iter_mut().rev() {
                if !e.relative() || e.reduce_only() {
                    break;
                }
                if e.expire() == m.expire() && (e.amount as i32 + m.amount as i32).abs() < 100 {
                    e.amount += m.amount;
                    return;
                }
            }
        }
        if card.mods.len() >= 4 {
            card.mods.remove(0);
        }
        card.mods.push(m);
    }

    fn cost_guard_set(&self, c: CardIdx, amount: i32) -> bool {
        amount != 0 || self.card_def(c).cost >= 0
    }

    pub fn set_cost_this_turn_or_until_played(&mut self, c: CardIdx, amount: i32, reduce_only: bool) {
        if self.cost_guard_set(c, amount) {
            self.push_cost_mod(c, CostMod::new(amount as i8, false, reduce_only, EXPIRE_END_OF_TURN | EXPIRE_WHEN_PLAYED));
        }
    }

    pub fn set_cost_this_combat(&mut self, c: CardIdx, amount: i32, reduce_only: bool) {
        if self.cost_guard_set(c, amount) {
            self.push_cost_mod(c, CostMod::new(amount as i8, false, reduce_only, 0));
        }
    }

    pub fn add_cost_until_played(&mut self, c: CardIdx, delta: i32, reduce_only: bool) {
        if delta != 0 {
            self.push_cost_mod(c, CostMod::new(delta as i8, true, reduce_only, EXPIRE_WHEN_PLAYED));
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

    pub fn add_keyword(&mut self, c: CardIdx, k: u8) {
        let card = &mut self.cards[c as usize];
        card.kw_add |= k;
        card.kw_remove &= !k;
    }

    pub fn remove_keyword(&mut self, c: CardIdx, k: u8) {
        let card = &mut self.cards[c as usize];
        card.kw_remove |= k;
        card.kw_add &= !k;
    }

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
