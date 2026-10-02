//! Act 1a "Overgrowth" encounters (spec 04 §4.2). Monsters in creation order; `rng` = the encounter-local RNG.
//! (`NibbitsWeak` lives in basic.rs.)

use crate::content::monsters::{fogmog, kin, phrog};
use crate::content::{Spawn, Spawns};
use crate::ids;
use crate::rng::Rng;
use crate::types::NO;

fn one(m: u16) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: m, slot: NO, vars: [0, 0] });
    s
}

fn push(s: &mut Spawns, m: u16) {
    s.push(Spawn { monster: m, slot: NO, vars: [0, 0] });
}

const SMALL_SLIMES: [u16; 2] = [ids::monster::LEAF_SLIME_S, ids::monster::TWIG_SLIME_S];
const MEDIUM_SLIMES: [u16; 2] = [ids::monster::LEAF_SLIME_M, ids::monster::TWIG_SLIME_M];

/// `Rng.NextItem(list)` on an array.
fn next_item(rng: &mut Rng, items: &[u16]) -> u16 {
    items[rng.next_int(items.len() as i32) as usize]
}

pub fn spawn_fuzzy_wurm_crawler_weak(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::FUZZY_WURM_CRAWLER)
}

pub fn spawn_shrinker_beetle_weak(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::SHRINKER_BEETLE)
}

/// [small1, medium, small2]; draws: small1 (n=2), small2 (n=1), medium (n=2).
pub fn spawn_slimes_weak(rng: &mut Rng, _ascension: u8) -> Spawns {
    let small1_i = rng.next_int(2) as usize;
    let small1 = SMALL_SLIMES[small1_i];
    let remaining = [SMALL_SLIMES[1 - small1_i]];
    let small2 = next_item(rng, &remaining);
    let medium = next_item(rng, &MEDIUM_SLIMES);
    let mut s = Spawns::new();
    push(&mut s, small1);
    push(&mut s, medium);
    push(&mut s, small2);
    s
}

pub fn spawn_cubex_construct_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::CUBEX_CONSTRUCT)
}

pub fn spawn_flyconid_normal(rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    let m = next_item(rng, &MEDIUM_SLIMES);
    push(&mut s, m);
    push(&mut s, ids::monster::FLYCONID);
    s
}

/// Slots [illusion, fogmog]; Fogmog sits in `fogmog`.
pub fn spawn_fogmog_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::FOGMOG, slot: fogmog::SLOT_FOGMOG, vars: [0, 0] });
    s
}

/// Three Inklets, the middle one with `MiddleInklet = true` (vars[0]).
pub fn spawn_inklets_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    push(&mut s, ids::monster::INKLET);
    s.push(Spawn { monster: ids::monster::INKLET, slot: NO, vars: [1, 0] });
    push(&mut s, ids::monster::INKLET);
    s
}

pub fn spawn_mawler_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::MAWLER)
}

/// Slots [front, back] -> indices 0 / 1; the front Nibbit has `IsFront` (vars[1]); vars[0] = IsAlone = false.
pub fn spawn_nibbits_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::NIBBIT, slot: 0, vars: [0, 1] });
    s.push(Spawn { monster: ids::monster::NIBBIT, slot: 1, vars: [0, 0] });
    s
}

pub fn spawn_overgrowth_crawlers(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    push(&mut s, ids::monster::SHRINKER_BEETLE);
    push(&mut s, ids::monster::FUZZY_WURM_CRAWLER);
    s
}

/// Three distinct raiders: `NextItem(types not yet used)` x3, key order Axe, Assassin, Brute, Crossbow, Tracker.
pub fn spawn_ruby_raiders_normal(rng: &mut Rng, _ascension: u8) -> Spawns {
    let all = [
        ids::monster::AXE_RUBY_RAIDER,
        ids::monster::ASSASSIN_RUBY_RAIDER,
        ids::monster::BRUTE_RUBY_RAIDER,
        ids::monster::CROSSBOW_RUBY_RAIDER,
        ids::monster::TRACKER_RUBY_RAIDER,
    ];
    let mut used = [false; 5];
    let mut s = Spawns::new();
    for _ in 0..3 {
        let mut avail = [0u16; 5];
        let mut idx = [0usize; 5];
        let mut n = 0;
        for i in 0..5 {
            if !used[i] {
                avail[n] = all[i];
                idx[n] = i;
                n += 1;
            }
        }
        let k = rng.next_int(n as i32) as usize;
        used[idx[k]] = true;
        push(&mut s, avail[k]);
    }
    s
}

/// [TwigSlimeM, LeafSlimeM, a, b], `flag = NextBool()`: a = flag ? LeafSlimeS : TwigSlimeS, b = the other.
pub fn spawn_slimes_normal(rng: &mut Rng, _ascension: u8) -> Spawns {
    let flag = rng.next_bool();
    let (a, b) = if flag {
        (ids::monster::LEAF_SLIME_S, ids::monster::TWIG_SLIME_S)
    } else {
        (ids::monster::TWIG_SLIME_S, ids::monster::LEAF_SLIME_S)
    };
    let mut s = Spawns::new();
    push(&mut s, ids::monster::TWIG_SLIME_M);
    push(&mut s, ids::monster::LEAF_SLIME_M);
    push(&mut s, a);
    push(&mut s, b);
    s
}

/// `NextItem({SnappingJaxfruit, MediumSlime, SmallSlimes})` then its own draws; the Strangler is added last.
pub fn spawn_slithering_strangler_normal(rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    match rng.next_int(3) {
        0 => push(&mut s, ids::monster::SNAPPING_JAXFRUIT),
        1 => {
            let m = next_item(rng, &MEDIUM_SLIMES);
            push(&mut s, m);
        }
        _ => {
            let a = next_item(rng, &SMALL_SLIMES);
            push(&mut s, a);
            let b = next_item(rng, &SMALL_SLIMES);
            push(&mut s, b);
        }
    }
    push(&mut s, ids::monster::SLITHERING_STRANGLER);
    s
}

pub fn spawn_snapping_jaxfruit_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    push(&mut s, ids::monster::SNAPPING_JAXFRUIT);
    push(&mut s, ids::monster::FLYCONID);
    s
}

pub fn spawn_vine_shambler_normal(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::VINE_SHAMBLER)
}

pub fn spawn_bygone_effigy_elite(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::BYGONE_EFFIGY)
}

pub fn spawn_byrdonis_elite(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::BYRDONIS)
}

/// Slots [phrog, wriggler1..4]; the Phrog sits in `phrog`.
pub fn spawn_phrog_parasite_elite(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::PHROG_PARASITE, slot: phrog::SLOT_PHROG, vars: [0, 0] });
    s
}

pub fn spawn_ceremonial_beast_boss(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::CEREMONIAL_BEAST)
}

/// KinFollower(StartsWithDance)@slot1, KinFollower@slot2, KinPriest@leaderSlot.
pub fn spawn_the_kin_boss(_rng: &mut Rng, _ascension: u8) -> Spawns {
    let mut s = Spawns::new();
    s.push(Spawn { monster: ids::monster::KIN_FOLLOWER, slot: kin::SLOT_1, vars: [1, 0] });
    s.push(Spawn { monster: ids::monster::KIN_FOLLOWER, slot: kin::SLOT_2, vars: [0, 0] });
    s.push(Spawn { monster: ids::monster::KIN_PRIEST, slot: kin::SLOT_LEADER, vars: [0, 0] });
    s
}

pub fn spawn_vantom_boss(_rng: &mut Rng, _ascension: u8) -> Spawns {
    one(ids::monster::VANTOM)
}
