//! Act 1a Overgrowth monsters without summons: Fuzzy Wurm Crawler, Shrinker Beetle, Flyconid, Inklet, Mawler,
//! Slithering Strangler, Snapping Jaxfruit, Vine Shambler, Cubex Construct, Vantom. Spec 04 §3.1.

use super::ovg_util::*;
use crate::dec::Dec;
use crate::defs::*;
use crate::ids;
use crate::state::*;
use crate::types::*;

// ---- FuzzyWurmCrawler: FIRST_ACID_GOOP -> INHALE -> ACID_GOOP -> FIRST_ACID_GOOP ------------------------------------------
fn acid_goop(cx: &mut Combat, me: Cid) {
    let d = a9(cx, 6, 4);
    atk(cx, me, d);
}
pub static FUZZY_WURM_CRAWLER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::FUZZY_WURM_CRAWLER,
    hp: |a| hp(a, (58, 59), (55, 57)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv("FIRST_ACID_GOOP", acid_goop, &[attack(|cx, _| a9(cx, 6, 4))], 2),
        mv("ACID_GOOP", acid_goop, &[attack(|cx, _| a9(cx, 6, 4))], 0),
        mv("INHALE", |cx, me| power_self(cx, me, ids::power::STRENGTH_POWER, 7), &[Intent::Buff], 1),
    ],
};

// ---- ShrinkerBeetle: SHRINKER_MOVE -> CHOMP_MOVE -> STOMP_MOVE -> CHOMP_MOVE ... ------------------------------------------
pub static SHRINKER_BEETLE_DEF: MonsterDef = MonsterDef {
    id: ids::monster::SHRINKER_BEETLE,
    hp: |a| hp(a, (40, 42), (38, 40)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv("SHRINKER_MOVE", |cx, me| power_player(cx, me, ids::power::SHRINK_POWER, -1), &[Intent::DebuffStrong], 1),
        mv(
            "CHOMP_MOVE",
            |cx, me| {
                let d = a9(cx, 8, 7);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 8, 7))],
            2,
        ),
        mv(
            "STOMP_MOVE",
            |cx, me| {
                let d = a9(cx, 14, 13);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 14, 13))],
            1,
        ),
    ],
};

// ---- Flyconid ------------------------------------------------------------------------------------------------------
// 0 VULNERABLE_SPORES, 1 FRAIL_SPORES, 2 SMASH, 3 RAND, 4 INITIAL (initial)
pub static FLYCONID_DEF: MonsterDef = MonsterDef {
    id: ids::monster::FLYCONID,
    hp: |a| hp(a, (51, 53), (47, 49)),
    initial: 4,
    on_spawn: None,
    nodes: &[
        mv("VULNERABLE_SPORES_MOVE", |cx, me| power_player(cx, me, ids::power::VULNERABLE_POWER, 2), &[Intent::Debuff], 3),
        mv(
            "FRAIL_SPORES_MOVE",
            |cx, me| {
                let d = a9(cx, 9, 8);
                atk(cx, me, d);
                power_player(cx, me, ids::power::FRAIL_POWER, 2);
            },
            &[attack(|cx, _| a9(cx, 9, 8)), Intent::Debuff],
            3,
        ),
        mv(
            "SMASH_MOVE",
            |cx, me| {
                let d = a9(cx, 12, 11);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 12, 11))],
            3,
        ),
        rand(
            "RAND",
            &[Branch::new(0).cooldown(3).cannot_repeat(), Branch::new(1).cooldown(2).cannot_repeat(), Branch::new(2).cannot_repeat()],
        ),
        rand("INITIAL", &[Branch::new(1).cooldown(2).cannot_repeat(), Branch::new(2).cannot_repeat()]),
    ],
};

// ---- Inklet (vars[0] = MiddleInklet) -----------------------------------------------------------------------------------
// 0 JAB, 1 PIERCING_GAZE, 2 WHIRLWIND, 3 RAND, 4 START (initial: middle -> WHIRLWIND else JAB)
pub static INKLET_DEF: MonsterDef = MonsterDef {
    id: ids::monster::INKLET,
    hp: |a| hp(a, (12, 18), (11, 17)),
    initial: 4,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::SLIPPERY_POWER, 1)),
    nodes: &[
        mv(
            "JAB_MOVE",
            |cx, me| {
                let d = a9(cx, 4, 3);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 4, 3))],
            3,
        ),
        mv(
            "PIERCING_GAZE_MOVE",
            |cx, me| {
                let d = a9(cx, 11, 10);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 11, 10))],
            0,
        ),
        mv(
            "WHIRLWIND_MOVE",
            |cx, me| {
                let d = a9(cx, 3, 2);
                atk_n(cx, me, d, 3)
            },
            &[multi(|cx, _| a9(cx, 3, 2), |_, _| 3)],
            0,
        ),
        rand("RAND", &[Branch::new(1).cannot_repeat(), Branch::new(2).cannot_repeat()]),
        cond("START", &[(2, |cx, c| cx.cr(c).monster.vars[0] != 0), (0, |cx, c| cx.cr(c).monster.vars[0] == 0)]),
    ],
};

// ---- Mawler: CLAW first, then RAND{RIP_AND_TEAR:CNR, ROAR:ONCE, CLAW:CNR} -------------------------------------------------
pub static MAWLER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::MAWLER,
    hp: |a| hp(a, (76, 76), (72, 72)),
    initial: 2,
    on_spawn: None,
    nodes: &[
        mv(
            "RIP_AND_TEAR_MOVE",
            |cx, me| {
                let d = a9(cx, 16, 14);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 16, 14))],
            3,
        ),
        mv("ROAR_MOVE", |cx, me| power_player(cx, me, ids::power::VULNERABLE_POWER, 3), &[Intent::Debuff], 3),
        mv(
            "CLAW_MOVE",
            |cx, me| {
                let d = a9(cx, 5, 4);
                atk_n(cx, me, d, 2)
            },
            &[multi(|cx, _| a9(cx, 5, 4), |_, _| 2)],
            3,
        ),
        rand("RAND", &[Branch::new(0).cannot_repeat(), Branch::new(1).once(), Branch::new(2).cannot_repeat()]),
    ],
};

// ---- SlitheringStrangler: CONSTRICT -> RAND{THWACK, LASH} -> CONSTRICT ... ---------------------------------------------------
pub static SLITHERING_STRANGLER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::SLITHERING_STRANGLER,
    hp: |a| hp(a, (54, 56), (53, 55)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv("CONSTRICT", |cx, me| power_player(cx, me, ids::power::CONSTRICT_POWER, 3), &[Intent::Debuff], 3),
        mv(
            "THWACK",
            |cx, me| {
                let d = a9(cx, 8, 7);
                atk(cx, me, d);
                block(cx, me, 5);
            },
            &[attack(|cx, _| a9(cx, 8, 7)), Intent::Defend],
            0,
        ),
        mv(
            "LASH",
            |cx, me| {
                let d = a9(cx, 13, 12);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 13, 12))],
            0,
        ),
        rand("rand", &[Branch::new(1), Branch::new(2)]),
    ],
};

// ---- SnappingJaxfruit: ENERGY_ORB_MOVE loop ----------------------------------------------------------------------------------
pub static SNAPPING_JAXFRUIT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::SNAPPING_JAXFRUIT,
    hp: |a| hp(a, (34, 36), (31, 33)),
    initial: 0,
    on_spawn: None,
    nodes: &[mv(
        "ENERGY_ORB_MOVE",
        |cx, me| {
            let d = a9(cx, 4, 3);
            atk(cx, me, d);
            power_self(cx, me, ids::power::STRENGTH_POWER, 2);
        },
        &[attack(|cx, _| a9(cx, 4, 3)), Intent::Buff],
        0,
    )],
};

// ---- VineShambler: SWIPE -> GRASPING_VINES -> CHOMP -> SWIPE ... ---------------------------------------------------------------
// 0 GRASPING_VINES, 1 SWIPE (initial), 2 CHOMP
pub static VINE_SHAMBLER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::VINE_SHAMBLER,
    hp: |a| hp(a, (64, 64), (61, 61)),
    initial: 1,
    on_spawn: None,
    nodes: &[
        mv(
            "GRASPING_VINES_MOVE",
            |cx, me| {
                let d = a9(cx, 9, 8);
                atk(cx, me, d);
                power_player(cx, me, ids::power::TANGLED_POWER, 1);
            },
            &[attack(|cx, _| a9(cx, 9, 8)), Intent::CardDebuff],
            2,
        ),
        mv(
            "SWIPE_MOVE",
            |cx, me| {
                let d = a9(cx, 7, 6);
                atk_n(cx, me, d, 2)
            },
            &[multi(|cx, _| a9(cx, 7, 6), |_, _| 2)],
            0,
        ),
        mv(
            "CHOMP_MOVE",
            |cx, me| {
                let d = a9(cx, 18, 16);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 18, 16))],
            1,
        ),
    ],
};

// ---- CubexConstruct: spawns with 13 block + Artifact 1 ---------------------------------------------------------------------------
fn repeater_blast(cx: &mut Combat, me: Cid) {
    let d = a9(cx, 8, 7);
    atk(cx, me, d);
    power_self(cx, me, ids::power::STRENGTH_POWER, 2);
}
pub static CUBEX_CONSTRUCT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::CUBEX_CONSTRUCT,
    hp: |a| hp(a, (70, 70), (65, 65)),
    initial: 0,
    on_spawn: Some(|cx, me| {
        cx.gain_block(me, Dec::int(13), ValueProp::MOVE, NO);
        power_self(cx, me, ids::power::ARTIFACT_POWER, 1);
    }),
    nodes: &[
        mv("CHARGE_UP_MOVE", |cx, me| power_self(cx, me, ids::power::STRENGTH_POWER, 2), &[Intent::Buff], 1),
        mv("REPEATER_BLAST_MOVE", repeater_blast, &[attack(|cx, _| a9(cx, 8, 7)), Intent::Buff], 2),
        mv("REPEATER_BLAST_MOVE_2", repeater_blast, &[attack(|cx, _| a9(cx, 8, 7)), Intent::Buff], 3),
        mv(
            "EXPEL_MOVE",
            |cx, me| {
                let d = a9(cx, 6, 5);
                atk_n(cx, me, d, 2)
            },
            &[multi(|cx, _| a9(cx, 6, 5), |_, _| 2)],
            1,
        ),
    ],
};

// ---- Vantom (boss): INK_BLOT -> INKY_LANCE -> DISMEMBER -> PREPARE -> INK_BLOT ... ----------------------------------------------
pub static VANTOM_DEF: MonsterDef = MonsterDef {
    id: ids::monster::VANTOM,
    hp: |a| hp(a, (183, 183), (173, 173)),
    initial: 0,
    on_spawn: Some(|cx, me| {
        let n = a8(cx, 9, 8);
        power_self(cx, me, ids::power::SLIPPERY_POWER, n);
    }),
    nodes: &[
        mv(
            "INK_BLOT_MOVE",
            |cx, me| {
                let d = a9(cx, 8, 7);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 8, 7))],
            1,
        ),
        mv(
            "INKY_LANCE_MOVE",
            |cx, me| {
                let d = a9(cx, 7, 6);
                atk_n(cx, me, d, 2)
            },
            &[multi(|cx, _| a9(cx, 7, 6), |_, _| 2)],
            2,
        ),
        mv(
            "DISMEMBER_MOVE",
            |cx, me| {
                let d = a9(cx, 30, 26);
                atk(cx, me, d);
                status_to_discard(cx, ids::card::WOUND, 3);
            },
            &[attack(|cx, _| a9(cx, 30, 26)), Intent::StatusCard],
            3,
        ),
        mv("PREPARE_MOVE", |cx, me| power_self(cx, me, ids::power::STRENGTH_POWER, 2), &[Intent::Buff], 0),
    ],
};
