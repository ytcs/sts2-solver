use super::ovg_util::*;
use crate::defs::*;
use crate::ids;

pub const SLOT_PHROG: u8 = 0;
pub fn wriggler_slot(i: u8) -> u8 {
    1 + i
}

pub static PHROG_PARASITE_DEF: MonsterDef = MonsterDef {
    id: ids::monster::PHROG_PARASITE,
    hp: |a| hp(a, (66, 68), (61, 64)),
    initial: 0,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::INFESTED_POWER, 4)),
    nodes: &[
        mv("INFECT_MOVE", |cx, _| status_to_discard(cx, ids::card::INFECTION, 3), &[Intent::StatusCard], 1),
        mv(
            "LASH_MOVE",
            |cx, me| {
                let d = a9(cx, 5, 4);
                atk_n(cx, me, d, 4)
            },
            &[multi(|cx, _| a9(cx, 5, 4), |_, _| 4)],
            0,
        ),
    ],
};

pub static WRIGGLER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::WRIGGLER,
    hp: |a| hp(a, (18, 22), (17, 21)),
    initial: 4,
    on_spawn: None,
    nodes: &[
        mv(
            "NASTY_BITE_MOVE",
            |cx, me| {
                let d = a9(cx, 7, 6);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 7, 6))],
            2,
        ),
        mv("SPAWNED_MOVE", nothing, &[Intent::Stun], 3),
        mv(
            "WRIGGLE_MOVE",
            |cx, me| {
                status_to_discard(cx, ids::card::INFECTION, 1);
                power_self(cx, me, ids::power::STRENGTH_POWER, 2);
            },
            &[Intent::Buff, Intent::StatusCard],
            0,
        ),
        cond(
            "INIT_MOVE",
            &[
                (0, |cx, c| cx.cr(c).slot == wriggler_slot(0)),
                (2, |cx, c| cx.cr(c).slot == wriggler_slot(1)),
                (0, |cx, c| cx.cr(c).slot == wriggler_slot(2)),
                (2, |cx, c| cx.cr(c).slot == wriggler_slot(3)),
            ],
        ),
        cond("START", &[(1, |cx, c| cx.cr(c).monster.vars[0] != 0), (3, |cx, c| cx.cr(c).monster.vars[0] == 0)]),
    ],
};
