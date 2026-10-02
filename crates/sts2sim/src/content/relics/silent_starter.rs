//! Silent starter relic. Own file so it can be dropped if the relics port ships it.

use crate::dec::Dec;
use crate::hooks::*;
use crate::listener;
use crate::state::*;

// +2 cards in the opening hand (first turn only).
listener!(RingOfTheSnake {
    fn modify_hand_draw(&self, cx: &Combat, _me: Me, amount: Dec) -> Dec {
        if cx.player.turn_number > 1 {
            return amount;
        }
        amount + Dec::int(2)
    }
});
