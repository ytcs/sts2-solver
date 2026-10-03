//! Act 2 "Hive" weak + normal encounters (spec 04 §4.2). Monsters in creation order; `rng` = the encounter-local RNG.
//! Slot indices follow each encounter's `Slots` list; encounters without a `Slots` list use `NO` (creation order).

use crate::content::monsters::hive_a as hive;
use crate::content::{Spawn, Spawns};
use crate::ids;
use crate::rng::Rng;
use crate::types::NO;

fn push(s: &mut Spawns, m: u16, slot: u8, vars: [i32; 2]) {
    s.push(Spawn { monster: m, slot, vars });
}

fn one(m: u16) -> Spawns {
    let mut s = Spawns::new();
    push(&mut s, m, NO, [0, 0]);
    s
}

/// BowlbugRock@odd, `NextItem([BowlbugEgg, BowlbugNectar])`@even (no `Slots` list).
pub fn spawn_bowlbugs_weak(rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    push(&mut s, ids::monster::BOWLBUG_ROCK, NO, [0, 0]);
    let bugs = [ids::monster::BOWLBUG_EGG, ids::monster::BOWLBUG_NECTAR];
    let bug = bugs[rng.next_int(2) as usize];
    push(&mut s, bug, NO, [0, 0]);
    s
}

/// 3x Exoskeleton in slots first, second, third.
pub fn spawn_exoskeletons_weak(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    for i in 0..3 {
        push(&mut s, ids::monster::EXOSKELETON, i, [0, 0]);
    }
    s
}

pub fn spawn_thieving_hopper_weak(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::THIEVING_HOPPER)
}

pub fn spawn_tunneler_weak(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::TUNNELER)
}

/// BowlbugRock@first, then two picks from {Egg, Silk, Nectar} without repeating a type
/// (`NextItem(types with count < 1)` twice; dictionary insertion order) @middle, @last (no `Slots` list).
pub fn spawn_bowlbugs_normal(rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    push(&mut s, ids::monster::BOWLBUG_ROCK, NO, [0, 0]);
    let mut left = [ids::monster::BOWLBUG_EGG, ids::monster::BOWLBUG_SILK, ids::monster::BOWLBUG_NECTAR];
    let mut n = 3usize;
    for _ in 0..2 {
        let k = rng.next_int(n as i32) as usize;
        push(&mut s, left[k], NO, [0, 0]);
        for j in k..n - 1 {
            left[j] = left[j + 1];
        }
        n -= 1;
    }
    s
}

/// Chomper, Chomper(`ScreamFirst`) (vars[0]).
pub fn spawn_chompers_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    push(&mut s, ids::monster::CHOMPER, NO, [0, 0]);
    push(&mut s, ids::monster::CHOMPER, NO, [1, 0]);
    s
}

/// 4x Exoskeleton in slots first..fourth.
pub fn spawn_exoskeletons_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    for i in 0..4 {
        push(&mut s, ids::monster::EXOSKELETON, i, [0, 0]);
    }
    s
}

pub fn spawn_hunter_killer_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::HUNTER_KILLER)
}

pub fn spawn_louse_progenitor_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::LOUSE_PROGENITOR)
}

/// Myte@first, Myte@second; Slots [first, second].
pub fn spawn_mytes_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    push(&mut s, ids::monster::MYTE, hive::SLOT_MYTE_FIRST, [0, 0]);
    push(&mut s, ids::monster::MYTE, hive::SLOT_MYTE_SECOND, [0, 0]);
    s
}

/// Ovicopter@ovicopter; Slots [egg1..egg5, ovicopter].
pub fn spawn_ovicopter_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    push(&mut s, ids::monster::OVICOPTER, hive::SLOT_OVICOPTER, [0, 0]);
    s
}

/// BowlbugRock@first, BowlbugSilk@second, SlumberingBeetle@third (no `Slots` list).
pub fn spawn_slumbering_beetle_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    push(&mut s, ids::monster::BOWLBUG_ROCK, NO, [0, 0]);
    push(&mut s, ids::monster::BOWLBUG_SILK, NO, [0, 0]);
    push(&mut s, ids::monster::SLUMBERING_BEETLE, NO, [0, 0]);
    s
}

pub fn spawn_spiny_toad_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::SPINY_TOAD)
}

/// TheObscura@obscura; Slots [illusion, obscura] (Parafright later fills `illusion`).
pub fn spawn_the_obscura_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    push(&mut s, ids::monster::THE_OBSCURA, hive::SLOT_OBSCURA, [0, 0]);
    s
}
