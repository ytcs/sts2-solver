use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

listener!(BoundPhylactery {
    fn before_combat_start(&self, cx: &mut Combat, _me: Me) {
        cx.summon(1);
    }
    fn after_energy_reset_late(&self, cx: &mut Combat, _me: Me) {
        if cx.player.turn_number != 1 {
            cx.summon(1);
        }
    }
});

listener!(PhylacteryUnbound {
    fn before_combat_start(&self, cx: &mut Combat, _me: Me) {
        cx.summon(5);
    }
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player {
            cx.summon(2);
        }
    }
});
