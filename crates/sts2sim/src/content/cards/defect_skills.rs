use super::defect_util::*;
use crate::defs::VarKind;
use crate::engine::Ask;
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

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

listener!(ChargeBattery {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        let e = cx.card_var(p.card, VarKind::Energy);
        apply_self(cx, p, ids::power::ENERGY_NEXT_TURN_POWER, e);
        Flow::Done
    }
});

listener!(Compact {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        let up = cx.cards[p.card as usize].upgrade;
        let hand = cx.player.hand;
        let mut originals: crate::util::ArrayVec<CardIdx, 16> = crate::util::ArrayVec::new();
        let mut repl: crate::util::ArrayVec<Option<(u16, u8)>, 16> = crate::util::ArrayVec::new();
        for &c in hand.iter() {
            if cx.card_def(c).ctype == CardType::Status {
                originals.push(c);
                repl.push(Some((ids::card::FUEL, up)));
            }
        }
        cx.transform_cards(originals.as_slice(), repl.as_slice());
        Flow::Done
    }
});

listener!(DoubleEnergy {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.player.energy;
        cx.gain_energy(e);
        Flow::Done
    }
});

listener!(EnergySurge {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        if cx.cr(PLAYER).is_alive() {
            let e = cx.card_var(p.card, VarKind::Energy);
            cx.gain_energy(e);
        }
        Flow::Done
    }
});

listener!(FightThrough {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        for _ in 0..2 {
            cx.create_card_for_player(ids::card::WOUND, 0, PileType::Discard, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

listener!(GeneticAlgorithm {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        block(cx, p);
        let inc = cx.card_var(p.card, VarKind::Named);
        cx.cards[p.card as usize].counter[0] += inc as i16;
        Flow::Done
    }
});

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

listener!(Overclock {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        cx.create_card_for_player(ids::card::BURN, 0, PileType::Discard, CardPilePosition::Bottom);
        Flow::Done
    }
});

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

listener!(Turbo {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        cx.create_card_for_player(ids::card::VOID, 0, PileType::Discard, CardPilePosition::Bottom);
        Flow::Done
    }
});

listener!(WhiteNoise {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        let pool = cx.character_pool();
        let cards = cx.get_distinct_for_combat(pool, 1, |dd| dd.ctype == CardType::Power);
        if let Some(c) = cards.first() {
            cx.set_to_free_this_turn(c);
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

listener!(ImitationLearning {});

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

listener!(BiasedCognition {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let f = cx.card_power_var(p.card, ids::power::FOCUS_POWER);
        apply_self(cx, p, ids::power::FOCUS_POWER, f);
        let b = cx.card_power_var(p.card, ids::power::BIASED_COGNITION_POWER);
        apply_self(cx, p, ids::power::BIASED_COGNITION_POWER, b);
        Flow::Done
    }
});

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
