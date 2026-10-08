//! Act 3 "Glory" weak + normal monsters (spec 04 §3.4): Axebot, DevotedSculptor, ScrollOfBiting, LivingShield, TurretOperator,
//! Fabricator (+ Guardbot, Noisebot, Zapbot, Stabbot), FrogKnight, GlobeHead, OwlMagistrate, SlimedBerserker, TheLost,
//! TheForgotten, PunchConstruct (ConstructMenagerieNormal; self-contained copy).
//!
//! Monsters whose C# initial state depends on a model field (`StarterMoveIdx`, `StockOverride`, `StartsWithFastPunch`) use an
//! initial `Cond` node over `vars`: its first walk draws nothing and logs the chosen move exactly like the C# machine that
//! is constructed with that move as `initialState`.

use super::ovg_util::*;
use crate::dec::Dec;
use crate::defs::*;
use crate::ids;
use crate::state::*;
use crate::types::*;

// ---- Axebot (AxebotsNormal, slot `front`) ------------------------------------------------------------------------------
// vars[0] = StockAmount + 1 for a respawned Axebot (`_stockOverrideAmount`), 0 for the original (StockAmount 2).
fn axebot_stock(cx: &Combat, me: Cid) -> i32 {
    let v = cx.cr(me).monster.vars[0];
    if v == 0 { 2 } else { v - 1 }
}
/// `RespawnCount = 2 - StockAmount`.
fn axebot_respawns(cx: &Combat, me: Cid) -> i32 {
    2 - axebot_stock(cx, me)
}
/// `RespawnMaxHpBonus` (+10 max HP per respawn) for the HP range of a monster created with these vars.
pub fn axebot_hp_bonus(vars: [i32; 2]) -> i32 {
    if vars[0] == 0 { 0 } else { 10 * (2 - (vars[0] - 1)) }
}

// 0 BOOT_UP_MOVE, 1 ONE_TWO_MOVE, 2 HAMMER_UPPERCUT_MOVE, 3 START (respawn -> BOOT_UP else HAMMER_UPPERCUT)
pub static AXEBOT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::AXEBOT,
    hp: |a| hp(a, (76, 86), (70, 78)),
    initial: 3,
    on_spawn: Some(|cx, me| {
        let stock = axebot_stock(cx, me);
        if stock > 0 {
            // PowerCmd.Apply<StockPower>(Creature, StockAmount, null, null)
            cx.apply_power(ids::power::STOCK_POWER, me, Dec::int(stock as i64), NO, NO);
        }
    }),
    nodes: &[
        mv(
            "BOOT_UP_MOVE",
            |cx, me| {
                let b = a9(cx, 15, 10);
                block(cx, me, b);
                let s = a9(cx, 4, 3) * axebot_respawns(cx, me);
                power_self(cx, me, ids::power::STRENGTH_POWER, s);
            },
            &[Intent::Defend, Intent::Buff],
            2,
        ),
        mv(
            "ONE_TWO_MOVE",
            |cx, me| {
                let d = a9(cx, 11, 10);
                atk_n(cx, me, d, 2);
            },
            &[multi(|cx, _| a9(cx, 11, 10), |_, _| 2)],
            2,
        ),
        mv(
            "HAMMER_UPPERCUT_MOVE",
            |cx, me| {
                let d = a9(cx, 18, 14);
                atk(cx, me, d);
                power_player(cx, me, ids::power::WEAK_POWER, 2);
                power_player(cx, me, ids::power::FRAIL_POWER, 2);
            },
            &[attack(|cx, _| a9(cx, 18, 14)), Intent::Debuff],
            1,
        ),
        cond("START", &[(0, |cx, c| cx.cr(c).monster.vars[0] != 0), (2, |cx, c| cx.cr(c).monster.vars[0] == 0)]),
    ],
};

// ---- DevotedSculptor (DevotedSculptorWeak) ------------------------------------------------------------------------------
// 0 FORBIDDEN_INCANTATION_MOVE, 1 SAVAGE_MOVE
pub static DEVOTED_SCULPTOR_DEF: MonsterDef = MonsterDef {
    id: ids::monster::DEVOTED_SCULPTOR,
    hp: |a| hp(a, (172, 172), (162, 162)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv(
            "FORBIDDEN_INCANTATION_MOVE",
            |cx, me| {
                // Ritual 9, applier null
                cx.apply_power(ids::power::RITUAL_POWER, me, Dec::int(9), NO, NO);
            },
            &[Intent::Buff],
            1,
        ),
        mv(
            "SAVAGE_MOVE",
            |cx, me| {
                let d = a9(cx, 15, 12);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 15, 12))],
            1,
        ),
    ],
};

// ---- ScrollOfBiting (ScrollsOfBitingWeak / Normal): vars[0] = StarterMoveIdx -----------------------------------------------
// 0 CHOMP, 1 CHEW, 2 MORE_TEETH, 3 rand, 4 START
pub static SCROLL_OF_BITING_DEF: MonsterDef = MonsterDef {
    id: ids::monster::SCROLL_OF_BITING,
    hp: |a| hp(a, (33, 39), (30, 37)),
    initial: 4,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::PAPER_CUTS_POWER, 2)),
    nodes: &[
        mv(
            "CHOMP",
            |cx, me| {
                let d = a9(cx, 16, 14);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 16, 14))],
            2,
        ),
        mv(
            "CHEW",
            |cx, me| {
                let d = a9(cx, 6, 5);
                atk_n(cx, me, d, 2);
            },
            &[multi(|cx, _| a9(cx, 6, 5), |_, _| 2)],
            3,
        ),
        mv("MORE_TEETH", |cx, me| power_self(cx, me, ids::power::STRENGTH_POWER, 2), &[Intent::Buff], 1),
        rand("rand", &[Branch::new(0).cannot_repeat(), Branch::new(1).max_repeats(2)]),
        cond(
            "START",
            &[
                (0, |cx, c| cx.cr(c).monster.vars[0] % 3 == 0),
                (1, |cx, c| cx.cr(c).monster.vars[0] % 3 == 1),
                (2, |cx, c| cx.cr(c).monster.vars[0] % 3 == 2),
            ],
        ),
    ],
};

// ---- LivingShield + TurretOperator (TurretOperatorWeak) -----------------------------------------------------------------
fn living_allies(cx: &Combat, me: Cid) -> usize {
    cx.enemies.iter().filter(|&&e| e != me && !cx.cr(e).is_dead()).count()
}

// 0 SHIELD_SLAM_MOVE, 1 SMASH_MOVE, 2 SHIELD_SLAM_BRANCH
pub static LIVING_SHIELD_DEF: MonsterDef = MonsterDef {
    id: ids::monster::LIVING_SHIELD,
    hp: |a| hp(a, (65, 65), (55, 55)),
    initial: 0,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::RAMPART_POWER, 25)),
    nodes: &[
        mv("SHIELD_SLAM_MOVE", |cx, me| atk(cx, me, 6), &[attack(|_, _| 6)], 2),
        mv(
            "SMASH_MOVE",
            |cx, me| {
                let d = a9(cx, 18, 16);
                atk(cx, me, d);
                power_self(cx, me, ids::power::STRENGTH_POWER, 3);
            },
            &[attack(|cx, _| a9(cx, 18, 16)), Intent::Buff],
            1,
        ),
        cond("SHIELD_SLAM_BRANCH", &[(0, |cx, c| living_allies(cx, c) > 0), (1, |cx, c| living_allies(cx, c) == 0)]),
    ],
};

fn unload(cx: &mut Combat, me: Cid) {
    let d = a9(cx, 4, 3);
    atk_n(cx, me, d, 5);
}

// 0 UNLOAD_MOVE, 1 UNLOAD_MOVE_2, 2 RELOAD_MOVE
pub static TURRET_OPERATOR_DEF: MonsterDef = MonsterDef {
    id: ids::monster::TURRET_OPERATOR,
    hp: |a| hp(a, (51, 51), (41, 41)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv("UNLOAD_MOVE", unload, &[multi(|cx, _| a9(cx, 4, 3), |_, _| 5)], 1),
        mv("UNLOAD_MOVE_2", unload, &[multi(|cx, _| a9(cx, 4, 3), |_, _| 5)], 2),
        mv("RELOAD_MOVE", |cx, me| power_self(cx, me, ids::power::STRENGTH_POWER, 1), &[Intent::Buff], 0),
    ],
};

// ---- Fabricator (FabricatorNormal) and the bots it builds --------------------------------------------------------------------
/// Encounter slot table [bot1, bot2, fabricator, bot3, bot4].
pub const FABRICATOR_SLOTS: u8 = 5;
pub const SLOT_FABRICATOR: u8 = 2;

fn can_fabricate(cx: &Combat, _me: Cid) -> bool {
    cx.enemies.iter().filter(|&&e| !cx.cr(e).is_dead()).count() < 4
}

/// `Fabricator.SpawnBot(options)`; `_lastSpawned` lives in `vars[0]` (monster id + 1, 0 = none).
fn spawn_bot(cx: &mut Combat, me: Cid, options: [u16; 2]) {
    let last = cx.cr(me).monster.vars[0];
    let mut items = [0u16; 2];
    let mut n = 0usize;
    for &m in options.iter() {
        if m as i32 + 1 != last {
            items[n] = m;
            n += 1;
        }
    }
    // RunRng.MonsterAi.NextItem(items): one draw even for a single candidate
    let pick = items[cx.rng.monster_ai.next_int(n as i32) as usize];
    cx.creatures[me as usize].monster.vars[0] = pick as i32 + 1;
    let slot = cx.next_free_slot(FABRICATOR_SLOTS);
    if let Some(bot) = cx.summon_enemy(pick, slot, [0, 0]) {
        // PowerCmd.Apply<MinionPower>(target, 1, Creature)
        cx.apply_power(ids::power::MINION_POWER, bot, Dec::ONE, me, NO);
    }
}
fn spawn_defensive(cx: &mut Combat, me: Cid) {
    spawn_bot(cx, me, [ids::monster::GUARDBOT, ids::monster::NOISEBOT]);
}
fn spawn_aggro(cx: &mut Combat, me: Cid) {
    spawn_bot(cx, me, [ids::monster::ZAPBOT, ids::monster::STABBOT]);
}

// 0 FABRICATE_MOVE, 1 FABRICATING_STRIKE_MOVE, 2 DISINTEGRATE_MOVE, 3 fabricateBranch (initial), 4 RAND
pub static FABRICATOR_DEF: MonsterDef = MonsterDef {
    id: ids::monster::FABRICATOR,
    hp: |a| hp(a, (155, 155), (150, 150)),
    initial: 3,
    on_spawn: None,
    nodes: &[
        mv(
            "FABRICATE_MOVE",
            |cx, me| {
                spawn_defensive(cx, me);
                spawn_aggro(cx, me);
            },
            &[Intent::Summon],
            3,
        ),
        mv(
            "FABRICATING_STRIKE_MOVE",
            |cx, me| {
                let d = a9(cx, 21, 18);
                atk(cx, me, d);
                spawn_aggro(cx, me);
            },
            &[attack(|cx, _| a9(cx, 21, 18)), Intent::Summon],
            3,
        ),
        mv(
            "DISINTEGRATE_MOVE",
            |cx, me| {
                let d = a9(cx, 13, 11);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 13, 11))],
            3,
        ),
        cond("fabricateBranch", &[(4, |cx, c| can_fabricate(cx, c)), (2, |cx, c| !can_fabricate(cx, c))]),
        rand("RAND", &[Branch::new(0), Branch::new(1)]),
    ],
};

/// `Guardbot.GuardMove`: 15 Unpowered block to every Fabricator.
pub static GUARDBOT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::GUARDBOT,
    hp: |a| hp(a, (17, 21), (16, 20)),
    initial: 0,
    on_spawn: None,
    nodes: &[mv(
        "GUARD_MOVE",
        |cx, _| {
            let list: crate::util::ArrayVec<Cid, MAX_CREATURES> = {
                let mut l = crate::util::ArrayVec::new();
                for &e in cx.enemies.iter() {
                    if cx.cr(e).monster.id == ids::monster::FABRICATOR {
                        l.push(e);
                    }
                }
                l
            };
            for &e in list.iter() {
                cx.gain_block(e, Dec::int(15), ValueProp::UNPOWERED, NO);
            }
        },
        &[Intent::Defend],
        0,
    )],
};

/// `Noisebot.NoiseMove`: one Dazed to the discard pile, one to a random draw-pile position.
pub static NOISEBOT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::NOISEBOT,
    hp: |a| hp(a, (19, 24), (18, 23)),
    initial: 0,
    on_spawn: None,
    nodes: &[mv(
        "NOISE_MOVE",
        |cx, _| {
            cx.add_status_cards(ids::card::DAZED, PileType::Discard, 1, CardPilePosition::Bottom);
            cx.add_status_cards(ids::card::DAZED, PileType::Draw, 1, CardPilePosition::Random);
        },
        &[Intent::StatusCard],
        0,
    )],
};

pub static ZAPBOT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::ZAPBOT,
    hp: |a| hp(a, (19, 24), (18, 23)),
    initial: 0,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::HIGH_VOLTAGE_POWER, 2)),
    nodes: &[mv(
        "ZAP",
        |cx, me| {
            let d = a9(cx, 15, 14);
            atk(cx, me, d)
        },
        &[attack(|cx, _| a9(cx, 15, 14))],
        0,
    )],
};

pub static STABBOT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::STABBOT,
    hp: |a| hp(a, (19, 24), (18, 23)),
    initial: 0,
    on_spawn: None,
    nodes: &[mv(
        "STAB_MOVE",
        |cx, me| {
            let d = a9(cx, 12, 11);
            atk(cx, me, d);
            power_player(cx, me, ids::power::FRAIL_POWER, 1);
        },
        &[attack(|cx, _| a9(cx, 12, 11)), Intent::Debuff],
        0,
    )],
};

// ---- FrogKnight (FrogKnightNormal): vars[0] = HasBeetleCharged ---------------------------------------------------------------
// 0 HALF_HEALTH, 1 FOR_THE_QUEEN, 2 STRIKE_DOWN_EVIL, 3 TONGUE_LASH (initial), 4 BEETLE_CHARGE
pub static FROG_KNIGHT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::FROG_KNIGHT,
    hp: |a| hp(a, (199, 199), (191, 191)),
    initial: 3,
    on_spawn: Some(|cx, me| {
        let p = a8(cx, 19, 15);
        power_self(cx, me, ids::power::PLATING_POWER, p);
        cx.creatures[me as usize].monster.vars[0] = 0;
    }),
    nodes: &[
        cond(
            "HALF_HEALTH",
            &[
                (3, |cx, c| cx.cr(c).monster.vars[0] != 0 || cx.cr(c).hp() >= cx.cr(c).max_hp / 2),
                (4, |cx, c| cx.cr(c).monster.vars[0] == 0 && cx.cr(c).hp() < cx.cr(c).max_hp / 2),
            ],
        ),
        mv("FOR_THE_QUEEN", |cx, me| power_self(cx, me, ids::power::STRENGTH_POWER, 5), &[Intent::Buff], 0),
        mv(
            "STRIKE_DOWN_EVIL",
            |cx, me| {
                let d = a9(cx, 23, 21);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 23, 21))],
            1,
        ),
        mv(
            "TONGUE_LASH",
            |cx, me| {
                let d = a9(cx, 14, 13);
                atk(cx, me, d);
                power_player(cx, me, ids::power::FRAIL_POWER, 2);
            },
            &[attack(|cx, _| a9(cx, 14, 13)), Intent::Debuff],
            2,
        ),
        mv(
            "BEETLE_CHARGE",
            |cx, me| {
                cx.creatures[me as usize].monster.vars[0] = 1;
                let d = a9(cx, 40, 35);
                atk(cx, me, d);
            },
            &[attack(|cx, _| a9(cx, 40, 35))],
            3,
        ),
    ],
};

// ---- GlobeHead (GlobeHeadNormal) ---------------------------------------------------------------------------------------------
// 0 SHOCKING_SLAP (initial), 1 THUNDER_STRIKE, 2 GALVANIC_BURST
pub static GLOBE_HEAD_DEF: MonsterDef = MonsterDef {
    id: ids::monster::GLOBE_HEAD,
    hp: |a| hp(a, (158, 158), (148, 148)),
    initial: 0,
    on_spawn: Some(|cx, me| {
        let g = a9(cx, 8, 6);
        power_self(cx, me, ids::power::GALVANIC_POWER, g);
    }),
    nodes: &[
        mv(
            "SHOCKING_SLAP",
            |cx, me| {
                let d = a9(cx, 14, 13);
                atk(cx, me, d);
                power_player(cx, me, ids::power::FRAIL_POWER, 2);
            },
            &[attack(|cx, _| a9(cx, 14, 13)), Intent::Debuff],
            1,
        ),
        mv(
            "THUNDER_STRIKE",
            |cx, me| {
                let d = a9(cx, 7, 6);
                atk_n(cx, me, d, 3);
            },
            &[multi(|cx, _| a9(cx, 7, 6), |_, _| 3)],
            2,
        ),
        mv(
            "GALVANIC_BURST",
            |cx, me| {
                let d = a9(cx, 17, 16);
                atk(cx, me, d);
                power_self(cx, me, ids::power::STRENGTH_POWER, 2);
            },
            &[attack(|cx, _| a9(cx, 17, 16)), Intent::Buff],
            0,
        ),
    ],
};

// ---- OwlMagistrate (OwlMagistrateNormal) -------------------------------------------------------------------------------------
// 0 MAGISTRATE_SCRUTINY, 1 PECK_ASSAULT, 2 JUDICIAL_FLIGHT, 3 VERDICT
pub static OWL_MAGISTRATE_DEF: MonsterDef = MonsterDef {
    id: ids::monster::OWL_MAGISTRATE,
    hp: |a| hp(a, (247, 247), (231, 231)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv(
            "MAGISTRATE_SCRUTINY",
            |cx, me| {
                let d = a9(cx, 17, 16);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 17, 16))],
            1,
        ),
        mv(
            "PECK_ASSAULT",
            |cx, me| {
                let d = a9(cx, 4, 4);
                atk_n(cx, me, d, 6);
            },
            &[multi(|cx, _| a9(cx, 4, 4), |_, _| 6)],
            2,
        ),
        mv("JUDICIAL_FLIGHT", |cx, me| power_self(cx, me, ids::power::SOAR_POWER, 1), &[Intent::Buff], 3),
        mv(
            "VERDICT",
            |cx, me| {
                let d = a9(cx, 36, 33);
                atk(cx, me, d);
                power_player(cx, me, ids::power::VULNERABLE_POWER, 4);
                cx.remove_power_by_id(me, ids::power::SOAR_POWER);
            },
            &[attack(|cx, _| a9(cx, 36, 33)), Intent::Debuff],
            0,
        ),
    ],
};

// ---- SlimedBerserker (SlimedBerserkerNormal) ---------------------------------------------------------------------------------
// 0 VOMIT_ICHOR_MOVE, 1 SMOTHER_MOVE, 2 LEECHING_HUG_MOVE, 3 FURIOUS_PUMMELING_MOVE
pub static SLIMED_BERSERKER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::SLIMED_BERSERKER,
    hp: |a| hp(a, (281, 281), (261, 261)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv("VOMIT_ICHOR_MOVE", |cx, _| status_to_discard(cx, ids::card::SLIMED, 10), &[Intent::StatusCard], 3),
        mv(
            "SMOTHER_MOVE",
            |cx, me| {
                let d = a9(cx, 33, 30);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 33, 30))],
            0,
        ),
        mv(
            "LEECHING_HUG_MOVE",
            |cx, me| {
                // Weak 3 on the player with applier null, Strength +3 on itself
                cx.apply_power(ids::power::WEAK_POWER, PLAYER, Dec::int(3), NO, NO);
                power_self(cx, me, ids::power::STRENGTH_POWER, 3);
            },
            &[Intent::Debuff, Intent::Buff],
            1,
        ),
        mv(
            "FURIOUS_PUMMELING_MOVE",
            |cx, me| {
                let d = a9(cx, 5, 4);
                atk_n(cx, me, d, 4);
            },
            &[multi(|cx, _| a9(cx, 5, 4), |_, _| 4)],
            2,
        ),
    ],
};

// ---- TheLost + TheForgotten (TheLostAndForgottenNormal) ------------------------------------------------------------------------
// 0 DEBILITATING_SMOG, 1 EYE_LASERS
pub static THE_LOST_DEF: MonsterDef = MonsterDef {
    id: ids::monster::THE_LOST,
    hp: |a| hp(a, (99, 99), (93, 93)),
    initial: 0,
    on_spawn: Some(|cx, me| {
        cx.apply_power(ids::power::POSSESS_STRENGTH_POWER, me, Dec::ONE, NO, NO);
    }),
    nodes: &[
        mv(
            "DEBILITATING_SMOG",
            |cx, me| {
                power_player(cx, me, ids::power::STRENGTH_POWER, -2);
                power_self(cx, me, ids::power::STRENGTH_POWER, 2);
            },
            &[Intent::Debuff, Intent::Buff],
            1,
        ),
        mv(
            "EYE_LASERS",
            |cx, me| {
                let d = a9(cx, 5, 4);
                atk_n(cx, me, d, 2);
            },
            &[multi(|cx, _| a9(cx, 5, 4), |_, _| 2)],
            0,
        ),
    ],
};

/// `TheForgotten.DreadDamage`: base + the monster's own Dexterity.
fn dread_damage(cx: &Combat, me: Cid) -> i32 {
    a9(cx, 15, 13) + cx.cr(me).power_amount(ids::power::DEXTERITY_POWER)
}

// 0 MIASMA, 1 DREAD
pub static THE_FORGOTTEN_DEF: MonsterDef = MonsterDef {
    id: ids::monster::THE_FORGOTTEN,
    hp: |a| hp(a, (111, 111), (106, 106)),
    initial: 0,
    on_spawn: Some(|cx, me| {
        cx.apply_power(ids::power::POSSESS_SPEED_POWER, me, Dec::ONE, NO, NO);
    }),
    nodes: &[
        mv(
            "MIASMA",
            |cx, me| {
                power_player(cx, me, ids::power::DEXTERITY_POWER, -2);
                block(cx, me, 8);
                power_self(cx, me, ids::power::DEXTERITY_POWER, 2);
            },
            &[Intent::Debuff, Intent::Defend, Intent::Buff],
            1,
        ),
        mv(
            "DREAD",
            |cx, me| {
                let d = dread_damage(cx, me);
                atk(cx, me, d)
            },
            &[attack(dread_damage)],
            0,
        ),
    ],
};

// ---- PunchConstruct (ConstructMenagerieNormal): vars[0] = StartsWithFastPunch, vars[1] = StartingHpReduction -----------------------
// 0 READY_MOVE, 1 FAST_PUNCH_MOVE, 2 STRONG_PUNCH_MOVE, 3 START
pub static PUNCH_CONSTRUCT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::PUNCH_CONSTRUCT,
    hp: |a| hp(a, (60, 60), (55, 55)),
    initial: 3,
    on_spawn: Some(|cx, me| {
        power_self(cx, me, ids::power::ARTIFACT_POWER, 1);
        let red = cx.cr(me).monster.vars[1];
        if red > 0 {
            let hp = (cx.cr(me).hp() - red).max(1);
            cx.set_current_hp_internal(me, Dec::int(hp as i64));
        }
    }),
    nodes: &[
        mv("READY_MOVE", |cx, me| block(cx, me, 10), &[Intent::Defend], 1),
        mv(
            "FAST_PUNCH_MOVE",
            |cx, me| {
                let d = a9(cx, 6, 5);
                atk_n(cx, me, d, 2);
                power_player(cx, me, ids::power::FRAIL_POWER, 1);
            },
            &[multi(|cx, _| a9(cx, 6, 5), |_, _| 2), Intent::Debuff],
            2,
        ),
        mv(
            "STRONG_PUNCH_MOVE",
            |cx, me| {
                let d = a9(cx, 16, 14);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 16, 14))],
            0,
        ),
        cond("START", &[(1, |cx, c| cx.cr(c).monster.vars[0] != 0), (0, |cx, c| cx.cr(c).monster.vars[0] == 0)]),
    ],
};
