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

    /// `CardFactory.GetForCombat(player, pool.Where(extra), count, CombatCardGeneration)`: `count` draws WITH
    /// replacement (`NextItem`: one draw each) over the generatable cards of `pool`.
    pub fn get_for_combat_where(&mut self, pool: &[u16], count: usize, extra: impl Fn(&CardDef) -> bool) -> ArrayVec<CardIdx, 16> {
        let mut list: ArrayVec<u16, 256> = ArrayVec::new();
        for &id in pool {
            let d = crate::content::card_def(id);
            if extra(d) && !d.multiplayer_only && d.can_be_generated_in_combat && !matches!(d.rarity, CardRarity::Basic | CardRarity::Ancient | CardRarity::Event) {
                list.push(id);
            }
        }
        let mut out = ArrayVec::new();
        if list.is_empty() {
            return out;
        }
        for _ in 0..count {
            let i = self.rng.combat_card_generation.next_int_range(0, list.len() as i32) as usize;
            if let Some(c) = self.new_card(list[i], 0) {
                out.push(c);
            }
        }
        out
    }
}
