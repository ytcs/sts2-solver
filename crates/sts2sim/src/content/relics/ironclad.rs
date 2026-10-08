use crate::content::gen_relics as g;
use crate::dec::Dec;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

listener!(BlackBlood {
    fn after_combat_victory(&self, cx: &mut Combat, _me: Me) {
        if cx.cr(PLAYER).is_alive() {
            cx.heal(PLAYER, Dec::int(g::black_blood::HEAL as i64));
        }
    }
});

listener!(Brimstone {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side != Side::Player {
            return;
        }
        cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(g::brimstone::SELF_STRENGTH as i64), PLAYER, NO);
        let targets = cx.alive_enemies();
        for &t in targets.iter() {
            cx.apply_power(ids::power::STRENGTH_POWER, t, Dec::int(g::brimstone::ENEMY_STRENGTH as i64), NO, NO);
        }
    }
});

listener!(CharonsAshes {
    fn after_card_exhausted(&self, cx: &mut Combat, _me: Me, _card: CardIdx, _by_ethereal: bool) {
        cx.damage_hittable_enemies(g::charons_ashes::DAMAGE, ValueProp::UNPOWERED);
    }
});

listener!(DemonTongue {
    fn after_damage_received(&self, cx: &mut Combat, me: Me, target: Cid, unblocked: i32, _props: ValueProp, _dealer: Cid) {
        if cx.side == Side::Player && target == PLAYER && unblocked > 0 && !cx.rel(me).flag(0) {
            cx.rel_mut(me).set_flag(0, true);
            cx.heal(PLAYER, Dec::int(unblocked as i64));
        }
    }
    fn before_side_turn_start(&self, cx: &mut Combat, me: Me, side: Side) {
        if side == Side::Player {
            cx.rel_mut(me).set_flag(0, false);
        }
    }
});

listener!(PaperPhrog {});

fn red_skull_update(cx: &mut Combat, me: Me) {
    let hp = Dec::int(cx.cr(PLAYER).hp() as i64);
    let thr = Dec::int(cx.cr(PLAYER).max_hp as i64) * Dec::frac(g::red_skull::HP_THRESHOLD as i64, 2);
    let above = hp > thr;
    let amt = g::red_skull::STRENGTH_POWER as i64;
    let applied = cx.rel(me).flag(0);
    if above && applied {
        cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(-amt), PLAYER, NO);
        cx.rel_mut(me).set_flag(0, false);
    } else if !above && !applied {
        cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(amt), PLAYER, NO);
        cx.rel_mut(me).set_flag(0, true);
    }
}
listener!(RedSkull {
    fn after_room_entered(&self, cx: &mut Combat, me: Me) {
        red_skull_update(cx, me);
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, false);
    }
    fn after_current_hp_changed(&self, cx: &mut Combat, me: Me, _creature: Cid, _delta: i32) {
        if cx.in_progress {
            red_skull_update(cx, me);
        }
    }
});

listener!(RuinedHelmet {
    fn try_modify_power_amount_received(&self, cx: &Combat, me: Me, power_id: u16, target: Cid, amount: Dec, _applier: Cid) -> Option<Dec> {
        if power_id != ids::power::STRENGTH_POWER || target != PLAYER || amount <= Dec::ZERO || cx.rel(me).flag(0) {
            return None;
        }
        Some(amount * Dec::int(2))
    }
    fn after_modifying_power_amount_received(&self, cx: &mut Combat, me: Me, _power_id: u16) {
        cx.rel_mut(me).set_flag(0, true);
    }
    fn after_combat_end(&self, cx: &mut Combat, me: Me) {
        cx.rel_mut(me).set_flag(0, false);
    }
});

listener!(SelfFormingClay {
    fn after_damage_received(&self, cx: &mut Combat, _me: Me, target: Cid, unblocked: i32, _props: ValueProp, _dealer: Cid) {
        if cx.in_progress && target == PLAYER && unblocked > 0 {
            cx.apply_power(ids::power::SELF_FORMING_CLAY_POWER, PLAYER, Dec::int(g::self_forming_clay::BLOCK_NEXT_TURN as i64), PLAYER, NO);
        }
    }
});
