// FoolRelicPool (Song Bow: relics/fool.rs).
use crate::content::gen_relics as g;
use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;
use crate::util::ArrayVec;

const TRIGGERED_THIS_TURN: u8 = 1;

fn self_power(cx: &mut Combat, id: u16, n: i32) {
    cx.apply_power(id, PLAYER, Dec::int(n as i64), PLAYER, NO);
}

/// The cards of `UnlockState.CharacterCardPools` (`ModelDb.AllCharacters` order) carrying any tag of `t`.
pub(crate) fn tagged_character_cards(t: u16) -> ArrayVec<u16, 128> {
    use crate::content::gen_pools as gp;
    let mut v = ArrayVec::new();
    for pool in [&gp::IRONCLAD[..], &gp::SILENT[..], &gp::REGENT[..], &gp::NECROBINDER[..], &gp::DEFECT[..], &gp::FOOL[..]] {
        for &id in pool {
            if crate::content::card_def(id).tags & t != 0 {
                v.push(id);
            }
        }
    }
    v
}

listener!(BizarreLamp {
    fn before_hand_draw(&self, cx: &mut Combat, _me: Me) {
        if cx.turn_number() > 1 {
            return;
        }
        let joy = cx.new_card(ids::card::JOY, 0);
        let sad = cx.new_card(ids::card::SADNESS, 0);
        for c in [joy, sad].into_iter().flatten() {
            cx.add_generated_card_by(c, PileType::Hand, CardPilePosition::Bottom, true);
        }
    }
});

// `_triggeredThisTurn` (not saved) = flags bit.
listener!(ConductorsBaton {
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            cx.rel_mut(me).flags &= !TRIGGERED_THIS_TURN;
        }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        if cx.in_progress && cx.rel(me).flags & TRIGGERED_THIS_TURN == 0 && cx.melodies_this_turn() >= 3 {
            cx.rel_mut(me).flags |= TRIGGERED_THIS_TURN;
            cx.apply_power(ids::power::CONDUCTORS_BATON_POWER, PLAYER, Dec::ONE, PLAYER, NO);
        }
    }
});

listener!(ExerciseMat {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() <= 1 {
            self_power(cx, ids::power::STRETCH_POWER, g::exercise_mat::STRETCH_POWER);
        }
    }
});

listener!(Fertilizer {
    fn after_card_generated_for_combat(&self, cx: &mut Combat, _me: Me, card: CardIdx, added_by_player: bool) {
        if added_by_player && cx.cards[card as usize].id == ids::card::SADNESS {
            self_power(cx, ids::power::STRENGTH_POWER, g::fertilizer::STRENGTH_POWER);
        }
    }
});

listener!(Ocarina {
    fn before_hand_draw(&self, cx: &mut Combat, _me: Me) {
        if cx.turn_number() > 1 {
            return;
        }
        let list = tagged_character_cards(tag::MELODY);
        if list.is_empty() {
            return;
        }
        let cards = cx.get_distinct_for_combat(list.as_slice(), 1, |_| true);
        for &c in cards.iter() {
            cx.add_generated_card_by(c, PileType::Hand, CardPilePosition::Bottom, true);
        }
    }
});

listener!(Twine {
    fn after_arrow_dislodged(&self, cx: &mut Combat, _me: Me, _target: Cid, _power_uid: u16, _arrow: u8, _source: CardIdx) {
        cx.gain_block(PLAYER, Dec::int(g::twine::BLOCK as i64), ValueProp::UNPOWERED, NO);
    }
});

listener!(WhittlingKnife {
    fn after_shuffle(&self, cx: &mut Combat, _me: Me) {
        self_power(cx, ids::power::AIM_POWER, g::whittling_knife::AIM_POWER);
    }
});
