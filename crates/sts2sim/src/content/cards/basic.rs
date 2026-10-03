//! Card definitions and behaviour. (Stat tables are intended to be generated from the decompiled source.)

use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Ask, Attack, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

listener!(StrikeIronclad {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        Flow::Done
    }
});

listener!(DefendIronclad {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let block = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(block as i64), ValueProp::MOVE, p.card);
        Flow::Done
    }
});

listener!(Bash {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        let vuln = cx.card_power_var(p.card, ids::power::VULNERABLE_POWER);
        cx.apply_power(ids::power::VULNERABLE_POWER, p.target, Dec::int(vuln as i64), PLAYER, p.card);
        Flow::Done
    }
});

// ---- Ironclad commons / uncommons ported from the decompiled OnPlay bodies ----------------------------------------

listener!(ShrugItOff {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let block = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(block as i64), ValueProp::MOVE, p.card);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(PommelStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

listener!(TwinStrike {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)).hits(2));
        Flow::Done
    }
});

listener!(IronWave {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let block = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(block as i64), ValueProp::MOVE, p.card);
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        Flow::Done
    }
});

listener!(Thunderclap {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::AllOpponents));
        let v = cx.card_power_var(p.card, ids::power::VULNERABLE_POWER);
        cx.apply_power_to_hittable_enemies(ids::power::VULNERABLE_POWER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(SwordBoomerang {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        let hits = cx.card_var(p.card, VarKind::Repeat);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Random).hits(hits));
        Flow::Done
    }
});

listener!(Bloodletting {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let loss = cx.card_var(p.card, VarKind::HpLoss);
        // CreatureCmd.Damage(target = owner, props = Unblockable | Unpowered | Move, cardSource = this) => dealer = owner.
        cx.damage(&[PLAYER], Dec::int(loss as i64), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED).or(ValueProp::MOVE), PLAYER, p.card);
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        Flow::Done
    }
});

listener!(Inflame {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::STRENGTH_POWER);
        cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

listener!(Anger {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        if let Some(c) = cx.clone_card(p.card) {
            cx.add_generated_card(c, PileType::Discard, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

listener!(Cinder {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let dmg = cx.card_var(p.card, VarKind::Damage);
        cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
        if let Some(c) = cx.random_hand_card() {
            cx.exhaust_card(c, false);
        }
        Flow::Done
    }
});

// Block, then upgrade one card in hand (all of them when upgraded). Decision purpose tag: `ARMAMENTS`.
listener!(Armaments {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let block = cx.card_var(p.card, VarKind::Block);
                cx.gain_block(PLAYER, Dec::int(block as i64), ValueProp::MOVE, p.card);
                if cx.cards[p.card as usize].upgrade > 0 {
                    let hand = cx.player.hand;
                    for &c in hand.iter() {
                        cx.upgrade_in_combat(c);
                    }
                    return Flow::Done;
                }
                // FromHandForUpgrade: candidates = upgradable hand cards; exactly one; auto-resolves for <= 1.
                match cx.ask_hand(ids::card::ARMAMENTS, 1, 1, |cx, c| cx.is_upgradable(c)) {
                    Ask::Resolved(cards) => {
                        if let Some(&c) = cards.first().as_ref() {
                            cx.upgrade_in_combat(c);
                        }
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.upgrade_in_combat(c);
                }
                Flow::Done
            }
        }
    }
});

// Block, then exhaust a card from hand: random when un-upgraded, chosen when upgraded.
listener!(TrueGrit {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let block = cx.card_var(p.card, VarKind::Block);
                cx.gain_block(PLAYER, Dec::int(block as i64), ValueProp::MOVE, p.card);
                if cx.cards[p.card as usize].upgrade > 0 {
                    return match cx.ask_hand(ids::card::TRUE_GRIT, 1, 1, |_, _| true) {
                        Ask::Resolved(cards) => {
                            if let Some(c) = cards.first() {
                                cx.exhaust_card(c, false);
                            }
                            Flow::Done
                        }
                        Ask::Pending => Flow::Suspend(1),
                    };
                }
                if let Some(c) = cx.random_hand_card() {
                    cx.exhaust_card(c, false);
                }
                Flow::Done
            }
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.exhaust_card(c, false);
                }
                Flow::Done
            }
        }
    }
});

// Damage, then choose a card from the discard pile to put on top of the draw pile.
listener!(Headbutt {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let dmg = cx.card_var(p.card, VarKind::Damage);
                cx.execute_attack(&Attack::from_card(PLAYER, p.card, dmg, Targeting::Single(p.target)));
                match cx.ask_pile(ids::card::HEADBUTT, PileType::Discard, 1, 1, |_, _| true) {
                    Ask::Resolved(cards) => {
                        if let Some(c) = cards.first() {
                            cx.move_card(c, PileType::Draw, CardPilePosition::Top);
                        }
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.move_card(c, PileType::Draw, CardPilePosition::Top);
                }
                Flow::Done
            }
        }
    }
});

// Exhaust a card from hand, then draw.
listener!(BurningPact {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => match cx.ask_hand(ids::card::BURNING_PACT, 1, 1, |_, _| true) {
                Ask::Resolved(cards) => {
                    if let Some(c) = cards.first() {
                        cx.exhaust_card(c, false);
                    }
                    let n = cx.card_var(p.card, VarKind::Cards);
                    cx.draw_cards(n, false);
                    Flow::Done
                }
                Ask::Pending => Flow::Suspend(1),
            },
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.exhaust_card(c, false);
                }
                let n = cx.card_var(p.card, VarKind::Cards);
                cx.draw_cards(n, false);
                Flow::Done
            }
        }
    }
});

// Generate three distinct cards from the character's pool, choose one (or skip), add it to hand costing 0 this turn.
listener!(Discovery {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let pool = cx.character_pool();
                let cards = cx.get_distinct_for_combat(pool, 3, |_| true);
                match cx.ask_options(ids::card::DISCOVERY, cards.as_slice(), true) {
                    Ask::Resolved(cards) => {
                        // synchronous answer (Whispering Earring's selector, empty option list): same continuation as the resumed phase
                        cx.choice.cards = cards;
                        self.on_play(cx, _p, 1)
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.set_to_free_this_turn(c);
                    cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
                }
                Flow::Done
            }
        }
    }
});
