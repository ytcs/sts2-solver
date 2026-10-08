use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Attack, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

listener!(StrikeDefect {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        Flow::Done
    }
});

listener!(DefendDefect {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let block = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(block as i64), ValueProp::MOVE, p.card);
        Flow::Done
    }
});

listener!(Zap {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        cx.channel_orb(ids::orb::LIGHTNING_ORB);
        Flow::Done
    }
});

listener!(Dualcast {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        if !cx.player.orbs.is_empty() {
            cx.evoke_next(false);
            cx.evoke_next(true);
        }
        Flow::Done
    }
});
