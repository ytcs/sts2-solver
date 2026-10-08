use super::ovg_util::*;
use crate::defs::*;
use crate::ids;
use crate::state::*;

pub const SLOT_ILLUSION: u8 = 0;
pub const SLOT_FOGMOG: u8 = 1;

fn swipe(cx: &mut Combat, me: Cid) {
    let d = a9(cx, 9, 8);
    atk(cx, me, d);
    power_self(cx, me, ids::power::STRENGTH_POWER, 1);
}

pub static FOGMOG_DEF: MonsterDef = MonsterDef {
    id: ids::monster::FOGMOG,
    hp: |a| hp(a, (78, 78), (74, 74)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv(
            "ILLUSION_MOVE",
            |cx, _| {
                if cx.in_progress {
                    cx.summon_enemy(ids::monster::EYE_WITH_TEETH, SLOT_ILLUSION, [0, 0]);
                }
            },
            &[Intent::Summon],
            1,
        ),
        mv("SWIPE_MOVE", swipe, &[attack(|cx, _| a9(cx, 9, 8)), Intent::Buff], 3),
        mv("SWIPE_RANDOM_MOVE", swipe, &[attack(|cx, _| a9(cx, 9, 8)), Intent::Buff], 4),
        rand("BRANCH", &[Branch::new(2).cannot_repeat().weight(0.4), Branch::new(4).cannot_repeat().weight(0.6)]),
        mv(
            "HEADBUTT_MOVE",
            |cx, me| {
                let d = a9(cx, 16, 14);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 16, 14))],
            1,
        ),
    ],
};

pub static EYE_WITH_TEETH_DEF: MonsterDef = MonsterDef {
    id: ids::monster::EYE_WITH_TEETH,
    hp: |_| (6, 6),
    initial: 0,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::ILLUSION_POWER, 1)),
    nodes: &[
        mv("DISTRACT_MOVE", |cx, _| status_to_discard(cx, ids::card::DAZED, 3), &[Intent::StatusCard], 0),
        mv_once("REVIVE_MOVE", |cx, me| crate::content::powers::overgrowth::illusion_revive(cx, me), &[Intent::Heal], FOLLOW_STORED),
    ],
};
