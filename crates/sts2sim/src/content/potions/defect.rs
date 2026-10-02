//! Defect potions that need the orb subsystem.

use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// Channel one Dark orb per orb slot (the slot count is read once; a full queue keeps evoking its front orb).
listener!(EssenceOfDarkness {
    fn on_use_potion(&self, cx: &mut Combat, _potion: u16, _target: Cid, _phase: u8) -> Flow {
        let n = cx.player.orb_slots;
        for _ in 0..n {
            cx.channel_orb(ids::orb::DARK_ORB);
        }
        Flow::Done
    }
});
