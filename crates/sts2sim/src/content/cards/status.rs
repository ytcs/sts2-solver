use crate::dec::Dec;
use crate::defs::VarKind;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

listener!(Beckon {
    fn on_turn_end_in_hand(&self, cx: &mut Combat, card: CardIdx) {
        let n = cx.card_var(card, VarKind::HpLoss);
        cx.self_damage_from_card(card, Dec::int(n as i64), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED).or(ValueProp::MOVE));
    }
});

listener!(Burn {
    fn on_turn_end_in_hand(&self, cx: &mut Combat, card: CardIdx) {
        let n = cx.card_var(card, VarKind::Damage);
        let props = cx.card_var_props(card, VarKind::Damage);
        cx.self_damage_from_card(card, Dec::int(n as i64), props);
    }
});

listener!(Dazed {});

listener!(Debris {});

listener!(FranticEscape {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let sandpit = cx.enemies.iter().copied().find(|&e| cx.has_power(e, ids::power::SANDPIT_POWER));
        if let Some(e) = sandpit {
            if let Some(pw) = cx.cr(e).powers.iter().find(|pw| pw.id == ids::power::SANDPIT_POWER).copied() {
                cx.modify_power_amount(e, pw.uid, Dec::int(1), e, p.card);
            }
        }
        cx.add_cost_this_combat(p.card, 1, false);
        Flow::Done
    }
});

listener!(Infection {
    fn on_turn_end_in_hand(&self, cx: &mut Combat, card: CardIdx) {
        let n = cx.card_var(card, VarKind::Damage);
        let props = cx.card_var_props(card, VarKind::Damage);
        cx.self_damage_from_card(card, Dec::int(n as i64), props);
    }
});

listener!(Wither {
    fn on_turn_end_in_hand(&self, cx: &mut Combat, card: CardIdx) {
        let n = cx.card_var(card, VarKind::Damage) + 3 * cx.cards[card as usize].counter[0] as i32;
        let props = cx.card_var_props(card, VarKind::Damage);
        cx.self_damage_from_card(card, Dec::int(n as i64), props);
    }
});

pub fn wither_fake_upgrade(cx: &mut Combat, card: CardIdx) {
    cx.cards[card as usize].counter[0] += 1;
}

listener!(Slimed {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(Soot {});

listener!(Toxic {
    fn on_turn_end_in_hand(&self, cx: &mut Combat, card: CardIdx) {
        let n = cx.card_var(card, VarKind::Damage);
        let props = cx.card_var_props(card, VarKind::Damage);
        cx.self_damage_from_card(card, Dec::int(n as i64), props);
    }
});

listener!(Void {
    fn after_card_drawn(&self, cx: &mut Combat, me: Me, card: CardIdx, _from_hand_draw: bool) {
        if card as u16 == me.idx {
            let n = cx.card_var(card, VarKind::Energy);
            cx.lose_energy(n);
        }
    }
});

listener!(Wound {});
