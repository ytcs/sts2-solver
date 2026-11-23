use crate::content::gen_cards::var_name;
use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Ask, Attack, HKind, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn add_sadness(cx: &mut Combat, n: i32) {
    for _ in 0..n {
        if let Some(c) = cx.new_card(ids::card::SADNESS, 0) {
            cx.add_generated_card_by(c, PileType::Hand, CardPilePosition::Bottom, true);
        }
    }
}

fn add_joy(cx: &mut Combat, n: i32) {
    for _ in 0..n {
        if let Some(c) = cx.new_card(ids::card::JOY, 0) {
            cx.add_generated_card_by(c, PileType::Hand, CardPilePosition::Bottom, true);
        }
    }
}

fn apply_self(cx: &mut Combat, p: &CardPlay, power: u16, amount: i32) -> Option<u16> {
    cx.apply_power(power, PLAYER, Dec::int(amount as i64), PLAYER, p.card)
}

fn apply_var(cx: &mut Combat, p: &CardPlay, power: u16) -> Option<u16> {
    let v = cx.card_power_var(p.card, power);
    apply_self(cx, p, power, v)
}

fn single(cx: &mut Combat, p: &CardPlay, dmg: i32) -> crate::engine::Results {
    cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)))
}

fn upgraded(cx: &Combat, c: CardIdx) -> bool {
    cx.cards[c as usize].upgrade > 0
}

fn sadness_in_hand(cx: &Combat) -> crate::util::ArrayVec<CardIdx, MAX_HAND> {
    let mut v = crate::util::ArrayVec::new();
    for &c in cx.player.hand.iter() {
        if cx.cards[c as usize].id == ids::card::SADNESS {
            v.push(c);
        }
    }
    v
}

fn calc_damage(cx: &Combat, card: CardIdx, mult: i32) -> Dec {
    let m = if cx.in_progress { mult } else { 0 };
    Dec::int(cx.card_var(card, VarKind::CalcBase) as i64 + cx.card_var(card, VarKind::ExtraDamage) as i64 * m as i64)
}

listener!(AllIsWell {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_var(cx, p, ids::power::ALL_IS_WELL_POWER);
        Flow::Done
    }
});

// `CardGeneratedEntry` with `Creator == Owner` and `Card is Sadness`: `HistLog::sadness_generated_by_player`.
fn anguish_mult(cx: &Combat, _c: CardIdx, _t: Cid) -> i32 {
    cx.hist_log.sadness_generated_by_player as i32
}

listener!(Anguish {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.execute_attack(&Attack::from_card_calc(PLAYER, p.card, Targeting::Single(p.target), anguish_mult));
        Flow::Done
    }
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        Some(calc_damage(cx, card, anguish_mult(cx, card, target)))
    }
});

listener!(BlotOutTheSky {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        let pool = cx.character_pool();
        let got = cx.get_distinct_for_combat(pool, 1, |d| d.tags & tag::SHOOT != 0);
        if let Some(c) = got.first() {
            cx.set_to_free_this_turn(c);
            cx.add_generated_card_by(c, PileType::Hand, CardPilePosition::Bottom, true);
        }
        Flow::Done
    }
});

listener!(Bounce {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_var(cx, p, ids::power::STRETCH_POWER);
        Flow::Done
    }
    fn before_hand_draw(&self, cx: &mut Combat, me: Me) {
        let c = me.idx as CardIdx;
        if cx.hist_any_last_player_turn(HKind::CardPlayStarted, |e| e.card == c) && cx.card_pile_type(c) != PileType::Hand {
            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
    }
});

listener!(Brace {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let x = cx.x_value(p.card);
        for _ in 0..x {
            apply_var(cx, p, ids::power::AIM_POWER);
        }
        Flow::Done
    }
});

listener!(BrightenUp {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let s = cx.card_named_var(p.card, var_name::SADNESS);
        add_sadness(cx, s);
        let j = cx.card_named_var(p.card, var_name::JOY);
        add_joy(cx, j);
        Flow::Done
    }
});

listener!(Compost {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        apply_self(cx, p, ids::power::COMPOST_POWER, e);
        Flow::Done
    }
});

listener!(Ecstasy {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        apply_self(cx, p, ids::power::ECSTASY_POWER, n);
        Flow::Done
    }
});

listener!(Ennui {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        if let Some(uid) = apply_var(cx, p, ids::power::ENNUI_POWER) {
            let a = cx.power_aux(PLAYER, uid);
            cx.set_power_aux(PLAYER, uid, a + 1);
        }
        Flow::Done
    }
});

listener!(Enrapture {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::VIGOR_POWER);
        cx.apply_power(ids::power::VIGOR_POWER, p.target, Dec::int(v as i64), PLAYER, p.card);
        let s = cx.card_power_var(p.card, ids::power::STRETCH_POWER);
        cx.apply_power(ids::power::STRETCH_POWER, p.target, Dec::int(s as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Florescence {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let mut list: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
        for &c in cx.all_cards().iter() {
            if cx.card_def(c).tags & tag::SHOOT != 0 && cx.card_pile_type(c) != PileType::Exhaust {
                list.push(c);
            }
        }
        for &c in list.iter() {
            cx.exhaust_card(c, false);
        }
        if let Some(shot) = cx.new_card(ids::card::THE_SHOT, 0) {
            if upgraded(cx, p.card) {
                cx.upgrade_in_combat(shot);
            }
            let k = &mut cx.cards[shot as usize];
            k.counter[0] = k.counter[0].saturating_add(list.len() as i16);
            cx.add_generated_card_by(shot, PileType::Hand, CardPilePosition::Bottom, true);
        }
        Flow::Done
    }
});

// counter[0] = `CardsExhaustedByFlorescence`.
fn the_shot_mult(cx: &Combat, c: CardIdx, _t: Cid) -> i32 {
    cx.cards[c as usize].counter[0] as i32
}

listener!(TheShot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.shoot_arrow_attack(p.target, &Attack::from_card_calc(PLAYER, p.card, Targeting::Single(p.target), the_shot_mult).from_arrow());
        Flow::Done
    }
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        Some(calc_damage(cx, card, the_shot_mult(cx, card, target)))
    }
});

listener!(Growth {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let list = sadness_in_hand(cx);
        for &c in list.iter() {
            cx.exhaust_card(c, false);
        }
        let extra = cx.card_var(p.card, VarKind::CalcExtra);
        apply_self(cx, p, ids::power::STRETCH_POWER, extra * list.len() as i32);
        Flow::Done
    }
    fn calculated_value(&self, cx: &Combat, card: CardIdx, _target: Cid) -> Option<Dec> {
        Some(crate::engine::calc_extra_with(cx, card, sadness_in_hand(cx).len() as i32))
    }
});

listener!(HarmonicShield {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let b = crate::engine::calc_extra_with(cx, p.card, cx.melodies_this_turn());
        cx.gain_block(PLAYER, b, ValueProp::MOVE, p.card);
        Flow::Done
    }
    fn calculated_value(&self, cx: &Combat, card: CardIdx, _target: Cid) -> Option<Dec> {
        Some(crate::engine::calc_extra_with(cx, card, cx.melodies_this_turn()))
    }
});

listener!(HiddenRoots {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_var(cx, p, ids::power::HIDDEN_ROOTS_POWER);
        Flow::Done
    }
});

listener!(Jest {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let b = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card);
        let n = cx.card_var(p.card, VarKind::Cards);
        add_sadness(cx, n);
        Flow::Done
    }
});

listener!(LockedIn {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, phase: u8) -> Flow {
        let picked = if phase == 0 {
            match cx.ask_pile(ids::card::LOCKED_IN, PileType::Draw, 1, 1, |cx, c| cx.card_def(c).tags & tag::SHOOT != 0) {
                Ask::Pending => return Flow::Suspend(1),
                Ask::Resolved(cards) => cards.first(),
            }
        } else {
            cx.choice.cards.first()
        };
        if let Some(c) = picked {
            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

listener!(MourningBlow {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        single(cx, p, dmg);
        if p.play_index + 1 == p.play_count {
            cx.cards[p.card as usize].flags |= cflag::TRANSFORM_AFTER_PLAY;
        }
        Flow::Done
    }
    fn transform_into_after_play(&self) -> (u16, u8) {
        (ids::card::SADNESS, 0)
    }
    fn get_result_location_for_card_play(&self, _cx: &Combat, _me: Me, _card: CardIdx, base: CardLocation) -> CardLocation {
        if base.pile == PileType::Discard {
            CardLocation::new(PileType::Hand, base.pos)
        } else {
            base
        }
    }
});

listener!(MythicForm {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        if let Some(uid) = apply_self(cx, p, ids::power::MYTHIC_FORM_POWER, 1) {
            if upgraded(cx, p.card) {
                let a = cx.power_aux(PLAYER, uid);
                cx.set_power_aux(PLAYER, uid, a + 1);
            }
        }
        Flow::Done
    }
});

listener!(PunchOfJoy {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        single(cx, p, dmg);
        let mut list = sadness_in_hand(cx);
        if !list.is_empty() {
            cx.stable_shuffle_selection(list.as_mut_slice());
            cx.transform_cards(&list.as_slice()[..1], &[Some((ids::card::JOY, 0))]);
        }
        Flow::Done
    }
});

listener!(Rummage {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        add_sadness(cx, 1);
        Flow::Done
    }
});

listener!(SelfDeprecate {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let s = cx.card_named_var(p.card, var_name::SADNESS);
        add_sadness(cx, s);
        Flow::Done
    }
});

// counter[0] = `IncreasedAim` (`CurrentAim = BaseAim + IncreasedAim`; the upgrade does not change it).
listener!(ShowOff {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let aim = cx.card_named_var(p.card, var_name::BASE_AIM) + cx.cards[p.card as usize].counter[0] as i32;
        apply_self(cx, p, ids::power::AIM_POWER, aim);
        if let Some(uid) = apply_var(cx, p, ids::power::SHOW_OFF_POWER) {
            cx.set_power_aux(PLAYER, uid, p.card as i32 + 1);
        }
        Flow::Done
    }
});

listener!(Simpleton {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_var(cx, p, ids::power::SIMPLETON_POWER);
        Flow::Done
    }
});

listener!(SorrowfulStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        single(cx, p, dmg);
        let n = cx.card_var(p.card, VarKind::Cards);
        add_sadness(cx, n);
        Flow::Done
    }
});

listener!(Timber {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let r = single(cx, p, dmg);
        if r.iter().any(|x| x.killed) {
            let d = cx.card_named_var(p.card, var_name::ON_KILL_DAMAGE);
            cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::AllOpponents));
        }
        Flow::Done
    }
});

listener!(Vitriol {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let w = cx.card_power_var(p.card, ids::power::WEAK_POWER);
        cx.apply_power(ids::power::WEAK_POWER, p.target, Dec::int(w as i64), PLAYER, p.card);
        add_sadness(cx, 1);
        Flow::Done
    }
});

listener!(Yoga {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        if !cx.cr(PLAYER).is_dead() {
            apply_var(cx, p, ids::power::STRETCH_POWER);
        }
        Flow::Done
    }
});
