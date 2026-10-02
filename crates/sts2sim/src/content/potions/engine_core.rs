//! Representative potions that exercise engine mechanisms (kept in an `engine_*` file; a content owner may replace them).

use crate::dec::Dec;
use crate::hooks::*;
use crate::listener;
use crate::state::*;

// Fairy in a Bottle: an Automatic potion that vetoes the owner's death (`ShouldDie`) and, via `AfterPreventingDeath`,
// uses itself to heal `max(MaxHp * 0.3, 1)` (truncated).
listener!(FairyInABottle {
    fn on_use_potion(&self, cx: &mut Combat, _potion: u16, target: crate::state::Cid, _phase: u8) -> Flow {
        let max = Dec::int(cx.cr(target).max_hp as i64) * Dec::frac(3, 1);
        cx.heal(target, max.max(Dec::ONE));
        Flow::Done
    }
    fn should_die(&self, _cx: &Combat, _me: Me, creature: Cid) -> bool {
        creature != PLAYER
    }
    fn after_preventing_death(&self, cx: &mut Combat, me: Me, creature: Cid) {
        cx.use_potion_now(me.idx as usize, creature);
    }
});
