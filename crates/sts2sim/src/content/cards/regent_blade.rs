//! Sovereign Blade (Forge token). Growth lives in `counter[0]` (added damage), repeats override in `counter[1]`
//! (`SetRepeats`; 0 = the canonical `RepeatVar`). Spec 05 §8, `Models/Cards/SovereignBlade.cs`.

use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Attack, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

listener!(SovereignBlade {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let card = p.card;
        let dmg = cx.card_base_damage(card);
        let hits = match cx.cards[card as usize].counter[1] {
            0 => cx.card_var(card, VarKind::Repeat),
            n => n as i32,
        };
        let t = if cx.has_power(PLAYER, ids::power::SEEKING_EDGE_POWER) { Targeting::AllOpponents } else { Targeting::Single(p.target) };
        cx.execute_attack(&Attack::from_card(PLAYER, card, dmg, t).hits(hits));
        // Parry: CalculatedBlock = CalculationBase (0) + CalculationExtra (1) * Parry amount.
        let parry = cx.power_amount(PLAYER, ids::power::PARRY_POWER);
        if parry > 0 {
            cx.gain_block(PLAYER, Dec::int(parry as i64), ValueProp::MOVE, card);
        }
        Flow::Done
    }
});
