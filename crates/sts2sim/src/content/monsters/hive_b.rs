//! Act 2 "Hive" elites and bosses (hive_b slice): Decimillipede segments, Entomancer, Infested Prism, Kaiser Crab
//! (Crusher + Rocket), Knowledge Demon, The Insatiable. Spec 04 §3.3.

use super::ovg_util::*;
use crate::content::powers::hive_b::do_reattach;
use crate::defs::*;
use crate::engine::Ask;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// ---- DecimillipedeSegment{Front,Middle,Back} (DecimillipedeElite, slots segment1..3 = 0..2) --------------------------------
// vars[0] = StarterMoveIdx. Nodes: 0 WRITHE, 1 BULK, 2 CONSTRICT, 3 DEAD_MOVE, 4 REATTACH_MOVE, 5 RAND, 6 INIT (initial).
pub const SEGMENT_SLOT_FRONT: u8 = 0;
pub const SEGMENT_SLOT_MIDDLE: u8 = 1;
pub const SEGMENT_SLOT_BACK: u8 = 2;

fn segment_spawn(cx: &mut Combat, me: Cid) {
    // AfterAddedToRoom: even max HP, distinct from every other segment (+2 steps, wrapping to Min once Max is exceeded).
    let (lo, hi) = hp(cx.ascension, (46, 52), (40, 46));
    let mut max = cx.cr(me).max_hp;
    if max % 2 == 1 {
        max += 1;
    }
    while cx.enemies.iter().any(|&e| e != me && cx.cr(e).max_hp == max) {
        max += 2;
        if max > hi {
            max = lo;
        }
    }
    cx.set_max_hp(me, crate::dec::Dec::int(max as i64));
    cx.set_current_hp(me, crate::dec::Dec::int(max as i64));
    power_self(cx, me, ids::power::REATTACH_POWER, 25);
}

fn writhe_damage(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 6, 5)
}
fn bulk_damage(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 7, 6)
}
fn constrict_damage(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 9, 8)
}

const SEGMENT_NODES: &[MonsterNode] = &[
    mv(
        "WRITHE_MOVE",
        |cx, me| {
            let d = writhe_damage(cx, me);
            atk_n(cx, me, d, 2)
        },
        &[multi(writhe_damage, |_, _| 2)],
        2,
    ),
    mv(
        "BULK_MOVE",
        |cx, me| {
            let d = bulk_damage(cx, me);
            atk(cx, me, d);
            power_self(cx, me, ids::power::STRENGTH_POWER, 2);
        },
        &[attack(bulk_damage), Intent::Buff],
        0,
    ),
    mv(
        "CONSTRICT_MOVE",
        |cx, me| {
            let d = constrict_damage(cx, me);
            atk(cx, me, d);
            power_player(cx, me, ids::power::WEAK_POWER, 1);
        },
        &[attack(constrict_damage), Intent::Debuff],
        1,
    ),
    mv("DEAD_MOVE", nothing, &[], 4),
    mv_once("REATTACH_MOVE", |cx, me| do_reattach(cx, me), &[Intent::Heal], 5),
    rand("RAND", &[Branch::new(0).cannot_repeat(), Branch::new(1).cannot_repeat(), Branch::new(2).cannot_repeat()]),
    cond(
        "INIT",
        &[(0, |cx, c| cx.cr(c).monster.vars[0] % 3 == 0), (1, |cx, c| cx.cr(c).monster.vars[0] % 3 == 1), (2, |cx, c| cx.cr(c).monster.vars[0] % 3 == 2)],
    ),
];

pub static DECIMILLIPEDE_SEGMENT_FRONT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::DECIMILLIPEDE_SEGMENT_FRONT,
    hp: |a| hp(a, (46, 52), (40, 46)),
    initial: 6,
    on_spawn: Some(segment_spawn),
    nodes: SEGMENT_NODES,
};
pub static DECIMILLIPEDE_SEGMENT_MIDDLE_DEF: MonsterDef = MonsterDef {
    id: ids::monster::DECIMILLIPEDE_SEGMENT_MIDDLE,
    hp: |a| hp(a, (46, 52), (40, 46)),
    initial: 6,
    on_spawn: Some(segment_spawn),
    nodes: SEGMENT_NODES,
};
pub static DECIMILLIPEDE_SEGMENT_BACK_DEF: MonsterDef = MonsterDef {
    id: ids::monster::DECIMILLIPEDE_SEGMENT_BACK,
    hp: |a| hp(a, (46, 52), (40, 46)),
    initial: 6,
    on_spawn: Some(segment_spawn),
    nodes: SEGMENT_NODES,
};
listener!(DecimillipedeSegmentFront {});
listener!(DecimillipedeSegmentMiddle {});
listener!(DecimillipedeSegmentBack {});

// ---- Entomancer: BEES -> SPEAR -> PHEROMONE_SPIT -> BEES ... ---------------------------------------------------------------------
fn bees_damage(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 3, 3)
}
fn bees_repeat(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 8, 7)
}
fn spear_damage(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 20, 18)
}
// 0 PHEROMONE_SPIT_MOVE, 1 BEES_MOVE (initial), 2 SPEAR_MOVE
pub static ENTOMANCER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::ENTOMANCER,
    hp: |a| hp(a, (165, 165), (145, 145)),
    initial: 1,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::PERSONAL_HIVE_POWER, 1)),
    nodes: &[
        mv(
            "PHEROMONE_SPIT_MOVE",
            |cx, me| match cx.cr(me).power(ids::power::PERSONAL_HIVE_POWER).map(|p| p.amount) {
                Some(n) if n < 3 => {
                    power_self(cx, me, ids::power::PERSONAL_HIVE_POWER, 1);
                    power_self(cx, me, ids::power::STRENGTH_POWER, 1);
                }
                _ => power_self(cx, me, ids::power::STRENGTH_POWER, 2),
            },
            &[Intent::Buff],
            1,
        ),
        mv(
            "BEES_MOVE",
            |cx, me| {
                let (d, n) = (bees_damage(cx, me), bees_repeat(cx, me));
                atk_n(cx, me, d, n)
            },
            &[multi(bees_damage, bees_repeat)],
            2,
        ),
        mv(
            "SPEAR_MOVE",
            |cx, me| {
                let d = spear_damage(cx, me);
                atk(cx, me, d)
            },
            &[attack(spear_damage)],
            0,
        ),
    ],
};
listener!(Entomancer {});

// ---- InfestedPrism: JAB -> RADIATE -> WHIRLWIND -> PULSATE -> JAB ... --------------------------------------------------------------
fn prism_jab(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 17, 15)
}
fn prism_radiate(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 13, 11)
}
fn prism_whirlwind(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 6, 5)
}
fn prism_pulsate(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 10, 8)
}
fn vital_spark_amount(cx: &Combat) -> i32 {
    a9(cx, 3, 2)
}
pub static INFESTED_PRISM_DEF: MonsterDef = MonsterDef {
    id: ids::monster::INFESTED_PRISM,
    hp: |a| hp(a, (171, 171), (161, 161)),
    initial: 0,
    on_spawn: Some(|cx, me| {
        let n = vital_spark_amount(cx);
        power_self(cx, me, ids::power::VITAL_SPARK_POWER, n)
    }),
    nodes: &[
        mv(
            "JAB_MOVE",
            |cx, me| {
                let d = prism_jab(cx, me);
                atk(cx, me, d)
            },
            &[attack(prism_jab)],
            1,
        ),
        mv(
            "RADIATE_MOVE",
            |cx, me| {
                let d = prism_radiate(cx, me);
                atk(cx, me, d);
                let b = a9(cx, 13, 11);
                block(cx, me, b);
            },
            &[attack(prism_radiate), Intent::Defend],
            2,
        ),
        mv(
            "WHIRLWIND_MOVE",
            |cx, me| {
                let d = prism_whirlwind(cx, me);
                atk_n(cx, me, d, 3)
            },
            &[multi(prism_whirlwind, |_, _| 3)],
            3,
        ),
        mv(
            "PULSATE_MOVE",
            |cx, me| {
                let d = prism_pulsate(cx, me);
                atk(cx, me, d);
                let b = a8(cx, 22, 20);
                block(cx, me, b);
                let n = vital_spark_amount(cx);
                power_self(cx, me, ids::power::VITAL_SPARK_POWER, n);
            },
            &[attack(prism_pulsate), Intent::Buff, Intent::Defend],
            0,
        ),
    ],
};
listener!(InfestedPrism {});

// ---- KaiserCrab: Crusher (slot 0) + Rocket (slot 1) ------------------------------------------------------------------------------------
pub const SLOT_CRUSHER: u8 = 0;
pub const SLOT_ROCKET: u8 = 1;

fn crusher_thrash(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 14, 12)
}
fn crusher_sting(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 7, 6)
}
fn crusher_guarded(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 14, 12)
}
// 0 THRASH, 1 ENLARGING_STRIKE, 2 BUG_STING, 3 ADAPT, 4 GUARDED_STRIKE
pub static CRUSHER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::CRUSHER,
    hp: |a| hp(a, (219, 219), (209, 209)),
    initial: 0,
    on_spawn: Some(|cx, me| {
        power_self(cx, me, ids::power::BACK_ATTACK_LEFT_POWER, 1);
        power_self(cx, me, ids::power::CRAB_RAGE_POWER, 1);
    }),
    nodes: &[
        mv(
            "THRASH_MOVE",
            |cx, me| {
                let d = crusher_thrash(cx, me);
                atk(cx, me, d)
            },
            &[attack(crusher_thrash)],
            1,
        ),
        mv("ENLARGING_STRIKE_MOVE", |cx, me| atk(cx, me, 4), &[attack(|cx, _| a9(cx, 4, 4))], 2),
        mv(
            "BUG_STING_MOVE",
            |cx, me| {
                let d = crusher_sting(cx, me);
                atk_n(cx, me, d, 2);
                power_player(cx, me, ids::power::WEAK_POWER, 2);
                power_player(cx, me, ids::power::FRAIL_POWER, 2);
            },
            &[multi(crusher_sting, |_, _| 2), Intent::Debuff],
            3,
        ),
        mv(
            "ADAPT_MOVE",
            |cx, me| {
                let s = a9(cx, 3, 2);
                power_self(cx, me, ids::power::STRENGTH_POWER, s)
            },
            &[Intent::Buff],
            4,
        ),
        mv(
            "GUARDED_STRIKE_MOVE",
            |cx, me| {
                let d = crusher_guarded(cx, me);
                atk(cx, me, d);
                block(cx, me, 18);
            },
            &[attack(crusher_guarded), Intent::Defend],
            0,
        ),
    ],
};
listener!(Crusher {});

fn rocket_reticle(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 4, 3)
}
fn rocket_beam(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 20, 18)
}
fn rocket_laser(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 35, 31)
}
// 0 TARGETING_RETICLE, 1 PRECISION_BEAM, 2 CHARGE_UP, 3 LASER, 4 RECHARGE
pub static ROCKET_DEF: MonsterDef = MonsterDef {
    id: ids::monster::ROCKET,
    hp: |a| hp(a, (209, 209), (199, 199)),
    initial: 0,
    on_spawn: Some(|cx, me| {
        // SurroundedPower on every opponent (the player and its pets), applied by the Rocket.
        let targets = cx.allies;
        for &t in targets.iter() {
            cx.apply_power(ids::power::SURROUNDED_POWER, t, crate::dec::Dec::ONE, me, NO);
        }
        power_self(cx, me, ids::power::BACK_ATTACK_RIGHT_POWER, 1);
        power_self(cx, me, ids::power::CRAB_RAGE_POWER, 1);
    }),
    nodes: &[
        mv(
            "TARGETING_RETICLE_MOVE",
            |cx, me| {
                let d = rocket_reticle(cx, me);
                atk(cx, me, d)
            },
            &[attack(rocket_reticle)],
            1,
        ),
        mv(
            "PRECISION_BEAM_MOVE",
            |cx, me| {
                let d = rocket_beam(cx, me);
                atk(cx, me, d)
            },
            &[attack(rocket_beam)],
            2,
        ),
        mv(
            "CHARGE_UP_MOVE",
            |cx, me| {
                let s = a9(cx, 3, 2);
                power_self(cx, me, ids::power::STRENGTH_POWER, s)
            },
            &[Intent::Buff],
            3,
        ),
        mv(
            "LASER_MOVE",
            |cx, me| {
                let d = rocket_laser(cx, me);
                atk(cx, me, d)
            },
            &[attack(rocket_laser)],
            4,
        ),
        mv("RECHARGE_MOVE", nothing, &[Intent::Sleep], 0),
    ],
};
listener!(Rocket {});

// ---- KnowledgeDemon (vars[0] = CurseOfKnowledgeCounter) ---------------------------------------------------------------------------------
// 0 CURSE_OF_KNOWLEDGE, 1 SLAP, 2 KNOWLEDGE_OVERWHELMING, 3 PONDER, 4 CurseOfKnowledgeBranch (cond)
const DISINTEGRATION_DAMAGE: [i32; 3] = [6, 7, 8];
const CURSE_PARTNERS: [u16; 3] = [ids::card::MIND_ROT, ids::card::SLOTH, ids::card::WASTE_AWAY];

fn slap_damage(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 18, 17)
}
fn overwhelming_damage(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 9, 8)
}
fn ponder_damage(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 13, 11)
}

/// `CurseOfKnowledgeMove` for the (single) player: `CardSelectCmd.FromChooseACardScreen` over [Disintegration, partner];
/// the chosen card's `OnChosen` applies its power. The move suspends on the decision (`resume_hook`).
fn curse_of_knowledge(cx: &mut Combat, me: Cid) {
    let counter = cx.cr(me).monster.vars[0] as usize;
    if cx.cr(PLAYER).is_alive() {
        let a = cx.new_card(ids::card::DISINTEGRATION, 0);
        let b = cx.new_card(CURSE_PARTNERS[counter], 0);
        if let (Some(a), Some(b)) = (a, b) {
            match cx.ask_options(purpose::monster(ids::monster::KNOWLEDGE_DEMON), &[a, b], false) {
                Ask::Pending => {
                    cx.hook_ctx = Some((cx.monster_me(me), 1));
                    cx.stage = Stage::AwaitChoice;
                    return;
                }
                Ask::Resolved(_) => {}
            }
        }
    }
    cx.creatures[me as usize].monster.vars[0] += 1;
}

listener!(KnowledgeDemon {
    fn resume_hook(&self, cx: &mut Combat, me: Me, _phase: u8) {
        let counter = cx.cr(me.owner).monster.vars[0] as usize;
        if let Some(card) = cx.choice.cards.first() {
            let id = cx.cards[card as usize].id;
            let (power, amount) = if id == ids::card::DISINTEGRATION {
                (ids::power::DISINTEGRATION_POWER, DISINTEGRATION_DAMAGE[counter])
            } else {
                let p = match id {
                    ids::card::MIND_ROT => ids::power::MIND_ROT_POWER,
                    ids::card::SLOTH => ids::power::SLOTH_POWER,
                    _ => ids::power::WASTE_AWAY_POWER,
                };
                (p, cx.card_power_var(card, p))
            };
            cx.apply_power(power, PLAYER, crate::dec::Dec::int(amount as i64), PLAYER, card);
        }
        cx.creatures[me.owner as usize].monster.vars[0] += 1;
    }
});

pub static KNOWLEDGE_DEMON_DEF: MonsterDef = MonsterDef {
    id: ids::monster::KNOWLEDGE_DEMON,
    hp: |a| hp(a, (399, 399), (379, 379)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv("CURSE_OF_KNOWLEDGE_MOVE", curse_of_knowledge, &[Intent::Debuff], 1),
        mv(
            "SLAP_MOVE",
            |cx, me| {
                let d = slap_damage(cx, me);
                atk(cx, me, d)
            },
            &[attack(slap_damage)],
            2,
        ),
        mv(
            "KNOWLEDGE_OVERWHELMING_MOVE",
            |cx, me| {
                let d = overwhelming_damage(cx, me);
                atk_n(cx, me, d, 3)
            },
            &[multi(overwhelming_damage, |_, _| 3)],
            3,
        ),
        mv(
            "PONDER_MOVE",
            |cx, me| {
                let d = ponder_damage(cx, me);
                atk(cx, me, d);
                cx.heal(me, crate::dec::Dec::int(30));
                let s = a9(cx, 3, 2);
                power_self(cx, me, ids::power::STRENGTH_POWER, s);
            },
            &[attack(ponder_damage), Intent::Heal, Intent::Buff],
            4,
        ),
        cond("CurseOfKnowledgeBranch", &[(0, |cx, c| cx.cr(c).monster.vars[0] < 3), (1, |cx, c| cx.cr(c).monster.vars[0] >= 3)]),
    ],
};

// ---- TheInsatiable: LIQUIFY_GROUND -> THRASH -> LUNGING_BITE -> SALIVATE -> THRASH_2 -> THRASH ... -----------------------------------
// 0 LIQUIFY_GROUND, 1 LUNGING_BITE, 2 THRASH, 3 THRASH_2, 4 SALIVATE
fn insatiable_thrash(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 9, 8)
}
fn insatiable_bite(cx: &Combat, _: Cid) -> i32 {
    a9(cx, 31, 28)
}
fn thrash_move(cx: &mut Combat, me: Cid) {
    let d = insatiable_thrash(cx, me);
    atk_n(cx, me, d, 2)
}
fn liquify_ground(cx: &mut Combat, me: Cid) {
    // SandpitPower(4) applied to the Insatiable itself, targeting the player.
    if let Some(uid) = cx.apply_power(ids::power::SANDPIT_POWER, me, crate::dec::Dec::int(4), me, NO) {
        cx.set_power_aux(me, uid, PLAYER as i32);
    }
    // 6 FranticEscape: the first 3 into the draw pile, the last 3 into the discard pile, each at a random position.
    for i in 0..6 {
        if let Some(c) = cx.new_card(ids::card::FRANTIC_ESCAPE, 0) {
            let pile = if i < 3 { PileType::Draw } else { PileType::Discard };
            cx.add_generated_card(c, pile, CardPilePosition::Random);
        }
    }
}
pub static THE_INSATIABLE_DEF: MonsterDef = MonsterDef {
    id: ids::monster::THE_INSATIABLE,
    hp: |a| hp(a, (341, 341), (321, 321)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv("LIQUIFY_GROUND_MOVE", liquify_ground, &[Intent::Buff, Intent::StatusCard], 2),
        mv(
            "LUNGING_BITE_MOVE",
            |cx, me| {
                let d = insatiable_bite(cx, me);
                atk(cx, me, d)
            },
            &[attack(insatiable_bite)],
            4,
        ),
        mv("THRASH_MOVE", thrash_move, &[multi(insatiable_thrash, |_, _| 2)], 1),
        mv("THRASH_MOVE_2", thrash_move, &[multi(insatiable_thrash, |_, _| 2)], 2),
        mv(
            "SALIVATE_MOVE",
            |cx, me| {
                let s = a9(cx, 3, 2);
                power_self(cx, me, ids::power::STRENGTH_POWER, s)
            },
            &[Intent::Buff],
            3,
        ),
    ],
};
listener!(TheInsatiable {});
