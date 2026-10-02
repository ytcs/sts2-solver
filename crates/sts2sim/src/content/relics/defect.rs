//! Defect relics.

use crate::dec::Dec;
use crate::engine::HKind;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// BeforeSideTurnStart: on the player's first turn channel `Lightning` (DynamicVar = 1) orbs, before energy reset / draw.
listener!(CrackedCore {
    fn before_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.player.turn_number <= 1 {
            cx.channel_orb(ids::orb::LIGHTNING_ORB);
        }
    }
});

// ---- relics that read / feed the orb queue (hooks live in engine/orbs.rs) -----------------------------------------------

// ModifyOrbPassiveTriggerCounts: the FRONT orb's passive triggers one extra time.
listener!(GoldPlatedCables {
    fn modify_orb_passive_trigger_counts(&self, cx: &Combat, _me: Me, orb: &Orb, count: i32) -> i32 {
        match cx.player.orbs.first() {
            Some(front) if front.uid == orb.uid => count + 1,
            _ => count,
        }
    }
});

// Starter-relic alternative: 3 Lightning on turn 1 (after the turn-start hooks) and +1 to Lightning orb values.
listener!(InfusedCore {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.player.turn_number <= 1 {
            for _ in 0..3 {
                cx.channel_orb(ids::orb::LIGHTNING_ORB);
            }
        }
    }
    fn modify_orb_value(&self, _cx: &Combat, _me: Me, orb: &Orb, value: Dec) -> Dec {
        if orb.kind == ids::orb::LIGHTNING_ORB {
            value + Dec::int(1)
        } else {
            value
        }
    }
});

// +3 orb slots on turn 1.
listener!(RunicCapacitor {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.player.turn_number <= 1 {
            cx.add_orb_slots(3);
        }
    }
});

// 1 Dark orb on turn 1.
listener!(SymbioticVirus {
    fn after_side_turn_start(&self, cx: &mut Combat, _me: Me, side: Side) {
        if side == Side::Player && cx.player.turn_number <= 1 {
            cx.channel_orb(ids::orb::DARK_ORB);
        }
    }
});

// Counts channeled orbs (`Relic::counter`, reset by the combat); the 7th channel deals 30 Unpowered to all enemies.
listener!(Metronome {
    fn after_orb_channeled(&self, cx: &mut Combat, me: Me, _orb: &Orb) {
        let i = me.idx as usize;
        cx.player.relics[i].counter += 1;
        if cx.player.relics[i].counter == 7 {
            let targets = cx.hittable_enemies();
            cx.damage(targets.as_slice(), Dec::int(30), ValueProp::UNPOWERED, PLAYER, NO);
        }
    }
});

// Emotion Chip: at the start of the player's turn, if the player took damage that was not fully blocked during the
// previous turn (player or enemy side), trigger every orb's passive (counted through the trigger-count hooks).
listener!(EmotionChip {
    fn after_player_turn_start(&self, cx: &mut Combat, _me: Me) {
        // LostHpInPreviousTurn: a `DamageReceivedEntry` on the player that was not fully blocked (flags & 1) last player turn.
        if !cx.hist_any_last_player_turn(HKind::DamageReceived, |e| e.actor == PLAYER && e.flags & 1 == 0) {
            return;
        }
        let orbs = cx.player.orbs;
        for o in orbs.iter() {
            cx.orb_passive(*o, NO, true);
        }
    }
});
