//! Representative powers that exercise engine mechanisms (kept in an `engine_*` file; a content owner may replace them).

use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

// Rebound: the next card(s) played go to the top of the draw pile instead of the discard pile.
listener!(ReboundPower {
    fn modify_card_play_result_location(&self, _cx: &Combat, me: Me, _card: CardIdx, _is_auto: bool, _energy_spent: i32, loc: CardLocation) -> CardLocation {
        if me.owner != PLAYER || loc.pile != PileType::Discard {
            return loc;
        }
        CardLocation::new(PileType::Draw, CardPilePosition::Top)
    }
    fn after_modifying_card_play_result_location(&self, cx: &mut Combat, me: Me, _card: CardIdx, _loc: CardLocation) {
        if me.owner == PLAYER {
            cx.decrement_power(me.owner, me.idx);
        }
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.is_turn_participant(side, me.owner) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// Ambergris: an extra player turn (spec 01 §9.2) for each stack; consumed in `AfterTakingExtraTurn`. Invisible.
listener!(AmbergrisPower {
    fn should_take_extra_turn(&self, cx: &Combat, me: Me) -> bool {
        cx.power_amount(me.owner, me.id) > 0 && me.owner == PLAYER
    }
    fn after_taking_extra_turn(&self, cx: &mut Combat, me: Me) {
        if me.owner == PLAYER {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// Void Form: after the turn it is played, the first `Amount` cards played each turn cost 0 (energy and stars, applied in
// the LATE cost pass). `Power::aux` = cards played this turn (starts "maxed" so the effect begins next turn).
listener!(VoidFormPower {
    fn initial_power_aux(&self) -> i32 {
        999_999_999
    }
    fn before_power_amount_changed(&self, cx: &mut Combat, me: Me, power_id: u16, _amount: crate::dec::Dec, _target: Cid, _applier: Cid) {
        if power_id == me.id {
            cx.set_power_aux(me.owner, me.idx, 999_999_999);
        }
    }
    fn try_modify_energy_cost_in_combat_late(&self, cx: &Combat, me: Me, card: CardIdx, _cost: crate::dec::Dec) -> Option<crate::dec::Dec> {
        if void_form_skip(cx, me, card) { None } else { Some(crate::dec::Dec::ZERO) }
    }
    fn try_modify_star_cost(&self, cx: &Combat, me: Me, card: CardIdx, _cost: crate::dec::Dec) -> Option<crate::dec::Dec> {
        if void_form_skip(cx, me, card) { None } else { Some(crate::dec::Dec::ZERO) }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if me.owner == PLAYER && !play.is_auto && play.play_index + 1 == play.play_count {
            let v = cx.power_aux(me.owner, me.idx);
            cx.set_power_aux(me.owner, me.idx, v.saturating_add(1));
        }
    }
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.is_turn_participant(side, me.owner) {
            cx.set_power_aux(me.owner, me.idx, 0);
        }
    }
});

fn void_form_skip(cx: &Combat, me: Me, card: CardIdx) -> bool {
    let in_hand_or_play = matches!(cx.card_pile_type(card), PileType::Hand | PileType::Play);
    !in_hand_or_play || cx.power_aux(me.owner, me.idx) >= cx.power_amount(me.owner, me.id)
}
