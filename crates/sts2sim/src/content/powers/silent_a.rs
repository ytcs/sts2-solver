//! Powers applied by the first half of the Silent card pool (Poison, Accuracy, Thorns, Afterimage, Burst, ...).

use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

/// `participants.Contains(Owner)` for a side-turn hook: the power owner is on the side whose turn it is.
fn owner_on(cx: &Combat, me: Me, side: Side) -> bool {
    cx.cr(me.owner).side == side
}

// ---- Poison -----------------------------------------------------------------------------------------------------------


// Only read by PoisonPower.
listener!(AccelerantPower {});

// ---- Shiv support ---------------------------------------------------------------------------------------------------------

listener!(AccuracyPower {
    fn modify_damage_additive(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if me.owner != q.dealer || !q.props.is_powered() || q.card == NO {
            return Dec::ZERO;
        }
        if cx.card_def(q.card).tags & tag::SHIV == 0 {
            return Dec::ZERO;
        }
        Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
});

// Only read by Shiv (targeting).
listener!(FanOfKnivesPower {});

// ---- Thorns / Envenom ---------------------------------------------------------------------------------------------------------


listener!(EnvenomPower {
    fn after_damage_given(&self, cx: &mut Combat, me: Me, dealer: Cid, target: Cid, unblocked: i32, props: ValueProp) {
        if dealer == me.owner && props.is_powered() && unblocked > 0 {
            let amt = cx.power_amount(me.owner, me.id);
            cx.apply_power(ids::power::POISON_POWER, target, Dec::int(amt as i64), me.owner, NO);
        }
    }
});

// ---- Afterimage -----------------------------------------------------------------------------------------------------------------

// Remembers (card, power amount) at `BeforeCardPlayed` so only plays that started with the power active pay out at
// `AfterCardPlayed`. `aux` packs up to two nested entries: bits 0..8 card index + 1, bits 8..16 amount (per slot of 16 bits).
fn afterimage_slot(aux: i32, slot: u32) -> (u8, u8) {
    let v = (aux >> (16 * slot)) & 0xFFFF;
    ((v & 0xFF) as u8, ((v >> 8) & 0xFF) as u8)
}
fn afterimage_set(aux: i32, slot: u32, card_plus1: u8, amount: u8) -> i32 {
    let mask = 0xFFFF << (16 * slot);
    (aux & !mask) | (((card_plus1 as i32) | ((amount as i32) << 8)) << (16 * slot))
}

listener!(AfterimagePower {
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        let amount = cx.cr(me.owner).powers[i].amount.clamp(0, 255) as u8;
        let mut aux = cx.cr(me.owner).powers[i].aux;
        for slot in 0..2 {
            if afterimage_slot(aux, slot).0 == 0 {
                aux = afterimage_set(aux, slot, play.card + 1, amount);
                break;
            }
        }
        cx.cr_mut(me.owner).powers[i].aux = aux;
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        let mut aux = cx.cr(me.owner).powers[i].aux;
        for slot in 0..2 {
            let (c, amount) = afterimage_slot(aux, slot);
            if c == play.card + 1 {
                aux = afterimage_set(aux, slot, 0, 0);
                cx.cr_mut(me.owner).powers[i].aux = aux;
                if amount > 0 {
                    cx.gain_block(me.owner, Dec::int(amount as i64), ValueProp::UNPOWERED, NO);
                }
                return;
            }
        }
    }
});

// ---- Temporary Dexterity (TemporaryDexterityPower base class; AnticipatePower is the Silent user) ---------------------------------

listener!(AnticipatePower {
    // BeforeApplied: grant the Dexterity up front (before this power is attached).
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        cx.apply_power(ids::power::DEXTERITY_POWER, target, amount, applier, card);
    }
    // Stacking: mirror the delta into Dexterity (the initial application already matches `Amount`).
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        if ch.power_id == me.id && ch.target == me.owner && ch.amount != cx.power_amount(me.owner, me.id) {
            cx.apply_power(ids::power::DEXTERITY_POWER, me.owner, Dec::int(ch.amount as i64), me.owner, NO);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) {
            let amt = cx.power_amount(me.owner, me.id);
            cx.remove_power(me.owner, me.idx);
            cx.apply_power(ids::power::DEXTERITY_POWER, me.owner, Dec::int(-(amt as i64)), me.owner, NO);
        }
    }
});

// ---- Block / energy / draw ------------------------------------------------------------------------------------------------------

listener!(BlurPower {
    fn should_clear_block(&self, _cx: &Combat, me: Me, creature: Cid) -> bool {
        me.owner != creature
    }
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});




// The next `Amount` Skills are played twice; expires at the end of the turn.
listener!(BurstPower {
    fn modify_card_play_count(&self, cx: &Combat, me: Me, card: CardIdx, _target: Cid, count: i32) -> i32 {
        if me.owner != PLAYER || cx.card_def(card).ctype != CardType::Skill {
            return count;
        }
        count + 1
    }
    fn after_modifying_card_play_count(&self, cx: &mut Combat, me: Me, _card: CardIdx) {
        cx.decrement_power(me.owner, me.idx);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// Every card drawn this turn poisons all hittable enemies.
listener!(CorrosiveWavePower {
    fn after_card_drawn(&self, cx: &mut Combat, me: Me, _card: CardIdx, _from_hand_draw: bool) {
        if me.owner != PLAYER {
            return;
        }
        let amt = cx.power_amount(me.owner, me.id);
        cx.apply_power_to_hittable_enemies(ids::power::POISON_POWER, Dec::int(amt as i64), me.owner, NO);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// ---- Multiplayer leftovers -------------------------------------------------------------------------------------------------------

// Damage multiplier for everyone but the applier; expires at the end of the owner's side turn.
listener!(FlankingPower {
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner || !q.props.is_powered() {
            return Dec::ONE;
        }
        let applier = cx.power_idx(me.owner, me.idx).map_or(NO, |i| cx.cr(me.owner).powers[i].applier);
        if q.dealer == applier {
            return Dec::ONE;
        }
        Dec::int(cx.power_amount(me.owner, me.id) as i64)
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if owner_on(cx, me, side) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});
