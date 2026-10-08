use crate::content::gen_relics as g;
use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::state::*;
use crate::types::*;
use crate::{listener, relic_props};

fn self_power(cx: &mut Combat, id: u16, n: i32) {
    cx.apply_power(id, PLAYER, Dec::int(n as i64), PLAYER, NO);
}
fn relic_block(cx: &mut Combat, n: i32) {
    cx.gain_block(PLAYER, Dec::int(n as i64), ValueProp::UNPOWERED, NO);
}

listener!(DivineRight {
    fn after_room_entered(&self, cx: &mut Combat, _me: Me) {
        cx.gain_stars(g::divine_right::STARS);
    }
});

listener!(DivineDestiny {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() <= 1 {
            cx.gain_stars(g::divine_destiny::STARS);
        }
    }
});

listener!(LunarPastry {
    fn after_side_turn_end(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player {
            cx.gain_stars(g::lunar_pastry::STARS);
        }
    }
});

listener!(GalacticDust {
    fn after_stars_spent(&self, cx: &mut Combat, me: Me, amount: i32) {
        cx.rel_mut(me).counter += amount;
        let n = g::galactic_dust::STARS;
        if cx.rel(me).counter >= n {
            let blocks = cx.rel(me).counter / n;
            relic_block(cx, blocks * g::galactic_dust::BLOCK);
            cx.rel_mut(me).counter %= n;
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("StarsSpent", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter % g::galactic_dust::STARS)
    }
});

listener!(MiniRegent {
    fn after_stars_spent(&self, cx: &mut Combat, me: Me, _amount: i32) {
        if !cx.rel(me).flag(0) {
            cx.rel_mut(me).set_flag(0, true);
            self_power(cx, ids::power::STRENGTH_POWER, g::mini_regent::STRENGTH_POWER);
        }
    }
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            cx.rel_mut(me).set_flag(0, false);
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, false);
    }
});

fn brilliant_scarf_applies(cx: &Combat, me: Me, card: CardIdx) -> bool {
    cx.in_progress
        && cx.rel(me).counter == g::brilliant_scarf::CARDS - 1
        && matches!(cx.card_pile_type(card), PileType::Hand | PileType::Play)
}
listener!(BrilliantScarf {
    fn try_modify_energy_cost_in_combat_late(&self, cx: &Combat, me: Me, card: CardIdx, _cost: Dec) -> Option<Dec> {
        brilliant_scarf_applies(cx, me, card).then_some(Dec::ZERO)
    }
    fn try_modify_star_cost(&self, cx: &Combat, me: Me, card: CardIdx, _cost: Dec) -> Option<Dec> {
        brilliant_scarf_applies(cx, me, card).then_some(Dec::ZERO)
    }
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            cx.rel_mut(me).counter = 0;
        }
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.in_progress && !play.is_auto {
            cx.rel_mut(me).counter += 1;
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).counter = 0;
    }
    fn meta_display(&self, cx: &Combat, r: &Relic) -> Option<i32> {
        (cx.in_progress && r.counter < g::brilliant_scarf::CARDS).then(|| r.counter)
    }
});

listener!(PaelsTears {
    fn before_side_turn_end(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            let leftover = cx.player.energy > 0;
            cx.rel_mut(me).set_flag(0, leftover);
        }
    }
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player && cx.rel(me).flag(0) {
            cx.gain_energy(g::paels_tears::ENERGY);
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, false);
    }
});

listener!(PaelsFlesh {
    fn modify_max_energy(&self, cx: &Combat, _me: Me, amount: Dec) -> Dec {
        if cx.turn_number() < 3 {
            return amount;
        }
        amount + Dec::int(g::paels_flesh::ENERGY as i64)
    }
    fn after_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player && cx.turn_number() >= 3 {
            cx.rel_mut(me).set_flag(0, true);
        }
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, false);
    }
    fn meta_display(&self, cx: &Combat, r: &Relic) -> Option<i32> {
        (cx.in_progress && !r.flag(0)).then(|| cx.turn_number())
    }
});

listener!(PumpkinCandle {
    fn modify_max_energy(&self, cx: &Combat, me: Me, amount: Dec) -> Dec {
        if cx.rel(me).counter <= 0 {
            return amount;
        }
        amount + Dec::int(g::pumpkin_candle::ENERGY as i64)
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        let r = cx.rel_mut(me);
        r.counter = (r.counter - 1).max(0);
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("KindleCount", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter)
    }
});

listener!(VenerableTeaSet {
    fn after_energy_reset(&self, cx: &mut Combat, me: Me) {
        if cx.rel(me).flag(0) {
            cx.gain_energy(g::venerable_tea_set::ENERGY);
            cx.rel_mut(me).set_flag(0, false);
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::flag("GainEnergyInNextCombat", 0)]
    }
});

listener!(FakeVenerableTeaSet {
    fn after_energy_reset(&self, cx: &mut Combat, me: Me) {
        if cx.rel(me).flag(0) {
            cx.gain_energy(g::fake_venerable_tea_set::ENERGY);
            cx.rel_mut(me).set_flag(0, false);
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::flag("GainEnergyInNextCombat", 0)]
    }
});

listener!(ChosenCheese {
    fn after_combat_end(&self, cx: &mut Combat, _me: Me) {
        cx.gain_max_hp(PLAYER, Dec::int(g::chosen_cheese::MAX_HP as i64));
    }
});

listener!(WongosMysteryTicket {
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).counter += 1;
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("CombatsFinished", Slot::Counter), PropDef::flag("GaveRelic", 0)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        let n = 5 - r.counter;
        (n > 0).then_some(n)
    }
});

listener!(ToyBox {
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        if cx.rel(me).counter < g::toy_box::COMBATS * g::toy_box::RELICS {
            cx.rel_mut(me).counter += 1;
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("CombatsSeen", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        (r.counter < g::toy_box::COMBATS * g::toy_box::RELICS).then(|| r.counter % g::toy_box::COMBATS)
    }
});

listener!(FishingRod {
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        if cx.room_type != 0 {
            return;
        }
        cx.rel_mut(me).counter += 1;
        if cx.rel(me).counter % g::fishing_rod::COMBATS == 0 {
            let n = cx.deck_upgradable_count() as i32;
            if n > 0 {
                cx.rng.niche.next_int_range(0, n);
            }
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("CombatsSeen", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter % g::fishing_rod::COMBATS)
    }
});

listener!(WarHammer {
    fn after_combat_victory(&self, cx: &mut Combat, _me: Me) {
        if cx.room_type != 1 {
            return;
        }
        let n = cx.deck_upgradable_count();
        let mut dummy = [0u8; 128];
        cx.rng.niche.shuffle(&mut dummy[..n]);
    }
});

listener!(SwordOfStone {
    fn after_combat_victory(&self, cx: &mut Combat, me: Me) {
        if cx.room_type != 1 {
            return;
        }
        cx.rel_mut(me).counter += 1;
        if cx.rel(me).counter >= g::sword_of_stone::ELITES {
            cx.player.relics[me.idx as usize] = Relic { id: ids::relic::SWORD_OF_JADE, ..Default::default() };
            cx.listen |= crate::content::relic_mask(ids::relic::SWORD_OF_JADE);
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("ElitesDefeated", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter)
    }
});

listener!(LavaLamp {
    fn after_room_entered(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, false);
    }
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, props: ValueProp, _dealer: Cid) {
        if target == PLAYER && unblocked > 0 && !props.unblockable() {
            cx.rel_mut(me).set_flag(0, true);
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::flag("TookDamageThisCombat", 0)]
    }
});

listener!(BookRepairKnife {
    fn after_died_to_doom(&self, cx: &mut Combat, _me: Me, creatures: &[Cid]) {
        let n = creatures.iter().filter(|&&c| c != PLAYER && cx.all_powers_trigger_fatal(c)).count() as i32;
        if n != 0 {
            cx.heal(PLAYER, Dec::int((g::book_repair_knife::HEAL * n) as i64));
        }
    }
});

listener!(BowlerHat {
    fn modify_gold_gained(&self, _cx: &Combat, _me: Me, amount: Dec) -> Dec {
        amount * g::bowler_hat::GOLD_INCREASE
    }
});

listener!(BookOfFiveRings {
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("CardsAdded", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter % g::book_of_five_rings::CARDS)
    }
});

listener!(LastingCandy {
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("CombatRewardsSeen", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter % 2)
    }
});

listener!(WingedBoots {
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("TimesUsed", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        (r.counter < 3).then(|| 3 - r.counter)
    }
});

listener!(SilverCrucible {
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("TimesUsed", Slot::Counter), PropDef::int("TreasureRoomsEntered", Slot::Aux)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        (r.counter < g::silver_crucible::CARDS).then(|| g::silver_crucible::CARDS - r.counter)
    }
});

listener!(PaelsWing {
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("RewardsSacrificed", Slot::Counter)]
    }
    fn meta_display(&self, _cx: &Combat, r: &Relic) -> Option<i32> {
        Some(r.counter % g::paels_wing::SACRIFICES)
    }
});

listener!(MawBank {
    fn after_room_entered(&self, cx: &mut Combat, me: Me) {
        if !cx.rel(me).flag(0) {
            cx.gain_gold(g::maw_bank::GOLD);
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::flag("HasItemBeenBought", 0)]
    }
});

listener!(LavaRock {
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::flag("HasTriggered", 0)]
    }
});

listener!(SilkenTress {
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::flag("IsUsed", 0)]
    }
});

listener!(GoldenCompass {
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::int("GoldenPathAct", Slot::Counter)]
    }
    fn meta_initial(&self) -> (i32, u8, i32) {
        (-1, 0, 0)
    }
});

listener!(FurCoat {
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![
            PropDef::int("FurCoatActIndex", Slot::Counter),
            PropDef::constant("FurCoatCoordCols", "[]"),
            PropDef::constant("FurCoatCoordRows", "[]"),
            PropDef::constant("FurCoatCoordsSet", "false"),
        ]
    }
    fn meta_initial(&self) -> (i32, u8, i32) {
        (-1, 0, 0)
    }
});
