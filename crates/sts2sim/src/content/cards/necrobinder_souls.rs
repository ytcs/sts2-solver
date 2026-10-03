//! Necrobinder cards that create / consume Souls, plus Ethereal-theme and misc cards.

use crate::content::gen_cards::var_name;
use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Ask, Attack, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

/// `PowerCmd.Apply<T>(Owner.Creature, amount, Owner.Creature, card)`.
fn apply_self(cx: &mut Combat, power: u16, amount: i32, p: &CardPlay) {
    cx.apply_power(power, PLAYER, Dec::int(amount as i64), PLAYER, p.card);
}

fn attack(cx: &mut Combat, p: &CardPlay, t: Targeting) -> crate::engine::Results {
    let d = cx.card_var(p.card, VarKind::Damage);
    cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, t))
}

fn block(cx: &mut Combat, p: &CardPlay) {
    let b = cx.card_var(p.card, VarKind::Block);
    cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card);
}

// ---- Souls -----------------------------------------------------------------------------------------------------------

// Unblockable unpowered damage (dealer = you), then Souls into the draw pile.
listener!(CaptureSpirit {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let d = cx.card_var(p.card, VarKind::Damage);
        let props = ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED).or(ValueProp::MOVE);
        cx.damage(&[p.target], Dec::int(d as i64), props, PLAYER, p.card);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.add_souls_to_draw_pile(n, false);
        Flow::Done
    }
});

listener!(Reave {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p, Targeting::Single(p.target));
        let n = cx.card_var(p.card, VarKind::Cards);
        let up = cx.cards[p.card as usize].upgrade > 0;
        cx.add_souls_to_draw_pile(n, up);
        Flow::Done
    }
});

// Three Souls: one to the draw pile (random position), one to the discard pile, one to the hand.
listener!(Severance {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p, Targeting::Single(p.target));
        let a = cx.new_soul(false);
        let b = cx.new_soul(false);
        let c = cx.new_soul(false);
        if let Some(a) = a {
            cx.add_generated_card(a, PileType::Draw, CardPilePosition::Random);
        }
        if let Some(b) = b {
            cx.add_generated_card(b, PileType::Discard, CardPilePosition::Bottom);
        }
        if let Some(c) = c {
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

listener!(GraveWarden {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.add_souls_to_draw_pile(n, false);
        Flow::Done
    }
});

// Transform a chosen card of the draw pile into a Soul.
listener!(Seance {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards).max(0) as u8;
        match phase {
            0 => match cx.ask_pile(ids::card::SEANCE, PileType::Draw, n, n, |_, _| true) {
                Ask::Resolved(cards) => {
                    transform_to_souls(cx, cards.as_slice());
                    Flow::Done
                }
                Ask::Pending => Flow::Suspend(1),
            },
            _ => {
                let cards = cx.choice.cards;
                transform_to_souls(cx, cards.as_slice());
                Flow::Done
            }
        }
    }
});

fn transform_to_souls(cx: &mut Combat, cards: &[CardIdx]) {
    for &c in cards {
        cx.transform_cards(&[c], &[Some((ids::card::SOUL, 0))]);
    }
}

// 9 (+2 upgrade) + 4 (+2) x Souls in the exhaust pile.
listener!(SoulStorm {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let souls = cx.player.exhaust.iter().filter(|&&c| cx.cards[c as usize].id == ids::card::SOUL).count() as i32;
        let d = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::ExtraDamage) * souls;
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::Single(p.target)));
        Flow::Done
    }
    // `CalculatedDamageVar.Calculate(target)` read generically (Thrash exhausting this card).
    fn calculated_damage(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<Dec> {
        let _ = target;
        let souls = if cx.in_progress { cx.player.exhaust.iter().filter(|&&c| cx.cards[c as usize].id == ids::card::SOUL).count() as i64 } else { 0 };
        Some(Dec::int(cx.card_var(card, VarKind::CalcBase) as i64 + cx.card_var(card, VarKind::ExtraDamage) as i64 * souls))
    }
});

// ---- Ethereal / draw theme ----------------------------------------------------------------------------------------------

listener!(Defile {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p, Targeting::Single(p.target));
        Flow::Done
    }
});

listener!(Defy {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        let w = cx.card_power_var(p.card, ids::power::WEAK_POWER);
        cx.apply_power(ids::power::WEAK_POWER, p.target, Dec::int(w as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Fear {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p, Targeting::Single(p.target));
        let v = cx.card_power_var(p.card, ids::power::VULNERABLE_POWER);
        cx.apply_power(ids::power::VULNERABLE_POWER, p.target, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Parse {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(Veilpiercer {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p, Targeting::Single(p.target));
        apply_self(cx, ids::power::VEILPIERCER_POWER, 1, p);
        Flow::Done
    }
});

// Hits = number of Ethereal cards you have played this combat.
listener!(PullFromBelow {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let hits = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::CalcExtra) * cx.hist_log.ethereal_finished as i32;
        let d = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, d, Targeting::Single(p.target)).hits(hits));
        Flow::Done
    }
});

// Costs 2 less (this combat) for each Ethereal card played so far, and 2 less after each one you play.
listener!(BansheesCry {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p, Targeting::AllOpponents);
        Flow::Done
    }
    fn after_card_entered_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {
        if card as u16 != me.idx || cx.cards[card as usize].flags & cflag::IS_CLONE != 0 {
            return;
        }
        let n = cx.hist_log.ethereal_finished as i32;
        let e = cx.card_var(card, VarKind::Energy);
        cx.add_cost_this_combat(card, -n * e, false);
    }
    fn after_card_played(&self, cx: &mut Combat, me: Me, play: &CardPlay) {
        if cx.card_keywords(play.card) & kw::ETHEREAL == 0 {
            return;
        }
        let c = me.idx as CardIdx;
        let e = cx.card_var(c, VarKind::Energy);
        cx.add_cost_this_combat(c, -e, false);
    }
});

// Auto-play every Ethereal, playable card in the exhaust pile.
listener!(Eidolon {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        let list = cx.player.exhaust;
        let mut todo: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
        for &c in list.iter() {
            let k = cx.card_keywords(c);
            if k & kw::ETHEREAL != 0 && k & kw::UNPLAYABLE == 0 {
                todo.push(c);
            }
        }
        // (the list is fixed up front; one queue so a decision raised by one play suspends the rest of the list)
        let _ = cx.auto_play_list(todo.as_slice());
        Flow::Done
    }
});

// Strength loss for the enemy (temporary).
listener!(EnfeeblingTouch {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_named_var(p.card, var_name::STRENGTH_LOSS);
        cx.apply_power(ids::power::ENFEEBLING_TOUCH_POWER, p.target, Dec::int(n as i64), PLAYER, p.card);
        Flow::Done
    }
});

// Attack, then give a non-Ethereal card in hand Ethereal.
listener!(SculptingStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                attack(cx, p, Targeting::Single(p.target));
                match cx.ask_hand(ids::card::SCULPTING_STRIKE, 1, 1, |cx, c| cx.card_keywords_local(c) & kw::ETHEREAL == 0) {
                    Ask::Resolved(cards) => {
                        if let Some(c) = cards.first() {
                            cx.apply_keyword(c, kw::ETHEREAL);
                        }
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.apply_keyword(c, kw::ETHEREAL);
                }
                Flow::Done
            }
        }
    }
});
