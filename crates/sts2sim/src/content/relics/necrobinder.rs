//! Necrobinder relics.

use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

// Starter relic: summons Osty (1) at combat start and again at the start of every turn but the first, after the energy
// reset (late, so effects that check Osty's existence run before the re-summon).
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

// Upgraded starter relic: summons 5 at combat start and 2 at the start of each of your turns (`participants.Contains(Owner)`).
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
