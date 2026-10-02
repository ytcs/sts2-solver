//! Overgrowth slimes: LeafSlimeS, TwigSlimeS, LeafSlimeM, TwigSlimeM. Spec 04 §3.1.

use super::ovg_util::*;
use crate::defs::*;
use crate::ids;
use crate::state::*;

// ---- LeafSlimeS: RAND{TACKLE:CNR, GOOP:CNR} (initial is the branch: one AI draw at the first roll) ----------------------------
// 0 TACKLE, 1 GOOP, 2 RAND (initial)
pub static LEAF_SLIME_S_DEF: MonsterDef = MonsterDef {
    id: ids::monster::LEAF_SLIME_S,
    hp: |a| hp(a, (12, 16), (11, 15)),
    initial: 2,
    on_spawn: None,
    nodes: &[
        mv(
            "TACKLE_MOVE",
            |cx, me| {
                let d = a9(cx, 4, 3);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 4, 3))],
            2,
        ),
        mv("GOOP_MOVE", |cx, _| status_to_discard(cx, ids::card::SLIMED, 1), &[Intent::StatusCard], 2),
        rand("RAND", &[Branch::new(0).cannot_repeat(), Branch::new(1).cannot_repeat()]),
    ],
};

// ---- TwigSlimeS: TACKLE loop ------------------------------------------------------------------------------------------------
pub static TWIG_SLIME_S_DEF: MonsterDef = MonsterDef {
    id: ids::monster::TWIG_SLIME_S,
    hp: |a| hp(a, (8, 12), (7, 11)),
    initial: 0,
    on_spawn: None,
    nodes: &[mv(
        "TACKLE_MOVE",
        |cx, me| {
            let d = a9(cx, 5, 4);
            atk(cx, me, d)
        },
        &[attack(|cx, _| a9(cx, 5, 4))],
        0,
    )],
};

// ---- LeafSlimeM: STICKY_SHOT <-> CLUMP_SHOT (initial STICKY_SHOT) ------------------------------------------------------------------
pub static LEAF_SLIME_M_DEF: MonsterDef = MonsterDef {
    id: ids::monster::LEAF_SLIME_M,
    hp: |a| hp(a, (33, 36), (32, 35)),
    initial: 1,
    on_spawn: None,
    nodes: &[
        mv(
            "CLUMP_SHOT",
            |cx, me| {
                let d = a9(cx, 9, 8);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 9, 8))],
            1,
        ),
        mv("STICKY_SHOT", |cx, _| status_to_discard(cx, ids::card::SLIMED, 2), &[Intent::StatusCard], 0),
    ],
};

// ---- TwigSlimeM: STICKY_SHOT_MOVE -> RAND{POKEY_POUNCE:x2, STICKY_SHOT:CNR} -----------------------------------------------------------
// 0 POKEY_POUNCE, 1 STICKY_SHOT (initial), 2 RAND
pub static TWIG_SLIME_M_DEF: MonsterDef = MonsterDef {
    id: ids::monster::TWIG_SLIME_M,
    hp: |a| hp(a, (27, 29), (26, 28)),
    initial: 1,
    on_spawn: None,
    nodes: &[
        mv(
            "POKEY_POUNCE_MOVE",
            |cx, me| {
                let d = a9(cx, 12, 11);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 12, 11))],
            2,
        ),
        mv("STICKY_SHOT_MOVE", |cx, _| status_to_discard(cx, ids::card::SLIMED, 1), &[Intent::StatusCard], 2),
        rand("RAND", &[Branch::new(0).max_repeats(2), Branch::new(1).cannot_repeat()]),
    ],
};
