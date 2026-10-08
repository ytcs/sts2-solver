use super::ovg_util::*;
use crate::defs::*;
use crate::ids;

pub const SLOT_1: u8 = 0;
pub const SLOT_2: u8 = 1;
pub const SLOT_LEADER: u8 = 2;

pub static KIN_FOLLOWER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::KIN_FOLLOWER,
    hp: |a| hp(a, (62, 63), (58, 59)),
    initial: 3,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::MINION_POWER, 1)),
    nodes: &[
        mv("QUICK_SLASH_MOVE", |cx, me| atk(cx, me, 5), &[attack(|_, _| 5)], 1),
        mv("BOOMERANG_MOVE", |cx, me| atk_n(cx, me, 2, 2), &[multi(|_, _| 2, |_, _| 2)], 2),
        mv(
            "POWER_DANCE_MOVE",
            |cx, me| {
                let s = a9(cx, 3, 2);
                power_self(cx, me, ids::power::STRENGTH_POWER, s);
            },
            &[Intent::Buff],
            0,
        ),
        cond("START", &[(2, |cx, c| cx.cr(c).monster.vars[0] != 0), (0, |cx, c| cx.cr(c).monster.vars[0] == 0)]),
    ],
};

pub static KIN_PRIEST_DEF: MonsterDef = MonsterDef {
    id: ids::monster::KIN_PRIEST,
    hp: |a| hp(a, (199, 199), (190, 190)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv(
            "ORB_OF_FRAILTY_MOVE",
            |cx, me| {
                let d = a9(cx, 9, 8);
                atk(cx, me, d);
                power_player(cx, me, ids::power::FRAIL_POWER, 1);
            },
            &[attack(|cx, _| a9(cx, 9, 8)), Intent::Debuff],
            1,
        ),
        mv(
            "ORB_OF_WEAKNESS_MOVE",
            |cx, me| {
                let d = a9(cx, 9, 8);
                atk(cx, me, d);
                power_player(cx, me, ids::power::WEAK_POWER, 1);
            },
            &[attack(|cx, _| a9(cx, 9, 8)), Intent::Debuff],
            2,
        ),
        mv("BEAM_MOVE", |cx, me| atk_n(cx, me, 3, 3), &[multi(|_, _| 3, |_, _| 3)], 3),
        mv(
            "RITUAL_MOVE",
            |cx, me| {
                let s = a9(cx, 3, 2);
                power_self(cx, me, ids::power::STRENGTH_POWER, s);
            },
            &[Intent::Buff],
            0,
        ),
    ],
};
