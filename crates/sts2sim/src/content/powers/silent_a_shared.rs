use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn owner_on(cx: &Combat, me: Me, side: Side) -> bool {
    cx.cr(me.owner).side == side
}

listener!(PoisonPower {
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if !owner_on(cx, me, side) {
            return;
        }
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        let amount = cx.cr(me.owner).powers[i].amount;
        let mut accelerant = 0;
        let owner_side = cx.cr(me.owner).side;
        for c in 0..MAX_CREATURES {
            let cr = cx.cr(c as Cid);
            if cr.in_combat && cr.is_alive() && cr.side != owner_side {
                accelerant += cr.power_amount(ids::power::ACCELERANT_POWER);
            }
        }
        let iterations = amount.min(1 + accelerant);
        let mut cur = amount;
        for _ in 0..iterations {
            if let Some(i) = cx.power_idx(me.owner, me.idx) {
                cur = cx.cr(me.owner).powers[i].amount;
            }
            cx.damage(&[me.owner], Dec::int(cur as i64), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED), NO, NO);
            if cx.cr(me.owner).is_alive() {
                if cx.power_idx(me.owner, me.idx).is_some() {
                    cx.decrement_power(me.owner, me.idx);
                } else {
                    cur -= 1;
                }
            }
        }
    }
});

listener!(ThornsPower {
    fn before_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, _amount: Dec, props: ValueProp, dealer: Cid) {
        let omnislice = cx.dmg_card != NO && cx.cards[cx.dmg_card as usize].id == ids::card::OMNISLICE;
        if target == me.owner && dealer != NO && (props.is_powered() || omnislice) {
            let amt = cx.power_amount(me.owner, me.id);
            cx.damage(&[dealer], Dec::int(amt as i64), ValueProp::UNPOWERED.or(ValueProp::SKIP_HURT_ANIM), me.owner, NO);
        }
    }
});

listener!(BlockNextTurnPower {
    fn after_block_cleared(&self, cx: &mut Combat, me: Me, creature: Cid) {
        if creature == me.owner {
            let amt = cx.power_amount(me.owner, me.id);
            cx.gain_block(me.owner, Dec::int(amt as i64), ValueProp::UNPOWERED, NO);
            cx.remove_power(me.owner, me.idx);
        }
    }
});

listener!(EnergyNextTurnPower {
    fn after_energy_reset(&self, cx: &mut Combat, me: Me) {
        let amt = cx.power_amount(me.owner, me.id);
        cx.gain_energy(amt);
        cx.remove_power(me.owner, me.idx);
    }
});
