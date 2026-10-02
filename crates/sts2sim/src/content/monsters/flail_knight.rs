//! FlailKnight (Act 2 "Hive" monster, spec 04 §3.3) and its event subclass MysteriousKnight (The Lantern Key event, +6 Strength
//! and +6 Plating on spawn). Ported here because the event encounter needs it; the Hive port should reuse `FLAIL_KNIGHT_DEF`.

use super::ovg_util::*;
use crate::defs::*;
use crate::ids;

// 0 WAR_CHANT, 1 FLAIL_MOVE, 2 RAM_MOVE, 3 RAND; initial RAM
const FLAIL_KNIGHT_NODES: &[MonsterNode] = &[
    mv("WAR_CHANT", |cx, me| power_self(cx, me, ids::power::STRENGTH_POWER, 3), &[Intent::Buff], 3),
    mv(
        "FLAIL_MOVE",
        |cx, me| {
            let d = a9(cx, 10, 9);
            atk_n(cx, me, d, 2)
        },
        &[multi(|cx, _| a9(cx, 10, 9), |_, _| 2)],
        3,
    ),
    mv(
        "RAM_MOVE",
        |cx, me| {
            let d = a9(cx, 17, 15);
            atk(cx, me, d)
        },
        &[attack(|cx, _| a9(cx, 17, 15))],
        3,
    ),
    rand("RAND", &[Branch::new(0).cannot_repeat(), Branch::new(1).weight(2.0), Branch::new(2).weight(2.0)]),
];

pub static FLAIL_KNIGHT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::FLAIL_KNIGHT,
    hp: |a| hp(a, (108, 108), (101, 101)),
    initial: 2,
    on_spawn: None,
    nodes: FLAIL_KNIGHT_NODES,
};

pub static MYSTERIOUS_KNIGHT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::MYSTERIOUS_KNIGHT,
    hp: |a| hp(a, (108, 108), (101, 101)),
    initial: 2,
    on_spawn: Some(|cx, me| {
        power_self(cx, me, ids::power::STRENGTH_POWER, 6);
        power_self(cx, me, ids::power::PLATING_POWER, 6);
    }),
    nodes: FLAIL_KNIGHT_NODES,
};
