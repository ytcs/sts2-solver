//! Encounter compositions (spec 04 §4): monsters in creation order. Encounter-local RNG draws (R0) use `rng`.

use crate::content::{Spawn, Spawns};
use crate::ids;
use crate::rng::Rng;
use crate::types::NO;

pub fn spawn_nibbits_weak(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::NIBBIT, slot: NO, vars: [1, 0] }); // IsAlone
    s
}
