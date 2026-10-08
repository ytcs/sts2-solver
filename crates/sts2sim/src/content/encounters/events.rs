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

pub fn spawn_dense_vegetation_event_encounter(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    for i in 0..4u8 {
        s.push(Spawn { monster: ids::monster::WRIGGLER, slot: crate::content::monsters::phrog::wriggler_slot(i), vars: [0, 0] });
    }
    s
}

pub fn spawn_fake_merchant_event_encounter(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::FAKE_MERCHANT_MONSTER, slot: 0, vars: [0, 0] });
    s
}

pub fn spawn_mysterious_knight_event_encounter(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::MYSTERIOUS_KNIGHT)
}

pub fn spawn_punch_off_event_encounter(rng: &mut Rng, _ascension: u8) -> Spawns {
    let r0 = rng.next_int_range(2, 10);
    let r1 = rng.next_int_range(2, 10);
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::PUNCH_CONSTRUCT, slot: NO, vars: [1, r0] });
    s.push(Spawn { monster: ids::monster::PUNCH_CONSTRUCT, slot: NO, vars: [0, r1] });
    s
}

pub fn spawn_tunneler_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::CHOMPER, slot: NO, vars: [1, 0] });
    s.push(Spawn { monster: ids::monster::TUNNELER, slot: NO, vars: [0, 0] });
    s
}

pub fn spawn_the_architect_event_encounter(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::ARCHITECT)
}
