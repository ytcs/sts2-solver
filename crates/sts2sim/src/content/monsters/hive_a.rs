//! Act 2 "Hive" weak / normal monsters (spec 04 §3.3): the Bowlbugs, Chomper, Tunneler, Exoskeleton, HunterKiller,
//! LouseProgenitor, Myte, Ovicopter + ToughEgg, SlumberingBeetle, SpinyToad, TheObscura + Parafright, ThievingHopper.
//!
//! Encounter slot indices (positions in the encounter's `Slots` list; `NO` where the encounter has no `Slots` list):
//! Exoskeletons first..fourth = 0..3; Mytes first/second = 0/1; TheObscura illusion/obscura = 0/1;
//! Ovicopter egg1..egg5 = 0..4, ovicopter = 5.

use super::ovg_util::*;
use crate::dec::Dec;
use crate::defs::*;
use crate::ids;
use crate::state::*;
use crate::types::*;

pub const SLOT_OVICOPTER: u8 = 5;
pub const OVICOPTER_SLOTS: u8 = 6;
pub const SLOT_OBSCURA_ILLUSION: u8 = 0;
pub const SLOT_OBSCURA: u8 = 1;
pub const SLOT_MYTE_FIRST: u8 = 0;
pub const SLOT_MYTE_SECOND: u8 = 1;

// ---- Bowlbugs ----------------------------------------------------------------------------------------------------------

// 0 BITE_MOVE (self loop)
pub static BOWLBUG_EGG_DEF: MonsterDef = MonsterDef {
    id: ids::monster::BOWLBUG_EGG,
    hp: |a| hp(a, (23, 24), (21, 22)),
    initial: 0,
    on_spawn: None,
    nodes: &[mv(
        "BITE_MOVE",
        |cx, me| {
            let d = a9(cx, 8, 7);
            atk(cx, me, d);
            let b = a9(cx, 8, 7);
            block(cx, me, b);
        },
        &[attack(|cx, _| a9(cx, 8, 7)), Intent::Defend],
        0,
    )],
};

// 0 THRASH_MOVE -> 1 BUFF_MOVE -> 2 THRASH2_MOVE (loop)
fn nectar_thrash(cx: &mut Combat, me: Cid) {
    atk(cx, me, 3);
}
pub static BOWLBUG_NECTAR_DEF: MonsterDef = MonsterDef {
    id: ids::monster::BOWLBUG_NECTAR,
    hp: |a| hp(a, (36, 39), (35, 38)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv("THRASH_MOVE", nectar_thrash, &[attack(|_, _| 3)], 1),
        mv(
            "BUFF_MOVE",
            |cx, me| {
                let s = a9(cx, 16, 15);
                power_self(cx, me, ids::power::STRENGTH_POWER, s)
            },
            &[Intent::Buff],
            2,
        ),
        mv("THRASH2_MOVE", nectar_thrash, &[attack(|_, _| 3)], 2),
    ],
};

// BowlbugRock: vars[0] = IsOffBalance.
// 0 HEADBUTT_MOVE -> 1 POST_HEADBUTT {DIZZY if off balance else HEADBUTT}; 2 DIZZY_MOVE -> HEADBUTT
fn rock_dizzy(cx: &mut Combat, me: Cid) {
    cx.creatures[me as usize].monster.vars[0] = 0;
}
pub static BOWLBUG_ROCK_DEF: MonsterDef = MonsterDef {
    id: ids::monster::BOWLBUG_ROCK,
    hp: |a| hp(a, (46, 49), (45, 48)),
    initial: 0,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::IMBALANCED_POWER, 1)),
    nodes: &[
        mv(
            "HEADBUTT_MOVE",
            |cx, me| {
                let d = a9(cx, 16, 15);
                atk(cx, me, d);
                if cx.cr(me).monster.vars[0] != 0 {
                    // Stun(): CreatureCmd.Stun(Creature, DizzyMove) — next = last logged move
                    cx.stun(me, Some(rock_dizzy), None);
                }
            },
            &[attack(|cx, _| a9(cx, 16, 15))],
            1,
        ),
        cond("POST_HEADBUTT", &[(2, |cx, c| cx.cr(c).monster.vars[0] != 0), (0, |cx, c| cx.cr(c).monster.vars[0] == 0)]),
        mv("DIZZY_MOVE", rock_dizzy, &[Intent::Stun], 0),
    ],
};

// 0 THRASH_MOVE <-> 1 TOXIC_SPIT_MOVE; initial TOXIC_SPIT
pub static BOWLBUG_SILK_DEF: MonsterDef = MonsterDef {
    id: ids::monster::BOWLBUG_SILK,
    hp: |a| hp(a, (41, 44), (40, 43)),
    initial: 1,
    on_spawn: None,
    nodes: &[
        mv(
            "THRASH_MOVE",
            |cx, me| {
                let d = a9(cx, 5, 4);
                atk_n(cx, me, d, 2)
            },
            &[multi(|cx, _| a9(cx, 5, 4), |_, _| 2)],
            1,
        ),
        mv("TOXIC_SPIT_MOVE", |cx, me| power_player(cx, me, ids::power::WEAK_POWER, 1), &[Intent::Debuff], 0),
    ],
};

// ---- Chomper -----------------------------------------------------------------------------------------------------------

// vars[0] = ScreamFirst. 0 CLAMP_MOVE <-> 1 SCREECH_MOVE; 2 INIT (initial state = screamFirst ? SCREECH : CLAMP)
pub static CHOMPER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::CHOMPER,
    hp: |a| hp(a, (63, 67), (60, 64)),
    initial: 2,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::ARTIFACT_POWER, 2)),
    nodes: &[
        mv(
            "CLAMP_MOVE",
            |cx, me| {
                let d = a9(cx, 9, 8);
                atk_n(cx, me, d, 2)
            },
            &[multi(|cx, _| a9(cx, 9, 8), |_, _| 2)],
            1,
        ),
        mv("SCREECH_MOVE", |cx, _| status_to_discard(cx, ids::card::DAZED, 3), &[Intent::StatusCard], 0),
        cond("INIT", &[(1, |cx, c| cx.cr(c).monster.vars[0] != 0), (0, |cx, c| cx.cr(c).monster.vars[0] == 0)]),
    ],
};

// ---- Tunneler ----------------------------------------------------------------------------------------------------------

const TUNNELER_BITE: u8 = 0;

/// `Tunneler.StillDizzyMove` (`IsStunned = false`): no combat-visible effect.
fn tunneler_still_dizzy(_cx: &mut Combat, _me: Cid) {}

/// `BurrowedPower.AfterBlockBroken`: `GetStunned()` (`IsStunned = true`) then `Stun(StillDizzyMove, "BITE_MOVE")`.
pub fn tunneler_get_stunned(cx: &mut Combat, me: Cid) {
    cx.stun(me, Some(tunneler_still_dizzy), Some(TUNNELER_BITE));
}

// 0 BITE -> 1 BURROW -> 2 BELOW (loop); 3 DIZZY (stun, -> BITE; only used through `Stun`)
pub static TUNNELER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::TUNNELER,
    hp: |a| hp(a, (92, 92), (87, 87)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv(
            "BITE_MOVE",
            |cx, me| {
                let d = a9(cx, 15, 13);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 15, 13))],
            1,
        ),
        mv(
            "BURROW_MOVE",
            |cx, me| {
                power_self(cx, me, ids::power::BURROWED_POWER, 1);
                let b = a8(cx, 37, 32);
                block(cx, me, b);
            },
            &[Intent::Buff, Intent::Defend],
            2,
        ),
        mv(
            "BELOW_MOVE",
            |cx, me| {
                let d = a9(cx, 26, 23);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 26, 23))],
            2,
        ),
        mv("DIZZY_MOVE", tunneler_still_dizzy, &[Intent::Stun], 0),
    ],
};

// ---- Exoskeleton -------------------------------------------------------------------------------------------------------

// 0 INIT_MOVE (slot cond), 1 RAND {SKITTER:CNR, MANDIBLES:CNR}, 2 SKITTER -> RAND, 3 MANDIBLES -> ENRAGE, 4 ENRAGE -> RAND
pub static EXOSKELETON_DEF: MonsterDef = MonsterDef {
    id: ids::monster::EXOSKELETON,
    hp: |a| hp(a, (26, 30), (24, 28)),
    initial: 0,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::HARD_TO_KILL_POWER, 9)),
    nodes: &[
        cond(
            "INIT_MOVE",
            &[
                (2, |cx, c| cx.cr(c).slot == 0),
                (3, |cx, c| cx.cr(c).slot == 1),
                (4, |cx, c| cx.cr(c).slot == 2),
                (1, |cx, c| cx.cr(c).slot == 3),
            ],
        ),
        rand("RAND", &[Branch::new(2).cannot_repeat(), Branch::new(3).cannot_repeat()]),
        mv(
            "SKITTER_MOVE",
            |cx, me| {
                let n = a9(cx, 4, 3);
                atk_n(cx, me, 1, n)
            },
            &[multi(|_, _| 1, |cx, _| a9(cx, 4, 3))],
            1,
        ),
        mv(
            "MANDIBLES_MOVE",
            |cx, me| {
                let d = a9(cx, 9, 8);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 9, 8))],
            4,
        ),
        mv("ENRAGE_MOVE", |cx, me| power_self(cx, me, ids::power::STRENGTH_POWER, 2), &[Intent::Buff], 1),
    ],
};

// ---- HunterKiller ------------------------------------------------------------------------------------------------------

// 0 TENDERIZING_GOOP, 1 BITE, 2 PUNCTURE, 3 RAND {BITE:CNR, PUNCTURE:x2}; all -> RAND
pub static HUNTER_KILLER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::HUNTER_KILLER,
    hp: |a| hp(a, (126, 126), (121, 121)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv("TENDERIZING_GOOP_MOVE", |cx, me| power_player(cx, me, ids::power::TENDER_POWER, 1), &[Intent::Debuff], 3),
        mv(
            "BITE_MOVE",
            |cx, me| {
                let d = a9(cx, 19, 17);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 19, 17))],
            3,
        ),
        mv(
            "PUNCTURE_MOVE",
            |cx, me| {
                let d = a9(cx, 8, 7);
                atk_n(cx, me, d, 3)
            },
            &[multi(|cx, _| a9(cx, 8, 7), |_, _| 3)],
            3,
        ),
        rand("RAND", &[Branch::new(1).cannot_repeat(), Branch::new(2).max_repeats(2)]),
    ],
};

// ---- LouseProgenitor ---------------------------------------------------------------------------------------------------

// vars[0] = Curled (cosmetic). 0 WEB_CANNON -> 1 CURL_AND_GROW -> 2 POUNCE -> WEB_CANNON
pub static LOUSE_PROGENITOR_DEF: MonsterDef = MonsterDef {
    id: ids::monster::LOUSE_PROGENITOR,
    hp: |a| hp(a, (138, 141), (134, 136)),
    initial: 0,
    on_spawn: Some(|cx, me| {
        let n = a8(cx, 18, 14);
        power_self(cx, me, ids::power::CURL_UP_POWER, n)
    }),
    nodes: &[
        mv(
            "WEB_CANNON_MOVE",
            |cx, me| {
                cx.creatures[me as usize].monster.vars[0] = 0;
                let d = a9(cx, 10, 9);
                atk(cx, me, d);
                power_player(cx, me, ids::power::FRAIL_POWER, 2);
            },
            &[attack(|cx, _| a9(cx, 10, 9)), Intent::Debuff],
            1,
        ),
        mv(
            "CURL_AND_GROW_MOVE",
            |cx, me| {
                let b = a8(cx, 18, 14);
                block(cx, me, b);
                let s = a9(cx, 7, 5);
                power_self(cx, me, ids::power::STRENGTH_POWER, s);
                cx.creatures[me as usize].monster.vars[0] = 1;
            },
            &[Intent::Defend, Intent::Buff],
            2,
        ),
        mv(
            "POUNCE_MOVE",
            |cx, me| {
                cx.creatures[me as usize].monster.vars[0] = 0;
                let d = a9(cx, 16, 14);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 16, 14))],
            0,
        ),
    ],
};

// ---- Myte --------------------------------------------------------------------------------------------------------------

// 0 TOXIC -> 1 BITE -> 2 SUCK -> TOXIC; 3 INIT_MOVE by slot (first: TOXIC, second: SUCK)
pub static MYTE_DEF: MonsterDef = MonsterDef {
    id: ids::monster::MYTE,
    hp: |a| hp(a, (64, 69), (61, 67)),
    initial: 3,
    on_spawn: None,
    nodes: &[
        mv(
            "TOXIC_MOVE",
            |cx, _| {
                cx.add_status_cards(ids::card::TOXIC, PileType::Hand, 2, CardPilePosition::Bottom);
            },
            &[Intent::StatusCard],
            1,
        ),
        mv(
            "BITE_MOVE",
            |cx, me| {
                let d = a9(cx, 15, 13);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 15, 13))],
            2,
        ),
        mv(
            "SUCK_MOVE",
            |cx, me| {
                let d = a9(cx, 6, 4);
                atk(cx, me, d);
                let s = a9(cx, 3, 2);
                power_self(cx, me, ids::power::STRENGTH_POWER, s);
            },
            &[attack(|cx, _| a9(cx, 6, 4)), Intent::Buff],
            0,
        ),
        cond(
            "INIT_MOVE",
            &[(0, |cx, c| cx.cr(c).slot == SLOT_MYTE_FIRST), (2, |cx, c| cx.cr(c).slot == SLOT_MYTE_SECOND)],
        ),
    ],
};

// ---- Ovicopter + ToughEgg ----------------------------------------------------------------------------------------------

/// `CanLay`: at most 3 living teammates (itself included).
fn ovicopter_can_lay(cx: &Combat, _c: Cid) -> bool {
    cx.enemies.iter().filter(|&&e| cx.cr(e).is_alive()).count() <= 3
}

// 0 LAY_EGGS -> 1 SMASH -> 2 TENDERIZER -> 3 SUMMON_BRANCH {LAY_EGGS if CanLay else NUTRITIONAL_PASTE}; 4 NUTRITIONAL_PASTE -> SMASH
pub static OVICOPTER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::OVICOPTER,
    hp: |a| hp(a, (126, 132), (124, 130)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv(
            "LAY_EGGS_MOVE",
            |cx, me| {
                for _ in 0..3 {
                    // Slots.LastOrDefault(s => Enemies.All(c => c.SlotName != s))
                    let slot = cx.last_free_slot(OVICOPTER_SLOTS);
                    if slot == NO {
                        continue;
                    }
                    if let Some(egg) = cx.summon_enemy(ids::monster::TOUGH_EGG, slot, [0, 0]) {
                        cx.apply_power(ids::power::MINION_POWER, egg, Dec::ONE, me, NO);
                    }
                }
            },
            &[Intent::Summon],
            1,
        ),
        mv(
            "SMASH_MOVE",
            |cx, me| {
                let d = a9(cx, 17, 16);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 17, 16))],
            2,
        ),
        mv(
            "TENDERIZER_MOVE",
            |cx, me| {
                let d = a9(cx, 8, 7);
                atk(cx, me, d);
                power_player(cx, me, ids::power::VULNERABLE_POWER, 2);
            },
            &[attack(|cx, _| a9(cx, 8, 7)), Intent::Debuff],
            3,
        ),
        cond("SUMMON_BRANCH_STATE", &[(0, ovicopter_can_lay), (4, |cx, c| !ovicopter_can_lay(cx, c))]),
        mv(
            "NUTRITIONAL_PASTE_MOVE",
            |cx, me| {
                let s = a9(cx, 4, 3);
                power_self(cx, me, ids::power::STRENGTH_POWER, s)
            },
            &[Intent::Buff],
            1,
        ),
    ],
};

fn tough_egg_hatch(cx: &mut Combat, me: Cid) {
    // `PowerCmd.Remove<HatchPower>`, then every power that is not a MinionPower (list order), then the hatchling HP.
    cx.remove_power_by_id(me, ids::power::HATCH_POWER);
    let mut uids: crate::util::ArrayVec<u16, MAX_POWERS> = crate::util::ArrayVec::new();
    for p in cx.cr(me).powers.iter() {
        if p.id != ids::power::MINION_POWER {
            uids.push(p.uid);
        }
    }
    for &u in uids.iter() {
        cx.remove_power(me, u);
    }
    let lo = a8(cx, 20, 19);
    let hi = a8(cx, 23, 22);
    let hp = cx.rng.niche.next_int_range(lo, hi + 1);
    cx.set_max_hp(me, Dec::int(hp as i64));
    cx.set_current_hp(me, Dec::int(hp as i64));
}

// 0 HATCH_MOVE -> 1 NIBBLE_MOVE (loop)
pub static TOUGH_EGG_DEF: MonsterDef = MonsterDef {
    id: ids::monster::TOUGH_EGG,
    hp: |a| hp(a, (15, 19), (14, 18)),
    initial: 0,
    on_spawn: Some(|cx, me| {
        // `IsHatched` is false for every egg this port creates
        let n = if cx.side != Side::Enemy { 1 } else { 2 };
        power_self(cx, me, ids::power::HATCH_POWER, n);
    }),
    nodes: &[
        mv("HATCH_MOVE", tough_egg_hatch, &[Intent::Summon], 1),
        mv(
            "NIBBLE_MOVE",
            |cx, me| {
                let d = a9(cx, 5, 4);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 5, 4))],
            1,
        ),
    ],
};

// ---- SlumberingBeetle --------------------------------------------------------------------------------------------------

const BEETLE_ROLL_OUT: u8 = 2;

/// `SlumberingBeetle.WakeUpMove`: `IsAwake = true`, strip the Plating.
pub fn beetle_wake_up(cx: &mut Combat, me: Cid) {
    cx.remove_power_by_id(me, ids::power::PLATING_POWER);
}
fn beetle_wake_up_move(cx: &mut Combat, me: Cid) {
    beetle_wake_up(cx, me)
}
/// `SlumberPower`: `CreatureCmd.Stun(Owner, WakeUpMove, "ROLL_OUT_MOVE")`.
pub fn beetle_wake_up_stun(cx: &mut Combat, me: Cid) {
    cx.stun(me, Some(beetle_wake_up_move), Some(BEETLE_ROLL_OUT));
}

// 0 SNORE_MOVE -> 1 SNORE_NEXT {SNORE while Slumber else ROLL_OUT}; 2 ROLL_OUT_MOVE (loop)
pub static SLUMBERING_BEETLE_DEF: MonsterDef = MonsterDef {
    id: ids::monster::SLUMBERING_BEETLE,
    hp: |a| hp(a, (89, 89), (86, 86)),
    initial: 0,
    on_spawn: Some(|cx, me| {
        let p = a8(cx, 18, 15);
        power_self(cx, me, ids::power::PLATING_POWER, p);
        power_self(cx, me, ids::power::SLUMBER_POWER, 3);
    }),
    nodes: &[
        mv("SNORE_MOVE", nothing, &[Intent::Sleep], 1),
        cond(
            "SNORE_NEXT",
            &[
                (0, |cx, c| cx.has_power(c, ids::power::SLUMBER_POWER)),
                (BEETLE_ROLL_OUT, |cx, c| !cx.has_power(c, ids::power::SLUMBER_POWER)),
            ],
        ),
        mv(
            "ROLL_OUT_MOVE",
            |cx, me| {
                let d = a9(cx, 18, 16);
                atk(cx, me, d);
                power_self(cx, me, ids::power::STRENGTH_POWER, 2);
            },
            &[attack(|cx, _| a9(cx, 18, 16)), Intent::Buff],
            BEETLE_ROLL_OUT,
        ),
    ],
};

// ---- SpinyToad ---------------------------------------------------------------------------------------------------------

// vars[0] = IsSpiny (cosmetic). 0 PROTRUDING_SPIKES -> 1 SPIKE_EXPLOSION -> 2 TONGUE_LASH -> PROTRUDING_SPIKES
pub static SPINY_TOAD_DEF: MonsterDef = MonsterDef {
    id: ids::monster::SPINY_TOAD,
    hp: |a| hp(a, (121, 124), (116, 119)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv(
            "PROTRUDING_SPIKES_MOVE",
            |cx, me| {
                cx.creatures[me as usize].monster.vars[0] = 1;
                power_self(cx, me, ids::power::THORNS_POWER, 5);
            },
            &[Intent::Buff],
            1,
        ),
        mv(
            "SPIKE_EXPLOSION_MOVE",
            |cx, me| {
                cx.creatures[me as usize].monster.vars[0] = 0;
                let d = a9(cx, 25, 23);
                atk(cx, me, d);
                power_self(cx, me, ids::power::THORNS_POWER, -5);
            },
            &[attack(|cx, _| a9(cx, 25, 23))],
            2,
        ),
        mv(
            "TONGUE_LASH_MOVE",
            |cx, me| {
                let d = a9(cx, 19, 17);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 19, 17))],
            0,
        ),
    ],
};

// ---- TheObscura + Parafright -------------------------------------------------------------------------------------------

// 0 ILLUSION_MOVE -> RAND; 1 RAND {PIERCING_GAZE:CNR, SAIL:CNR, HARDENING_STRIKE:CNR}; 2 PIERCING_GAZE, 3 SAIL, 4 HARDENING_STRIKE
pub static THE_OBSCURA_DEF: MonsterDef = MonsterDef {
    id: ids::monster::THE_OBSCURA,
    hp: |a| hp(a, (129, 129), (123, 123)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv(
            "ILLUSION_MOVE",
            |cx, _| {
                if cx.in_progress {
                    cx.summon_enemy(ids::monster::PARAFRIGHT, SLOT_OBSCURA_ILLUSION, [0, 0]);
                }
            },
            &[Intent::Summon],
            1,
        ),
        rand("RAND", &[Branch::new(2).cannot_repeat(), Branch::new(3).cannot_repeat(), Branch::new(4).cannot_repeat()]),
        mv(
            "PIERCING_GAZE_MOVE",
            |cx, me| {
                let d = a9(cx, 11, 10);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 11, 10))],
            1,
        ),
        mv(
            "SAIL_MOVE",
            |cx, me| {
                // PowerCmd.Apply<StrengthPower>(CombatState.GetTeammatesOf(Creature), 3m, Creature): every enemy, list order
                let team = cx.enemies.clone();
                for &t in team.iter() {
                    cx.apply_power(ids::power::STRENGTH_POWER, t, Dec::int(3), me, NO);
                }
            },
            &[Intent::Buff],
            1,
        ),
        mv(
            "HARDENING_STRIKE_MOVE",
            |cx, me| {
                let d = a9(cx, 7, 6);
                atk(cx, me, d);
                let b = a9(cx, 7, 6);
                block(cx, me, b);
            },
            &[attack(|cx, _| a9(cx, 7, 6)), Intent::Defend],
            1,
        ),
    ],
};

// 0 SLAM_MOVE (loop), 1 REVIVE_MOVE (IllusionPower; successor stored at runtime)
pub static PARAFRIGHT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::PARAFRIGHT,
    hp: |_| (21, 21),
    initial: 0,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::ILLUSION_POWER, 1)),
    nodes: &[
        mv(
            "SLAM_MOVE",
            |cx, me| {
                let d = a9(cx, 17, 16);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 17, 16))],
            0,
        ),
        mv_once("REVIVE_MOVE", |cx, me| crate::content::powers::overgrowth::illusion_revive(cx, me), &[Intent::Heal], FOLLOW_STORED),
    ],
};

// ---- ThievingHopper ----------------------------------------------------------------------------------------------------

/// `ThievingHopper._stealPriorities[tier]` on a combat card with a deck version.
fn steal_tier_matches(cx: &Combat, tier: usize, c: CardIdx) -> bool {
    let card = &cx.cards[c as usize];
    let imbued = card.enchant != 0 && (card.enchant - 1) as u16 == ids::enchantment::IMBUED;
    let rarity = cx.card_def(c).rarity;
    match tier {
        0 => !imbued && rarity == CardRarity::Uncommon,
        1 => !imbued && matches!(rarity, CardRarity::Common | CardRarity::Rare | CardRarity::Event),
        2 => !imbued && matches!(rarity, CardRarity::Basic | CardRarity::Quest),
        _ => rarity == CardRarity::Ancient || imbued,
    }
}

/// `ThievingHopper.ThieveryMove`: steal a card of the first non-empty priority tier from Draw + Discard (only cards
/// with a deck version), then attack. The card leaves the combat; the run-level deck / reward bookkeeping of
/// `SwipePower` is not modelled (the oracle trace only shows the combat piles).
fn hopper_thievery(cx: &mut Combat, me: Cid) {
    let mut stolen: crate::util::ArrayVec<CardIdx, 2> = crate::util::ArrayVec::new();
    if !cx.cr(PLAYER).is_dead() {
        let mut list: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
        for &c in cx.player.draw.iter().chain(cx.player.discard.iter()) {
            if cx.cards[c as usize].deck_idx != NO {
                list.push(c);
            }
        }
        let mut chosen: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
        for tier in 0..4 {
            for &c in list.iter() {
                if steal_tier_matches(cx, tier, c) {
                    chosen.push(c);
                }
            }
            if !chosen.is_empty() {
                break;
            }
        }
        if chosen.is_empty() {
            chosen = list;
        }
        if !chosen.is_empty() {
            let k = cx.rng.combat_card_generation.next_int(chosen.len() as i32) as usize;
            let card = chosen[k];
            cx.remove_card_from_combat(card);
            stolen.push(card);
        }
    }
    for &card in stolen.iter() {
        if let Some(uid) = cx.apply_power(ids::power::SWIPE_POWER, me, Dec::ONE, me, NO) {
            cx.set_power_aux(me, uid, card as i32);
        }
    }
    let d = a9(cx, 19, 17);
    atk(cx, me, d);
}

// 0 THIEVERY -> 1 FLUTTER -> 2 HAT_TRICK -> 3 NAB -> 4 ESCAPE (loop)
pub static THIEVING_HOPPER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::THIEVING_HOPPER,
    hp: |a| hp(a, (84, 84), (79, 79)),
    initial: 0,
    on_spawn: Some(|cx, me| power_self(cx, me, ids::power::ESCAPE_ARTIST_POWER, 5)),
    nodes: &[
        mv("THIEVERY_MOVE", hopper_thievery, &[attack(|cx, _| a9(cx, 19, 17)), Intent::CardDebuff], 1),
        mv("FLUTTER_MOVE", |cx, me| power_self(cx, me, ids::power::FLUTTER_POWER, 5), &[Intent::Buff], 2),
        mv(
            "HAT_TRICK_MOVE",
            |cx, me| {
                let d = a9(cx, 23, 21);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 23, 21))],
            3,
        ),
        mv(
            "NAB_MOVE",
            |cx, me| {
                let d = a9(cx, 16, 14);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 16, 14))],
            4,
        ),
        mv("ESCAPE_MOVE", |cx, me| cx.escape(me), &[Intent::Escape], 4),
    ],
};
