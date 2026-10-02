//! Power definitions and behaviour.

use crate::dec::Dec;
use crate::defs::*;
use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

pub static DEFAULT: PowerDef = PowerDef::new(PowerType::Buff);
pub static BUFF_ALLOW_NEGATIVE: PowerDef = PowerDef::new(PowerType::Buff).allow_negative();
pub static DEBUFF: PowerDef = PowerDef::new(PowerType::Debuff);

listener!(StrengthPower {
    fn modify_damage_additive(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if me.owner != q.dealer || !q.props.is_powered() {
            return Dec::ZERO;
        }
        Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
});

listener!(DexterityPower {
    fn modify_block_additive(&self, cx: &Combat, me: Me, q: &BlockQ) -> Dec {
        // card source: the card's owner must be the power owner; otherwise the block target must be.
        if q.card != NO {
            if me.owner != PLAYER {
                return Dec::ZERO;
            }
        } else if me.owner != q.target {
            return Dec::ZERO;
        }
        if !q.props.is_powered() {
            return Dec::ZERO;
        }
        Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
});

listener!(VulnerablePower {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner || !q.props.is_powered() {
            return Dec::ONE;
        }
        let mut mult = Dec::frac(15, 1);
        // The dealer's Cruelty power adds Amount/100 (CrueltyPower.ModifyVulnerableMultiplier; target != owner here).
        if q.dealer != NO && q.dealer != q.target {
            let cruelty = cx.power_amount(q.dealer, crate::ids::power::CRUELTY_POWER);
            if cruelty != 0 {
                mult += Dec::frac(cruelty as i64, 2);
            }
        }
        // TODO(fidelity): dealer's PaperPhrog relic (+PetOwner's Cruelty) and target's Debilitate also adjust the multiplier.
        mult
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Enemy {
            cx.tick_down_power(me.owner, me.idx);
        }
    }
});

listener!(WeakPower {
    fn modify_damage_multiplicative(&self, _cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.dealer != me.owner || !q.props.is_powered() {
            return Dec::ONE;
        }
        // TODO(fidelity): target's PaperKrane relic and dealer's Debilitate adjust the multiplier.
        Dec::frac(75, 2)
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Enemy {
            cx.tick_down_power(me.owner, me.idx);
        }
    }
});

listener!(FrailPower {
    fn modify_block_multiplicative(&self, _cx: &Combat, me: Me, q: &BlockQ) -> Dec {
        if q.target != me.owner || !q.props.is_powered() {
            return Dec::ONE;
        }
        Dec::frac(75, 2)
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Enemy {
            cx.tick_down_power(me.owner, me.idx);
        }
    }
});
