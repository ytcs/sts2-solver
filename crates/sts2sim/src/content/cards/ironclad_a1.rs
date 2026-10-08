//! Ironclad cards, batch a1: pool positions [0,45) of the Ironclad pool (Aggression .. Impervious) that are not in basic.rs.

use crate::engine::calc_with;
use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Ask, Attack, HKind, RunResult, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

/// `CreatureCmd.Damage(owner, HpLoss, Unblockable | Unpowered | Move, this, cardPlay)` — card self-damage.
fn lose_hp(cx: &mut Combat, card: CardIdx, n: i32) {
    cx.damage(&[PLAYER], Dec::int(n as i64), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED).or(ValueProp::MOVE), PLAYER, card);
}

fn block(cx: &mut Combat, card: CardIdx) {
    let b = cx.card_var(card, VarKind::Block);
    cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, card);
}

fn attack(cx: &mut Combat, p: &CardPlay, hits: i32) {
    let dmg = cx.card_var(p.card, VarKind::Damage);
    cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)).hits(hits));
}

// ---- powers (apply-only cards) ---------------------------------------------------------------------------------------

listener!(Aggression {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.apply_power(ids::power::AGGRESSION_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
        Flow::Done
    }
});

listener!(Barricade {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.apply_power(ids::power::BARRICADE_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
        Flow::Done
    }
});

listener!(Corruption {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_var(p.card, VarKind::Named);
        cx.apply_power(ids::power::CORRUPTION_POWER, PLAYER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(CrimsonMantle {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::CRIMSON_MANTLE_POWER);
        if let Some(uid) = cx.apply_power(ids::power::CRIMSON_MANTLE_POWER, PLAYER, Dec::int(v as i64), PLAYER, p.card) {
            // IncrementSelfDamage
            if let Some(pw) = cx.power_mut(PLAYER, uid) {
                pw.aux += 1;
            }
        }
        Flow::Done
    }
});

listener!(Cruelty {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::CRUELTY_POWER);
        cx.apply_power(ids::power::CRUELTY_POWER, PLAYER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(DarkEmbrace {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.apply_power(ids::power::DARK_EMBRACE_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
        Flow::Done
    }
});

listener!(DemonForm {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::STRENGTH_POWER);
        cx.apply_power(ids::power::DEMON_FORM_POWER, PLAYER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(FeelNoPain {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_var(p.card, VarKind::Named);
        cx.apply_power(ids::power::FEEL_NO_PAIN_POWER, PLAYER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

// ---- skills ---------------------------------------------------------------------------------------------------------

listener!(BattleTrance {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards_nosuspend(n, false);
        cx.apply_power(ids::power::NO_DRAW_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
        Flow::Done
    }
});

// Multiplayer-only (AnyAlly target: never playable in single player).
listener!(Blaze {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::STRENGTH_POWER);
        cx.apply_power(ids::power::STRENGTH_POWER, p.target, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(BloodWall {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let loss = cx.card_var(p.card, VarKind::HpLoss);
        lose_hp(cx, p.card, loss);
        block(cx, p.card);
        Flow::Done
    }
});

// HP loss, exhaust a card from hand (chosen), gain Strength.
listener!(Brand {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        let finish = |cx: &mut Combat, p: &CardPlay| {
            let v = cx.card_power_var(p.card, ids::power::STRENGTH_POWER);
            cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(v as i64), PLAYER, p.card);
        };
        match phase {
            0 => {
                let loss = cx.card_var(p.card, VarKind::HpLoss);
                lose_hp(cx, p.card, loss);
                match cx.ask_hand(ids::card::BRAND, 1, 1, |_, _| true) {
                    Ask::Resolved(cards) => {
                        if let Some(c) = cards.first() {
                            cx.exhaust_card(c, false);
                        }
                        finish(cx, p);
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.exhaust_card(c, false);
                }
                finish(cx, p);
                Flow::Done
            }
        }
    }
});

listener!(Colossus {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p.card);
        let v = cx.card_var(p.card, VarKind::Named);
        cx.apply_power(ids::power::COLOSSUS_POWER, PLAYER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

// Multiplayer-only.
listener!(DemonicShield {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let loss = cx.card_var(p.card, VarKind::HpLoss);
        lose_hp(cx, p.card, loss);
        let amt = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::CalcExtra) * cx.cr(PLAYER).block;
        cx.gain_block(p.target, Dec::int(amt as i64), ValueProp::MOVE, p.card);
        Flow::Done
    }
    fn calculated_value(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<crate::dec::Dec> {
        let _ = target;
        Some(crate::engine::calc_extra_with(cx, card, cx.cr(PLAYER).block))
    }
});

// Apply Vulnerable, then gain Strength equal to the target's resulting Vulnerable amount.
listener!(Dominate {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::VULNERABLE_POWER);
        cx.apply_power(ids::power::VULNERABLE_POWER, p.target, Dec::int(v as i64), PLAYER, p.card);
        let n = cx.power_amount(p.target, ids::power::VULNERABLE_POWER);
        cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

// Draw; when this card is exhausted, gain Energy (once per play count, like a replay).
listener!(DrumOfBattle {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
    fn after_card_exhausted(&self, cx: &mut Combat, me: Me, card: CardIdx, _by_ethereal: bool) {
        if card as u16 != me.idx {
            return;
        }
        let count = cx.generate_play_count(card, NO);
        let e = cx.card_var(card, VarKind::Energy);
        for _ in 0..count {
            cx.gain_energy(e);
        }
    }
});

// Block twice if a card was exhausted this turn.
listener!(EvilEye {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let times = if cx.hist_count_this_turn(HKind::CardExhausted, |_| true) > 0 { 2 } else { 1 };
        for _ in 0..times {
            block(cx, p.card);
        }
        Flow::Done
    }
});

// Block = CalcBase + CalcExtra * max(0, Strength).
listener!(ExpectAFight {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let str = cx.power_amount(PLAYER, ids::power::STRENGTH_POWER).max(0);
        let amt = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::CalcExtra) * str;
        cx.gain_block(PLAYER, Dec::int(amt as i64), ValueProp::MOVE, p.card);
        Flow::Done
    }
    fn calculated_value(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<crate::dec::Dec> {
        let _ = target;
        Some(crate::engine::calc_extra_with(cx, card, cx.power_amount(PLAYER, ids::power::STRENGTH_POWER).max(0)))
    }
});

listener!(FlameBarrier {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p.card);
        let v = cx.card_var(p.card, VarKind::Named);
        cx.apply_power(ids::power::FLAME_BARRIER_POWER, PLAYER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(ForgottenRitual {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        Flow::Done
    }
});

listener!(Impervious {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p.card);
        Flow::Done
    }
});

// ---- attacks --------------------------------------------------------------------------------------------------------

listener!(AshenStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        // multiplier = number of cards in the exhaust pile
        let a = Attack::from_card_calc(PLAYER, p.card, Targeting::Single(p.target), |cx, _, _| cx.player.exhaust.len() as i32);
        cx.execute_attack(&a);
        Flow::Done
    }
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        let _ = target;
        Some(calc_with(cx, card, cx.player.exhaust.len() as i32))
    }
});

listener!(Bludgeon {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p, 1);
        Flow::Done
    }
});

// multiplier = owner's current Block
listener!(BodySlam {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = Attack::from_card_calc(PLAYER, p.card, Targeting::Single(p.target), |cx, _, _| cx.cr(PLAYER).block);
        cx.execute_attack(&a);
        Flow::Done
    }
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        let _ = target;
        Some(calc_with(cx, card, cx.cr(PLAYER).block))
    }
});

listener!(Break {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p, 1);
        let v = cx.card_power_var(p.card, ids::power::VULNERABLE_POWER);
        cx.apply_power(ids::power::VULNERABLE_POWER, p.target, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Breakthrough {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let loss = cx.card_var(p.card, VarKind::HpLoss);
        lose_hp(cx, p.card, loss);
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::AllOpponents));
        Flow::Done
    }
});

// multiplier = the target's Vulnerable amount
listener!(Bully {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = Attack::from_card_calc(PLAYER, p.card, Targeting::Single(p.target), |cx, _, t| {
            if t == NO { 0 } else { cx.power_amount(t, ids::power::VULNERABLE_POWER) }
        });
        cx.execute_attack(&a);
        Flow::Done
    }
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        Some(calc_with(cx, card, if target == NO { 0 } else { cx.power_amount(target, ids::power::VULNERABLE_POWER) }))
    }
});

listener!(Conflagration {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let hits = cx.card_var(p.card, VarKind::Repeat);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::AllOpponents).hits(hits));
        Flow::Done
    }
});

// Two hits against a Vulnerable target (decided before the attack).
listener!(Dismantle {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let hits = if cx.has_power(p.target, ids::power::VULNERABLE_POWER) { 2 } else { 1 };
        attack(cx, p, hits);
        Flow::Done
    }
});

// Gain max HP when the attack kills (unless the target does not trigger Fatal).
listener!(Feed {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let fatal = cx.all_powers_trigger_fatal(p.target);
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let res = cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        if fatal && res.iter().any(|r| r.killed) {
            let n = cx.card_var(p.card, VarKind::MaxHp);
            cx.gain_max_hp(PLAYER, Dec::int(n as i64));
        }
        Flow::Done
    }
});

// Exhaust the whole hand, then one hit per card exhausted.
listener!(FiendFire {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let hand = cx.player.hand;
        let n = hand.len() as i32;
        for &c in hand.iter() {
            cx.exhaust_card(c, false);
        }
        attack(cx, p, n);
        Flow::Done
    }
});

listener!(FightMe {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let hits = cx.card_var(p.card, VarKind::Repeat);
        attack(cx, p, hits);
        let s = cx.card_power_var(p.card, ids::power::STRENGTH_POWER);
        cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(s as i64), PLAYER, p.card);
        let e = cx.card_var(p.card, VarKind::Named);
        cx.apply_power(ids::power::STRENGTH_POWER, p.target, Dec::int(e as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Hemokinesis {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let loss = cx.card_var(p.card, VarKind::HpLoss);
        lose_hp(cx, p.card, loss);
        attack(cx, p, 1);
        Flow::Done
    }
});

// ---- auto-play -------------------------------------------------------------------------------------------------------

// Auto-play the top card of the draw pile, exhausting it. Phase 1 = resume after a nested decision.
listener!(Havoc {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, phase: u8) -> Flow {
        if phase == 0 && cx.auto_play_from_draw_pile(1, CardPilePosition::Top, true) == RunResult::Suspended {
            return Flow::Suspend(1);
        }
        Flow::Done
    }
});

// X (+1 if upgraded) cards from the top of the draw pile are auto-played (without forced exhaust).
listener!(Cascade {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        if phase == 0 {
            let mut n = cx.x_value(p.card);
            if cx.cards[p.card as usize].upgrade > 0 {
                n += 1;
            }
            if cx.auto_play_from_draw_pile(n, CardPilePosition::Top, false) == RunResult::Suspended {
                return Flow::Suspend(1);
            }
        }
        Flow::Done
    }
});

// All-enemies attack; when it sits in the exhaust pile at the start of the post-play phase it plays itself.
listener!(HowlFromBeyond {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::AllOpponents));
        Flow::Done
    }
    fn after_auto_post_play_phase_entered(&self, cx: &mut Combat, me: Me) {
        let c = me.idx as CardIdx;
        if cx.card_pile_type(c) == PileType::Exhaust {
            let _ = cx.auto_play(c, NO, AutoPlayType::Default, false);
        }
    }
});

listener!(Hellraiser {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        cx.apply_power(ids::power::HELLRAISER_POWER, PLAYER, Dec::ONE, PLAYER, p.card);
        Flow::Done
    }
});
