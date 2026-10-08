use crate::defs::VarKind;
use crate::engine::{Attack, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;

listener!(Shiv {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let targeting = if cx.has_power(PLAYER, ids::power::FAN_OF_KNIVES_POWER) { Targeting::AllOpponents } else { Targeting::Single(p.target) };
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, targeting));
        Flow::Done
    }
});
