//! Small card-effect helpers of the colorless / curse / status / token content that have no canonical equivalent
//! (gold, energy, shuffles, named vars, transform and cost helpers are the canonical ones in cmds.rs / energy.rs / cost.rs /
//! autoplay.rs).

use crate::dec::Dec;
use crate::defs::*;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

impl Combat {
    /// `ValueProp` flags of the card's first dynamic var of `kind` (`DamageVar.Props` / `BlockVar.Props`).
    pub fn card_var_props(&self, c: CardIdx, kind: VarKind) -> ValueProp {
        for v in crate::content::card_def(self.cards[c as usize].id).vars {
            if v.kind == kind {
                return ValueProp(v.props);
            }
        }
        ValueProp::NONE
    }

    /// `CreatureCmd.Damage(ctx, owner, card.DynamicVars.<Damage|HpLoss>, card, null)`: the player damages themselves
    /// with a card-sourced hit (dealer = card owner).
    pub fn self_damage_from_card(&mut self, c: CardIdx, amount: Dec, props: ValueProp) {
        self.damage(&[PLAYER], amount, props, PLAYER, c);
    }

    /// `PowerModel.SetAmount`: sets the amount directly (no hooks, no removal at 0).
    pub fn set_power_amount(&mut self, c: Cid, uid: u16, amount: i32) {
        if let Some(i) = self.power_idx(c, uid) {
            self.cr_mut(c).powers[i].amount = amount;
        }
    }
}
