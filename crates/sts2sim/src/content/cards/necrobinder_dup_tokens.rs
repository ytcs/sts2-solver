//! DUPLICATE (stand-in) copies of the Soul / Sweeping Gaze tokens (the colorless/status/token teammate owns the real ones in
//! `cards/tokens.rs`). DELETE THIS FILE when merging that branch (duplicate registrations are a build error).

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
