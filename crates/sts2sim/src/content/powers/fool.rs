use crate::content::gen_cards::var_name;
use crate::dec::Dec;
use crate::engine::{Attack, IMPALED_HITS};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn aux(cx: &Combat, me: &Me) -> i32 {
    cx.power_aux(me.owner, me.idx)
}
fn amount(cx: &Combat, me: &Me) -> i32 {
    cx.power_idx(me.owner, me.idx).map_or(me.amount, |i| cx.cr(me.owner).powers[i].amount)
}
fn applier(cx: &Combat, me: &Me) -> Cid {
    cx.power_idx(me.owner, me.idx).map_or(NO, |i| cx.cr(me.owner).powers[i].applier)
}

// aux = HitCounter; the arrow list is on the player's arrow slots (`ArrowSlot::impaled`).
listener!(ImpaledPower {
    fn initial_power_aux(&self) -> i32 {
        IMPALED_HITS
    }
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, _unblocked: i32, props: ValueProp, dealer: Cid) {
        if target != me.owner || dealer == NO || dealer != applier(cx, &me) || !props.is_powered() {
            return;
        }
        let source = cx.dmg_card;
        let n = aux(cx, &me) - 1;
        cx.set_power_aux(me.owner, me.idx, n);
        if n <= 0 {
            cx.set_power_aux(me.owner, me.idx, IMPALED_HITS);
            cx.dislodge_first(me.owner, Some(me.idx), source);
            cx.remove_impaled_if_no_arrows(me.owner, me.idx);
        }
    }
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, _was_removal_prevented: bool) {
        let a = applier(cx, &me);
        if creature != me.owner || a == NO || cx.cr(a).is_dead() {
            return;
        }
        let source = cx.dmg_card;
        cx.dislodge_all_arrows(me.owner, me.idx, source);
        cx.remove_power(me.owner, me.idx);
    }
});

fn claim_of(card: CardIdx) -> i32 {
    if card == NO { 0x8000 } else { card as i32 + 1 }
}

// aux = claimed attack (card + 1, 0x8000 sourceless) | amount when it started << 16, as VigorPower.
listener!(AimPower {
    fn before_attack(&self, cx: &mut Combat, me: Me, attack: &Attack) {
        if attack.dealer != me.owner || !attack.props.is_powered() || !attack.props.has(ValueProp::FROM_ARROW) || aux(cx, &me) != 0 {
            return;
        }
        let a = amount(cx, &me);
        cx.set_power_aux(me.owner, me.idx, claim_of(attack.card) | (a << 16));
    }
    fn modify_damage_additive(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.dealer != me.owner || !q.props.is_powered() || !q.props.has(ValueProp::FROM_ARROW) {
            return Dec::ZERO;
        }
        let x = aux(cx, &me);
        if x != 0 && q.card != NO && (q.card as i32 + 1) != (x & 0xFFFF) {
            return Dec::ZERO;
        }
        let n = Dec::int(amount(cx, &me) as i64);
        if q.card != NO && cx.cards[q.card as usize].id == ids::card::HEAVY_SHOT {
            return n * Dec::int(cx.card_named_var(q.card, var_name::AIM_MULTIPLIER) as i64);
        }
        n
    }
    fn after_attack(&self, cx: &mut Combat, me: Me, attack: &Attack) {
        let x = aux(cx, &me);
        if x != 0 && claim_of(attack.card) == (x & 0xFFFF) {
            cx.modify_power_amount(me.owner, me.idx, Dec::int(-((x >> 16) as i64)), NO, NO);
        }
    }
});

const STRETCH_CLAIMED: i32 = 1 << 16;
const STRETCH_GAVE: i32 = 1 << 17;

// aux = claimed play serial | STRETCH_CLAIMED | STRETCH_GAVE (`gaveBonusBlock`); the amount when the play started is
// in `hist.play_amounts`. `gaveBonusBlock` is set from `after_modifying_block_amount` (Stretch is listed as a modifier
// exactly when its additive was non-zero): modify hooks cannot mutate.
listener!(StretchPower {
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if me.owner != PLAYER || !cx.card_gains_block(play.card) || aux(cx, &me) & STRETCH_CLAIMED != 0 {
            return;
        }
        cx.set_power_aux(me.owner, me.idx, play.serial as i32 | STRETCH_CLAIMED);
        let a = amount(cx, &me);
        cx.play_amount_add(me.idx, play.card, a);
    }
    fn modify_block_additive(&self, cx: &Combat, me: Me, q: &BlockQ) -> Dec {
        if q.card == NO || me.owner != PLAYER {
            return Dec::ZERO;
        }
        let x = aux(cx, &me);
        let claimed = x & STRETCH_CLAIMED != 0;
        let serial = cx.active_play_serial(q.card);
        if claimed && serial != (x & 0xFFFF) as u16 {
            return Dec::ZERO;
        }
        if !q.props.is_powered() {
            return Dec::ZERO;
        }
        if serial == 0 {
            return if cx.card_gains_block(q.card) { Dec::int(amount(cx, &me) as i64) } else { Dec::ZERO };
        }
        if !claimed {
            return Dec::ZERO;
        }
        Dec::int(amount(cx, &me) as i64)
    }
    fn after_modifying_block_amount(&self, cx: &mut Combat, me: Me, _modified: Dec, card: CardIdx) {
        let x = aux(cx, &me);
        if x & STRETCH_CLAIMED != 0 && card != NO && cx.active_play_serial(card) == (x & 0xFFFF) as u16 {
            cx.set_power_aux(me.owner, me.idx, x | STRETCH_GAVE);
        }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let x = aux(cx, &me);
        if x & STRETCH_CLAIMED == 0 || play.serial != (x & 0xFFFF) as u16 {
            return;
        }
        cx.set_power_aux(me.owner, me.idx, 0);
        let started = cx.play_amount_take(me.idx, play.card).unwrap_or(0);
        if x & STRETCH_GAVE != 0 {
            cx.modify_power_amount(me.owner, me.idx, Dec::int(-(started as i64)), NO, NO);
        }
    }
});
