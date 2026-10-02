//! Act 1b "Underdocks" monsters, part B: elites and bosses (spec 04 §3.2).

use crate::dec::Dec;
use crate::defs::*;
use crate::engine::Attack;
use crate::ids;
use crate::state::*;
use crate::types::*;

#[inline]
fn deadly(cx: &Combat, asc9: i32, base: i32) -> i32 {
    asc::val(asc::DEADLY_ENEMIES, cx.ascension, asc9, base)
}
#[inline]
fn tough(cx: &Combat, asc8: i32, base: i32) -> i32 {
    asc::val(asc::TOUGH_ENEMIES, cx.ascension, asc8, base)
}
fn hits(cx: &mut Combat, me: Cid, dmg: i32, n: i32) {
    cx.execute_attack(&Attack::from_monster(me, dmg).hits(n));
}
fn debuff(cx: &mut Combat, power: u16, amount: i32, me: Cid) {
    cx.apply_power(power, PLAYER, Dec::int(amount as i64), me, NO);
}
fn buff(cx: &mut Combat, power: u16, amount: i32, me: Cid) {
    cx.apply_power(power, me, Dec::int(amount as i64), me, NO);
}

macro_rules! mv {
    ($id:expr, $perform:expr, [$($intent:expr),*], $follow:expr) => {
        MonsterNode::Move { id: $id, perform: $perform, intents: &[$($intent),*], follow_up: $follow, must_perform_once: false }
    };
}
macro_rules! atk {
    ($f:path) => {
        Intent::Attack { damage: |cx, _| $f(cx), hits: |_, _| 1 }
    };
    ($f:path, $n:expr) => {
        Intent::Attack { damage: |cx, _| $f(cx), hits: |_, _| $n }
    };
}
macro_rules! hp {
    ($lo:expr, $hi:expr, $lo8:expr, $hi8:expr) => {
        |a| if a >= asc::TOUGH_ENEMIES { ($lo8, $hi8) } else { ($lo, $hi) }
    };
}

// ---- PhantasmalGardener (elite, x4 in slots first..fourth) ----------------------------------------------------------------

mod gardener {
    use super::*;
    pub fn bite(_cx: &Combat) -> i32 {
        5
    }
    pub fn lash(_cx: &Combat) -> i32 {
        7
    }
    pub fn flail(_cx: &Combat) -> i32 {
        1
    }
    pub fn enlarge(cx: &Combat) -> i32 {
        deadly(cx, 3, 2)
    }
    pub fn on_spawn(cx: &mut Combat, me: Cid) {
        let a = tough(cx, 7, 6);
        buff(cx, ids::power::SKITTISH_POWER, a, me);
    }
}
// slots: first=0 second=1 third=2 fourth=3. nodes: 0 INIT (cond on slot), 1 BITE, 2 LASH, 3 ENLARGE, 4 FLAIL
pub static PHANTASMAL_GARDENER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::PHANTASMAL_GARDENER,
    hp: hp!(26, 31, 27, 32),
    initial: 0,
    on_spawn: Some(gardener::on_spawn),
    nodes: &[
        MonsterNode::Cond {
            id: "INIT_MOVE",
            arms: &[(4, |cx, c| cx.cr(c).slot == 0), (1, |cx, c| cx.cr(c).slot == 1), (2, |cx, c| cx.cr(c).slot == 2), (3, |cx, c| cx.cr(c).slot == 3)],
        },
        mv!("BITE_MOVE", |cx, me| hits(cx, me, 5, 1), [atk!(gardener::bite)], 2),
        mv!("LASH_MOVE", |cx, me| hits(cx, me, 7, 1), [atk!(gardener::lash)], 4),
        mv!(
            "ENLARGE_MOVE",
            |cx, me| {
                let s = gardener::enlarge(cx);
                buff(cx, ids::power::STRENGTH_POWER, s, me);
                cx.cr_mut(me).monster.vars[2] += 1; // EnlargeTriggers (cosmetic scale only)
            },
            [Intent::Buff],
            1
        ),
        mv!("FLAIL_MOVE", |cx, me| hits(cx, me, 1, 3), [atk!(gardener::flail, 3)], 3),
    ],
};

// ---- SkulkingColony (elite) ----------------------------------------------------------------------------------------------

mod colony {
    use super::*;
    pub fn zoom(cx: &Combat) -> i32 {
        deadly(cx, 16, 14)
    }
    pub fn inertia(cx: &Combat) -> i32 {
        deadly(cx, 11, 9)
    }
    pub fn inertia_str(cx: &Combat) -> i32 {
        deadly(cx, 4, 2)
    }
    pub fn stabs(cx: &Combat) -> i32 {
        deadly(cx, 8, 7)
    }
    pub fn on_spawn(cx: &mut Combat, me: Cid) {
        buff(cx, ids::power::HARDENED_SHELL_POWER, 20, me);
    }
}
// nodes: 0 ZOOM, 1 ZOOM_2, 2 INERTIA, 3 PIERCING_STABS
pub static SKULKING_COLONY_DEF: MonsterDef = MonsterDef {
    id: ids::monster::SKULKING_COLONY,
    hp: hp!(75, 75, 80, 80),
    initial: 0,
    on_spawn: Some(colony::on_spawn),
    nodes: &[
        mv!("ZOOM_MOVE", |cx, me| hits(cx, me, colony::zoom(cx), 1), [atk!(colony::zoom)], 1),
        mv!("ZOOM_MOVE_2", |cx, me| hits(cx, me, colony::zoom(cx), 1), [atk!(colony::zoom)], 2),
        mv!(
            "INERTIA_MOVE",
            |cx, me| {
                hits(cx, me, colony::inertia(cx), 1);
                let s = colony::inertia_str(cx);
                buff(cx, ids::power::STRENGTH_POWER, s, me);
            },
            [atk!(colony::inertia), Intent::Buff],
            3
        ),
        mv!("PIERCING_STABS_MOVE", |cx, me| hits(cx, me, colony::stabs(cx), 2), [atk!(colony::stabs, 2)], 0),
    ],
};

// ---- TerrorEel (elite) ---------------------------------------------------------------------------------------------------

mod eel {
    use super::*;
    pub fn crash(cx: &Combat) -> i32 {
        deadly(cx, 18, 16)
    }
    pub fn thrash(cx: &Combat) -> i32 {
        deadly(cx, 4, 3)
    }
    pub fn on_spawn(cx: &mut Combat, me: Cid) {
        let a = tough(cx, 75, 70);
        buff(cx, ids::power::SHRIEK_POWER, a, me);
    }
}
// nodes: 0 CRASH, 1 THRASH, 2 TERROR (reached only through Shriek's stun; follow-up CRASH)
pub static TERROR_EEL_DEF: MonsterDef = MonsterDef {
    id: ids::monster::TERROR_EEL,
    hp: hp!(140, 140, 150, 150),
    initial: 0,
    on_spawn: Some(eel::on_spawn),
    nodes: &[
        mv!("CRASH_MOVE", |cx, me| hits(cx, me, eel::crash(cx), 1), [atk!(eel::crash)], 1),
        mv!(
            "THRASH_MOVE",
            |cx, me| {
                hits(cx, me, eel::thrash(cx), 3);
                buff(cx, ids::power::VIGOR_POWER, 6, me);
            },
            [atk!(eel::thrash, 3), Intent::Buff],
            0
        ),
        mv!("TERROR_MOVE", |cx, me| debuff(cx, ids::power::VULNERABLE_POWER, 99, me), [Intent::Debuff], 0),
    ],
};

// ---- LagavulinMatriarch (boss) -------------------------------------------------------------------------------------------

mod matriarch {
    use super::*;
    pub fn slash(cx: &Combat) -> i32 {
        deadly(cx, 21, 19)
    }
    pub fn slash2(cx: &Combat) -> i32 {
        deadly(cx, 14, 12)
    }
    pub fn slash2_block(cx: &Combat) -> i32 {
        tough(cx, 14, 12)
    }
    pub fn disembowel(cx: &Combat) -> i32 {
        deadly(cx, 10, 9)
    }
    pub fn on_spawn(cx: &mut Combat, me: Cid) {
        // Sleep(): Plating 12, then Asleep 3
        buff(cx, ids::power::PLATING_POWER, 12, me);
        buff(cx, ids::power::ASLEEP_POWER, 3, me);
    }
}
// nodes: 0 SLEEP_BRANCH, 1 SLEEP (INIT), 2 SLASH, 3 DISEMBOWEL, 4 SLASH2, 5 SOUL_SIPHON
pub static LAGAVULIN_MATRIARCH_DEF: MonsterDef = MonsterDef {
    id: ids::monster::LAGAVULIN_MATRIARCH,
    hp: hp!(222, 222, 233, 233),
    initial: 1,
    on_spawn: Some(matriarch::on_spawn),
    nodes: &[
        MonsterNode::Cond {
            id: "SLEEP_BRANCH",
            arms: &[(1, |cx, c| cx.has_power(c, ids::power::ASLEEP_POWER)), (2, |cx, c| !cx.has_power(c, ids::power::ASLEEP_POWER))],
        },
        mv!("SLEEP_MOVE", |_, _| {}, [Intent::Sleep], 0),
        mv!("SLASH_MOVE", |cx, me| hits(cx, me, matriarch::slash(cx), 1), [atk!(matriarch::slash)], 3),
        mv!("DISEMBOWEL_MOVE", |cx, me| hits(cx, me, matriarch::disembowel(cx), 2), [atk!(matriarch::disembowel, 2)], 4),
        mv!(
            "SLASH2_MOVE",
            |cx, me| {
                hits(cx, me, matriarch::slash2(cx), 1);
                let b = matriarch::slash2_block(cx);
                cx.gain_block(me, Dec::int(b as i64), ValueProp::MOVE, NO);
            },
            [atk!(matriarch::slash2), Intent::Defend],
            5
        ),
        mv!(
            "SOUL_SIPHON_MOVE",
            |cx, me| {
                debuff(cx, ids::power::STRENGTH_POWER, -2, me);
                debuff(cx, ids::power::DEXTERITY_POWER, -2, me);
                buff(cx, ids::power::STRENGTH_POWER, 2, me);
            },
            [Intent::Debuff, Intent::Buff],
            2
        ),
    ],
};

// ---- SoulFysh (boss) -----------------------------------------------------------------------------------------------------

mod fysh {
    use super::*;
    pub fn de_gas(cx: &Combat) -> i32 {
        deadly(cx, 18, 16)
    }
    pub fn scream(cx: &Combat) -> i32 {
        deadly(cx, 15, 13)
    }
    pub fn gaze(cx: &Combat) -> i32 {
        deadly(cx, 8, 7)
    }
    fn beckon(cx: &mut Combat, pile: PileType, pos: CardPilePosition) {
        if let Some(c) = cx.new_card(ids::card::BECKON, 0) {
            cx.add_generated_card(c, pile, pos);
        }
    }
    pub fn beckon_move(cx: &mut Combat, _me: Cid) {
        // 1 Beckon to the draw pile (random position), 1 to the discard pile
        beckon(cx, PileType::Draw, CardPilePosition::Random);
        beckon(cx, PileType::Discard, CardPilePosition::Bottom);
    }
    pub fn gaze_move(cx: &mut Combat, me: Cid) {
        let d = gaze(cx);
        hits(cx, me, d, 1);
        beckon(cx, PileType::Discard, CardPilePosition::Bottom);
    }
}
// nodes: 0 BECKON, 1 DE_GAS, 2 GAZE, 3 FADE, 4 SCREAM
pub static SOUL_FYSH_DEF: MonsterDef = MonsterDef {
    id: ids::monster::SOUL_FYSH,
    hp: hp!(211, 211, 221, 221),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv!("BECKON_MOVE", fysh::beckon_move, [Intent::StatusCard], 1),
        mv!("DE_GAS_MOVE", |cx, me| hits(cx, me, fysh::de_gas(cx), 1), [atk!(fysh::de_gas)], 2),
        mv!("GAZE_MOVE", fysh::gaze_move, [atk!(fysh::gaze), Intent::StatusCard], 3),
        mv!("FADE_MOVE", |cx, me| buff(cx, ids::power::INTANGIBLE_POWER, 2, me), [Intent::Buff], 4),
        mv!(
            "SCREAM_MOVE",
            |cx, me| {
                hits(cx, me, fysh::scream(cx), 1);
                debuff(cx, ids::power::VULNERABLE_POWER, 3, me);
            },
            [atk!(fysh::scream), Intent::Debuff],
            0
        ),
    ],
};

// ---- WaterfallGiant (boss) -----------------------------------------------------------------------------------------------

mod giant {
    use super::*;
    pub fn stomp(cx: &Combat) -> i32 {
        deadly(cx, 16, 15)
    }
    pub fn ram(cx: &Combat) -> i32 {
        deadly(cx, 11, 10)
    }
    pub fn pressure_up(cx: &Combat) -> i32 {
        deadly(cx, 14, 13)
    }
    pub fn pressure_gun(cx: &Combat, me: Cid) -> i32 {
        cx.cr(me).monster.vars[0]
    }
    fn steam(cx: &mut Combat, me: Cid, n: i32) {
        buff(cx, ids::power::STEAM_ERUPTION_POWER, n, me);
    }
    pub fn on_spawn(cx: &mut Combat, me: Cid) {
        // vars[0] = CurrentPressureGunDamage, vars[1] = SteamEruptionDamage
        cx.cr_mut(me).monster.vars[0] = deadly(cx, 23, 20);
    }
    pub fn pressurize(cx: &mut Combat, me: Cid) {
        let a = deadly(cx, 20, 15);
        steam(cx, me, a);
    }
    pub fn stomp_move(cx: &mut Combat, me: Cid) {
        hits(cx, me, stomp(cx), 1);
        debuff(cx, ids::power::WEAK_POWER, 1, me);
        steam(cx, me, 3);
    }
    pub fn ram_move(cx: &mut Combat, me: Cid) {
        hits(cx, me, ram(cx), 1);
        steam(cx, me, 3);
    }
    pub fn siphon_move(cx: &mut Combat, me: Cid) {
        let h = tough(cx, 15, 10);
        cx.heal(me, Dec::int(h as i64));
        steam(cx, me, 3);
    }
    pub fn pressure_gun_move(cx: &mut Combat, me: Cid) {
        let d = cx.cr(me).monster.vars[0];
        hits(cx, me, d, 1);
        cx.cr_mut(me).monster.vars[0] += 5;
        steam(cx, me, 3);
    }
    pub fn pressure_up_move(cx: &mut Combat, me: Cid) {
        hits(cx, me, pressure_up(cx), 1);
        steam(cx, me, 3);
    }
    pub fn about_to_blow(cx: &mut Combat, me: Cid) {
        let a = cx.power_amount(me, ids::power::STEAM_ERUPTION_POWER);
        cx.cr_mut(me).monster.vars[1] = a;
        if let Some(uid) = cx.cr(me).power(ids::power::STEAM_ERUPTION_POWER).map(|p| p.uid) {
            cx.remove_power(me, uid);
        }
    }
    pub fn explode(cx: &mut Combat, me: Cid) {
        let d = cx.cr(me).monster.vars[1];
        hits(cx, me, d, 1);
        cx.kill(&[me]);
    }
}
// nodes: 0 PRESSURIZE, 1 STOMP, 2 RAM, 3 SIPHON, 4 PRESSURE_GUN, 5 PRESSURE_UP, 6 EXPLODE, 7 ABOUT_TO_BLOW (must perform once)
pub static WATERFALL_GIANT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::WATERFALL_GIANT,
    hp: hp!(240, 240, 250, 250),
    initial: 0,
    on_spawn: Some(giant::on_spawn),
    nodes: &[
        mv!("PRESSURIZE_MOVE", giant::pressurize, [Intent::Buff], 1),
        mv!("STOMP_MOVE", giant::stomp_move, [atk!(giant::stomp), Intent::Debuff, Intent::Buff], 2),
        mv!("RAM_MOVE", giant::ram_move, [atk!(giant::ram), Intent::Buff], 3),
        mv!("SIPHON_MOVE", giant::siphon_move, [Intent::Heal, Intent::Buff], 4),
        mv!(
            "PRESSURE_GUN_MOVE",
            giant::pressure_gun_move,
            [Intent::Attack { damage: |cx, c| giant::pressure_gun(cx, c), hits: |_, _| 1 }, Intent::Buff],
            5
        ),
        mv!("PRESSURE_UP_MOVE", giant::pressure_up_move, [atk!(giant::pressure_up), Intent::Buff], 1),
        mv!("EXPLODE_MOVE", giant::explode, [Intent::DeathBlowAttack { damage: |cx, c| cx.cr(c).monster.vars[1] }], 6),
        MonsterNode::Move { id: "ABOUT_TO_BLOW_MOVE", perform: giant::about_to_blow, intents: &[Intent::Stun], follow_up: 6, must_perform_once: true },
    ],
};
