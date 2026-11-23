use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn amount(cx: &Combat, me: &Me) -> i32 {
    cx.my_amount(me)
}

/// `MotifPower` / `LostLutePower`: song tags of `play` for which it is this turn's only started play with the tag
/// (`CardPlaysStarted.Count(...) == 1`).
fn newly_started_melodies(cx: &Combat, play: &CardPlay) -> i32 {
    let tags = cx.card_def(play.card).tags;
    let mut n = 0;
    for t in [tag::WAH, tag::CHO, tag::KI] {
        if tags & t != 0 && cx.plays_this_turn(|e| e.actor == PLAYER && cx.card_def(e.card).tags & t != 0) == 1 {
            n += 1;
        }
    }
    n
}

fn all_melodies_this_turn(cx: &Combat) -> bool {
    cx.melody_played_this_turn(tag::CHO) && cx.melody_played_this_turn(tag::WAH) && cx.melody_played_this_turn(tag::KI)
}

listener!(MotifPower {
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if me.owner != PLAYER {
            return;
        }
        let n = newly_started_melodies(cx, play);
        if n > 0 {
            let a = amount(cx, &me);
            cx.draw_cards(n * a, false);
        }
    }
});

listener!(LostLutePower {
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if me.owner != PLAYER {
            return;
        }
        let n = newly_started_melodies(cx, play);
        if n > 0 {
            let a = amount(cx, &me);
            let _ = cx.auto_play_from_draw_pile(n * a, CardPilePosition::Top, false);
        }
    }
});

listener!(SongPathPower {
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if me.owner == PLAYER && cx.card_def(play.card).tags & tag::CHO != 0 && cx.melody_is_first_play_this_turn(play, tag::CHO) {
            let a = amount(cx, &me);
            cx.gain_block(me.owner, Dec::int(a as i64), ValueProp::UNPOWERED, NO);
        }
    }
});

listener!(ViolentChordPower {
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if me.owner == PLAYER && cx.card_def(play.card).tags & tag::WAH != 0 && cx.melody_is_first_play_this_turn(play, tag::WAH) {
            let a = amount(cx, &me);
            let targets = cx.hittable_enemies();
            cx.damage(targets.as_slice(), Dec::int(a as i64), ValueProp::UNPOWERED, me.owner, NO);
        }
    }
});

listener!(TunePower {
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side != Side::Enemy && cx.melody_played_this_turn(tag::KI) {
            let a = amount(cx, &me);
            cx.apply_power(ids::power::FREE_CARD_POWER, me.owner, Dec::int(a as i64), me.owner, NO);
        }
    }
});

fn free_card_applies(cx: &Combat, me: &Me, card: CardIdx) -> bool {
    me.owner == PLAYER && matches!(cx.card_pile_type(card), PileType::Hand | PileType::Play)
}

listener!(FreeCardPower {
    fn try_modify_energy_cost_in_combat_late(&self, cx: &Combat, me: Me, card: CardIdx, _cost: Dec) -> Option<Dec> {
        if free_card_applies(cx, &me, card) { Some(Dec::ZERO) } else { None }
    }
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if free_card_applies(cx, &me, play.card) {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// aux = `_proccdThisTurn`.
listener!(LullabyPower {
    fn after_applied(&self, cx: &mut Combat, me: Me) {
        if all_melodies_this_turn(cx) {
            cx.set_power_aux(me.owner, me.idx, 1);
        }
    }
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.is_turn_participant(side, me.owner) {
            cx.set_power_aux(me.owner, me.idx, 0);
        }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        if me.owner != PLAYER || !cx.in_progress || cx.my_aux(me) != 0 || !all_melodies_this_turn(cx) {
            return;
        }
        cx.set_power_aux(me.owner, me.idx, 1);
        let a = amount(cx, &me);
        cx.gain_block(me.owner, Dec::int(a as i64), ValueProp::UNPOWERED, NO);
    }
});

// Multiplayer-only: it frees and counts other players' cards, so single-player it never acts.
listener!(EnsemblePower {
    fn try_modify_energy_cost_in_combat_late(&self, cx: &Combat, me: Me, card: CardIdx, _cost: Dec) -> Option<Dec> {
        let skip = me.owner == PLAYER || !matches!(cx.card_pile_type(card), PileType::Hand | PileType::Play);
        if skip { None } else { Some(Dec::ZERO) }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, _play: &CardPlay) {
        if me.owner != PLAYER {
            cx.decrement_power(me.owner, me.idx);
        }
    }
});

// aux = `numberOfCardsPlayedSinceApplied`; `cardsBeingPlayed` = this power's entries in `hist.play_amounts`.
listener!(EncorePower {
    fn before_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if me.owner == PLAYER {
            cx.play_amount_add(me.idx, play.card, 0);
        }
    }
    fn should_play(&self, cx: &Combat, me: Me, _card: CardIdx) -> bool {
        if me.owner != PLAYER {
            return true;
        }
        let being_played = cx.hist.play_amounts.iter().filter(|e| e.uid == me.idx).count() as i32;
        cx.my_aux(me) + being_played < amount(cx, &me)
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if me.owner != PLAYER || cx.play_amount_take(me.idx, play.card).is_none() {
            return;
        }
        let n = cx.my_aux(me);
        cx.set_power_aux(me.owner, me.idx, n + 1);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.is_turn_participant(side, me.owner) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

listener!(KiVersePower {
    fn modify_card_play_count(&self, cx: &Combat, me: Me, card: CardIdx, _target: Cid, count: i32) -> i32 {
        if me.owner != PLAYER || cx.card_def(card).tags & tag::MELODY == 0 {
            return count;
        }
        count + 1
    }
    fn after_modifying_card_play_count(&self, cx: &mut Combat, me: Me, _card: CardIdx) {
        cx.decrement_power(me.owner, me.idx);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.is_turn_participant(side, me.owner) {
            cx.remove_power(me.owner, me.idx);
        }
    }
});

// `TemporaryThornsPower`: Thorns until the owner's next turn (removed at the end of the other side's turn).
listener!(ChoBarbsPower {
    fn before_applied(&self, cx: &mut Combat, _me: Me, target: Cid, amount: Dec, applier: Cid, card: CardIdx) {
        cx.temp_before_applied(ids::power::THORNS_POWER, 1, target, amount, applier, card);
    }
    fn after_power_amount_changed(&self, cx: &mut Combat, me: Me, ch: &PowerChange) {
        cx.temp_after_amount_changed(me, ids::power::THORNS_POWER, 1, ch);
    }
    fn after_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if cx.cr(me.owner).side == side {
            return;
        }
        let a = amount(cx, &me);
        cx.remove_power(me.owner, me.idx);
        let thorns = cx.cr(me.owner).powers.iter().find(|p| p.id == ids::power::THORNS_POWER).map(|p| p.uid);
        if let Some(uid) = thorns {
            cx.modify_power_amount(me.owner, uid, Dec::int(-(a as i64)), me.owner, NO);
        }
    }
});

