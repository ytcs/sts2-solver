//! VigorPower (player + monster flavour, spec 02 §6.6): the complete card-sourced + monster-sourced version (the single
//! registration; other branches' copies were dropped).

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

// VigorPower (player + monster flavour, spec 02 §6.6): +Amount damage to the next powered attack, consumed after it.
// `aux` = 0 (no attack claimed yet) or claim | (amount when the attack started << 16), where claim = card index + 1 for a
// card attack and 0x8000 for a monster attack (`ModelSource == null`; oracle-verified: TerrorEel's Vigor IS consumed by
// its next attack). `commandToModify` is never cleared in C#; here the claim of a monster attack matches any later
// attack of the owner without a card (only differs if Vigor survives its consumption, which never happens for the Eel).
fn claim_of(card: CardIdx) -> i32 {
    if card == NO { 0x8000 } else { card as i32 + 1 }
}
listener!(VigorPower {
    fn before_attack(&self, cx: &mut Combat, me: Me, attack: &Attack) {
        if attack.dealer != me.owner || !attack.props.is_powered() || aux(cx, &me) != 0 {
            return;
        }
        let a = amount(cx, &me);
        set_aux(cx, &me, claim_of(attack.card) | (a << 16));
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
        if x != 0 && attack.dealer == me.owner && claim_of(attack.card) == (x & 0xFFFF) {
            cx.modify_power_amount(me.owner, me.idx, Dec::int(-((x >> 16) as i64)), NO, NO);
        }
    }
});
