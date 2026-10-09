pub mod gen_cards;
pub mod gen_pools;
pub mod gen_potions;
pub mod gen_relics;
pub mod gen_powers;

use crate::defs::*;
use crate::hooks::*;
use crate::ids;

pub struct NoListener;
impl Listener for NoListener {}
static NO_LISTENER: NoListener = NoListener;

macro_rules! registry {
    ($lfn:ident, $mfn:ident, $ids:ident, $default:expr; $($id:ident => $ty:path),* $(,)?) => {
        #[inline]
        pub fn $lfn(id: u16) -> &'static dyn Listener {
            const fn listener_of(id: u16) -> &'static dyn Listener {
                match id { $( ids::$ids::$id => &$ty, )* _ => $default }
            }
            static TABLE: [&'static dyn Listener; ids::$ids::COUNT] = {
                let mut t: [&'static dyn Listener; ids::$ids::COUNT] = [$default; ids::$ids::COUNT];
                let mut i = 0;
                while i < ids::$ids::COUNT {
                    t[i] = listener_of(i as u16);
                    i += 1;
                }
                t
            };
            match TABLE.get(id as usize) {
                Some(l) => *l,
                None => listener_of(id),
            }
        }
        #[inline]
        pub fn $mfn(id: u16) -> Mask {
            const fn mask_of(id: u16) -> Mask {
                match id { $( ids::$ids::$id => <$ty as HasMask>::MASK, )* _ => Mask::EMPTY }
            }
            static TABLE: [Mask; ids::$ids::COUNT] = {
                let mut t = [Mask::EMPTY; ids::$ids::COUNT];
                let mut i = 0;
                while i < ids::$ids::COUNT {
                    t[i] = mask_of(i as u16);
                    i += 1;
                }
                t
            };
            match TABLE.get(id as usize) {
                Some(m) => *m,
                None => mask_of(id),
            }
        }
    };
    (impl $ifn:ident, $ids:ident; $($id:ident => $ty:path),* $(,)?) => {
        #[inline]
        pub fn $ifn(id: u16) -> bool {
            const fn implemented(id: u16) -> bool {
                $( if id == ids::$ids::$id { return true; } )*
                let _ = id;
                false
            }
            static TABLE: [bool; ids::$ids::COUNT] = {
                let mut t = [false; ids::$ids::COUNT];
                let mut i = 0;
                while i < ids::$ids::COUNT {
                    t[i] = implemented(i as u16);
                    i += 1;
                }
                t
            };
            match TABLE.get(id as usize) {
                Some(b) => *b,
                None => implemented(id),
            }
        }
    };
}

#[inline]
pub fn listener(me: &Me) -> &'static dyn Listener {
    match me.kind {
        Kind::Power => power_listener(me.id),
        Kind::Relic => relic_listener(me.id),
        Kind::Potion => potion_listener(me.id),
        Kind::Card => card_listener(me.id),
        Kind::Monster => monster_listener(me.id),
        Kind::Enchantment => enchantment_listener(me.id),
        Kind::Affliction => affliction_listener(me.id),
        Kind::Orb => &NO_LISTENER,
    }
}

#[inline(always)]
pub fn card_def(id: u16) -> &'static CardDef {
    &gen_cards::CARD_DEFS[id as usize]
}

#[inline(always)]
pub fn potion_def(id: u16) -> &'static PotionDef {
    &gen_potions::POTION_DEFS[id as usize]
}

#[inline(always)]
pub fn power_def(id: u16) -> &'static PowerDef {
    &gen_powers::POWER_DEFS[id as usize]
}

#[derive(Clone, Copy)]
pub struct Spawn {
    pub monster: u16,
    pub slot: u8,
    pub vars: [i32; 2],
}
pub type Spawns = crate::util::ArrayVec<Spawn, 16>;

include!(concat!(env!("OUT_DIR"), "/registry.rs"));

pub fn monster_hp_bonus(id: u16, vars: [i32; 2]) -> i32 {
    if id == ids::monster::AXEBOT {
        return monsters::glory_a::axebot_hp_bonus(vars);
    }
    0
}

pub fn node_by_name(monster: u16, name: &str) -> Option<u8> {
    let def = monster_def(monster);
    def.nodes.iter().position(|n| match n {
        MonsterNode::Move { id, .. } | MonsterNode::Random { id, .. } | MonsterNode::Cond { id, .. } => *id == name,
    }).map(|i| i as u8)
}
