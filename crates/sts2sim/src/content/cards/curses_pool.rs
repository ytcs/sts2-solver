//! CURSE pool cards (Ascender's Bane lives in `curses.rs`).

use crate::dec::Dec;
use crate::defs::VarKind;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn in_hand(cx: &Combat, c: CardIdx) -> bool {
    cx.cards[c as usize].pile == PileType::Hand as u8
}

// 13 unblockable HP loss at turn end.
listener!(BadLuck {
    fn on_turn_end_in_hand(&self, cx: &mut Combat, card: CardIdx) {
        let n = cx.card_var(card, VarKind::HpLoss);
        cx.self_damage_from_card(card, Dec::int(n as i64), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED).or(ValueProp::MOVE));
    }
});

listener!(Clumsy {});
listener!(CurseOfTheBell {});

// Lose `min(10, gold)` gold at turn end.
listener!(Debt {
    fn on_turn_end_in_hand(&self, cx: &mut Combat, card: CardIdx) {
        let n = cx.card_var(card, VarKind::Gold).min(cx.gold);
        cx.lose_gold(n);
    }
});

listener!(Decay {
    fn on_turn_end_in_hand(&self, cx: &mut Combat, card: CardIdx) {
        let n = cx.card_var(card, VarKind::Damage);
        let props = cx.card_var_props(card, VarKind::Damage);
        cx.self_damage_from_card(card, Dec::int(n as i64), props);
    }
});

// 1 Weak at turn end; a freshly applied Weak skips its first tick.
listener!(Doubt {
    fn on_turn_end_in_hand(&self, cx: &mut Combat, card: CardIdx) {
        let already = cx.has_power(PLAYER, ids::power::WEAK_POWER);
        let n = cx.card_power_var(card, ids::power::WEAK_POWER);
        if let Some(uid) = cx.apply_power(ids::power::WEAK_POWER, PLAYER, Dec::int(n as i64), NO, card) {
            if !already {
                if let Some(i) = cx.power_idx(PLAYER, uid) {
                    cx.cr_mut(PLAYER).powers[i].skip_next_tick = true;
                }
            }
        }
    }
});

// While in hand, no other card can be played manually (auto-plays and other Enthralleds are exempt).
listener!(Enthralled {
    fn should_play_kind(&self, cx: &Combat, me: Me, card: CardIdx, kind: AutoPlayType) -> bool {
        if !in_hand(cx, me.idx as CardIdx) || kind != AutoPlayType::None {
            return true;
        }
        cx.cards[card as usize].id == ids::card::ENTHRALLED
    }
});

listener!(Folly {});
listener!(Greed {});
// Deck-only `AfterCombatEnd` bookkeeping (CombatsSeen) - nothing happens inside a combat.
listener!(Guilty {});
listener!(Injury {});

// After three cards were played this turn no card in hand can be played.
listener!(Normality {
    fn should_play(&self, cx: &Combat, me: Me, _card: CardIdx) -> bool {
        if !in_hand(cx, me.idx as CardIdx) {
            return true;
        }
        cx.plays_this_turn(|_| true) < 3
    }
    fn calculated_value(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<crate::dec::Dec> {
        let _ = target;
        Some(crate::engine::calc_extra_with(cx, card, (cx.plays_this_turn(|_| true) as i32).min(3)))
    }
});

listener!(PoorSleep {});

// BeforeSideTurnEnd (player side): remember the hand size; OnTurnEndInHand: that much unblockable damage.
listener!(Regret {
    fn before_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side != Side::Player || !in_hand(cx, me.idx as CardIdx) {
            return;
        }
        cx.cards[me.idx as usize].counter[0] = cx.player.hand.len() as i16;
    }
    fn on_turn_end_in_hand(&self, cx: &mut Combat, card: CardIdx) {
        let n = cx.cards[card as usize].counter[0] as i32;
        cx.self_damage_from_card(card, Dec::int(n as i64), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED).or(ValueProp::MOVE));
        cx.cards[card as usize].counter[0] = 0;
    }
});

listener!(Shame {
    fn on_turn_end_in_hand(&self, cx: &mut Combat, card: CardIdx) {
        let already = cx.has_power(PLAYER, ids::power::FRAIL_POWER);
        let n = cx.card_named_var(card, crate::content::gen_cards::var_name::FRAIL);
        if let Some(uid) = cx.apply_power(ids::power::FRAIL_POWER, PLAYER, Dec::int(n as i64), NO, card) {
            if !already {
                if let Some(i) = cx.power_idx(PLAYER, uid) {
                    cx.cr_mut(PLAYER).powers[i].skip_next_tick = true;
                }
            }
        }
    }
});

// Costs 1, exhausts, does nothing.
listener!(SporeMind {});
listener!(Writhe {});
