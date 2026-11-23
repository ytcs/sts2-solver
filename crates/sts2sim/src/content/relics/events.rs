// Event relics: Museum of Torture (Obsidian Vice, Thumbscrew, Tormentous Collar), Shrine of the Warlord (Mighty Wings),
// Thermae Nonupeipe (Bath Towel, Bath Robes)
use crate::content::gen_relics as g;
use crate::dec::Dec;
use crate::engine::Attack;
use crate::hooks::*;
use crate::ids;
use crate::state::*;
use crate::types::*;
use crate::{listener, relic_props};

listener!(BathRobes {
    fn before_combat_start(&self, cx: &mut Combat, me: Me) {
        if !cx.rel(me).flag(0) && cx.cr(PLAYER).is_alive() && cx.room_type == 2 {
            let missing = cx.cr(PLAYER).max_hp - cx.cr(PLAYER).hp;
            cx.heal(PLAYER, Dec::int(missing as i64));
            cx.rel_mut(me).set_flag(0, true);
        }
    }
    fn meta_props(&self) -> &'static [PropDef] {
        relic_props![PropDef::flag("WasUsed", 0)]
    }
});

listener!(BathTowel {
    fn after_player_turn_start_late(&self, cx: &mut Combat, _me: Me) {
        if cx.turn_number() <= 1 {
            cx.heal(PLAYER, Dec::int(g::bath_towel::HEAL as i64));
        }
    }
});

listener!(MightyWings {
    fn after_damage_given(&self, cx: &mut Combat, _me: Me, dealer: Cid, _target: Cid, _unblocked: i32, props: ValueProp) {
        let total = cx.dmg_result.unblocked + cx.dmg_result.blocked;
        if props.is_powered() && dealer != NO && (dealer == PLAYER || Some(dealer) == cx.osty()) && total >= g::mighty_wings::DAMAGE_THRESHOLD {
            cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(g::mighty_wings::STRENGTH_POWER as i64), NO, NO);
        }
    }
});

listener!(ObsidianVice {
    fn before_side_turn_end(&self, cx: &mut Combat, _me: Me, side: Side) {
        let count = cx.player.hand.len() as i32;
        if side == Side::Player && count != 0 {
            cx.damage_random_hittable_enemy(g::obsidian_vice::DAMAGE * count, ValueProp::UNPOWERED);
        }
    }
});

listener!(Thumbscrew {
    fn modify_attack_hit_count(&self, cx: &Combat, _me: Me, attack: &Attack, hits: i32) -> i32 {
        if hits <= 1 || attack.card == NO || cx.card_def(attack.card).ctype != CardType::Attack {
            return hits;
        }
        hits + 1
    }
});

// Touch of Orobas upgrade of Song Bow.
listener!(Storyteller {
    fn after_room_entered(&self, cx: &mut Combat, _me: Me) {
        cx.apply_power(ids::power::AIM_POWER, PLAYER, Dec::int(g::storyteller::AIM_POWER as i64), PLAYER, NO);
    }
    fn before_side_turn_end(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player {
            cx.apply_power(ids::power::AIM_POWER, PLAYER, Dec::int(g::storyteller::END_OF_TURN_AIM as i64), PLAYER, NO);
        }
    }
});

listener!(TormentousCollar {
    fn before_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if cx.room_type == 1 && side == Side::Player && cx.turn_number() <= 1 {
            cx.apply_power_to_hittable_enemies(ids::power::WEAK_POWER, Dec::int(g::tormentous_collar::WEAK_POWER as i64), PLAYER, NO);
        }
    }
});
