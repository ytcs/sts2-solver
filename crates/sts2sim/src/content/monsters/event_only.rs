//! Event-only combatants (spec 04 §3.5): BattleFriendV1-3 (Battleworn Dummy), FakeMerchantMonster and Architect. None of
//! them is in an act pool; the events only choose the encounter, so the combat itself needs no run-level state.
//! (FlailKnight / MysteriousKnight live in `flail_knight.rs`.)

use super::ovg_util::*;
use crate::defs::*;
use crate::ids;

// ---- BattleFriendV1/V2/V3: NOTHING_MOVE (no intents) forever; the time-limit power makes them escape -----------------------------
fn friend_spawn(cx: &mut crate::state::Combat, me: u8) {
    // `PowerCmd.Apply<BattlewornDummyTimeLimitPower>(Creature, 3m, null, null)` (no applier)
    cx.apply_power(ids::power::BATTLEWORN_DUMMY_TIME_LIMIT_POWER, me, crate::dec::Dec::int(3), crate::types::NO, crate::types::NO);
}

pub static BATTLE_FRIEND_V1_DEF: MonsterDef = MonsterDef {
    id: ids::monster::BATTLE_FRIEND_V1,
    hp: |_| (75, 75),
    initial: 0,
    on_spawn: Some(friend_spawn),
    nodes: &[mv("NOTHING_MOVE", nothing, &[], 0)],
};
pub static BATTLE_FRIEND_V2_DEF: MonsterDef = MonsterDef {
    id: ids::monster::BATTLE_FRIEND_V2,
    hp: |_| (150, 150),
    initial: 0,
    on_spawn: Some(friend_spawn),
    nodes: &[mv("NOTHING_MOVE", nothing, &[], 0)],
};
pub static BATTLE_FRIEND_V3_DEF: MonsterDef = MonsterDef {
    id: ids::monster::BATTLE_FRIEND_V3,
    hp: |_| (300, 300),
    initial: 0,
    on_spawn: Some(friend_spawn),
    nodes: &[mv("NOTHING_MOVE", nothing, &[], 0)],
};

// ---- Architect: 9999 HP dummy, hidden intent ------------------------------------------------------------------------------------
pub static ARCHITECT_DEF: MonsterDef = MonsterDef {
    id: ids::monster::ARCHITECT,
    hp: |_| (9999, 9999),
    initial: 0,
    on_spawn: None,
    nodes: &[mv("NOTHING", nothing, &[Intent::Hidden], 0)],
};

// ---- FakeMerchantMonster ---------------------------------------------------------------------------------------------------------
// 0 SWIPE, 1 SPEW_COINS, 2 THROW_RELIC, 3 ENRAGE, 4 RAND_MOVE, 5 RAND_ATTACK_MOVE; initial SWIPE.
pub static FAKE_MERCHANT_MONSTER_DEF: MonsterDef = MonsterDef {
    id: ids::monster::FAKE_MERCHANT_MONSTER,
    hp: |a| hp(a, (175, 175), (165, 165)),
    initial: 0,
    on_spawn: None,
    nodes: &[
        mv(
            "SWIPE_MOVE",
            |cx, me| {
                let d = a9(cx, 15, 13);
                atk(cx, me, d)
            },
            &[attack(|cx, _| a9(cx, 15, 13))],
            4,
        ),
        mv("SPEW_COINS_MOVE", |cx, me| atk_n(cx, me, 2, 8), &[multi(|_, _| 2, |_, _| 8)], 4),
        mv(
            "THROW_RELIC_MOVE",
            |cx, me| {
                let d = a9(cx, 10, 9);
                atk(cx, me, d);
                power_player(cx, me, ids::power::FRAIL_POWER, 1);
            },
            &[attack(|cx, _| a9(cx, 10, 9)), Intent::Debuff],
            5,
        ),
        mv("ENRAGE_MOVE", |cx, me| power_self(cx, me, ids::power::STRENGTH_POWER, 2), &[Intent::Buff], 4),
        rand(
            "RAND_MOVE",
            &[
                Branch::new(0).cannot_repeat(),
                Branch::new(1).cannot_repeat(),
                Branch::new(2).cannot_repeat(),
                Branch::new(3).cannot_repeat().weight(3.0),
            ],
        ),
        rand("RAND_ATTACK_MOVE", &[Branch::new(0).cannot_repeat(), Branch::new(1).cannot_repeat(), Branch::new(2).cannot_repeat()]),
    ],
};
