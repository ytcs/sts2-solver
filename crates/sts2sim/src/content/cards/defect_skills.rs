//! Defect skills and power cards (ported from the decompiled `OnPlay` bodies).

use super::defect_util::*;
use crate::defs::VarKind;
use crate::engine::Ask;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

// ---- skills --------------------------------------------------------------------------------------------------------

// Block, then a Dazed goes to the discard pile.
listener!(BoostAway {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        cx.create_card_for_player(ids::card::DAZED, 0, PileType::Discard, CardPilePosition::Bottom);
        Flow::Done
    }
});

listener!(BootSequence {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        Flow::Done
    }
});

// Block, then `Energy` extra energy next turn.
listener!(ChargeBattery {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        let e = cx.card_var(p.card, VarKind::Energy);
        apply_self(cx, p, ids::power::ENERGY_NEXT_TURN_POWER, e);
        Flow::Done
    }
});

/// `CardCmd.Transform(list, null)` for cards in the HAND (`Compact`): originals leave the hand (list indices are read
/// one at a time while earlier originals are already removed), then the replacements are inserted at those indices in
/// ascending index order; each replacement enters combat (`AfterCardEnteredCombat`, `AfterCardChangedPiles(Hand)`).
fn transform_hand_cards(cx: &mut Combat, pairs: &[(CardIdx, CardIdx)]) {
    if cx.is_ending() || pairs.is_empty() {
        return;
    }
    let mut work: crate::util::ArrayVec<(usize, CardIdx), 16> = crate::util::ArrayVec::new();
    for &(orig, repl) in pairs {
        let idx = cx.player.hand.position(orig).unwrap_or(0);
        cx.player.hand.remove_value(orig);
        cx.cards[orig as usize].pile = PileType::None as u8;
        work.push((idx, repl));
    }
    // List.Sort on (pile type, index): the hand is the only pile, so ascending index (stable enough: indices differ or
    // equal keys keep the creation order for the same index, which the game's introsort also leaves in place for 2-3 items).
    let sl = work.as_mut_slice();
    for i in 1..sl.len() {
        let x = sl[i];
        let mut j = i;
        while j > 0 && sl[j - 1].0 > x.0 {
            sl[j] = sl[j - 1];
            j -= 1;
        }
        sl[j] = x;
    }
    for &(idx, repl) in work.iter() {
        let idx = idx.min(cx.player.hand.len());
        cx.player.hand.insert(idx, repl);
        cx.cards[repl as usize].pile = PileType::Hand as u8;
        cx.dispatch_g(hookbit::after_card_entered_combat, |cx, me, l| l.after_card_entered_combat(cx, me, repl));
        cx.dispatch_u(hookbit::after_card_changed_piles, |cx, me, l| l.after_card_changed_piles(cx, me, repl, PileType::Hand));
    }
}

// Block, then every Status card in hand becomes a Fuel (upgraded if this card is).
listener!(Compact {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        let up = cx.cards[p.card as usize].upgrade;
        let hand = cx.player.hand;
        let mut pairs: crate::util::ArrayVec<(CardIdx, CardIdx), 16> = crate::util::ArrayVec::new();
        for &c in hand.iter() {
            if cx.card_def(c).ctype == CardType::Status {
                if let Some(f) = cx.new_card(ids::card::FUEL, up) {
                    pairs.push((c, f));
                }
            }
        }
        transform_hand_cards(cx, pairs.as_slice());
        Flow::Done
    }
});

// Gain as much energy as you currently have.
listener!(DoubleEnergy {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.player.energy;
        cx.gain_energy(e);
        Flow::Done
    }
});

// Multiplayer-only (targets all allies): `Energy` for every living player.
listener!(EnergySurge {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        if cx.cr(PLAYER).is_alive() {
            let e = cx.card_var(p.card, VarKind::Energy);
            cx.gain_energy(e);
        }
        Flow::Done
    }
});

// Block, then two Wounds go to the discard pile.
listener!(FightThrough {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        for _ in 0..2 {
            cx.create_card_for_player(ids::card::WOUND, 0, PileType::Discard, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

// Block (1 + everything gained so far), then this card (and its deck version) gains `Increase` block permanently.
listener!(GeneticAlgorithm {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        let inc = cx.card_var(p.card, VarKind::Named);
        cx.cards[p.card as usize].counter[0] += inc as i16;
        Flow::Done
    }
});

// Block, then choose a card from the discard pile and put it into hand.
listener!(Hologram {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                block(cx, p);
                match cx.ask_pile(ids::card::HOLOGRAM, PileType::Discard, 1, 1, |_, _| true) {
                    Ask::Resolved(cards) => {
                        if let Some(c) = cards.first() {
                            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                        }
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                }
                Flow::Done
            }
        }
    }
});

// +Focus (`Focus` var) until the end of the turn.
listener!(Hotfix {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::FOCUS_POWER);
        apply_self(cx, p, ids::power::HOTFIX_POWER, v);
        Flow::Done
    }
});

listener!(Leap {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        Flow::Done
    }
});

listener!(LightningRod {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        let v = cx.card_power_var(p.card, ids::power::LIGHTNING_ROD_POWER);
        apply_self(cx, p, ids::power::LIGHTNING_ROD_POWER, v);
        Flow::Done
    }
});

// Draw `Cards`, then a Burn goes to the discard pile.
listener!(Overclock {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        cx.create_card_for_player(ids::card::BURN, 0, PileType::Discard, CardPilePosition::Bottom);
        Flow::Done
    }
});

// Put the whole hand on the bottom of the draw pile, shuffle the discard + draw piles together, draw `Cards`.
listener!(Reboot {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let hand = cx.player.hand;
        for &c in hand.iter() {
            cx.move_card(c, PileType::Draw, CardPilePosition::Bottom);
        }
        cx.shuffle_discard_into_draw();
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

// Choose a card in hand to exhaust, then `Energy` extra energy next turn.
listener!(Scavenge {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => match cx.ask_hand(ids::card::SCAVENGE, 1, 1, |_, _| true) {
                Ask::Resolved(cards) => {
                    if let Some(c) = cards.first() {
                        cx.exhaust_card(c, false);
                    }
                    let e = cx.card_var(p.card, VarKind::Energy);
                    apply_self(cx, p, ids::power::ENERGY_NEXT_TURN_POWER, e);
                    Flow::Done
                }
                Ask::Pending => Flow::Suspend(1),
            },
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.exhaust_card(c, false);
                }
                let e = cx.card_var(p.card, VarKind::Energy);
                apply_self(cx, p, ids::power::ENERGY_NEXT_TURN_POWER, e);
                Flow::Done
            }
        }
    }
});

listener!(Skim {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(Supercritical {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        Flow::Done
    }
});

// Gain `Energy`, then a Void goes to the discard pile.
listener!(Turbo {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        cx.create_card_for_player(ids::card::VOID, 0, PileType::Discard, CardPilePosition::Bottom);
        Flow::Done
    }
});

// A random Power card from the Defect pool joins the hand, free this turn.
listener!(WhiteNoise {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        let pool = cx.character_pool();
        let cards = cx.get_distinct_for_combat(pool, 1, |dd| dd.ctype == CardType::Power);
        if let Some(c) = cards.first() {
            cx.set_to_free_this_turn(c);
            cx.add_generated_card_by(c, PileType::Hand, CardPilePosition::Bottom, PLAYER);
        }
        Flow::Done
    }
});

// Multiplayer-only (targets an ally): never playable in single player; copies another player's Power plays.
listener!(ImitationLearning {});

// ---- power cards ---------------------------------------------------------------------------------------------------

// Power cards whose only effect is `PowerCmd.Apply<T>(owner, T-var)`.
listener!(Buffer {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::BUFFER_POWER);
        apply_self(cx, p, ids::power::BUFFER_POWER, v);
        Flow::Done
    }
});

listener!(Coolant {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::COOLANT_POWER);
        apply_self(cx, p, ids::power::COOLANT_POWER, v);
        Flow::Done
    }
});

listener!(Hailstorm {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::HAILSTORM_POWER);
        apply_self(cx, p, ids::power::HAILSTORM_POWER, v);
        Flow::Done
    }
});

listener!(Iteration {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::ITERATION_POWER);
        apply_self(cx, p, ids::power::ITERATION_POWER, v);
        Flow::Done
    }
});

listener!(Smokestack {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::SMOKESTACK_POWER);
        apply_self(cx, p, ids::power::SMOKESTACK_POWER, v);
        Flow::Done
    }
});

listener!(Storm {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::STORM_POWER);
        apply_self(cx, p, ids::power::STORM_POWER, v);
        Flow::Done
    }
});

listener!(Thunder {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::THUNDER_POWER);
        apply_self(cx, p, ids::power::THUNDER_POWER, v);
        Flow::Done
    }
});

listener!(Defragment {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::FOCUS_POWER);
        apply_self(cx, p, ids::power::FOCUS_POWER, v);
        Flow::Done
    }
});

listener!(OneForAll {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::ONE_FOR_ALL_POWER);
        apply_self(cx, p, ids::power::ONE_FOR_ALL_POWER, v);
        Flow::Done
    }
});

// Focus +5 and Biased Cognition (lose 1 Focus every turn).
listener!(BiasedCognition {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let f = cx.card_power_var(p.card, ids::power::FOCUS_POWER);
        apply_self(cx, p, ids::power::FOCUS_POWER, f);
        let b = cx.card_power_var(p.card, ids::power::BIASED_COGNITION_POWER);
        apply_self(cx, p, ids::power::BIASED_COGNITION_POWER, b);
        Flow::Done
    }
});

// Named-var powers: `DynamicVars["Loop"]`, `["CreativeAi"]`.
listener!(Loop {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_var(p.card, VarKind::Named);
        apply_self(cx, p, ids::power::LOOP_POWER, v);
        Flow::Done
    }
});

listener!(CreativeAi {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_var(p.card, VarKind::Named);
        apply_self(cx, p, ids::power::CREATIVE_AI_POWER, v);
        Flow::Done
    }
});

// `CardsVar` is the power's amount.
listener!(MachineLearning {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_var(p.card, VarKind::Cards);
        apply_self(cx, p, ids::power::MACHINE_LEARNING_POWER, v);
        Flow::Done
    }
});

listener!(EchoForm {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_var(p.card, VarKind::Named);
        apply_self(cx, p, ids::power::ECHO_FORM_POWER, v);
        Flow::Done
    }
});

listener!(Feral {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::FERAL_POWER);
        apply_self(cx, p, ids::power::FERAL_POWER, v);
        Flow::Done
    }
});

listener!(SignalBoost {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::SIGNAL_BOOST_POWER);
        apply_self(cx, p, ids::power::SIGNAL_BOOST_POWER, v);
        Flow::Done
    }
});

listener!(Subroutine {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_self(cx, p, ids::power::SUBROUTINE_POWER, 1);
        Flow::Done
    }
});

listener!(TrashToTreasure {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_self(cx, p, ids::power::TRASH_TO_TREASURE_POWER, 1);
        Flow::Done
    }
});
