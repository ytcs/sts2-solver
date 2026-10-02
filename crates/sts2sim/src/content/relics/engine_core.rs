//! Representative relics that exercise engine mechanisms (kept in an `engine_*` file; a content owner may replace them).

use crate::dec::Dec;
use crate::hooks::*;
use crate::listener;
use crate::state::*;

// Lizard Tail: once per run (`WasUsed` saved property, stored in `Relic::counter`), vetoes the owner's death in the LATE
// `ShouldDie` pass and heals `max(1, MaxHp * 50 / 100)` (truncated).
listener!(LizardTail {
    fn should_die_late(&self, cx: &Combat, me: Me, creature: Cid) -> bool {
        creature != PLAYER || cx.player.relics[me.idx as usize].counter != 0
    }
    fn after_preventing_death(&self, cx: &mut Combat, me: Me, creature: Cid) {
        cx.player.relics[me.idx as usize].counter = 1;
        let amount = (Dec::int(cx.cr(creature).max_hp as i64) * (Dec::int(50) * Dec::frac(1, 2))).max(Dec::ONE);
        cx.heal(creature, amount);
    }
});
