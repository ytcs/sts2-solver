use crate::dec::Dec;
use crate::hooks::*;
use crate::listener;
use crate::state::*;

listener!(BurningBlood {
    fn after_combat_victory(&self, cx: &mut Combat, _me: Me) {
        if cx.cr(PLAYER).is_alive() {
            cx.heal(PLAYER, Dec::int(6));
        }
    }
});
