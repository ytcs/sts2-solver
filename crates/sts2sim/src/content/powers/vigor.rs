use crate::dec::Dec;
use crate::engine::Attack;
use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn aux(cx: &Combat, me: &Me) -> i32 {
    cx.my_power(me).map_or(0, |p| p.aux)
}
fn set_aux(cx: &mut Combat, me: &Me, v: i32) {
    if cx.my_power(me).is_some() {
        if let Some(p) = cx.my_power_mut(me) { p.aux = v; }
    }
}
fn amount(cx: &Combat, me: &Me) -> i32 {
    cx.my_amount(me)
}

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
