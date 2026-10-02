//! Powers that potions apply (ported so the potion sweeps can exercise them). Hook bodies follow the decompiled
//! `Models/Powers/*.cs`; powers needing a subsystem the engine lacks (Ambergris/extra turns) are NOT registered
//! here, so using them is flagged as unimplemented at runtime.

use crate::dec::Dec;
use crate::engine::Attack;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// ClarityPower: +1 card draw each turn for `Amount` turns.
listener!(ClarityPower {
    fn modify_hand_draw(&self, _cx: &Combat, me: Me, amount: Dec) -> Dec {
        if me.owner != PLAYER {
            return amount;
        }
        amount + Dec::ONE
    }
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// IntangiblePower: every HP loss is capped at 1; decrements after the enemy turn.
listener!(IntangiblePower {
    fn modify_hp_lost_after_osty(&self, cx: &Combat, me: Me, target: Cid, amount: Dec, _props: ValueProp, _dealer: Cid, _card: CardIdx) -> Dec {
        if !cx.in_progress || target != me.owner || amount < Dec::ONE {
            return amount;
        }
        Dec::ONE
    }
    fn modify_damage_cap(&self, _cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.target != me.owner {
            return Dec::MAX;
        }
        Dec::ONE
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Enemy {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// BufferPower: prevents the next HP loss.
listener!(BufferPower {
    fn modify_hp_lost_after_osty_late(&self, _cx: &Combat, me: Me, target: Cid, amount: Dec, _props: ValueProp, _dealer: Cid, _card: CardIdx) -> Dec {
        if target != me.owner {
            return amount;
        }
        Dec::ZERO
    }
    fn after_modifying_hp_lost_after_osty(&self, cx: &mut Combat, me: Me) {
        cx.decrement_power(me.owner, me.idx);
    }
});

// RitualPower: +Strength at the end of the owner's turn (an enemy-applied ritual skips its first tick).
// `aux` = `_wasJustAppliedByEnemy`.
listener!(RitualPower {
    fn after_applied(&self, cx: &mut Combat, me: Me) {
        if cx.cr(me.owner).side == Side::Enemy {
            if let Some(i) = cx.power_idx(me.owner, me.idx) {
                cx.cr_mut(me.owner).powers[i].aux = 1;
            }
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side != side {
            return;
        }
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        if cx.cr(me.owner).powers[i].aux != 0 {
            cx.cr_mut(me.owner).powers[i].aux = 0;
            return;
        }
        let a = cx.power_amount(me.owner, me.id);
        cx.apply_power(ids::power::STRENGTH_POWER, me.owner, Dec::int(a as i64), me.owner, NO);
    }
});

// RegenPower: heal `Amount` before the owner's turn ends, then decrement.
listener!(RegenPower {
    fn before_side_turn_end_early(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side && !cx.cr(me.owner).is_dead() {
            let a = cx.power_amount(me.owner, me.id);
            cx.heal(me.owner, Dec::int(a as i64));
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// RadiancePower: +1 energy after each energy reset, `Amount` times.
listener!(RadiancePower {
    fn after_energy_reset(&self, cx: &mut Combat, me: Me) {
        if me.owner == PLAYER {
            cx.gain_energy(1);
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// RetainHandPower: the hand is not discarded at end of turn; decrements after the owner's turn.
listener!(RetainHandPower {
    fn should_flush(&self, _cx: &Combat, me: Me) -> bool {
        me.owner != PLAYER
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// DuplicationPower: the next card(s) are played one extra time; expires at end of turn.
listener!(DuplicationPower {
    fn modify_card_play_count(&self, _cx: &Combat, me: Me, _card: CardIdx, _target: Cid, count: i32) -> i32 {
        if me.owner != PLAYER {
            return count;
        }
        count + 1
    }
    fn after_modifying_card_play_count(&self, cx: &mut Combat, me: Me, _card: CardIdx) {
        cx.decrement_power(me.owner, me.idx);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.is_turn_participant(side, me.owner) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// DemisePower: unblockable damage at the end of the owner's turn.
listener!(DemisePower {
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            let a = cx.power_amount(me.owner, me.id);
            cx.damage(&[me.owner], Dec::int(a as i64), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED), NO, NO);
        }
    }
});

// ShrinkPower: the owner's powered attacks deal 30% less; ticks down at the end of its turn (negative = infinite).
listener!(ShrinkPower {
    fn modify_damage_multiplicative(&self, _cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if me.owner != q.dealer || !q.props.is_powered() {
            return Dec::ONE;
        }
        Dec::frac(7, 1) // (100 - 30) / 100
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.is_turn_participant(side, me.owner) && cx.power_amount(me.owner, me.id) >= 0 {
            cx.decrement_power(me.owner, me.idx);
        }
    }
    fn after_death(&self, cx: &mut Combat, me: Me, creature: Cid, was_removal_prevented: bool) {
        let applier = cx.cr(me.owner).power(me.id).map_or(NO, |p| p.applier);
        if !was_removal_prevented && creature == applier {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// GigantificationPower: the next powered card attack deals triple damage. `aux` = (attack source card + 1) while in flight.
listener!(GigantificationPower {
    fn before_attack(&self, cx: &mut Combat, me: Me, attack: &Attack) {
        if attack.card == NO || me.owner != PLAYER || !attack.props.is_powered() {
            return;
        }
        if cx.card_def(attack.card).ctype != CardType::Attack {
            return;
        }
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        if cx.cr(me.owner).powers[i].aux == 0 {
            cx.cr_mut(me.owner).powers[i].aux = attack.card as i32 + 1;
        }
    }
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.card == NO || me.owner != PLAYER || !q.props.is_powered() {
            return Dec::ONE;
        }
        let aux = cx.cr(me.owner).powers.iter().find(|p| p.uid == me.idx).map_or(0, |p| p.aux);
        if aux == 0 || q.card as i32 + 1 == aux {
            return Dec::int(3);
        }
        Dec::ONE
    }
    fn after_attack(&self, cx: &mut Combat, me: Me, attack: &Attack) {
        let Some(i) = cx.power_idx(me.owner, me.idx) else { return };
        let aux = cx.cr(me.owner).powers[i].aux;
        if aux != 0 && attack.card != NO && attack.card as i32 + 1 == aux {
            cx.cr_mut(me.owner).powers[i].aux = 0;
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// FocusPower: scales orb values; orbs are not in the engine yet, so there is nothing to hook.
listener!(FocusPower {});

// DoomPower: creatures whose HP is <= Doom are killed at the end of their side's turn (enemies: BeforeSideTurnEnd, the
// player side: AfterSideTurnEnd). Only the first doomed creature on the side triggers the kill of all of them.
// TODO(fidelity): `Hook.AfterDiedToDoom` (BookRepairKnife) is not dispatched yet.
fn doomed_on(cx: &Combat, side: Side) -> crate::util::ArrayVec<Cid, MAX_CREATURES> {
    let mut v = crate::util::ArrayVec::new();
    for &c in cx.creatures_on(side).iter() {
        let amount = cx.power_amount(c, ids::power::DOOM_POWER);
        if amount > 0 && cx.cr(c).hp <= amount {
            v.push(c);
        }
    }
    v
}

fn doom_trigger(cx: &mut Combat, owner: Cid, side: Side) {
    if cx.is_over_or_ending() || cx.cr(owner).side != side || cx.cr(owner).is_dead() {
        return;
    }
    let doomed = doomed_on(cx, side);
    if doomed.first() != Some(owner) {
        return;
    }
    for &c in doomed.iter() {
        cx.kill(&[c]);
    }
}

listener!(DoomPower {
    fn before_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side != Side::Player {
            doom_trigger(cx, me.owner, side);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side != Side::Enemy {
            doom_trigger(cx, me.owner, side);
        }
    }
});
