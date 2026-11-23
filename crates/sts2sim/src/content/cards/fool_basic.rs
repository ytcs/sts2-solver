use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Attack, Targeting};
use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

listener!(StrikeFool {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        Flow::Done
    }
});

listener!(DefendFool {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let block = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(block as i64), ValueProp::MOVE, p.card);
        Flow::Done
    }
});

listener!(FoolishShot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.shoot_arrow_attack(p.target, &Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)).from_arrow());
        Flow::Done
    }
});

listener!(Joy {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(Sadness {});

// Ported with the engine core to validate arrows, melodies and Stretch against the oracle.
listener!(Quiver {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        for _ in 0..cx.card_var(p.card, VarKind::Repeat) {
            cx.gain_arrow_slot();
        }
        Flow::Done
    }
});

listener!(KiHop {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let was_impaled = cx.is_impaled(p.target);
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        if was_impaled {
            let n = cx.card_var(p.card, VarKind::Cards);
            cx.draw_cards(n, false);
        }
        Flow::Done
    }
});

listener!(ChoFirm {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let gains = if cx.has_any_arrows() { 2 } else { 1 };
        let block = cx.card_var(p.card, VarKind::Block);
        for _ in 0..gains {
            cx.gain_block(PLAYER, Dec::int(block as i64), ValueProp::MOVE, p.card);
        }
        Flow::Done
    }
});

listener!(DeepStretch {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let s = cx.card_power_var(p.card, crate::ids::power::STRETCH_POWER);
        cx.apply_power(crate::ids::power::STRETCH_POWER, PLAYER, Dec::int(s as i64), PLAYER, p.card);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});
