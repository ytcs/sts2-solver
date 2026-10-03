//! Relics whose hooks raise player decisions (hand / option screens) or auto-play cards. Their hooks suspend through the
//! canonical `Combat::hook_ctx = Some((me, phase))` and continue in `resume_hook` (the turn start that raised them resumes
//! afterwards through `Combat::turn_cont`).

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
    // CardCmd.DiscardAndDraw: every discard (with its hooks), then the draw, then the Sly cards auto-play. A Sly card whose
    // play raises a decision leaves it pending; the turn start stops there (`turn.rs`) and resumes afterwards.
    cx.discard_cards(cards.as_slice(), cards.len() as i32);
}
listener!(GamblingChip {
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        if cx.turn_number() > 1 {
            return;
        }
        // CardSelectorPrefs(prompt, 0, 999999999)
        match cx.ask_hand(ids::relic::GAMBLING_CHIP, 0, u8::MAX, |_, _| true) {
            Ask::Resolved(cards) => gambling_chip_finish(cx, &cards),
            Ask::Pending => {
                cx.hook_ctx = Some((me, 0));
                cx.stage = Stage::AwaitChoice;
            }
        }
    }
    fn resume_hook(&self, cx: &mut Combat, _me: Me, _phase: u8) {
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
            Ask::Pending => {
                cx.hook_ctx = Some((me, 0));
                cx.stage = Stage::AwaitChoice;
            }
        }
    }
    fn resume_hook(&self, cx: &mut Combat, _me: Me, _phase: u8) {
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
            Ask::Pending => {
                cx.hook_ctx = Some((me, 0));
                cx.stage = Stage::AwaitChoice;
            }
        }
    }
    fn resume_hook(&self, cx: &mut Combat, _me: Me, _phase: u8) {
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
            Ask::Pending => {
                cx.hook_ctx = Some((me, 0));
                cx.stage = Stage::AwaitChoice;
            }
        }
    }
    fn resume_hook(&self, cx: &mut Combat, _me: Me, _phase: u8) {
        let cards = cx.choice.cards;
        choices_paradox_finish(cx, &cards);
    }
});

// ---- Whispering Earring: the first hand is played automatically (up to 13 cards) ---------------------------------------------------

const EARRING_MAX_CARDS: i32 = 13;

listener!(WhisperingEarring {
    fn modify_max_energy(&self, _cx: &Combat, _me: Me, amount: Dec) -> Dec {
        amount + Dec::int(g::whispering_earring::ENERGY as i64)
    }
    // Plays the first playable card of the hand (resources spent like a manual play, then `AutoPlay`) until nothing is
    // playable. The relic pushes `VakuuCardSelector` while it does so: card-selection screens resolve to the first
    // candidates (`Combat::auto_select`), so no decision is ever raised.
    fn after_auto_pre_play_phase_entered_late(&self, cx: &mut Combat, _me: Me) {
        if cx.turn_number() > 1 {
            return;
        }
        let was = cx.auto_select;
        cx.auto_select = true;
        let mut played = 0;
        while played < EARRING_MAX_CARDS {
            if cx.is_over_or_ending() || cx.player.turn_number != 1 {
                break;
            }
            let hand = cx.player.hand;
            let Some(card) = hand.iter().copied().find(|&c| cx.can_play(c)) else { break };
            let target = match cx.card_target_type(card) {
                TargetType::AnyEnemy => cx.hittable_enemies().first().unwrap_or(NO),
                TargetType::AnyPlayer => PLAYER,
                _ => NO,
            };
            cx.spend_resources(card);
            let _ = cx.auto_play(card, target, AutoPlayType::Default, true);
            played += 1;
        }
        cx.auto_select = was;
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
            let _ = cx.auto_play(c, NO, AutoPlayType::Default, false);
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        let r = cx.rel_mut(me);
        r.counter = 0;
        r.aux = 0;
    }
});
