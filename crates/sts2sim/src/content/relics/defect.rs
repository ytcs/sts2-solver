//! Defect relics.

use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// BeforeSideTurnStart: on the player's first turn channel `Lightning` (DynamicVar = 1) orbs, before energy reset / draw.
listener!(CrackedCore {
    fn before_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.player.turn_number <= 1 {
            cx.channel_orb(ids::orb::LIGHTNING_ORB);
        }
    }
});
