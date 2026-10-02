//! Representative cards that exercise engine mechanisms (kept in an `engine_*` file; a content owner may replace them).

use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Ask, Attack, RunResult, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// Whistle: damage, then `CreatureCmd.Stun(target)` (the STUNNED interrupt of spec 04 §1.8).
listener!(Whistle {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        cx.stun(p.target, None, None);
        Flow::Done
    }
});

// Minion Strike (token): damage, draw.
listener!(MinionStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

// Rebound (event card): damage, then the next card played returns to the top of the draw pile.
listener!(Rebound {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        cx.apply_power(ids::power::REBOUND_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
        Flow::Done
    }
});

// Normality (curse, unplayable): while it is in hand, no more than 3 cards may be played per turn.
listener!(Normality {
    fn should_play(&self, cx: &Combat, me: Me, _card: CardIdx) -> bool {
        if cx.card_pile_type(me.idx as CardIdx) != PileType::Hand {
            return true;
        }
        cx.plays_this_turn(|_| true) < 3
    }
});
