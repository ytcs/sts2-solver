//! Representative powers that exercise engine mechanisms (kept in an `engine_*` file; a content owner may replace them).

use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

// Duplication: the owner's next card is played twice (`ModifyCardPlayCount` +1); consumed in `AfterModifyingCardPlayCount`.
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

// No Draw: cards cannot be drawn (except the turn's hand draw) until the end of this turn.
listener!(NoDrawPower {
    fn should_draw(&self, _cx: &Combat, me: Me, from_hand_draw: bool) -> bool {
        from_hand_draw || me.owner != PLAYER
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.is_turn_participant(side, me.owner) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});
