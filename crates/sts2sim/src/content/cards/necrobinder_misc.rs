use crate::engine::calc_with;
use crate::content::gen_cards::var_name;
use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Ask, Attack, HKind, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn apply_self(cx: &mut Combat, power: u16, amount: i32, p: &CardPlay) {
    cx.apply_power(power, PLAYER, Dec::int(amount as i64), PLAYER, p.card);
}

fn attack(cx: &mut Combat, p: &CardPlay, t: Targeting) -> crate::engine::Results {
    let d = cx.card_var(p.card, VarKind::Damage);
    cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, t))
}

listener!(Bury {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p, Targeting::Single(p.target));
        Flow::Done
    }
});

listener!(Reap {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p, Targeting::Single(p.target));
        Flow::Done
    }
});

listener!(Sow {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p, Targeting::AllOpponents);
        Flow::Done
    }
});

listener!(Eradicate {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let x = cx.x_value(p.card);
        let d = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::Single(p.target)).hits(x));
        Flow::Done
    }
});

listener!(DeathMarch {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.hist_count_this_turn(HKind::CardDrawn, |e| e.flags & 1 == 0) as i32;
        let d = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::ExtraDamage) * n;
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::Single(p.target)));
        Flow::Done
    }
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        let _ = target;
        Some(calc_with(cx, card, cx.hist_count_this_turn(HKind::CardDrawn, |e| e.flags & 1 == 0) as i32))
    }
});

listener!(DrainPower {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p, Targeting::Single(p.target));
        let mut list: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
        for &c in cx.player.discard.iter() {
            if cx.is_upgradable(c) {
                list.push(c);
            }
        }
        cx.rng.combat_card_selection.shuffle(list.as_mut_slice());
        let n = cx.card_var(p.card, VarKind::Cards).max(0) as usize;
        for &c in list.iter().take(n) {
            cx.upgrade_in_combat(c);
        }
        Flow::Done
    }
});

listener!(Graveblast {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                attack(cx, p, Targeting::Single(p.target));
                match cx.ask_pile(ids::card::GRAVEBLAST, PileType::Discard, 1, 1, |_, _| true) {
                    Ask::Resolved(cards) => {
                        if let Some(c) = cards.first() {
                            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                        }
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                }
                Flow::Done
            }
        }
    }
});

listener!(Hang {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p, Targeting::Single(p.target));
        let have = cx.power_amount(p.target, ids::power::HANG_POWER);
        let mut num = 2.max(have);
        if have + num > 999_999_999 {
            num = 0.max(999_999_999 - have);
        }
        cx.apply_power(ids::power::HANG_POWER, p.target, Dec::int(num as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Misery {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let mut list: crate::util::ArrayVec<(u16, i32, Cid, i32), MAX_POWERS> = crate::util::ArrayVec::new();
        for pw in cx.cr(p.target).powers.iter() {
            if Combat::power_type_for_amount(pw.id, pw.amount) == PowerType::Debuff {
                list.push((pw.id, pw.amount, pw.applier, pw.amount_on_turn_start));
            }
        }
        let snapshot = list;
        for &(id, amt, _, _) in snapshot.iter() {
            if let Some(inner) = crate::engine::temporary_inner_power(id) {
                if let Some(e) = list.as_mut_slice().iter_mut().find(|e| e.0 == inner) {
                    e.1 += amt;
                }
            }
        }
        attack(cx, p, Targeting::Single(p.target));
        let enemies = cx.hittable_enemies();
        for &e in enemies.iter() {
            if e == p.target {
                continue;
            }
            for &(id, amt, applier, aots) in list.iter() {
                if amt != 0 {
                    let existed = cx.has_power(e, id);
                    if let Some(uid) = cx.apply_power(id, e, Dec::int(amt as i64), applier, p.card) {
                        if !existed {
                            if let Some(i) = cx.power_idx(e, uid) {
                                cx.cr_mut(e).powers[i].amount_on_turn_start = aots;
                            }
                        }
                    }
                }
            }
        }
        Flow::Done
    }
});

listener!(BorrowedTime {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        apply_self(cx, ids::power::BORROWED_TIME_POWER, 1, p);
        Flow::Done
    }
});

listener!(Delay {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let b = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card);
        let e = cx.card_var(p.card, VarKind::Energy);
        apply_self(cx, ids::power::ENERGY_NEXT_TURN_POWER, e, p);
        Flow::Done
    }
});

listener!(Dredge {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let space = MAX_HAND as i32 - cx.player.hand.len() as i32;
                let n = cx.card_var(p.card, VarKind::Cards).min(space);
                if n <= 0 {
                    return Flow::Done;
                }
                match cx.ask_pile(ids::card::DREDGE, PileType::Discard, n as u8, n as u8, |_, _| true) {
                    Ask::Resolved(cards) => {
                        for &c in cards.iter() {
                            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                        }
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                let cards = cx.choice.cards;
                for &c in cards.iter() {
                    cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                }
                Flow::Done
            }
        }
    }
});

listener!(Melancholy {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let b = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card);
        Flow::Done
    }
    fn after_death(&self, cx: &mut Combat, me: Me, _creature: Cid, was_removal_prevented: bool) {
        let c = me.idx as CardIdx;
        if was_removal_prevented || !cx.card_in_combat_pile(c) {
            return;
        }
        let e = cx.card_var(c, VarKind::Energy);
        cx.add_cost_this_combat(c, -e, false);
    }
});

listener!(Putrefy {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_named_var(p.card, var_name::POWER);
        cx.apply_power(ids::power::WEAK_POWER, p.target, Dec::int(n as i64), PLAYER, p.card);
        cx.apply_power(ids::power::VULNERABLE_POWER, p.target, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(SharedFate {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let mine = cx.card_named_var(p.card, var_name::PLAYER_STRENGTH_LOSS);
        let theirs = cx.card_named_var(p.card, var_name::ENEMY_STRENGTH_LOSS);
        apply_self(cx, ids::power::STRENGTH_POWER, -mine, p);
        cx.apply_power(ids::power::STRENGTH_POWER, p.target, Dec::int(-(theirs as i64)), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Transfigure {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => match cx.ask_hand(ids::card::TRANSFIGURE, 1, 1, |_, _| true) {
                Ask::Resolved(cards) => {
                    transfigure(cx, cards.as_slice());
                    Flow::Done
                }
                Ask::Pending => Flow::Suspend(1),
            },
            _ => {
                let _ = p;
                let cards = cx.choice.cards;
                transfigure(cx, cards.as_slice());
                Flow::Done
            }
        }
    }
});

fn transfigure(cx: &mut Combat, cards: &[CardIdx]) {
    for &c in cards {
        if !cx.card_def(c).x_cost && cx.cards[c as usize].cost_base >= 0 {
            cx.add_cost_this_combat(c, 1, false);
        }
        cx.cards[c as usize].base_replay = cx.cards[c as usize].base_replay.saturating_add(1);
    }
}

listener!(Undeath {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let b = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card);
        if let Some(c) = cx.clone_card(p.card) {
            cx.add_generated_card(c, PileType::Discard, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

listener!(Wisp {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        Flow::Done
    }
});

listener!(TheScythe {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let d = 13 + cx.cards[p.card as usize].counter[0] as i32;
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::Single(p.target)));
        let inc = cx.card_named_var(p.card, var_name::INCREASE);
        let c = &mut cx.cards[p.card as usize];
        c.counter[0] = c.counter[0].saturating_add(inc as i16);
        Flow::Done
    }
});

listener!(Cacophony {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let d = cx.card_var(p.card, VarKind::Damage);
        apply_self(cx, ids::power::CACOPHONY_POWER, d, p);
        Flow::Done
    }
});

listener!(GlimpseBeyond {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        if cx.cr(PLAYER).is_alive() {
            let n = cx.card_var(p.card, VarKind::Cards);
            cx.add_souls_to_draw_pile(n, false);
        }
        Flow::Done
    }
});

listener!(LegionOfBone {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        if cx.cr(PLAYER).is_alive() {
            let n = cx.card_var(p.card, VarKind::Summon);
            cx.summon(n);
        }
        Flow::Done
    }
});

listener!(Soulbound {});

listener!(Underworld {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_self(cx, ids::power::UNDERWORLD_POWER, 1, p);
        Flow::Done
    }
});
