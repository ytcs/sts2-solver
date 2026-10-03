//! Act 1b "Underdocks" monsters, part A: the normal / weak-pool monsters (spec 04 §3.2).
//! Node indices are per-monster; `follow_up` / branch targets refer to them. All ascension values come from the
//! monster properties via `asc::val` (A8 = tough enemies, A9 = deadly enemies).

use crate::dec::Dec;
use crate::defs::*;
use crate::engine::Attack;
use crate::ids;
use crate::state::*;
use crate::types::*;

// ---- shared helpers ------------------------------------------------------------------------------------------------

/// `Encounter.GetNextSlot` (`last == false`: first free slot) / `Slots.LastOrDefault(free)` (`last == true`) over the
/// encounter's `n` slots; `NO` if none is free. (Only TwoTailedRatsNormal (5 slots) and LivingFogNormal (6) summon.)
pub fn free_slot(cx: &Combat, n: u8, last: bool) -> u8 {
    if last {
        (0..n).rev().find(|&s| !cx.enemies.iter().any(|&e| cx.cr(e).slot == s)).unwrap_or(NO)
    } else {
        cx.next_free_slot(n)
    }
}

#[inline]
fn deadly(cx: &Combat, asc9: i32, base: i32) -> i32 {
    asc::val(asc::DEADLY_ENEMIES, cx.ascension, asc9, base)
}
#[inline]
fn tough(cx: &Combat, asc8: i32, base: i32) -> i32 {
    asc::val(asc::TOUGH_ENEMIES, cx.ascension, asc8, base)
}
/// `DamageCmd.Attack(dmg).WithHitCount(n).FromMonster(me)` against the player.
fn hits(cx: &mut Combat, me: Cid, dmg: i32, n: i32) {
    cx.execute_attack(&Attack::from_monster(me, dmg).hits(n));
}
/// `PowerCmd.Apply<T>(player, amount, me, null)`.
fn debuff(cx: &mut Combat, power: u16, amount: i32, me: Cid) {
    cx.apply_power(power, PLAYER, Dec::int(amount as i64), me, NO);
}
/// `PowerCmd.Apply<T>(me, amount, me, null)`.
fn buff(cx: &mut Combat, power: u16, amount: i32, me: Cid) {
    cx.apply_power(power, me, Dec::int(amount as i64), me, NO);
}

macro_rules! mv {
    ($id:expr, $perform:expr, [$($intent:expr),*], $follow:expr) => {
        MonsterNode::Move { id: $id, perform: $perform, intents: &[$($intent),*], follow_up: $follow, must_perform_once: false }
    };
}
/// Single-hit attack intent whose damage is `$f(cx)`.
macro_rules! atk {
    ($f:path) => {
        Intent::Attack { damage: |cx, _| $f(cx), hits: |_, _| 1 }
    };
    ($f:path, $n:expr) => {
        Intent::Attack { damage: |cx, _| $f(cx), hits: |_, _| $n }
    };
}
macro_rules! hp {
    ($lo:expr, $hi:expr, $lo8:expr, $hi8:expr) => {
        |a| if a >= asc::TOUGH_ENEMIES { ($lo8, $hi8) } else { ($lo, $hi) }
    };
}

// ---- cultists ---------------------------------------------------------------------------------------------------------

mod calcified_cultist {
    use super::*;
    pub fn dark_strike(cx: &Combat) -> i32 {
        deadly(cx, 11, 9)
    }
}
// nodes: 0 INCANTATION, 1 DARK_STRIKE
pub static CALCIFIED_CULTIST_DEF: MonsterDef = MonsterDef {
    id: ids::monster::CALCIFIED_CULTIST,
    hp: hp!(38, 41, 39, 42),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv!("INCANTATION_MOVE", |cx, me| buff(cx, ids::power::RITUAL_POWER, 2, me), [Intent::Buff], 1),
        mv!("DARK_STRIKE_MOVE", |cx, me| hits(cx, me, calcified_cultist::dark_strike(cx), 1), [atk!(calcified_cultist::dark_strike)], 1),
    ],
};

mod damp_cultist {
    use super::*;
    pub fn dark_strike(cx: &Combat) -> i32 {
        deadly(cx, 3, 1)
    }
    pub fn incantation(cx: &Combat) -> i32 {
        deadly(cx, 6, 5)
    }
}
pub static DAMP_CULTIST_DEF: MonsterDef = MonsterDef {
    id: ids::monster::DAMP_CULTIST,
    hp: hp!(51, 53, 52, 54),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv!("INCANTATION_MOVE", |cx, me| buff(cx, ids::power::RITUAL_POWER, damp_cultist::incantation(cx), me), [Intent::Buff], 1),
        mv!("DARK_STRIKE_MOVE", |cx, me| hits(cx, me, damp_cultist::dark_strike(cx), 1), [atk!(damp_cultist::dark_strike)], 1),
    ],
};

// ---- Seapunk -----------------------------------------------------------------------------------------------------------

mod seapunk {
    use super::*;
    pub fn sea_kick(cx: &Combat) -> i32 {
        deadly(cx, 13, 11)
    }
    pub fn bubble_block(cx: &Combat) -> i32 {
        tough(cx, 8, 7)
    }
    pub fn bubble_str(cx: &Combat) -> i32 {
        deadly(cx, 2, 1)
    }
    pub fn spinning(_cx: &Combat) -> i32 {
        2
    }
}
// nodes: 0 SEA_KICK, 1 SPINNING_KICK, 2 BUBBLE_BURP
pub static SEAPUNK_DEF: MonsterDef = MonsterDef {
    id: ids::monster::SEAPUNK,
    hp: hp!(44, 46, 47, 49),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv!("SEA_KICK_MOVE", |cx, me| hits(cx, me, seapunk::sea_kick(cx), 1), [atk!(seapunk::sea_kick)], 1),
        mv!("SPINNING_KICK_MOVE", |cx, me| hits(cx, me, 2, 4), [atk!(seapunk::spinning, 4)], 2),
        mv!(
            "BUBBLE_BURP_MOVE",
            |cx, me| {
                let b = seapunk::bubble_block(cx);
                cx.gain_block(me, Dec::int(b as i64), ValueProp::MOVE, NO);
                let s = seapunk::bubble_str(cx);
                buff(cx, ids::power::STRENGTH_POWER, s, me);
            },
            [Intent::Buff, Intent::Defend],
            0
        ),
    ],
};

// ---- SludgeSpinner -----------------------------------------------------------------------------------------------------

mod sludge_spinner {
    use super::*;
    pub fn oil_spray(cx: &Combat) -> i32 {
        deadly(cx, 9, 8)
    }
    pub fn slam(cx: &Combat) -> i32 {
        deadly(cx, 12, 11)
    }
    pub fn rage(cx: &Combat) -> i32 {
        deadly(cx, 7, 6)
    }
}
// nodes: 0 RAND, 1 OIL_SPRAY, 2 SLAM, 3 RAGE; INIT = OIL_SPRAY
pub static SLUDGE_SPINNER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::SLUDGE_SPINNER,
    hp: hp!(37, 39, 41, 42),
    initial: 1,
    on_spawn: None,
    nodes: &[
        MonsterNode::Random {
            id: "RAND",
            branches: &[Branch::new(1).cannot_repeat(), Branch::new(2).cannot_repeat(), Branch::new(3).cannot_repeat()],
        },
        mv!(
            "OIL_SPRAY_MOVE",
            |cx, me| {
                hits(cx, me, sludge_spinner::oil_spray(cx), 1);
                debuff(cx, ids::power::WEAK_POWER, 1, me);
            },
            [atk!(sludge_spinner::oil_spray), Intent::Debuff],
            0
        ),
        mv!("SLAM_MOVE", |cx, me| hits(cx, me, sludge_spinner::slam(cx), 1), [atk!(sludge_spinner::slam)], 0),
        mv!(
            "RAGE_MOVE",
            |cx, me| {
                hits(cx, me, sludge_spinner::rage(cx), 1);
                buff(cx, ids::power::STRENGTH_POWER, 3, me);
            },
            [atk!(sludge_spinner::rage), Intent::Buff],
            0
        ),
    ],
};

// ---- HauntedShip -------------------------------------------------------------------------------------------------------

mod haunted_ship {
    use super::*;
    pub fn swipe(cx: &Combat) -> i32 {
        deadly(cx, 14, 13)
    }
    pub fn stomp(cx: &Combat) -> i32 {
        deadly(cx, 5, 4)
    }
}
// nodes: 0 SWIPE, 1 STOMP, 2 HAUNT (INIT, once)
pub static HAUNTED_SHIP_DEF: MonsterDef = MonsterDef {
    id: ids::monster::HAUNTED_SHIP,
    hp: hp!(63, 63, 67, 67),
    initial: 2,
    on_spawn: None,
    nodes: &[
        mv!("SWIPE_MOVE", |cx, me| hits(cx, me, haunted_ship::swipe(cx), 1), [atk!(haunted_ship::swipe)], 1),
        mv!("STOMP_MOVE", |cx, me| hits(cx, me, haunted_ship::stomp(cx), 3), [atk!(haunted_ship::stomp, 3)], 0),
        mv!(
            "HAUNT_MOVE",
            |cx, me| {
                debuff(cx, ids::power::WEAK_POWER, 3, me);
                // CardPileCmd.AddToCombatAndPreview<Dazed>(player, Discard, 5)
                if cx.cr(PLAYER).is_alive() {
                    for _ in 0..5 {
                        if let Some(c) = cx.new_card(ids::card::DAZED, 0) {
                            cx.add_generated_card(c, PileType::Discard, CardPilePosition::Bottom);
                        }
                    }
                }
            },
            [Intent::Debuff, Intent::StatusCard],
            0
        ),
    ],
};

// (PunchConstruct lives in glory_a.rs.)

// ---- SewerClam ---------------------------------------------------------------------------------------------------------

mod sewer_clam {
    use super::*;
    pub fn jet(cx: &Combat) -> i32 {
        deadly(cx, 11, 10)
    }
    pub fn on_spawn(cx: &mut Combat, me: Cid) {
        let p = tough(cx, 9, 8);
        buff(cx, ids::power::PLATING_POWER, p, me);
    }
}
// nodes: 0 PRESSURIZE, 1 JET (INIT)
pub static SEWER_CLAM_DEF: MonsterDef = MonsterDef {
    id: ids::monster::SEWER_CLAM,
    hp: hp!(56, 56, 58, 58),
    initial: 1,
    on_spawn: Some(sewer_clam::on_spawn),
    nodes: &[
        mv!("PRESSURIZE_MOVE", |cx, me| buff(cx, ids::power::STRENGTH_POWER, 4, me), [Intent::Buff], 1),
        mv!("JET_MOVE", |cx, me| hits(cx, me, sewer_clam::jet(cx), 1), [atk!(sewer_clam::jet)], 0),
    ],
};

// ---- FossilStalker -----------------------------------------------------------------------------------------------------

mod fossil_stalker {
    use super::*;
    pub fn tackle(cx: &Combat) -> i32 {
        deadly(cx, 11, 9)
    }
    pub fn latch(cx: &Combat) -> i32 {
        deadly(cx, 14, 12)
    }
    pub fn lash(cx: &Combat) -> i32 {
        deadly(cx, 4, 3)
    }
    pub fn on_spawn(cx: &mut Combat, me: Cid) {
        buff(cx, ids::power::SUCK_POWER, 3, me);
    }
}
// nodes: 0 RAND, 1 TACKLE, 2 LATCH (INIT), 3 LASH
pub static FOSSIL_STALKER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::FOSSIL_STALKER,
    hp: hp!(51, 53, 54, 56),
    initial: 2,
    on_spawn: Some(fossil_stalker::on_spawn),
    nodes: &[
        MonsterNode::Random { id: "RAND", branches: &[Branch::new(2).max_repeats(2), Branch::new(1).max_repeats(2), Branch::new(3).max_repeats(2)] },
        mv!(
            "TACKLE_MOVE",
            |cx, me| {
                hits(cx, me, fossil_stalker::tackle(cx), 1);
                debuff(cx, ids::power::FRAIL_POWER, 1, me);
            },
            [atk!(fossil_stalker::tackle), Intent::Debuff],
            0
        ),
        mv!("LATCH_MOVE", |cx, me| hits(cx, me, fossil_stalker::latch(cx), 1), [atk!(fossil_stalker::latch)], 0),
        mv!("LASH_MOVE", |cx, me| hits(cx, me, fossil_stalker::lash(cx), 2), [atk!(fossil_stalker::lash, 2)], 0),
    ],
};

// ---- Toadpole ----------------------------------------------------------------------------------------------------------

mod toadpole {
    use super::*;
    pub fn spit(cx: &Combat) -> i32 {
        deadly(cx, 4, 3)
    }
    pub fn whirl(cx: &Combat) -> i32 {
        deadly(cx, 8, 7)
    }
}
// vars[0] = IsFront. nodes: 0 INIT (cond), 1 SPIKEN, 2 SPIKE_SPIT, 3 WHIRL
pub static TOADPOLE_DEF: MonsterDef = MonsterDef {
    id: ids::monster::TOADPOLE,
    hp: hp!(21, 25, 22, 26),
    initial: 0,
    on_spawn: None,
    nodes: &[
        MonsterNode::Cond { id: "INIT_MOVE", arms: &[(3, |cx, c| cx.cr(c).monster.vars[0] == 0), (1, |cx, c| cx.cr(c).monster.vars[0] != 0)] },
        mv!("SPIKEN_MOVE", |cx, me| buff(cx, ids::power::THORNS_POWER, 2, me), [Intent::Buff], 2),
        mv!(
            "SPIKE_SPIT_MOVE",
            |cx, me| {
                buff(cx, ids::power::THORNS_POWER, -2, me);
                hits(cx, me, toadpole::spit(cx), 3);
            },
            [atk!(toadpole::spit, 3)],
            3
        ),
        mv!("WHIRL_MOVE", |cx, me| hits(cx, me, toadpole::whirl(cx), 1), [atk!(toadpole::whirl)], 1),
    ],
};

// ---- CorpseSlug --------------------------------------------------------------------------------------------------------

mod corpse_slug {
    use super::*;
    pub fn glomp(cx: &Combat) -> i32 {
        deadly(cx, 9, 8)
    }
    pub fn whip(_cx: &Combat) -> i32 {
        3
    }
    pub fn on_spawn(cx: &mut Combat, me: Cid) {
        let s = deadly(cx, 5, 4);
        buff(cx, ids::power::RAVENOUS_POWER, s, me);
    }
}
// vars[0] = StarterMoveIdx. nodes: 0 INIT (cond), 1 WHIP_SLAP, 2 GLOMP, 3 GOOP
pub static CORPSE_SLUG_DEF: MonsterDef = MonsterDef {
    id: ids::monster::CORPSE_SLUG,
    hp: hp!(25, 27, 27, 29),
    initial: 0,
    on_spawn: Some(corpse_slug::on_spawn),
    nodes: &[
        MonsterNode::Cond {
            id: "INIT_MOVE",
            arms: &[(1, |cx, c| cx.cr(c).monster.vars[0] % 3 == 0), (2, |cx, c| cx.cr(c).monster.vars[0] % 3 == 1), (3, |_, _| true)],
        },
        mv!("WHIP_SLAP_MOVE", |cx, me| hits(cx, me, 3, 2), [atk!(corpse_slug::whip, 2)], 2),
        mv!("GLOMP_MOVE", |cx, me| hits(cx, me, corpse_slug::glomp(cx), 1), [atk!(corpse_slug::glomp)], 3),
        mv!("GOOP_MOVE", |cx, me| debuff(cx, ids::power::FRAIL_POWER, 2, me), [Intent::Debuff], 1),
    ],
};

// ---- TwoTailedRat ------------------------------------------------------------------------------------------------------

mod two_tailed_rat {
    use super::*;
    pub const CALL_FOR_BACKUP: u8 = 4;
    pub const SLOTS: u8 = 5;
    pub fn scratch(cx: &Combat) -> i32 {
        deadly(cx, 9, 8)
    }
    pub fn bite(cx: &Combat) -> i32 {
        deadly(cx, 7, 6)
    }
    /// `CanSummon()`. vars[2] = non-summon moves performed (`_turnsUntilSummonable = 2 - vars[2]`), vars[3] = CallForBackupCount.
    pub fn can_summon(cx: &Combat, me: Cid) -> bool {
        let ms = &cx.cr(me).monster;
        if 2 - ms.vars[2] > 0 || ms.vars[3] >= 3 {
            return false;
        }
        if free_slot(cx, SLOTS, false) == NO {
            return false;
        }
        !cx.enemies.iter().any(|&e| e != me && cx.cr(e).monster.next_move == CALL_FOR_BACKUP)
    }
    pub fn w_normal(cx: &Combat, me: Cid) -> f32 {
        if can_summon(cx, me) { 1.0f32 / 12.0f32 } else { 1.0 }
    }
    pub fn w_call(cx: &Combat, me: Cid) -> f32 {
        if can_summon(cx, me) { 0.75 } else { 0.0 }
    }
    pub fn tick(cx: &mut Combat, me: Cid) {
        cx.cr_mut(me).monster.vars[2] += 1;
    }
    pub fn call_for_backup(cx: &mut Combat, me: Cid) {
        let slot = free_slot(cx, SLOTS, true);
        if slot != NO {
            cx.summon_enemy(ids::monster::TWO_TAILED_RAT, slot, [-1, 0]);
        }
        let mut max = 0;
        for &e in cx.enemies.iter() {
            if cx.cr(e).monster.id == ids::monster::TWO_TAILED_RAT {
                max = max.max(cx.cr(e).monster.vars[3] + 1);
            }
        }
        let list = cx.enemies;
        for &e in list.iter() {
            if cx.cr(e).monster.id == ids::monster::TWO_TAILED_RAT {
                cx.cr_mut(e).monster.vars[3] = max;
            }
        }
        let _ = me;
    }
}
// vars[0] = StarterMoveIndex (-1 = random start), vars[2], vars[3] (see `can_summon`).
// nodes: 0 INIT (cond), 1 RAND, 2 SCRATCH, 3 DISEASE_BITE, 4 CALL_FOR_BACKUP (index fixed by `CALL_FOR_BACKUP`), 5 SCREECH
pub static TWO_TAILED_RAT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::TWO_TAILED_RAT,
    hp: hp!(17, 21, 18, 22),
    initial: 0,
    on_spawn: None,
    nodes: &[
        MonsterNode::Cond {
            id: "INIT_MOVE",
            arms: &[
                (1, |cx, c| cx.cr(c).monster.vars[0] == -1),
                (2, |cx, c| cx.cr(c).monster.vars[0] % 3 == 0),
                (3, |cx, c| cx.cr(c).monster.vars[0] % 3 == 1),
                (5, |_, _| true),
            ],
        },
        MonsterNode::Random {
            id: "RAND",
            branches: &[
                Branch { target: 2, repeat: Repeat::CanRepeatXTimes(1), cooldown: 0, weight: 1.0, weight_fn: Some(two_tailed_rat::w_normal) },
                Branch { target: 3, repeat: Repeat::CanRepeatXTimes(1), cooldown: 0, weight: 1.0, weight_fn: Some(two_tailed_rat::w_normal) },
                Branch { target: 5, repeat: Repeat::CanRepeatXTimes(1), cooldown: 3, weight: 1.0, weight_fn: Some(two_tailed_rat::w_normal) },
                Branch { target: 4, repeat: Repeat::UseOnlyOnce, cooldown: 0, weight: 1.0, weight_fn: Some(two_tailed_rat::w_call) },
            ],
        },
        mv!(
            "SCRATCH_MOVE",
            |cx, me| {
                two_tailed_rat::tick(cx, me);
                hits(cx, me, two_tailed_rat::scratch(cx), 1);
            },
            [atk!(two_tailed_rat::scratch)],
            1
        ),
        mv!(
            "DISEASE_BITE_MOVE",
            |cx, me| {
                two_tailed_rat::tick(cx, me);
                hits(cx, me, two_tailed_rat::bite(cx), 1);
            },
            [atk!(two_tailed_rat::bite)],
            1
        ),
        mv!("CALL_FOR_BACKUP_MOVE", two_tailed_rat::call_for_backup, [Intent::Summon], 1),
        mv!(
            "SCREECH_MOVE",
            |cx, me| {
                two_tailed_rat::tick(cx, me);
                debuff(cx, ids::power::FRAIL_POWER, 1, me);
            },
            [Intent::Debuff],
            1
        ),
    ],
};

// ---- GremlinMerc / FatGremlin / SneakyGremlin ----------------------------------------------------------------------------

mod gremlin_merc {
    use super::*;
    pub fn gimme(cx: &Combat) -> i32 {
        tough(cx, 8, 7)
    }
    pub fn smash(cx: &Combat) -> i32 {
        tough(cx, 7, 6)
    }
    pub fn hehe(cx: &Combat) -> i32 {
        tough(cx, 9, 8)
    }
    pub fn on_spawn(cx: &mut Combat, me: Cid) {
        buff(cx, ids::power::SURPRISE_POWER, 1, me);
        buff(cx, ids::power::THIEVERY_POWER, 20, me); // one instance per player
    }
    /// `foreach ThieveryPower: Steal()`: takes up to Amount gold from the player; `aux` accumulates the loot.
    pub fn steal(cx: &mut Combat, me: Cid) {
        let uids: crate::util::ArrayVec<u16, MAX_POWERS> = {
            let mut v = crate::util::ArrayVec::new();
            for p in cx.cr(me).powers.iter() {
                if p.id == ids::power::THIEVERY_POWER {
                    v.push(p.uid);
                }
            }
            v
        };
        for &uid in uids.iter() {
            if cx.cr(PLAYER).is_dead() || cx.gold <= 0 {
                continue;
            }
            if let Some(i) = cx.power_idx(me, uid) {
                let a = cx.cr(me).powers[i].amount.min(cx.gold);
                cx.gold -= a;
                cx.cr_mut(me).powers[i].aux += a;
            }
        }
    }
}
// nodes: 0 GIMME, 1 DOUBLE_SMASH, 2 HEHE
pub static GREMLIN_MERC_DEF: MonsterDef = MonsterDef {
    id: ids::monster::GREMLIN_MERC,
    hp: hp!(47, 49, 51, 53),
    initial: 0,
    on_spawn: Some(gremlin_merc::on_spawn),
    nodes: &[
        mv!(
            "GIMME_MOVE",
            |cx, me| {
                hits(cx, me, gremlin_merc::gimme(cx), 2);
                gremlin_merc::steal(cx, me);
            },
            [atk!(gremlin_merc::gimme, 2)],
            1
        ),
        mv!(
            "DOUBLE_SMASH_MOVE",
            |cx, me| {
                hits(cx, me, gremlin_merc::smash(cx), 2);
                gremlin_merc::steal(cx, me);
                debuff(cx, ids::power::WEAK_POWER, 2, me);
            },
            [atk!(gremlin_merc::smash, 2), Intent::Debuff],
            2
        ),
        mv!(
            "HEHE_MOVE",
            |cx, me| {
                hits(cx, me, gremlin_merc::hehe(cx), 1);
                gremlin_merc::steal(cx, me);
                buff(cx, ids::power::STRENGTH_POWER, 2, me);
            },
            [atk!(gremlin_merc::hehe), Intent::Buff],
            0
        ),
    ],
};

// nodes: 0 SPAWNED, 1 FLEE
pub static FAT_GREMLIN_DEF: MonsterDef = MonsterDef {
    id: ids::monster::FAT_GREMLIN,
    hp: hp!(13, 17, 14, 18),
    initial: 0,
    on_spawn: None,
    nodes: &[mv!("SPAWNED_MOVE", |_, _| {}, [Intent::Stun], 1), mv!("FLEE_MOVE", |cx, me| cx.escape(me), [Intent::Escape], 1)],
};

mod sneaky_gremlin {
    use super::*;
    pub fn tackle(cx: &Combat) -> i32 {
        deadly(cx, 10, 9)
    }
}
// nodes: 0 SPAWNED, 1 TACKLE
pub static SNEAKY_GREMLIN_DEF: MonsterDef = MonsterDef {
    id: ids::monster::SNEAKY_GREMLIN,
    hp: hp!(10, 14, 11, 15),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv!("SPAWNED_MOVE", |_, _| {}, [Intent::Stun], 1),
        mv!("TACKLE_MOVE", |cx, me| hits(cx, me, sneaky_gremlin::tackle(cx), 1), [atk!(sneaky_gremlin::tackle)], 1),
    ],
};

// ---- LivingFog / GasBomb -------------------------------------------------------------------------------------------------

mod living_fog {
    use super::*;
    pub fn gas(cx: &Combat) -> i32 {
        deadly(cx, 9, 8)
    }
    pub fn bloat(cx: &Combat) -> i32 {
        deadly(cx, 6, 5)
    }
    pub fn blast(cx: &Combat) -> i32 {
        deadly(cx, 9, 8)
    }
}
// nodes: 0 ADVANCED_GAS, 1 BLOAT, 2 SUPER_GAS_BLAST
pub static LIVING_FOG_DEF: MonsterDef = MonsterDef {
    id: ids::monster::LIVING_FOG,
    hp: hp!(80, 80, 82, 82),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv!(
            "ADVANCED_GAS_MOVE",
            |cx, me| {
                hits(cx, me, living_fog::gas(cx), 1);
                debuff(cx, ids::power::SMOGGY_POWER, 1, me);
            },
            [atk!(living_fog::gas), Intent::CardDebuff],
            1
        ),
        mv!(
            "BLOAT_MOVE",
            |cx, me| {
                // BloatAmount = 1 GasBomb into the first free slot, then the attack
                let slot = free_slot(cx, 6, false); // slots bomb1..bomb5, livingFog
                if slot != NO {
                    cx.summon_enemy(ids::monster::GAS_BOMB, slot, [0, 0]);
                }
                hits(cx, me, living_fog::bloat(cx), 1);
            },
            [atk!(living_fog::bloat), Intent::Summon],
            2
        ),
        mv!("SUPER_GAS_BLAST_MOVE", |cx, me| hits(cx, me, living_fog::blast(cx), 1), [atk!(living_fog::blast)], 1),
    ],
};

mod gas_bomb {
    use super::*;
    pub fn explode(cx: &Combat) -> i32 {
        deadly(cx, 9, 8)
    }
    pub fn on_spawn(cx: &mut Combat, me: Cid) {
        buff(cx, ids::power::MINION_POWER, 1, me);
    }
}
// nodes: 0 EXPLODE (attack, then the bomb kills itself)
pub static GAS_BOMB_DEF: MonsterDef = MonsterDef {
    id: ids::monster::GAS_BOMB,
    hp: hp!(7, 7, 8, 8),
    initial: 0,
    on_spawn: Some(gas_bomb::on_spawn),
    nodes: &[mv!(
        "EXPLODE_MOVE",
        |cx, me| {
            hits(cx, me, gas_bomb::explode(cx), 1);
            cx.kill(&[me]);
        },
        [Intent::DeathBlowAttack { damage: |cx, _| gas_bomb::explode(cx) }],
        NO
    )],
};
