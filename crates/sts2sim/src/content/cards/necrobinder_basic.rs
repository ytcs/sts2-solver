//! Necrobinder starter cards.

use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Attack, Targeting};
use crate::hooks::*;
use crate::listener;
use crate::state::*;
use crate::types::*;

listener!(StrikeNecrobinder {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        Flow::Done
    }
});

listener!(DefendNecrobinder {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let block = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(block as i64), ValueProp::MOVE, p.card);
        Flow::Done
    }
});

// Summon 5 (7).
listener!(Bodyguard {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Summon);
        cx.summon(n);
        Flow::Done
    }
});

// Osty attacks: CalculationBase + ExtraDamage x Osty's current HP (0 if Osty is dead -> the card does nothing).
listener!(Unleash {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let osty = cx.living_osty();
        if osty == NO {
            return Flow::Done;
        }
        let dmg = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::ExtraDamage) * cx.cr(osty).hp;
        cx.execute_attack(&Attack::from_card(osty, p.card, dmg, Targeting::Single(p.target)));
        Flow::Done
    }
    // `CalculatedDamageVar.Calculate(target)` read generically (Thrash exhausting this card).
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        let _ = target;
        let osty = cx.living_osty();
        let m = if cx.in_progress && osty != NO { cx.cr(osty).hp as i64 } else { 0 };
        Some(Dec::int(cx.card_var(card, VarKind::CalcBase) as i64 + cx.card_var(card, VarKind::ExtraDamage) as i64 * m))
    }
});
