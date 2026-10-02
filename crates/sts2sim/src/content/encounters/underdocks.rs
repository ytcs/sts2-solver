//! Act 1b "Underdocks" encounter compositions (spec 04 §4.2). Monsters in creation order; `slot` is the index into the
//! encounter's `Slots` list (`NO` for encounters without one). Encounter-local RNG draws use `rng` (seed already
//! combines run seed, floor and encounter id).

use crate::content::{Spawn, Spawns};
use crate::ids;
use crate::rng::Rng;
use crate::types::NO;

/// Number of entries of the encounter's `Slots` list (0 = none); used by `Combat::free_slot` (`GetNextSlot`).
pub fn slot_count(enc: u16) -> u8 {
    match enc {
        ids::encounter::LIVING_FOG_NORMAL => 6,       // bomb1..bomb5, livingFog
        ids::encounter::TWO_TAILED_RATS_NORMAL => 5,  // first..fifth
        ids::encounter::PHANTASMAL_GARDENERS_ELITE => 4, // first..fourth
        _ => 0,
    }
}

fn one(monster: u16) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster, slot: NO, vars: [0, 0] });
    s
}

/// `CorpseSlug.EnsureCorpseSlugsStartWithDifferentMoves`: `n = NextInt(3)`, slug k starts with move `(n + k) % 3`.
fn slugs(rng: &mut Rng, count: usize) -> Spawns {
    let mut n = rng.next_int(3);
    let mut s = Spawns::new();
    for _ in 0..count {
        s.push(Spawn { monster: ids::monster::CORPSE_SLUG, slot: NO, vars: [n % 3, 0] });
        n += 1;
    }
    s
}

pub fn spawn_corpse_slugs_weak(rng: &mut Rng, _ascension: u8) -> Spawns {
    slugs(rng, 2)
}
pub fn spawn_corpse_slugs_normal(rng: &mut Rng, _ascension: u8) -> Spawns {
    slugs(rng, 3)
}
pub fn spawn_seapunk_weak(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::SEAPUNK)
}
pub fn spawn_sludge_spinner_weak(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::SLUDGE_SPINNER)
}
pub fn spawn_toadpoles_weak(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::TOADPOLE, slot: NO, vars: [1, 0] }); // IsFront
    s.push(Spawn { monster: ids::monster::TOADPOLE, slot: NO, vars: [0, 0] });
    s
}
pub fn spawn_cultists_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::CALCIFIED_CULTIST, slot: NO, vars: [0, 0] });
    s.push(Spawn { monster: ids::monster::DAMP_CULTIST, slot: NO, vars: [0, 0] });
    s
}
pub fn spawn_fossil_stalker_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::FOSSIL_STALKER)
}
pub fn spawn_gremlin_merc_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    // slot "merc" (the encounter has no `Slots` list, so it never reorders anything)
    one(ids::monster::GREMLIN_MERC)
}
pub fn spawn_haunted_ship_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::HAUNTED_SHIP)
}
pub fn spawn_living_fog_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::LIVING_FOG, slot: 5, vars: [0, 0] }); // "livingFog"
    s
}
pub fn spawn_punch_construct_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::PUNCH_CONSTRUCT)
}
pub fn spawn_seapunk_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::CALCIFIED_CULTIST, slot: NO, vars: [0, 0] });
    s.push(Spawn { monster: ids::monster::SEAPUNK, slot: NO, vars: [0, 0] });
    s
}
pub fn spawn_sewer_clam_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::SEWER_CLAM)
}
pub fn spawn_two_tailed_rats_normal(rng: &mut Rng, _ascension: u8) -> Spawns {
    let n = rng.next_int(3);
    let mut s = Spawns::new();
    for k in 0..3 {
        // slots third, fourth, fifth; StarterMoveIndex = (n + k) % 3
        s.push(Spawn { monster: ids::monster::TWO_TAILED_RAT, slot: 2 + k as u8, vars: [(n + k) % 3, 0] });
    }
    s
}
pub fn spawn_phantasmal_gardeners_elite(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    for k in 0..4u8 {
        s.push(Spawn { monster: ids::monster::PHANTASMAL_GARDENER, slot: k, vars: [0, 0] });
    }
    s
}
pub fn spawn_skulking_colony_elite(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::SKULKING_COLONY)
}
pub fn spawn_terror_eel_elite(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::TERROR_EEL)
}
pub fn spawn_lagavulin_matriarch_boss(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::LAGAVULIN_MATRIARCH)
}
pub fn spawn_soul_fysh_boss(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::SOUL_FYSH)
}
pub fn spawn_waterfall_giant_boss(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::WATERFALL_GIANT)
}
