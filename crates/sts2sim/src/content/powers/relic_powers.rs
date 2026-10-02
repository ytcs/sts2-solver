//! Powers that exist only for relics: SelfFormingClayPower, HelicalDartPower, ReptileTrinketPower, ConfusedPower
//! (Snecko Eye).

use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// Counter power: gain `Amount` block (unpowered) when the owner's block is cleared next turn, then remove itself.
listener!(SelfFormingClayPower {
    fn after_block_cleared(&self, cx: &mut Combat, me: Me, creature: Cid) {
        if creature == me.owner {
            let amount = cx.power_amount(me.owner, me.id);
            cx.gain_block(me.owner, Dec::int(amount as i64), ValueProp::UNPOWERED, NO);
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// `TemporaryDexterityPower` family: grants Dexterity immediately (BeforeApplied / on every stack change) and takes it back
// at the end of the owner's turn.
fn temp_before_applied(cx: &mut Combat, base: u16, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
    cx.apply_power(base, target, amount, applier, card);
}
fn temp_after_changed(cx: &mut Combat, me: Me, base: u16, power_id: u16, amount: i32) {
    if power_id == me.id && cx.cr(me.owner).power(me.id).map_or(false, |p| p.amount != amount) {
        cx.apply_power(base, me.owner, Dec::int(amount as i64), me.owner, NO);
    }
}
fn temp_end_of_turn(cx: &mut Combat, me: Me, base: u16, side: Side) {
    if cx.cr(me.owner).side == side {
        let amount = cx.power_amount(me.owner, me.id);
        cx.remove_power(me.owner, me.idx);
        cx.apply_power(base, me.owner, Dec::int(-(amount as i64)), me.owner, NO);
    }
}

listener!(HelicalDartPower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        temp_before_applied(cx, ids::power::DEXTERITY_POWER, target, amount, applier, card);
    }
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, power_id: u16, amount: i32) {
        temp_after_changed(cx, me, ids::power::DEXTERITY_POWER, power_id, amount);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        temp_end_of_turn(cx, me, ids::power::DEXTERITY_POWER, side);
    }
});

listener!(ReptileTrinketPower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        temp_before_applied(cx, ids::power::STRENGTH_POWER, target, amount, applier, card);
    }
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, power_id: u16, amount: i32) {
        temp_after_changed(cx, me, ids::power::STRENGTH_POWER, power_id, amount);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        temp_end_of_turn(cx, me, ids::power::STRENGTH_POWER, side);
    }
});

// Every drawn card with a cost gets a random cost 0..=3 for the rest of the combat (`Rng.CombatEnergyCosts.NextInt(4)`).
listener!(ConfusedPower {
    fn after_card_drawn(&self, cx: &mut Combat, _me: Me, card: CardIdx, _from_hand_draw: bool) {
        // `EnergyCost.Canonical < 0`: curses / statuses (an X-cost card's canonical cost is 0).
        let d = cx.card_def(card);
        if !d.x_cost && d.cost < 0 {
            return;
        }
        let cost = cx.rng.combat_energy_costs.next_int(4);
        cx.set_cost_this_combat(card, cost);
    }
});
