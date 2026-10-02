//! Defect attack cards that do not (only) touch orbs.

use super::defect_util::*;
use crate::defs::VarKind;
use crate::engine::{Attack, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// Damage, then a copy that costs 0 for the rest of the combat goes to the discard pile.
listener!(AdaptiveStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        if let Some(c) = cx.clone_card(p.card) {
            cx.cost_set_this_combat(c, 0);
            cx.add_generated_card_by(c, PileType::Discard, CardPilePosition::Bottom, PLAYER);
        }
        Flow::Done
    }
});

// Damage, then every 0-cost (non-X) Attack/Skill/Power in the discard pile returns to hand.
listener!(AllForOne {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        let discard = cx.player.discard;
        let mut list: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
        for &c in discard.iter() {
            let dd = cx.card_def(c);
            if cx.card_cost(c, true) == 0 && !dd.x_cost && matches!(dd.ctype, CardType::Attack | CardType::Skill | CardType::Power) {
                list.push(c);
            }
        }
        for &c in list.iter() {
            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

listener!(BeamCell {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        let v = cx.card_power_var(p.card, ids::power::VULNERABLE_POWER);
        cx.apply_power(ids::power::VULNERABLE_POWER, p.target, d(v), PLAYER, p.card);
        Flow::Done
    }
});

// Damage (`Damage` var + the growth every Claw has accumulated), then every Claw in the combat piles gains `Increase`.
listener!(Claw {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        let inc = cx.card_var(p.card, VarKind::Named);
        for c in cx.all_combat_cards().iter() {
            if cx.cards[*c as usize].id == ids::card::CLAW {
                cx.cards[*c as usize].counter[0] += inc as i16;
            }
        }
        Flow::Done
    }
});

listener!(FocusedStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        let v = cx.card_power_var(p.card, ids::power::FOCUS_POWER);
        apply_self(cx, p, ids::power::FOCUSED_STRIKE_POWER, v);
        Flow::Done
    }
});

// Damage, then draw a card if fewer than `PlayMax` cards have finished playing this turn.
listener!(Ftl {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        if (cx.hist.plays_finished as i32) < cx.card_var(p.card, VarKind::Named) {
            let n = cx.card_var(p.card, VarKind::Cards);
            cx.draw_cards(n, false);
        }
        Flow::Done
    }
});

// Damage, then Weak if the target intends to attack.
listener!(GoForTheEyes {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        if cx.intends_to_attack(p.target) {
            let v = cx.card_power_var(p.card, ids::power::WEAK_POWER);
            cx.apply_power(ids::power::WEAK_POWER, p.target, d(v), PLAYER, p.card);
        }
        Flow::Done
    }
});

// `Repeat` hits, then a Slimed goes to the discard pile.
listener!(GunkUp {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Repeat);
        attack_hits(cx, p, n);
        cx.create_card_for_player(ids::card::SLIMED, 0, PileType::Discard, CardPilePosition::Bottom);
        Flow::Done
    }
});

// Hits = CalculationBase (0) + CalculationExtra (1) x energy spent this turn (not counting this card's own cost).
listener!(HelixDrill {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let mut spent = cx.hist.energy_spent as i32;
        if cx.card_pile_type(p.card) == PileType::Play {
            spent -= cx.card_cost(p.card, true);
        }
        let hits = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::CalcExtra) * spent;
        attack_hits(cx, p, hits);
        Flow::Done
    }
});

// Damage all enemies, then -Focus until the end of the turn.
listener!(Hyperbeam {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_all(cx, p);
        let v = cx.card_power_var(p.card, ids::power::FOCUS_POWER);
        apply_self(cx, p, ids::power::HYPERBEAM_FOCUS_DOWN_POWER, v);
        Flow::Done
    }
});

// Damage, then this card costs 0 for the rest of the combat.
listener!(MomentumStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        cx.cost_set_this_combat(p.card, 0);
        Flow::Done
    }
});

// Damage, draw; whenever the owner generates a Status card this card costs 1 less until played.
listener!(RocketPunch {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
    fn after_card_generated_for_combat(&self, cx: &mut Combat, me: Me, card: CardIdx) {
        if cx.card_creator != PLAYER || cx.card_def(card).ctype != CardType::Status {
            return;
        }
        cx.cost_add_until_played(me.idx as CardIdx, -1);
    }
});

// Damage, draw `Cards`, discard the drawn cards that do not cost 0 (X-cost cards count as non-zero).
listener!(Scrape {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        let n = cx.card_var(p.card, VarKind::Cards);
        let drawn = cx.draw_cards_list(n, false);
        let mut discard: crate::util::ArrayVec<CardIdx, MAX_HAND> = crate::util::ArrayVec::new();
        for &c in drawn.iter() {
            if cx.card_cost(c, true) != 0 || cx.card_def(c).x_cost {
                discard.push(c);
            }
        }
        // CardCmd.Discard: nothing if the combat is over/ending; each card moves + fires AfterCardDiscarded (Sly: none here).
        if !cx.is_over_or_ending() {
            for &c in discard.iter() {
                cx.discard_card(c);
            }
        }
        Flow::Done
    }
});

// Damage; if it killed the target gain `Energy`.
listener!(Sunder {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let results = attack(cx, p);
        if results.iter().any(|r| r.killed) {
            let e = cx.card_var(p.card, VarKind::Energy);
            cx.gain_energy(e);
        }
        Flow::Done
    }
});

listener!(SweepingBeam {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack_all(cx, p);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

// Damage, then the next Power card played this turn is free.
listener!(Synthesis {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        attack(cx, p);
        apply_self(cx, p, ids::power::FREE_POWER_POWER, 1);
        Flow::Done
    }
});

// Exhaust every Status card in the combat piles (not already exhausted), then hit a random enemy once per status
// (the count is taken before exhausting).
listener!(FlakCannon {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let mut statuses: crate::util::ArrayVec<CardIdx, MAX_CARDS> = crate::util::ArrayVec::new();
        for &c in cx.all_combat_cards().iter() {
            if cx.card_def(c).ctype == CardType::Status && cx.card_pile_type(c) != PileType::Exhaust {
                statuses.push(c);
            }
        }
        let hits = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::CalcExtra) * statuses.len() as i32;
        for &c in statuses.iter() {
            cx.exhaust_card(c, false);
        }
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Random).hits(hits));
        Flow::Done
    }
});
