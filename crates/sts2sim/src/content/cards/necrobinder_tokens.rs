//! Soul / Sweeping Gaze tokens (needed by the Necrobinder cards). The colorless/status/token branch also ports them in
//! `cards/tokens.rs`: whoever merges that branch must keep exactly one `listener!` per class.

use crate::defs::VarKind;
use crate::engine::{Attack, Targeting};
use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

listener!(Soul {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(SweepingGaze {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let osty = cx.living_osty();
        if osty != NO {
            let dmg = cx.card_var(p.card, VarKind::OstyDamage);
            cx.execute_attack(&Attack::from_card(osty, p.card, dmg, Targeting::Random));
        }
        Flow::Done
    }
});
