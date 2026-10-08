use crate::content::monsters::hive_b::*;
use crate::content::{Spawn, Spawns};
use crate::ids;
use crate::rng::Rng;
use crate::types::NO;

fn one(m: u16) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: m, slot: NO, vars: [0, 0] });
    s
}

pub fn spawn_decimillipede_elite(rng: &mut Rng, _ascension: u8) -> Spawns {
    let n = rng.next_int(3);
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::DECIMILLIPEDE_SEGMENT_FRONT, slot: SEGMENT_SLOT_FRONT, vars: [n, 0] });
    s.push(Spawn { monster: ids::monster::DECIMILLIPEDE_SEGMENT_MIDDLE, slot: SEGMENT_SLOT_MIDDLE, vars: [(n + 1) % 3, 0] });
    s.push(Spawn { monster: ids::monster::DECIMILLIPEDE_SEGMENT_BACK, slot: SEGMENT_SLOT_BACK, vars: [(n + 2) % 3, 0] });
    s
}

pub fn spawn_entomancer_elite(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::ENTOMANCER)
}

pub fn spawn_infested_prisms_elite(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::INFESTED_PRISM)
}

pub fn spawn_kaiser_crab_boss(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::CRUSHER, slot: SLOT_CRUSHER, vars: [0, 0] });
    s.push(Spawn { monster: ids::monster::ROCKET, slot: SLOT_ROCKET, vars: [0, 0] });
    s
}

pub fn spawn_knowledge_demon_boss(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::KNOWLEDGE_DEMON)
}

pub fn spawn_the_insatiable_boss(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::THE_INSATIABLE)
}
