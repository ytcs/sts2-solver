//! Representative cards that exercise engine mechanisms (kept in an `engine_*` file; a content owner may replace them).

use crate::defs::VarKind;
use crate::engine::{Attack, Targeting};
use crate::hooks::*;
use crate::listener;
use crate::state::*;

// Whistle: damage, then `CreatureCmd.Stun(target)` (the STUNNED interrupt of spec 04 §1.8).
listener!(Whistle {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        cx.stun(p.target, None, None);
        Flow::Done
    }
});
