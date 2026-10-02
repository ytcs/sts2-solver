//! Shared / event relics that act at combat start or at the start / end of the player's turns (no card-play tracking).
//! `participants.Contains(Owner.Creature)` in the C# hooks == `side == Side::Player` here.

use crate::content::gen_relics as g;
use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::{listener, relic_props};
use crate::state::*;
use crate::types::*;

const UNPOWERED: ValueProp = ValueProp::UNPOWERED;

/// `PowerCmd.Apply<T>(ctx, owner, n, owner, null)`.
fn self_power(cx: &mut Combat, id: u16, n: i32) {
    cx.apply_power(id, PLAYER, Dec::int(n as i64), PLAYER, NO);
}

/// `CreatureCmd.GainBlock(owner, n, Unpowered, null)`.
fn relic_block(cx: &mut Combat, n: i32) {
    cx.gain_block(PLAYER, Dec::int(n as i64), UNPOWERED, NO);
}

// ---- start of combat / first turn -------------------------------------------------------------------------------------

listener!(Akabeko {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() <= 1 {
            self_power(cx, ids::power::VIGOR_POWER, g::akabeko::VIGOR_POWER);
        }
    }
});

listener!(Anchor {
    fn before_combat_start(&self, cx: &mut Combat, _me: Me) {
        relic_block(cx, g::anchor::BLOCK);
    }
});

listener!(FakeAnchor {
    fn before_combat_start(&self, cx: &mut Combat, _me: Me) {
        relic_block(cx, g::fake_anchor::BLOCK);
    }
});

listener!(BagOfMarbles {
    fn before_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() <= 1 {
            cx.apply_power_to_hittable_enemies(ids::power::VULNERABLE_POWER, Dec::int(g::bag_of_marbles::VULNERABLE_POWER as i64), PLAYER, NO);
        }
    }
});

listener!(RedMask {
    fn before_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() <= 1 {
            cx.apply_power_to_hittable_enemies(ids::power::WEAK_POWER, Dec::int(g::red_mask::WEAK_POWER as i64), PLAYER, NO);
        }
    }
});

listener!(BloodVial {
    fn after_player_turn_start_late(&self, cx: &mut Combat, _me: Me) {
        if cx.turn_number() <= 1 {
            cx.heal(PLAYER, Dec::int(g::blood_vial::HEAL as i64));
        }
    }
});

listener!(FakeBloodVial {
    fn after_player_turn_start_late(&self, cx: &mut Combat, _me: Me) {
        if cx.turn_number() <= 1 {
            cx.heal(PLAYER, Dec::int(g::fake_blood_vial::HEAL as i64));
        }
    }
});

listener!(Lantern {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() <= 1 {
            cx.gain_energy(g::lantern::ENERGY);
        }
    }
});

listener!(VeryHotCocoa {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() <= 1 {
            cx.gain_energy(g::very_hot_cocoa::ENERGY);
        }
    }
});

listener!(Candelabra {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() == 2 {
            cx.gain_energy(g::candelabra::ENERGY);
        }
    }
});

listener!(Chandelier {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() == 3 {
            cx.gain_energy(g::chandelier::ENERGY);
        }
    }
});

listener!(CaptainsWheel {
    fn after_block_cleared(&self, cx: &mut Combat, _me: Me, creature: Cid) {
        if creature == PLAYER && cx.turn_number() == 3 {
            relic_block(cx, g::captains_wheel::BLOCK);
        }
    }
});

listener!(HornCleat {
    fn after_block_cleared(&self, cx: &mut Combat, _me: Me, creature: Cid) {
        if creature == PLAYER && cx.turn_number() == 2 {
            relic_block(cx, g::horn_cleat::BLOCK);
        }
    }
});

listener!(SparklingRouge {
    fn after_block_cleared(&self, cx: &mut Combat, _me: Me, creature: Cid) {
        if creature == PLAYER && cx.turn_number() == 3 {
            self_power(cx, ids::power::STRENGTH_POWER, g::sparkling_rouge::STRENGTH_POWER);
            self_power(cx, ids::power::DEXTERITY_POWER, g::sparkling_rouge::DEXTERITY_POWER);
        }
    }
});

listener!(Sai {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player {
            relic_block(cx, g::sai::BLOCK);
        }
    }
});

listener!(DiamondDiadem {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() <= 1 {
            relic_block(cx, g::diamond_diadem::BLOCK);
            self_power(cx, ids::power::BLUR_POWER, 1);
        }
    }
});

// ---- AfterRoomEntered (combat rooms only: this simulator only runs combats) -------------------------------------------

listener!(Vajra {
    fn after_room_entered(&self, cx: &mut Combat, _me: Me) {
        self_power(cx, ids::power::STRENGTH_POWER, g::vajra::STRENGTH_POWER);
    }
});

listener!(OddlySmoothStone {
    fn after_room_entered(&self, cx: &mut Combat, _me: Me) {
        self_power(cx, ids::power::DEXTERITY_POWER, g::oddly_smooth_stone::DEXTERITY_POWER);
    }
});

listener!(BronzeScales {
    fn after_room_entered(&self, cx: &mut Combat, _me: Me) {
        self_power(cx, ids::power::THORNS_POWER, g::bronze_scales::THORNS_POWER);
    }
});

listener!(Gorget {
    fn after_room_entered(&self, cx: &mut Combat, _me: Me) {
        self_power(cx, ids::power::PLATING_POWER, g::gorget::PLATING_POWER);
    }
});

listener!(DataDisk {
    fn after_room_entered(&self, cx: &mut Combat, _me: Me) {
        self_power(cx, ids::power::FOCUS_POWER, g::data_disk::FOCUS_POWER);
    }
});

listener!(SlingOfCourage {
    fn after_room_entered(&self, cx: &mut Combat, _me: Me) {
        if cx.room_type == 1 {
            self_power(cx, ids::power::STRENGTH_POWER, g::sling_of_courage::STRENGTH_POWER);
        }
    }
});

// applier = null
listener!(SwordOfJade {
    fn after_room_entered(&self, cx: &mut Combat, _me: Me) {
        cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(g::sword_of_jade::STRENGTH_POWER as i64), NO, NO);
    }
});

// counter = `CombatsLeft` (saved, field initialiser 5); ShowCounter => DisplayAmount = max(0, CombatsLeft).
listener!(EmberTea {
    fn after_room_entered(&self, cx: &mut Combat, me: Me) {
        if cx.rel(me).counter > 0 {
            cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(g::ember_tea::STRENGTH_POWER as i64), NO, NO);
            cx.rel_mut(me).counter -= 1;
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("CombatsLeft", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter.max(0))
    }
    fn meta_initial(&self) -> (i32, u8, i32) {
        (5, 0, 0)
    }
});

// counter = `TimesLifted` (saved); ShowCounter.
listener!(Girya {
    fn after_room_entered(&self, cx: &mut Combat, me: Me) {
        let n = cx.rel(me).counter;
        if n > 0 {
            self_power(cx, ids::power::STRENGTH_POWER, n);
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("TimesLifted", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter)
    }
});

// applier = null for the strength given to the opponents.
listener!(PhilosophersStone {
    fn modify_max_energy(&self, _cx: &Combat, _me: Me, amount: Dec) -> Dec {
        amount + Dec::int(g::philosophers_stone::ENERGY as i64)
    }
    fn after_creature_added_to_combat(&self, cx: &mut Combat, _me: Me, creature: Cid) {
        if cx.cr(creature).side != Side::Player {
            cx.apply_power(ids::power::STRENGTH_POWER, creature, Dec::int(g::philosophers_stone::STRENGTH_POWER as i64), NO, NO);
        }
    }
    fn after_room_entered(&self, cx: &mut Combat, _me: Me) {
        let targets = cx.alive_enemies();
        for &t in targets.iter() {
            cx.apply_power(ids::power::STRENGTH_POWER, t, Dec::int(g::philosophers_stone::STRENGTH_POWER as i64), NO, NO);
        }
    }
});

listener!(Pantograph {
    fn before_combat_start(&self, cx: &mut Combat, _me: Me) {
        if cx.cr(PLAYER).is_alive() && cx.room_type == 2 {
            cx.heal(PLAYER, Dec::int(g::pantograph::HEAL as i64));
        }
    }
});

// `HpThreshold` percent of max HP (truncated), compared with `<=` once the combat is won.
listener!(MeatOnTheBone {
    fn after_combat_victory_early(&self, cx: &mut Combat, _me: Me) {
        if cx.cr(PLAYER).is_alive() {
            let thr = (Dec::int(cx.cr(PLAYER).max_hp as i64) * Dec::frac(g::meat_on_the_bone::HP_THRESHOLD as i64, 2)).trunc();
            if cx.cr(PLAYER).hp <= thr {
                cx.heal(PLAYER, Dec::int(g::meat_on_the_bone::HEAL as i64));
            }
        }
    }
});

// ---- damage at the start of turns --------------------------------------------------------------------------------------

listener!(FestivePopper {
    fn after_player_turn_start(&self, cx: &mut Combat, _me: Me) {
        if cx.turn_number() == 1 {
            cx.damage_hittable_enemies(g::festive_popper::DAMAGE, UNPOWERED);
        }
    }
});

listener!(MercuryHourglass {
    fn after_player_turn_start(&self, cx: &mut Combat, _me: Me) {
        cx.damage_hittable_enemies(g::mercury_hourglass::DAMAGE, UNPOWERED);
    }
});

listener!(MrStruggles {
    fn after_player_turn_start(&self, cx: &mut Combat, _me: Me) {
        let n = cx.turn_number();
        cx.damage_hittable_enemies(n, UNPOWERED);
    }
});

// `CreatureCmd.Damage(owner, 4, Unblockable | Unpowered, null, null)`: the owner hurts itself.
listener!(RoyalPoison {
    fn after_player_turn_start(&self, cx: &mut Combat, _me: Me) {
        if cx.turn_number() <= 1 {
            cx.damage(&[PLAYER], Dec::int(g::royal_poison::DAMAGE as i64), ValueProp::UNBLOCKABLE.or(UNPOWERED), NO, NO);
        }
    }
});

listener!(TwistedFunnel {
    fn before_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() <= 1 {
            let targets = cx.hittable_enemies();
            for &t in targets.iter() {
                cx.apply_power(ids::power::POISON_POWER, t, Dec::int(g::twisted_funnel::POISON_POWER as i64), PLAYER, NO);
            }
        }
    }
});

// ---- first-turn hand upgrades ------------------------------------------------------------------------------------------

listener!(Bellows {
    fn after_player_turn_start(&self, cx: &mut Combat, _me: Me) {
        if cx.turn_number() <= 1 {
            let hand = cx.player.hand;
            for &c in hand.iter() {
                cx.upgrade_in_combat(c);
            }
        }
    }
});

// counter = `CombatsLeft` (saved, initialiser 1); ShowCounter is false.
listener!(BoneTea {
    fn after_player_turn_start(&self, cx: &mut Combat, me: Me) {
        if cx.rel(me).counter <= 0 || cx.turn_number() > 1 {
            return;
        }
        let hand = cx.player.hand;
        for &c in hand.iter() {
            cx.upgrade_in_combat(c);
        }
        cx.rel_mut(me).counter -= 1;
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("CombatsLeft", Slot::Counter)]
    }
    fn meta_initial(&self) -> (i32, u8, i32) {
        (1, 0, 0)
    }
});

// ---- energy / draw shape -----------------------------------------------------------------------------------------------

listener!(BoomingConch {
    fn modify_hand_draw(&self, cx: &Combat, _me: Me, count: Dec) -> Dec {
        if cx.turn_number() > 1 || cx.room_type != 1 {
            return count;
        }
        count + Dec::int(g::booming_conch::CARDS as i64)
    }
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() <= 1 && cx.room_type == 1 {
            cx.gain_energy(g::booming_conch::ENERGY);
        }
    }
});

listener!(Bread {
    fn modify_max_energy(&self, cx: &Combat, _me: Me, amount: Dec) -> Dec {
        if cx.turn_number() == 1 {
            return amount;
        }
        amount + Dec::int(g::bread::GAIN_ENERGY as i64)
    }
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() == 1 {
            cx.lose_energy(g::bread::LOSE_ENERGY);
        }
    }
});

listener!(IceCream {
    fn should_player_reset_energy(&self, cx: &Combat, _me: Me) -> bool {
        cx.turn_number() == 1
    }
});

listener!(Ectoplasm {
    fn modify_max_energy(&self, _cx: &Combat, _me: Me, amount: Dec) -> Dec {
        amount + Dec::int(g::ectoplasm::ENERGY as i64)
    }
    // No gold from any source.
    fn modify_gold_gained(&self, _cx: &Combat, _me: Me, _amount: Dec) -> Dec {
        Dec::ZERO
    }
});

listener!(SealOfGold {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.gold >= g::seal_of_gold::GOLD {
            cx.gain_energy(g::seal_of_gold::ENERGY);
            cx.lose_gold(g::seal_of_gold::GOLD);
        }
    }
});

listener!(ParryingShield {
    fn after_side_turn_end(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.cr(PLAYER).block >= g::parrying_shield::BLOCK {
            cx.damage_random_hittable_enemy(g::parrying_shield::DAMAGE, UNPOWERED);
        }
    }
});

listener!(ForgottenSoul {
    fn after_card_exhausted(&self, cx: &mut Combat, _me: Me, _card: CardIdx, _by_ethereal: bool) {
        cx.damage_random_hittable_enemy(g::forgotten_soul::DAMAGE, UNPOWERED);
    }
});
listener!(PrismaticGem {
    fn modify_max_energy(&self, _cx: &Combat, _me: Me, amount: Dec) -> Dec {
        amount + Dec::int(g::prismatic_gem::ENERGY as i64)
    }
});
listener!(Sozu {
    fn modify_max_energy(&self, _cx: &Combat, _me: Me, amount: Dec) -> Dec {
        amount + Dec::int(g::sozu::ENERGY as i64)
    }
    fn should_procure_potion(&self, _cx: &Combat, _me: Me, _potion: u16) -> bool {
        false
    }
});
listener!(BloodSoakedRose {
    fn modify_max_energy(&self, _cx: &Combat, _me: Me, amount: Dec) -> Dec {
        amount + Dec::int(g::blood_soaked_rose::ENERGY as i64)
    }
});

listener!(SpikedGauntlets {
    fn modify_max_energy(&self, _cx: &Combat, _me: Me, amount: Dec) -> Dec {
        amount + Dec::int(g::spiked_gauntlets::ENERGY as i64)
    }
    // Power cards cost 1 more.
    fn try_modify_energy_cost_in_combat(&self, cx: &Combat, _me: Me, card: CardIdx, cost: Dec) -> Option<Dec> {
        if cx.card_def(card).ctype != CardType::Power {
            return None;
        }
        Some(cost + Dec::ONE)
    }
});

listener!(BagOfPreparation {
    fn modify_hand_draw(&self, cx: &Combat, _me: Me, count: Dec) -> Dec {
        if cx.turn_number() > 1 {
            return count;
        }
        count + Dec::int(g::bag_of_preparation::CARDS as i64)
    }
});

listener!(RingOfTheSnake {
    fn modify_hand_draw(&self, cx: &Combat, _me: Me, count: Dec) -> Dec {
        if cx.turn_number() > 1 {
            return count;
        }
        count + Dec::int(g::ring_of_the_snake::CARDS as i64)
    }
});

listener!(RingOfTheDrake {
    fn modify_hand_draw(&self, cx: &Combat, _me: Me, count: Dec) -> Dec {
        if cx.turn_number() > g::ring_of_the_drake::TURNS {
            return count;
        }
        count + Dec::int(g::ring_of_the_drake::CARDS as i64)
    }
});

listener!(PaelsBlood {
    fn modify_hand_draw(&self, _cx: &Combat, _me: Me, count: Dec) -> Dec {
        count + Dec::int(g::paels_blood::CARDS as i64)
    }
});

listener!(SneckoEye {
    fn before_combat_start(&self, cx: &mut Combat, _me: Me) {
        self_power(cx, ids::power::CONFUSED_POWER, 1);
    }
    fn modify_hand_draw(&self, _cx: &Combat, _me: Me, count: Dec) -> Dec {
        count + Dec::int(g::snecko_eye::CARDS as i64)
    }
});

listener!(FakeSneckoEye {
    fn before_combat_start(&self, cx: &mut Combat, _me: Me) {
        self_power(cx, ids::power::CONFUSED_POWER, 1);
    }
});

listener!(BigMushroom {
    fn modify_hand_draw(&self, cx: &Combat, _me: Me, count: Dec) -> Dec {
        if cx.turn_number() != 1 {
            return count;
        }
        count - Dec::int(g::big_mushroom::CARDS as i64)
    }
});

// `ShouldDraw`: only the start-of-turn hand draw is allowed while it is the player's turn.
listener!(Fiddle {
    fn modify_hand_draw(&self, _cx: &Combat, _me: Me, count: Dec) -> Dec {
        count + Dec::int(g::fiddle::CARDS as i64)
    }
    fn should_draw(&self, cx: &Combat, _me: Me, from_hand_draw: bool) -> bool {
        from_hand_draw || cx.side != Side::Player
    }
});

listener!(RingingTriangle {
    fn should_flush(&self, cx: &Combat, _me: Me) -> bool {
        cx.turn_number() > 1
    }
});

listener!(RunicPyramid {
    fn should_flush(&self, _cx: &Combat, _me: Me) -> bool {
        false
    }
});

// ---- block / hp ---------------------------------------------------------------------------------------------------------

// Block carries over, capped at `BLOCK` (`LoseBlock(block - 10)` when above it).
listener!(SturdyClamp {
    fn should_clear_block(&self, _cx: &Combat, _me: Me, creature: Cid) -> bool {
        creature != PLAYER
    }
    fn after_preventing_block_clear(&self, cx: &mut Combat, _me: Me, creature: Cid) {
        if creature != PLAYER {
            return;
        }
        let block = cx.cr(PLAYER).block;
        if block > g::sturdy_clamp::BLOCK {
            cx.lose_block(PLAYER, Dec::int((block - g::sturdy_clamp::BLOCK) as i64), NO);
        }
    }
});

// flag 0 = `ShouldTrigger` (not saved).
listener!(Orichalcum {
    fn before_side_turn_end_very_early(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player && cx.cr(PLAYER).block <= 0 {
            cx.rel_mut(me).set_flag(0, true);
        }
    }
    fn before_side_turn_end(&self, cx: &mut Combat, me: Me, _side: Side) {
        if cx.rel(me).flag(0) {
            cx.rel_mut(me).set_flag(0, false);
            relic_block(cx, g::orichalcum::BLOCK);
        }
    }
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            cx.rel_mut(me).set_flag(0, false);
        }
    }
});

listener!(FakeOrichalcum {
    fn before_side_turn_end_very_early(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player && cx.cr(PLAYER).block <= 0 {
            cx.rel_mut(me).set_flag(0, true);
        }
    }
    fn before_side_turn_end(&self, cx: &mut Combat, me: Me, _side: Side) {
        if cx.rel(me).flag(0) {
            cx.rel_mut(me).set_flag(0, false);
            relic_block(cx, g::fake_orichalcum::BLOCK);
        }
    }
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            cx.rel_mut(me).set_flag(0, false);
        }
    }
});

// `Math.Max(0, amount - HpLossReduction)` on every HP-loss the owner takes after the Osty redirect.
listener!(TungstenRod {
    fn modify_hp_lost_after_osty(&self, _cx: &Combat, _me: Me, target: Cid, amount: Dec, _props: ValueProp, _dealer: Cid, _card: CardIdx) -> Dec {
        if target != PLAYER {
            return amount;
        }
        (amount - Dec::int(g::tungsten_rod::HP_LOSS_REDUCTION as i64)).max(Dec::ZERO)
    }
});

// The first hit of a turn each time HP loss exceeds `MaxHpLoss` total (damage received this turn tracked in `aux`).
listener!(BeatingRemnant {
    fn modify_hp_lost_after_osty(&self, cx: &Combat, me: Me, target: Cid, amount: Dec, _props: ValueProp, _dealer: Cid, _card: CardIdx) -> Dec {
        if !cx.in_progress || target != PLAYER {
            return amount;
        }
        amount.min(Dec::int((g::beating_remnant::MAX_HP_LOSS - cx.rel(me).aux) as i64))
    }
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, _props: ValueProp, _dealer: Cid) {
        if cx.in_progress && target == PLAYER {
            cx.rel_mut(me).aux += unblocked;
        }
    }
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            cx.rel_mut(me).aux = 0;
        }
    }
});

// Minimum 5 damage on any powered attack of the owner that would deal 1..4.
listener!(TheBoot {
    fn modify_hp_lost_after_osty_late(&self, cx: &Combat, _me: Me, target: Cid, amount: Dec, props: ValueProp, dealer: Cid, _card: CardIdx) -> Dec {
        if !cx.is_owner_or_osty(dealer) || target == PLAYER || !props.is_powered() {
            return amount;
        }
        if amount < Dec::ONE || amount >= Dec::int(g::the_boot::DAMAGE_MINIMUM as i64) {
            return amount;
        }
        Dec::int(g::the_boot::DAMAGE_MINIMUM as i64)
    }
});
