//! Representative potions that exercise engine mechanisms (kept in an `engine_*` file; a content owner may replace them).

use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::NO;

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

// Duplicator: "this turn, your next card is played twice".
listener!(Duplicator {
    fn on_use_potion(&self, cx: &mut Combat, _potion: u16, target: Cid, _phase: u8) -> Flow {
        cx.apply_power(ids::power::DUPLICATION_POWER, target, Dec::ONE, target, NO);
        Flow::Done
    }
});

// Ambergris (event potion): heal 50% max HP, then take an extra turn after this one (AmbergrisPower).
listener!(Ambergris {
    fn on_use_potion(&self, cx: &mut Combat, _potion: u16, target: Cid, _phase: u8) -> Flow {
        let heal = Dec::int(cx.cr(target).max_hp as i64) * Dec::frac(5, 1);
        cx.heal(target, heal);
        if cx.in_progress {
            cx.apply_power(ids::power::AMBERGRIS_POWER, target, Dec::ONE, PLAYER, NO);
        }
        Flow::Done
    }
});
