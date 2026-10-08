use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Attack, Results, Targeting};
use crate::hooks::*;
use crate::state::*;
use crate::types::*;

pub fn d(v: i32) -> Dec {
    Dec::int(v as i64)
}

pub fn x_value(cx: &Combat, c: CardIdx) -> i32 {
    cx.x_value(c)
}

pub fn attack(cx: &mut Combat, p: &CardPlay) -> Results {
    let dmg = cx.card_var(p.card, VarKind::Damage);
    cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)))
}

pub fn attack_hits(cx: &mut Combat, p: &CardPlay, hits: i32) -> Results {
    let dmg = cx.card_var(p.card, VarKind::Damage);
    cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)).hits(hits))
}

pub fn attack_all(cx: &mut Combat, p: &CardPlay) -> Results {
    let dmg = cx.card_var(p.card, VarKind::Damage);
    cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::AllOpponents))
}

pub fn block(cx: &mut Combat, p: &CardPlay) {
    let b = cx.card_var(p.card, VarKind::Block);
    cx.gain_block(PLAYER, d(b), ValueProp::MOVE, p.card);
}

pub fn upgraded(cx: &Combat, p: &CardPlay) -> bool {
    cx.cards[p.card as usize].upgrade > 0
}

pub fn apply_self(cx: &mut Combat, p: &CardPlay, power: u16, amount: i32) {
    cx.apply_power(power, PLAYER, d(amount), PLAYER, p.card);
}
