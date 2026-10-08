use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;

listener!(EssenceOfDarkness {
    fn on_use_potion(&self, cx: &mut Combat, _potion: u16, _target: Cid, _phase: u8) -> Flow {
        let n = cx.player.orb_slots;
        for _ in 0..n {
            cx.channel_orb(ids::orb::DARK_ORB);
        }
        Flow::Done
    }
});
