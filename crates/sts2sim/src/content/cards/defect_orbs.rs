//! Defect cards that channel / evoke / read orbs (ported from the decompiled `OnPlay` bodies).

use super::defect_util::*;
use crate::defs::VarKind;
use crate::engine::{Attack, Targeting, VALID_ORBS};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

fn channel_n(cx: &mut Combat, kind: u16, n: i32) {
    for _ in 0..n {
        cx.channel_orb(kind);
    }
}

// Damage, then channel a Lightning.
listener!(BallLightning {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        cx.channel_orb(ids::orb::LIGHTNING_ORB);
        Flow::Done
    }
});

// Hits = CalculationBase (0) + CalculationExtra (1) x orbs in the queue.
listener!(Barrage {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let hits = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::CalcExtra) * cx.orb_count();
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)).hits(hits));
        Flow::Done
    }
});

// Remove orb slots, then +Strength, +Dexterity.
listener!(BulkUp {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let slots = cx.card_var(p.card, VarKind::Named);
        cx.remove_orb_slots(slots);
        let s = cx.card_power_var(p.card, ids::power::STRENGTH_POWER);
        cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, d(s), PLAYER, p.card);
        let x = cx.card_power_var(p.card, ids::power::DEXTERITY_POWER);
        cx.apply_power(ids::power::DEXTERITY_POWER, PLAYER, d(x), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Capacitor {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Repeat);
        cx.add_orb_slots(n);
        Flow::Done
    }
});

// Channel `Repeat` random orbs (`combat_orbs` stream, `_validOrbs` order).
listener!(Chaos {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Repeat);
        for _ in 0..n {
            let i = cx.rng.combat_orbs.next_int_range(0, VALID_ORBS.len() as i32) as usize;
            cx.channel_orb(VALID_ORBS[i]);
        }
        Flow::Done
    }
});

// One Frost per hittable enemy (counted once, before channeling).
listener!(Chill {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.hittable_enemies().len() as i32;
        channel_n(cx, ids::orb::FROST_ORB, n);
        Flow::Done
    }
});

listener!(ColdSnap {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        cx.channel_orb(ids::orb::FROST_ORB);
        Flow::Done
    }
});

// Damage, then draw CalculationBase (0) + CalculationExtra (1) x distinct orb types.
listener!(CompileDriver {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        let n = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::CalcExtra) * cx.distinct_orb_types();
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(Coolheaded {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.channel_orb(ids::orb::FROST_ORB);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

// Channel Dark, then trigger the passive of every Dark orb (1x, or 2x upgraded) directly (no trigger-count hooks).
listener!(Darkness {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.channel_orb(ids::orb::DARK_ORB);
        let times = if cx.cards[p.card as usize].upgrade > 0 { 2 } else { 1 };
        let orbs = cx.player.orbs;
        for o in orbs.iter().filter(|o| o.kind == ids::orb::DARK_ORB) {
            for _ in 0..times {
                cx.orb_passive(*o, NO, false);
            }
        }
        Flow::Done
    }
});

listener!(Fusion {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        cx.channel_orb(ids::orb::PLASMA_ORB);
        Flow::Done
    }
});

listener!(Glacier {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        channel_n(cx, ids::orb::FROST_ORB, 2);
        Flow::Done
    }
});

listener!(Glasswork {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        cx.channel_orb(ids::orb::GLASS_ORB);
        Flow::Done
    }
});

listener!(IceLance {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        let n = cx.card_var(p.card, VarKind::Repeat);
        channel_n(cx, ids::orb::FROST_ORB, n);
        Flow::Done
    }
});

// Multiplayer-only (targets an ally): channel Plasma for the target player (== the owner in single player).
listener!(Ignition {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        cx.channel_orb(ids::orb::PLASMA_ORB);
        Flow::Done
    }
});

listener!(MeteorStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        channel_n(cx, ids::orb::PLASMA_ORB, 3);
        Flow::Done
    }
});

// Add orb slots, draw, then the card costs 1 more for the rest of the combat.
listener!(Modded {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let slots = cx.card_var(p.card, VarKind::Repeat);
        cx.add_orb_slots(slots);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        cx.add_cost_this_combat(p.card, 1, false);
        Flow::Done
    }
});

// X-cost: evoke the front orb X (+1 upgraded) times; only the last evoke removes the orb.
listener!(MultiCast {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let mut n = x_value(cx, p.card);
        if cx.cards[p.card as usize].upgrade > 0 {
            n += 1;
        }
        for i in 0..n {
            cx.evoke_next(i == n - 1);
        }
        Flow::Done
    }
});

listener!(Null {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        let w = cx.card_power_var(p.card, ids::power::WEAK_POWER);
        cx.apply_power(ids::power::WEAK_POWER, p.target, d(w), PLAYER, p.card);
        cx.channel_orb(ids::orb::DARK_ORB);
        Flow::Done
    }
});

// If any orb: evoke the front orb `Repeat` times, removing it only on the last evoke.
listener!(Quadcast {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        if cx.orb_count() <= 0 {
            return Flow::Done;
        }
        let n = cx.card_var(p.card, VarKind::Repeat);
        for i in 0..n {
            cx.evoke_next(i == n - 1);
        }
        Flow::Done
    }
});

listener!(Rainbow {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        cx.channel_orb(ids::orb::LIGHTNING_ORB);
        cx.channel_orb(ids::orb::FROST_ORB);
        cx.channel_orb(ids::orb::DARK_ORB);
        Flow::Done
    }
});

// Two hits, then channel `Repeat` Glass.
listener!(Refract {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)).hits(2));
        let n = cx.card_var(p.card, VarKind::Repeat);
        channel_n(cx, ids::orb::GLASS_ORB, n);
        Flow::Done
    }
});

listener!(ShadowShield {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        cx.channel_orb(ids::orb::DARK_ORB);
        Flow::Done
    }
});

// Damage all, then for each orb (counted once): evoke the front orb without dequeuing, then again dequeuing.
listener!(Shatter {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::AllOpponents));
        let n = cx.orb_count();
        for _ in 0..n {
            cx.evoke_next(false);
            cx.evoke_next(true);
        }
        Flow::Done
    }
});

// Focus (until end of turn) = CalculationBase (0) + CalculationExtra x distinct orb types.
listener!(Synchronize {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::CalcExtra) * cx.distinct_orb_types();
        cx.apply_power(ids::power::SYNCHRONIZE_POWER, PLAYER, d(n), PLAYER, p.card);
        Flow::Done
    }
});

// X-cost: channel X (+1 upgraded) Lightning.
listener!(Tempest {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let mut n = x_value(cx, p.card);
        if cx.cards[p.card as usize].upgrade > 0 {
            n += 1;
        }
        channel_n(cx, ids::orb::LIGHTNING_ORB, n);
        Flow::Done
    }
});

// Damage, then trigger each Lightning orb's passive (1x, 2x upgraded) at the played card's target.
listener!(TeslaCoil {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        let times = if cx.cards[p.card as usize].upgrade > 0 { 2 } else { 1 };
        let orbs = cx.player.orbs;
        for o in orbs.iter().filter(|o| o.kind == ids::orb::LIGHTNING_ORB) {
            for _ in 0..times {
                cx.orb_passive(*o, p.target, false);
            }
        }
        Flow::Done
    }
});

// Channel as many Lightning as were channeled this combat (`OrbChanneledEntry` history).
listener!(Voltaic {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::CalcExtra) * cx.hist_log.lightning_channeled as i32;
        channel_n(cx, ids::orb::LIGHTNING_ORB, n);
        Flow::Done
    }
});

listener!(ConsumingShadow {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Repeat);
        channel_n(cx, ids::orb::DARK_ORB, n);
        let v = cx.card_power_var(p.card, ids::power::CONSUMING_SHADOW_POWER);
        cx.apply_power(ids::power::CONSUMING_SHADOW_POWER, PLAYER, d(v), PLAYER, p.card);
        Flow::Done
    }
});

// Multiplayer-only: Hibernate power (1), then `Repeat` Frost.
listener!(Hibernate {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.apply_power(ids::power::HIBERNATE_POWER, PLAYER, d(1), PLAYER, p.card);
        let n = cx.card_var(p.card, VarKind::Repeat);
        channel_n(cx, ids::orb::FROST_ORB, n);
        Flow::Done
    }
});

// Upgraded: channel a Glass first; then the Spinner power.
listener!(Spinner {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        if cx.cards[p.card as usize].upgrade > 0 {
            cx.channel_orb(ids::orb::GLASS_ORB);
        }
        let v = cx.card_power_var(p.card, ids::power::SPINNER_POWER);
        cx.apply_power(ids::power::SPINNER_POWER, PLAYER, d(v), PLAYER, p.card);
        Flow::Done
    }
});
