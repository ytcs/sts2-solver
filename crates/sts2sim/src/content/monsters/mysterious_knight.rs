use super::ovg_util::*;
use crate::defs::*;
use crate::ids;

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
    rand("RAND", &[Branch::new(0).cannot_repeat(), Branch::new(1).max_repeats(2), Branch::new(2).max_repeats(2)]),
];

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
