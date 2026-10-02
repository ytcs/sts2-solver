//! Shared helpers for the Act 1a Overgrowth monster ports (not a monster itself: registers nothing).

use crate::dec::Dec;
use crate::defs::*;
use crate::engine::Attack;
use crate::state::*;
use crate::types::*;

/// `AscensionHelper.GetValueIfAscension(DeadlyEnemies, ascended, base)` (A9).
#[inline(always)]
pub fn a9(cx: &Combat, ascended: i32, base: i32) -> i32 {
    asc::val(asc::DEADLY_ENEMIES, cx.ascension, ascended, base)
}
/// `AscensionHelper.GetValueIfAscension(ToughEnemies, ascended, base)` (A8).
#[inline(always)]
pub fn a8(cx: &Combat, ascended: i32, base: i32) -> i32 {
    asc::val(asc::TOUGH_ENEMIES, cx.ascension, ascended, base)
}
/// `(MinInitialHp, MaxInitialHp)` helper: `((min8, max8), (min0, max0))`.
#[inline(always)]
pub fn hp(ascension: u8, ascended: (i32, i32), base: (i32, i32)) -> (i32, i32) {
    asc::val(asc::TOUGH_ENEMIES, ascension, ascended, base)
}

/// `DamageCmd.Attack(dmg).FromMonster(me).Execute()`.
pub fn atk(cx: &mut Combat, me: Cid, dmg: i32) {
    cx.execute_attack(&Attack::from_monster(me, dmg));
}
/// `DamageCmd.Attack(dmg).WithHitCount(hits).FromMonster(me).Execute()`.
pub fn atk_n(cx: &mut Combat, me: Cid, dmg: i32, hits: i32) {
    cx.execute_attack(&Attack::from_monster(me, dmg).hits(hits));
}
/// `PowerCmd.Apply<T>(creature, amount, creature)` on the monster itself.
pub fn power_self(cx: &mut Combat, me: Cid, power: u16, amount: i32) {
    cx.apply_power(power, me, Dec::int(amount as i64), me, NO);
}
/// `PowerCmd.Apply<T>(targets, amount, creature)` on the player.
pub fn power_player(cx: &mut Combat, me: Cid, power: u16, amount: i32) {
    cx.apply_power(power, PLAYER, Dec::int(amount as i64), me, NO);
}
/// `CreatureCmd.GainBlock(me, amount, ValueProp.Move, null)`.
pub fn block(cx: &mut Combat, me: Cid, amount: i32) {
    cx.gain_block(me, Dec::int(amount as i64), ValueProp::MOVE, NO);
}
/// `CardPileCmd.AddToCombatAndPreview<T>(targets, Discard, n, null)`.
pub fn status_to_discard(cx: &mut Combat, card: u16, n: i32) {
    cx.add_status_cards(card, PileType::Discard, n, CardPilePosition::Bottom);
}

fn one(_: &Combat, _: Cid) -> i32 {
    1
}
/// `new SingleAttackIntent(dmg)`.
pub const fn attack(damage: fn(&Combat, Cid) -> i32) -> Intent {
    Intent::Attack { damage, hits: one }
}
/// `new MultiAttackIntent(dmg, hits)`.
pub const fn multi(damage: fn(&Combat, Cid) -> i32, hits: fn(&Combat, Cid) -> i32) -> Intent {
    Intent::Attack { damage, hits }
}

/// A plain move node.
pub const fn mv(id: &'static str, perform: MoveFn, intents: &'static [Intent], follow_up: u8) -> MonsterNode {
    MonsterNode::Move { id, perform, intents, follow_up, must_perform_once: false }
}
/// A `MustPerformOnceBeforeTransitioning` move node.
pub const fn mv_once(id: &'static str, perform: MoveFn, intents: &'static [Intent], follow_up: u8) -> MonsterNode {
    MonsterNode::Move { id, perform, intents, follow_up, must_perform_once: true }
}
pub const fn rand(id: &'static str, branches: &'static [Branch]) -> MonsterNode {
    MonsterNode::Random { id, branches }
}
pub const fn cond(id: &'static str, arms: &'static [(u8, CondFn)]) -> MonsterNode {
    MonsterNode::Cond { id, arms }
}

/// No-op move (`Task.CompletedTask`).
pub fn nothing(_: &mut Combat, _: Cid) {}
