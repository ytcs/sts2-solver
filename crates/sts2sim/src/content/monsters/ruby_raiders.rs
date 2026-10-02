//! Ruby Raiders (RubyRaidersNormal): Assassin, Axe, Brute, Crossbow, Tracker. Spec 04 §3.1.

use super::ovg_util::*;
use crate::defs::*;
use crate::ids;
use crate::state::*;

// ---- AssassinRubyRaider: KILLSHOT_MOVE loop ----------------------------------------------------------------------
pub static ASSASSIN_RUBY_RAIDER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::ASSASSIN_RUBY_RAIDER,
    hp: |a| hp(a, (19, 24), (18, 23)),
    initial: 0,
    on_spawn: None,
    nodes: &[mv(
        "KILLSHOT_MOVE",
        |cx, me| {
            let d = a9(cx, 11, 10);
            atk(cx, me, d)
        },
        &[attack(|cx, _| a9(cx, 11, 10))],
        0,
    )],
};

// ---- AxeRubyRaider: SWING_1 -> SWING_2 -> BIG_SWING -> SWING_1 ---------------------------------------------------------
fn axe_swing(cx: &mut Combat, me: Cid) {
    let d = a9(cx, 6, 5);
    atk(cx, me, d);
    let b = a9(cx, 6, 5);
    block(cx, me, b);
}
pub static AXE_RUBY_RAIDER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::AXE_RUBY_RAIDER,
    hp: |a| hp(a, (21, 23), (20, 22)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv("SWING_1", axe_swing, &[attack(|cx, _| a9(cx, 6, 5)), Intent::Defend], 1),
        mv("SWING_2", axe_swing, &[attack(|cx, _| a9(cx, 6, 5)), Intent::Defend], 2),
        mv(
            "BIG_SWING",
            |cx, me| {
                let d = a9(cx, 13, 12);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 13, 12))],
            0,
        ),
    ],
};

// ---- BruteRubyRaider: BEAT_MOVE <-> ROAR_MOVE ----------------------------------------------------------------------
pub static BRUTE_RUBY_RAIDER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::BRUTE_RUBY_RAIDER,
    hp: |a| hp(a, (31, 34), (30, 33)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv(
            "BEAT_MOVE",
            |cx, me| {
                let d = a9(cx, 8, 7);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 8, 7))],
            1,
        ),
        mv("ROAR_MOVE", |cx, me| power_self(cx, me, ids::power::STRENGTH_POWER, 3), &[Intent::Buff], 0),
    ],
};

// ---- CrossbowRubyRaider: RELOAD_MOVE <-> FIRE_MOVE (initial RELOAD) ----------------------------------------------------
pub static CROSSBOW_RUBY_RAIDER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::CROSSBOW_RUBY_RAIDER,
    hp: |a| hp(a, (19, 22), (18, 21)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv("RELOAD_MOVE", |cx, me| block(cx, me, 3), &[Intent::Defend], 1),
        mv(
            "FIRE_MOVE",
            |cx, me| {
                let d = a9(cx, 16, 14);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 16, 14))],
            0,
        ),
    ],
};

// ---- TrackerRubyRaider: TRACK_MOVE (Frail 2) -> HOUNDS_MOVE loop ---------------------------------------------------------
pub static TRACKER_RUBY_RAIDER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::TRACKER_RUBY_RAIDER,
    hp: |a| hp(a, (22, 26), (21, 25)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv("TRACK_MOVE", |cx, me| power_player(cx, me, ids::power::FRAIL_POWER, 2), &[Intent::Debuff], 1),
        mv(
            "HOUNDS_MOVE",
            |cx, me| {
                let (d, n) = (a9(cx, 1, 1), a9(cx, 9, 8));
                atk_n(cx, me, d, n)
            },
            &[multi(|cx, _| a9(cx, 1, 1), |cx, _| a9(cx, 9, 8))],
            1,
        ),
    ],
};
