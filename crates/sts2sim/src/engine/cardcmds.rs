use crate::dec::Dec;
use crate::defs::*;
use crate::state::*;
use crate::types::*;

impl Combat {
    pub fn card_var_props(&self, c: CardIdx, kind: VarKind) -> ValueProp {
        for v in crate::content::card_def(self.cards[c as usize].id).vars {
            if v.kind == kind {
                return ValueProp(v.props);
            }
        }
        ValueProp::NONE
    }

    pub fn self_damage_from_card(&mut self, c: CardIdx, amount: Dec, props: ValueProp) {
        self.damage(&[PLAYER], amount, props, PLAYER, c);
    }

    pub fn set_power_amount(&mut self, c: Cid, uid: u16, amount: i32) {
        if let Some(i) = self.power_idx(c, uid) {
            self.cr_mut(c).powers[i].amount = amount;
        }
    }
}
