//! Ceremonial Beast (boss, CeremonialBeastBoss). Spec 04 §3.1.
//! Phase 1: STAMP (Plow power on itself) -> PLOW_MOVE loop. When PlowPower sees HP <= Amount the Beast is stunned into
//! phase 2: STUNNED -> BEAST_CRY -> STOMP -> CRUSH -> BEAST_CRY ...

use super::ovg_util::*;
use crate::defs::*;
use crate::ids;
use crate::state::*;

const PLOW: u8 = 0;
const STAMP: u8 = 1;
const STUNNED: u8 = 2;
const BEAST_CRY: u8 = 3;
const STOMP: u8 = 4;
const CRUSH: u8 = 5;

/// `CeremonialBeast.SetStunned` + `CreatureCmd.Stun(Owner, StunnedMove, BeastCryState.StateId)`.
pub fn stun_into_phase_two(cx: &mut Combat, beast: Cid) {
    cx.stun(beast, None, Some(BEAST_CRY));
}

pub static CEREMONIAL_BEAST_DEF: MonsterDef = MonsterDef {
    id: ids::monster::CEREMONIAL_BEAST,
    hp: |a| hp(a, (262, 262), (252, 252)),
    initial: STAMP,
    on_spawn: None,
    nodes: &[
        mv(
            "PLOW_MOVE",
            |cx, me| {
                let d = a9(cx, 20, 18);
                atk(cx, me, d);
                power_self(cx, me, ids::power::STRENGTH_POWER, 2);
            },
            &[attack(|cx, _| a9(cx, 20, 18)), Intent::Buff],
            PLOW,
        ),
        mv(
            "STAMP_MOVE",
            |cx, me| {
                let n = a9(cx, 160, 150);
                power_self(cx, me, ids::power::PLOW_POWER, n);
            },
            &[Intent::Buff],
            PLOW,
        ),
        // The dynamically created `STUNNED` state (follow-up = BEAST_CRY_MOVE, MustPerformOnce).
        mv_once("STUNNED", nothing, &[Intent::Stun], BEAST_CRY),
        mv("BEAST_CRY_MOVE", |cx, me| power_player(cx, me, ids::power::RINGING_POWER, 1), &[Intent::Debuff], STOMP),
        mv(
            "STOMP_MOVE",
            |cx, me| {
                let d = a9(cx, 17, 15);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 17, 15))],
            CRUSH,
        ),
        mv(
            "CRUSH_MOVE",
            |cx, me| {
                let d = a9(cx, 19, 17);
                atk(cx, me, d);
                let s = a9(cx, 4, 3);
                power_self(cx, me, ids::power::STRENGTH_POWER, s);
            },
            &[attack(|cx, _| a9(cx, 19, 17)), Intent::Buff],
            BEAST_CRY,
        ),
    ],
};
