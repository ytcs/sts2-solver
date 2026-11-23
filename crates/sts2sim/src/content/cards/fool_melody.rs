use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Ask, Attack, RunResult, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn melody_block(cx: &mut Combat, p: &CardPlay) {
    let b = cx.card_var(p.card, VarKind::Block);
    cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card);
}

fn attack_single(cx: &mut Combat, p: &CardPlay, hits: i32) {
    let dmg = cx.card_var(p.card, VarKind::Damage);
    cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)).hits(hits));
}

fn attack_all(cx: &mut Combat, p: &CardPlay, hits: i32) {
    let dmg = cx.card_var(p.card, VarKind::Damage);
    cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::AllOpponents).hits(hits));
}

fn apply_self(cx: &mut Combat, p: &CardPlay, power: u16, amount: i32) {
    cx.apply_power(power, PLAYER, Dec::int(amount as i64), PLAYER, p.card);
}

fn apply_power_var(cx: &mut Combat, p: &CardPlay, power: u16) {
    let n = cx.card_power_var(p.card, power);
    apply_self(cx, p, power, n);
}

listener!(WahBlast {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let twice = cx.melody_played_this_turn(tag::CHO) || cx.melody_played_this_turn(tag::KI);
        attack_all(cx, p, if twice { 2 } else { 1 });
        Flow::Done
    }
});

listener!(WahCho {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        melody_block(cx, p);
        attack_single(cx, p, 1);
        Flow::Done
    }
});

listener!(WahChoKi {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        melody_block(cx, p);
        attack_single(cx, p, 1);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(WahHammer {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p, 1);
        Flow::Done
    }
    fn modify_damage_multiplicative(&self, cx: &Combat, me: Me, q: &DmgQ) -> Dec {
        if q.card != me.idx as CardIdx || q.target == NO || !cx.has_power(q.target, ids::power::IMPALED_POWER) {
            return Dec::ONE;
        }
        Dec::int(2)
    }
});

listener!(WahKi {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let hits = cx.card_var(p.card, VarKind::Repeat);
        attack_single(cx, p, hits);
        let e = cx.card_var(p.card, VarKind::Energy);
        apply_self(cx, p, ids::power::ENERGY_NEXT_TURN_POWER, e);
        let n = cx.card_var(p.card, VarKind::Cards);
        apply_self(cx, p, ids::power::DRAW_CARDS_NEXT_TURN_POWER, n);
        Flow::Done
    }
});

fn melody_cards_in_combat(cx: &Combat, _card: CardIdx, _target: Cid) -> i32 {
    cx.all_combat_cards().iter().filter(|&&c| cx.card_def(c).tags & tag::MELODY != 0).count() as i32
}

listener!(WahSymphony {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.execute_attack(&Attack::from_card_calc(PLAYER, p.card, Targeting::AllOpponents, melody_cards_in_combat));
        Flow::Done
    }
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        Some(crate::engine::calc_with(cx, card, melody_cards_in_combat(cx, card, target)))
    }
});

listener!(WahSweep {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_all(cx, p, 1);
        Flow::Done
    }
});

listener!(WahTouch {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p, 1);
        let n = cx.card_power_var(p.card, ids::power::VULNERABLE_POWER);
        cx.apply_power(ids::power::VULNERABLE_POWER, p.target, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(ChoBarbs {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        melody_block(cx, p);
        apply_power_var(cx, p, ids::power::CHO_BARBS_POWER);
        Flow::Done
    }
});

listener!(ChoChorus {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        if phase != 0 {
            return Flow::Done;
        }
        melody_block(cx, p);
        let mut list: crate::util::ArrayVec<CardIdx, MAX_HAND> = crate::util::ArrayVec::new();
        for &c in cx.player.hand.iter() {
            if cx.card_def(c).tags & (tag::WAH | tag::KI) != 0 {
                list.push(c);
            }
        }
        if cx.auto_play_list(list.as_slice()) == RunResult::Suspended {
            return Flow::Suspend(1);
        }
        Flow::Done
    }
});

listener!(ChoKi {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let stretch = cx.power_amount(PLAYER, ids::power::STRETCH_POWER);
        melody_block(cx, p);
        if stretch > 0 {
            apply_self(cx, p, ids::power::STRETCH_POWER, stretch);
        }
        Flow::Done
    }
});

/// `ChoLimbs._melodyCostReduction`, `Coda._costReductionPerMelody` (private consts).
const MELODY_COST_REDUCTION: i64 = 2;

fn melody_cost_reduction(me: Me, card: CardIdx, cost: Dec, reduction: i64) -> Option<Dec> {
    if card != me.idx as CardIdx || cost <= Dec::ZERO || reduction == 0 {
        return None;
    }
    Some((cost - Dec::int(reduction)).max(Dec::ZERO))
}

listener!(ChoLimbs {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        for _ in 0..2 {
            melody_block(cx, p);
        }
        Flow::Done
    }
    fn try_modify_energy_cost_in_combat(&self, cx: &Combat, me: Me, card: CardIdx, cost: Dec) -> Option<Dec> {
        let on = cx.melody_played_this_turn(tag::WAH) || cx.melody_played_this_turn(tag::KI);
        melody_cost_reduction(me, card, cost, if on { MELODY_COST_REDUCTION } else { 0 })
    }
});

listener!(ChoReady {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        melody_block(cx, p);
        apply_power_var(cx, p, ids::power::AIM_POWER);
        Flow::Done
    }
});

// The top `Cards` of the draw pile are the selection's options; the pick moves only after the answer, so a resume
// re-reads the same top cards.
fn ki_chant_top(cx: &Combat, p: &CardPlay) -> crate::util::ArrayVec<CardIdx, MAX_CARDS> {
    let n = cx.card_var(p.card, VarKind::Cards).max(0) as usize;
    let mut top = crate::util::ArrayVec::new();
    for &c in cx.player.draw.iter().take(n) {
        top.push(c);
    }
    top
}

fn ki_chant_finish(cx: &mut Combat, p: &CardPlay, picked: Option<CardIdx>) -> Flow {
    let top = ki_chant_top(cx, p);
    if let Some(c) = picked {
        cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
    }
    let mut rest: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
    for &c in top.iter() {
        if Some(c) != picked && cx.card_pile_type(c) == PileType::Draw {
            rest.push(c);
        }
    }
    if cx.discard_cards(rest.as_slice(), 0) == RunResult::Suspended {
        return Flow::Suspend(2);
    }
    Flow::Done
}

listener!(KiChant {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let count = cx.card_var(p.card, VarKind::Cards).max(0) as usize;
                if cx.player.draw.is_empty() {
                    cx.shuffle_if_necessary();
                } else if cx.player.draw.len() < count && !cx.player.discard.is_empty() {
                    let already = cx.player.draw;
                    cx.shuffle_discard_into_draw();
                    for &c in already.iter().rev() {
                        if cx.card_pile_type(c) == PileType::Draw {
                            cx.move_card(c, PileType::Draw, CardPilePosition::Top);
                        }
                    }
                }
                let top = ki_chant_top(cx, p);
                if top.is_empty() {
                    return Flow::Done;
                }
                match cx.ask_pile(ids::card::KI_CHANT, PileType::Draw, 1, 1, |_, c| top.contains(c)) {
                    Ask::Resolved(cards) => ki_chant_finish(cx, p, cards.first()),
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            1 => {
                let picked = cx.choice.cards.first();
                ki_chant_finish(cx, p, picked)
            }
            _ => Flow::Done,
        }
    }
});

listener!(KiRest {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        apply_self(cx, p, ids::power::ENERGY_NEXT_TURN_POWER, e);
        if cx.melody_played_this_turn(tag::WAH) || cx.melody_played_this_turn(tag::CHO) {
            apply_self(cx, p, ids::power::RETAIN_HAND_POWER, 1);
        }
        Flow::Done
    }
});

listener!(KiVerse {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::KI_VERSE_POWER);
        Flow::Done
    }
});

listener!(KiWhip {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p, 1);
        let e = cx.card_var(p.card, VarKind::Energy);
        apply_self(cx, p, ids::power::ENERGY_NEXT_TURN_POWER, e);
        Flow::Done
    }
});

listener!(Motif {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::MOTIF_POWER);
        Flow::Done
    }
});

listener!(SongPath {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::SONG_PATH_POWER);
        Flow::Done
    }
});

listener!(Tune {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        apply_self(cx, p, ids::power::TUNE_POWER, e);
        Flow::Done
    }
});

listener!(ViolentChord {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::VIOLENT_CHORD_POWER);
        Flow::Done
    }
});

listener!(Lullaby {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::LULLABY_POWER);
        Flow::Done
    }
});

listener!(Ensemble {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        apply_self(cx, p, ids::power::ENSEMBLE_POWER, n);
        Flow::Done
    }
});

listener!(Encore {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        apply_power_var(cx, p, ids::power::ENCORE_POWER);
        Flow::Done
    }
});

listener!(Coda {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p, 1);
        Flow::Done
    }
    fn try_modify_energy_cost_in_combat(&self, cx: &Combat, me: Me, card: CardIdx, cost: Dec) -> Option<Dec> {
        melody_cost_reduction(me, card, cost, MELODY_COST_REDUCTION * cx.melodies_this_turn() as i64)
    }
});

listener!(RhythmStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let hits = crate::engine::calc_extra_with(cx, p.card, cx.melodies_this_turn()).trunc();
        attack_single(cx, p, hits);
        Flow::Done
    }
    fn calculated_value(&self, cx: &Combat, card: CardIdx, _target: Cid) -> Option<Dec> {
        Some(crate::engine::calc_extra_with(cx, card, cx.melodies_this_turn()))
    }
});

listener!(SteadyCadence {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_single(cx, p, 1);
        let n = crate::engine::calc_extra_with(cx, p.card, cx.melodies_this_turn()).trunc();
        cx.draw_cards(n, false);
        Flow::Done
    }
    fn calculated_value(&self, cx: &Combat, card: CardIdx, _target: Cid) -> Option<Dec> {
        Some(crate::engine::calc_extra_with(cx, card, cx.melodies_this_turn()))
    }
});

listener!(LostLute {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_power_var(cx, p, ids::power::LOST_LUTE_POWER);
        Flow::Done
    }
});
