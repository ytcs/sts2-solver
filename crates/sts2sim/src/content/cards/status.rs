//! STATUS pool cards (Burn, Dazed, Wound, Slimed, Void, ...). Turn-end-in-hand effects run through
//! `Listener::on_turn_end_in_hand` (called by `do_turn_end`).

use crate::dec::Dec;
use crate::defs::VarKind;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// 6 unblockable HP loss at turn end if in hand.
listener!(Beckon {
    fn on_turn_end_in_hand(&self, cx: &mut Combat, card: CardIdx) {
        let n = cx.card_var(card, VarKind::HpLoss);
        cx.self_damage_from_card(card, Dec::int(n as i64), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED).or(ValueProp::MOVE));
    }
});

// `CreatureCmd.Damage(owner, DynamicVars.Damage, card)`: 2 blockable unpowered damage.
listener!(Burn {
    fn on_turn_end_in_hand(&self, cx: &mut Combat, card: CardIdx) {
        let n = cx.card_var(card, VarKind::Damage);
        let props = cx.card_var_props(card, VarKind::Damage);
        cx.self_damage_from_card(card, Dec::int(n as i64), props);
    }
});

listener!(Dazed {});

// OnPlay does nothing.
listener!(Debris {});

// Sandpit (The Insatiable) escape card: +1 to the Sandpit counter, then the card costs 1 more for the combat.
listener!(FranticEscape {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let sandpit = cx.enemies.iter().copied().find(|&e| cx.has_power(e, ids::power::SANDPIT_POWER));
        if let Some(e) = sandpit {
            // The Sandpit power's `Target` is the (single) player; instances are per applier.
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

// `FakeUpgrade()` (Aeonglass) adds +3 damage per level; the level lives in `card.counter[0]`.
listener!(Wither {
    fn on_turn_end_in_hand(&self, cx: &mut Combat, card: CardIdx) {
        let n = cx.card_var(card, VarKind::Damage) + 3 * cx.cards[card as usize].counter[0] as i32;
        let props = cx.card_var_props(card, VarKind::Damage);
        cx.self_damage_from_card(card, Dec::int(n as i64), props);
    }
});

/// `Wither.FakeUpgrade()`.
pub fn wither_fake_upgrade(cx: &mut Combat, card: CardIdx) {
    cx.cards[card as usize].counter[0] += 1;
}

// Draw `Cards` (1) card.
listener!(Slimed {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_then_done(n)
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

// Drawing it costs `Energy` (1) energy.
listener!(Void {
    fn after_card_drawn(&self, cx: &mut Combat, me: Me, card: CardIdx, _from_hand_draw: bool) {
        if card as u16 == me.idx {
            let n = cx.card_var(card, VarKind::Energy);
            cx.lose_energy(n);
        }
    }
});

listener!(Wound {});
