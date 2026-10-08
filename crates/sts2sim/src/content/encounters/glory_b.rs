use crate::content::{Spawn, Spawns};
use crate::ids;
use crate::rng::Rng;
use crate::types::NO;

fn one(m: u16) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: m, slot: NO, vars: [0, 0] });
    s
}

pub fn spawn_knights_elite(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::FLAIL_KNIGHT, slot: NO, vars: [0, 0] });
    s.push(Spawn { monster: ids::monster::SPECTRAL_KNIGHT, slot: NO, vars: [0, 0] });
    s.push(Spawn { monster: ids::monster::MAGI_KNIGHT, slot: NO, vars: [0, 0] });
    s
}

pub fn spawn_mecha_knight_elite(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::MECHA_KNIGHT)
}

pub fn spawn_soul_nexus_elite(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::SOUL_NEXUS)
}

pub fn spawn_aeonglass_boss(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::AEONGLASS)
}

pub fn spawn_queen_boss(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::TORCH_HEAD_AMALGAM, slot: 0, vars: [0, 0] });
    s.push(Spawn { monster: ids::monster::QUEEN, slot: 1, vars: [0, 0] });
    s
}

pub fn spawn_test_subject_boss(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::TEST_SUBJECT)
}
