//! TEMP-DEP: NIBBITS_NORMAL (two enemies) so multi-enemy behaviour can be swept in this branch. Drop before merge.
use crate::content::{Spawn, Spawns};
use crate::ids;
use crate::rng::Rng;

pub fn spawn_nibbits_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::NIBBIT, slot: 0, vars: [0, 1] }); // front
    s.push(Spawn { monster: ids::monster::NIBBIT, slot: 1, vars: [0, 0] }); // back
    s
}
