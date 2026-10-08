use super::ovg_util::*;
use crate::defs::*;
use crate::ids;

pub static BYGONE_EFFIGY_DEF: MonsterDef = MonsterDef {
    id: ids::monster::BYGONE_EFFIGY,
    hp: |a| hp(a, (132, 132), (127, 127)),
    initial: 0,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::SLOW_POWER, 1)),
    nodes: &[
        mv("SLEEP_MOVE", nothing, &[Intent::Sleep], 1),
        mv("WAKE_MOVE", |cx, me| power_self(cx, me, ids::power::STRENGTH_POWER, 10), &[Intent::Buff], 3),
        mv("SLEEP_MOVE_2", nothing, &[Intent::Sleep], 3),
        mv(
            "SLASHES_MOVE",
            |cx, me| {
                let d = a9(cx, 15, 13);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 15, 13))],
            3,
        ),
    ],
};

pub static BYRDONIS_DEF: MonsterDef = MonsterDef {
    id: ids::monster::BYRDONIS,
    hp: |a| hp(a, (90, 90), (81, 84)),
    initial: 0,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::TERRITORIAL_POWER, 1)),
    nodes: &[
        mv(
            "SWOOP_MOVE",
            |cx, me| {
                let d = a9(cx, 19, 17);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 19, 17))],
            1,
        ),
        mv(
            "PECK_MOVE",
            |cx, me| {
                let (d, n) = (a9(cx, 4, 3), a9(cx, 3, 3));
                atk_n(cx, me, d, n)
            },
            &[multi(|cx, _| a9(cx, 4, 3), |cx, _| a9(cx, 3, 3))],
            0,
        ),
    ],
};
