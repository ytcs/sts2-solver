//! Relics whose hooks raise player decisions (hand / option screens) or auto-play cards. Their hooks suspend through
//! `Combat::suspend_hook_for_decision` / `Combat::pending_hook` and continue in `hook_resume` (the turn start that raised
//! them resumes afterwards, see `Combat::run_turn_start`).

use crate::content::gen_pools;
use crate::content::gen_relics as g;
use crate::dec::Dec;
use crate::engine::Ask;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

// ---- Gambling Chip: discard any cards from the opening hand, draw as many ------------------------------------------------------

fn gambling_chip_finish(cx: &mut Combat, cards: &ArrayVec<CardIdx, 16>) {
    // CardCmd.DiscardAndDraw: every discard (with its hooks), then the draw.
    if cx.is_over_or_ending() || cards.is_empty() {
        return;
    }
    for &c in cards.iter() {
        cx.discard_card(c);
    }
    cx.draw_cards(cards.len() as i32, false);
}
listener!(GamblingChip {
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        if cx.turn_number() > 1 {
            return;
        }
        // CardSelectorPrefs(prompt, 0, 999999999)
        match cx.ask_hand(ids::relic::GAMBLING_CHIP, 0, u8::MAX, |_, _| true) {
            Ask::Resolved(cards) => gambling_chip_finish(cx, &cards),
            Ask::Pending => cx.suspend_hook_for_decision(me, 0),
        }
    }
    fn hook_resume(&self, cx: &mut Combat, _me: Me, _phase: u8) {
        let cards = cx.choice.cards;
        gambling_chip_finish(cx, &cards);
    }
});

// ---- Toasty Mittens: exhaust a card from the hand every turn, then +Strength ----------------------------------------------------

fn toasty_mittens_finish(cx: &mut Combat, cards: &ArrayVec<CardIdx, 16>) {
    for &c in cards.iter() {
        cx.exhaust_card(c, false);
    }
    cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(g::toasty_mittens::STRENGTH_POWER as i64), PLAYER, NO);
}
listener!(ToastyMittens {
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        match cx.ask_hand(ids::relic::TOASTY_MITTENS, 1, 1, |_, _| true) {
            Ask::Resolved(cards) => toasty_mittens_finish(cx, &cards),
            Ask::Pending => cx.suspend_hook_for_decision(me, 0),
        }
    }
    fn hook_resume(&self, cx: &mut Combat, _me: Me, _phase: u8) {
        let cards = cx.choice.cards;
        toasty_mittens_finish(cx, &cards);
    }
});

// ---- Toolbox: pick one of three colorless cards before the first draw ---------------------------------------------------------------

fn toolbox_finish(cx: &mut Combat, cards: &ArrayVec<CardIdx, 16>) {
    if let Some(&c) = cards.first().as_ref() {
        cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
    }
}
listener!(Toolbox {
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {
        if cx.turn_number() != 1 {
            return;
        }
        let cards = cx.get_distinct_for_combat(&gen_pools::COLORLESS, g::toolbox::CARDS as usize, |_| true);
        // `FromChooseACardScreen(...)`: the oracle's selector may also pick nothing (min 0), like Discovery.
        match cx.ask_options(ids::relic::TOOLBOX, cards.as_slice(), true) {
            Ask::Resolved(picked) => toolbox_finish(cx, &picked),
            Ask::Pending => cx.suspend_hook_for_decision(me, 0),
        }
    }
    fn hook_resume(&self, cx: &mut Combat, _me: Me, _phase: u8) {
        let cards = cx.choice.cards;
        toolbox_finish(cx, &cards);
    }
});

// ---- Choices Paradox: pick one of five Retain cards of the character's pool --------------------------------------------------------

fn choices_paradox_finish(cx: &mut Combat, cards: &ArrayVec<CardIdx, 16>) {
    for &c in cards.iter() {
        cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
    }
}
listener!(ChoicesParadox {
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        if cx.turn_number() != 1 {
            return;
        }
        let pool = cx.character_pool();
        let cards = cx.get_distinct_for_combat(pool, g::choices_paradox::CARDS as usize, |_| true);
        if cards.is_empty() {
            return;
        }
        for &c in cards.iter() {
            cx.apply_keyword(c, kw::RETAIN);
        }
        match cx.ask_options(ids::relic::CHOICES_PARADOX, cards.as_slice(), false) {
            Ask::Resolved(picked) => choices_paradox_finish(cx, &picked),
            Ask::Pending => cx.suspend_hook_for_decision(me, 0),
        }
    }
    fn hook_resume(&self, cx: &mut Combat, _me: Me, _phase: u8) {
        let cards = cx.choice.cards;
        choices_paradox_finish(cx, &cards);
    }
});

// ---- Whispering Earring: the first hand is played automatically (up to 13 cards) ---------------------------------------------------

const EARRING_MAX_CARDS: i32 = 13;

/// The `AfterAutoPrePlayPhaseEnteredLate` loop: `played` cards done so far. A card that suspends (decision) leaves the
/// loop parked in `pending_hook` with `phase = played`; `hook_resume` continues with the next card.
fn earring_loop(cx: &mut Combat, me: Me, mut played: i32) {
    if cx.turn_number() > 1 {
        return;
    }
    while played < EARRING_MAX_CARDS {
        if cx.is_over_or_ending() || cx.player.turn_number != 1 {
            break;
        }
        let hand = cx.player.hand;
        let Some(card) = hand.iter().copied().find(|&c| cx.can_play(c)) else { break };
        let target = match cx.card_def(card).target {
            TargetType::AnyEnemy => cx.hittable_enemies().first().unwrap_or(NO),
            TargetType::AnyPlayer => PLAYER,
            _ => NO,
        };
        cx.spend_resources(card);
        cx.pending_hook = Some(PendingHook { me, phase: played as u8 });
        cx.auto_play_card(card, target, true);
        if cx.stage == Stage::AwaitChoice {
            return;
        }
        cx.pending_hook = None;
        played += 1;
    }
}
listener!(WhisperingEarring {
    fn modify_max_energy(&self, _cx: &Combat, _me: Me, amount: Dec) -> Dec {
        amount + Dec::int(g::whispering_earring::ENERGY as i64)
    }
    fn after_auto_pre_play_phase_entered_late(&self, cx: &mut Combat, me: Me) {
        earring_loop(cx, me, 0);
    }
    fn hook_resume(&self, cx: &mut Combat, me: Me, phase: u8) {
        earring_loop(cx, me, phase as i32 + 1);
    }
});

// ---- History Course: replay (as a dupe) the last Attack of the previous turn at the start of each turn ----------------------------

// counter = last Attack played this turn + 1 (0 = none), aux = last Attack played last turn + 1 (neither saved).
listener!(HistoryCourse {
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.card_def(play.card).ctype == CardType::Attack && cx.cards[play.card as usize].flags & cflag::IS_DUPE == 0 {
            cx.rel_mut(me).counter = play.card as i32 + 1;
        }
    }
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            let r = cx.rel_mut(me);
            r.aux = r.counter;
            r.counter = 0;
        }
    }
    fn after_auto_pre_play_phase_entered(&self, cx: &mut Combat, me: Me) {
        if cx.turn_number() == 1 || cx.rel(me).aux == 0 {
            return;
        }
        let src = (cx.rel(me).aux - 1) as CardIdx;
        if let Some(c) = cx.create_dupe(src) {
            cx.pending_hook = Some(PendingHook { me, phase: 0 });
            cx.auto_play_card(c, NO, false);
            if cx.stage != Stage::AwaitChoice {
                cx.pending_hook = None;
            }
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        let r = cx.rel_mut(me);
        r.counter = 0;
        r.aux = 0;
    }
});
