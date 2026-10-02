//! Shared powers that several characters' cards apply (DrawCardsNextTurn, EnergyNextTurn, Vigor). Other branches port the same C#
//! classes (and `VigorPower` here is the complete card-sourced + monster-sourced version, spec 02 §6.6): when merging, keep ONE
//! definition of each and delete this file (or the duplicates) — duplicate `listener!` names are a build error.

use crate::dec::Dec;
use crate::engine::Attack;
use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn aux(cx: &Combat, me: &Me) -> i32 {
    cx.power_idx(me.owner, me.idx).map_or(0, |i| cx.cr(me.owner).powers[i].aux)
}
fn set_aux(cx: &mut Combat, me: &Me, v: i32) {
    if let Some(i) = cx.power_idx(me.owner, me.idx) {
        cx.cr_mut(me.owner).powers[i].aux = v;
    }
}
fn amount(cx: &Combat, me: &Me) -> i32 {
    cx.power_idx(me.owner, me.idx).map_or(me.amount, |i| cx.cr(me.owner).powers[i].amount)
}

// DrawCardsNextTurnPower: +Amount cards on the next turn's draw (only if it existed when that turn started).
listener!(DrawCardsNextTurnPower {
    fn modify_hand_draw(&self, cx: &Combat, me: Me, count: Dec) -> Dec {
        match cx.power_idx(me.owner, me.idx) {
            Some(i) => {
                let p = &cx.cr(me.owner).powers[i];
                if p.amount_on_turn_start == 0 { count } else { count + Dec::int(p.amount as i64) }
            }
            None => count,
        }
    }
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            if let Some(i) = cx.power_idx(me.owner, me.idx) {
                if cx.cr(me.owner).powers[i].amount_on_turn_start != 0 {
                    cx.remove_power(me.owner, me.idx);
                }
            }
        }
    }
});

// EnergyNextTurnPower: Amount energy after the next energy reset, then removed.
listener!(EnergyNextTurnPower {
    fn after_energy_reset(&self, cx: &mut Combat, me: Me) {
        let a = amount(cx, &me);
        cx.gain_energy(a);
        cx.remove_power(me.owner, me.idx);
    }
});

// VigorPower (player + monster flavour, spec 02 §6.6): +Amount damage to the next powered card attack, consumed after it.
// `aux` = 0 (no attack claimed yet) or (card index + 1) | (amount when the attack started << 16).
listener!(VigorPower {
    fn before_attack(&self, cx: &mut Combat, me: Me, attack: &Attack) {
        if attack.dealer != me.owner || !attack.props.is_powered() || aux(cx, &me) != 0 || attack.card == NO {
            return;
        }
        let a = amount(cx, &me);
        set_aux(cx, &me, (attack.card as i32 + 1) | (a << 16));
    }
    fn modify_damage_additive(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.dealer != me.owner || !q.props.is_powered() {
            return Dec::ZERO;
        }
        let x = aux(cx, &me);
        if x != 0 && q.card != NO && (q.card as i32 + 1) != (x & 0xFFFF) {
            return Dec::ZERO;
        }
        Dec::int(amount(cx, &me) as i64)
    }
    fn after_attack(&self, cx: &mut Combat, me: Me, attack: &Attack) {
        let x = aux(cx, &me);
        if x != 0 && attack.card != NO && (attack.card as i32 + 1) == (x & 0xFFFF) {
            cx.modify_power_amount(me.owner, me.idx, Dec::int(-((x >> 16) as i64)), NO, NO);
        }
    }
});
