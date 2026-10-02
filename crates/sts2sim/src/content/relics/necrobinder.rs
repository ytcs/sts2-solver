//! Necrobinder relics.

use crate::hooks::*;
use crate::listener;
use crate::state::*;

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
