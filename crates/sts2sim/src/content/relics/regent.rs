//! Regent relics.

use crate::hooks::*;
use crate::listener;
use crate::state::*;

// DivineRight (starter relic): +3 stars when a combat room is entered (`StarsVar(3)`; `AfterRoomEntered`).
listener!(DivineRight {
    fn after_room_entered(&self, cx: &mut Combat, _me: Me) {
        cx.gain_stars(3);
    }
});
