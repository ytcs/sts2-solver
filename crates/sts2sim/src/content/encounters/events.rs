//! Event-driven encounters (spec 04 §4.2 "Event-driven encounters"): real combats that are in no act pool. The events only pick
//! the encounter (no combat-time player modification), so the oracle scenario's `encounter` id is all that is needed.

use crate::content::{Spawn, Spawns};
use crate::ids;
use crate::rng::Rng;
use crate::types::NO;

fn one(m: u16) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: m, slot: NO, vars: [0, 0] });
    s
}

pub fn spawn_battleworn_dummy_event_v1_encounter(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::BATTLE_FRIEND_V1)
}
pub fn spawn_battleworn_dummy_event_v2_encounter(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::BATTLE_FRIEND_V2)
}
pub fn spawn_battleworn_dummy_event_v3_encounter(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::BATTLE_FRIEND_V3)
}

/// Four Wrigglers (`StartStunned = false`, vars[0] = 0) in slots wriggler1..4. The Wriggler's INIT_MOVE keys off the slot; its
/// port uses the Phrog encounter's numbering (phrog = 0, wriggler1..4 = 1..4), so the event uses slot indices 1..=4 (the
/// slot value is only used for ordering and for that comparison; slot names are not part of the compared state).
pub fn spawn_dense_vegetation_event_encounter(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    for i in 0..4u8 {
        s.push(Spawn { monster: ids::monster::WRIGGLER, slot: crate::content::monsters::phrog::wriggler_slot(i), vars: [0, 0] });
    }
    s
}

pub fn spawn_fake_merchant_event_encounter(_rng: &mut Rng, _ascension: u8) -> Spawns {
    // slot "merchant" (the only slot)
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::FAKE_MERCHANT_MONSTER, slot: 0, vars: [0, 0] });
    s
}

pub fn spawn_mysterious_knight_event_encounter(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::MYSTERIOUS_KNIGHT)
}

pub fn spawn_the_architect_event_encounter(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::ARCHITECT)
}
