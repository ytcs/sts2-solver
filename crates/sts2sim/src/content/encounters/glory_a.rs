use crate::content::monsters::glory_a::SLOT_FABRICATOR;
use crate::content::{Spawn, Spawns};
use crate::ids;
use crate::rng::Rng;
use crate::types::NO;

fn push(s: &mut Spawns, m: u16) {
    s.push(Spawn { monster: m, slot: NO, vars: [0, 0] });
}

fn one(m: u16) -> Spawns {
    let mut s = Spawns::new();
    push(&mut s, m);
    s
}

pub fn spawn_devoted_sculptor_weak(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::DEVOTED_SCULPTOR)
}

fn scrolls(rng: &mut Rng, count: usize) -> Spawns {
    let n = rng.next_int(3);
    let mut s = Spawns::new();
    for k in 0..count {
        let idx = if k == 3 { 2 } else { (n + k as i32) % 3 };
        s.push(Spawn { monster: ids::monster::SCROLL_OF_BITING, slot: NO, vars: [idx, 0] });
    }
    s
}

pub fn spawn_scrolls_of_biting_weak(rng: &mut Rng, _ascension: u8) -> Spawns {
    scrolls(rng, 3)
}

pub fn spawn_scrolls_of_biting_normal(rng: &mut Rng, _ascension: u8) -> Spawns {
    scrolls(rng, 4)
}

pub fn spawn_turret_operator_weak(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    push(&mut s, ids::monster::LIVING_SHIELD);
    push(&mut s, ids::monster::TURRET_OPERATOR);
    s
}

pub fn spawn_axebots_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::AXEBOT, slot: 0, vars: [0, 0] });
    s
}

pub fn spawn_construct_menagerie_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    push(&mut s, ids::monster::PUNCH_CONSTRUCT);
    push(&mut s, ids::monster::CUBEX_CONSTRUCT);
    push(&mut s, ids::monster::CUBEX_CONSTRUCT);
    s
}

pub fn spawn_fabricator_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::FABRICATOR, slot: SLOT_FABRICATOR, vars: [0, 0] });
    s
}

pub fn spawn_frog_knight_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::FROG_KNIGHT)
}

pub fn spawn_globe_head_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::GLOBE_HEAD)
}

pub fn spawn_owl_magistrate_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::OWL_MAGISTRATE)
}

pub fn spawn_slimed_berserker_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::SLIMED_BERSERKER)
}

pub fn spawn_the_lost_and_forgotten_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    push(&mut s, ids::monster::THE_LOST);
    push(&mut s, ids::monster::THE_FORGOTTEN);
    s
}
