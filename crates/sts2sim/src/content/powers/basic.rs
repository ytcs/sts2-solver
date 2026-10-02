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
        let mut num = Dec::frac(15, 1);
        // The dealer's Paper Phrog: +0.25 (only when the target is not the relic owner).
        if q.dealer == PLAYER && q.target != PLAYER && cx.has_relic(crate::ids::relic::PAPER_PHROG) {
            num += Dec::frac(25, 2);
        }
        // TODO(fidelity): dealer's Cruelty power and target's Debilitate adjust the multiplier.
        num
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Enemy {
            cx.tick_down_power(me.owner, me.idx);
        }
    }
});

listener!(WeakPower {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.dealer != me.owner || !q.props.is_powered() {
            return Dec::ONE;
        }
        let mut num = Dec::frac(75, 2);
        // The target's Paper Krane: -0.15 when the relic owner is the one being hit.
        if q.target == PLAYER && cx.has_relic(crate::ids::relic::PAPER_KRANE) {
            num -= Dec::frac(15, 2);
        }
        // TODO(fidelity): dealer's Debilitate adjusts the multiplier.
        num
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
