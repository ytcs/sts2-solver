use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Attack, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;

listener!(Whistle {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        cx.stun(p.target, None, None);
        Flow::Done
    }
});

listener!(Rebound {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        cx.apply_power(ids::power::REBOUND_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
        Flow::Done
    }
});
