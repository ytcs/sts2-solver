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

// Havoc: auto-play the top card of the draw pile, exhausting it (`AutoPlayFromDrawPile`). The played card may itself ask
// for a decision (nested play), in which case this card suspends until it has finished.
listener!(Havoc {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, phase: u8) -> Flow {
        if phase == 0 && cx.auto_play_from_draw_pile(1, CardPilePosition::Top, true) == RunResult::Suspended {
            return Flow::Suspend(1);
        }
        Flow::Done
    }
});

// Whirlwind: X-cost AoE attack, X hits (`ResolveEnergyXValue` through `ModifyXValue`).
listener!(Whirlwind {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let n = cx.x_value(p.card);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::AllOpponents).hits(n));
        Flow::Done
    }
});

// Survivor: block, then discard a card from hand (`CardCmd.Discard`: Sly cards auto-play afterwards).
listener!(Survivor {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let b = cx.card_var(p.card, VarKind::Block);
                cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card);
                match cx.ask_hand(ids::card::SURVIVOR, 1, 1, |_, _| true) {
                    Ask::Resolved(cards) => match cards.first() {
                        Some(c) if cx.discard_cards(&[c], 0) == RunResult::Suspended => Flow::Suspend(2),
                        _ => Flow::Done,
                    },
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            1 => {
                if let Some(c) = cx.choice.cards.first() {
                    if cx.discard_cards(&[c], 0) == RunResult::Suspended {
                        return Flow::Suspend(2);
                    }
                }
                Flow::Done
            }
            _ => Flow::Done,
        }
    }
});

// Begone: choose a card in hand; it is transformed into Minion Strike (upgraded iff Begone is).
listener!(Begone {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        let up = cx.cards[p.card as usize].upgrade;
        let go = |cx: &mut Combat, c: CardIdx| {
            cx.transform_cards(&[c], &[Some((ids::card::MINION_STRIKE, up))]);
        };
        match phase {
            0 => match cx.ask_hand(ids::card::BEGONE, 1, 1, |_, _| true) {
                Ask::Resolved(cards) => {
                    if let Some(c) = cards.first() {
                        go(cx, c);
                    }
                    Flow::Done
                }
                Ask::Pending => Flow::Suspend(1),
            },
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    go(cx, c);
                }
                Flow::Done
            }
        }
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

// Battle Trance: draw, then No Draw for the rest of the turn.
listener!(BattleTrance {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        cx.apply_power(ids::power::NO_DRAW_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
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
