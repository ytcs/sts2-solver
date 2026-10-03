//! `TargetType.AnyPlayer` potions that were not covered by the first potion pass (single player: the target is the owner).

use crate::defs::VarKind;
use crate::engine::RunResult;
use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

// `OstyCmd.Summon(player, SummonVar(15))`.
listener!(BoneBrew {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Summon);
        cx.summon(n);
        Flow::Done
    }
});

// `ForgeCmd.Forge(ForgeVar(15), player)`.
listener!(KingsCourage {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Forge);
        cx.forge(n);
        Flow::Done
    }
});

// `AutoPlayFromDrawPile(RepeatVar(3), Top, forceExhaust: false)`; a decision inside one of the plays suspends the potion
// (phase 1 = finished).
listener!(DistilledChaos {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, phase: u8) -> Flow {
        if phase == 0 {
            let n = cx.potion_var(potion, VarKind::Repeat);
            if cx.auto_play_from_draw_pile(n, CardPilePosition::Top, false) == RunResult::Suspended {
                return Flow::Suspend(1);
            }
        }
        Flow::Done
    }
});
