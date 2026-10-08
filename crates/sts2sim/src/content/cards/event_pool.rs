//! `EventCardPool` cards (gained from events / Ancients; Ancient and Event rarities) and the Quest cards.
//! Spec 03 for the generation helpers. Mad Science (per-instance type) lives in `mad_science.rs`.
//!
//! The Quest cards (Lantern Key, Spoils Map, Byrdonis Egg, Dowsing) are Unplayable and every override they have is run-level
//! (map generation, rest sites, room entry), so in combat they are inert deck clutter.

use crate::dec::Dec;
use crate::defs::VarKind;
use crate::engine::{Ask, Attack, Targeting};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

/// `DamageCmd.Attack(Damage var).FromCard(...)`, with the exact decimal value (permanent growth included).
fn card_attack(cx: &Combat, p: &CardPlay, targeting: Targeting) -> Attack {
    let mut a = Attack::from_card(PLAYER, p.card, 0, targeting);
    a.damage = cx.card_damage_dec(p.card);
    a
}

/// `PowerCmd.Apply<T>(Owner.Creature, amount, Owner.Creature, card)`.
fn apply_self(cx: &mut Combat, power: u16, amount: i32, p: &CardPlay) -> Option<u16> {
    cx.apply_power(power, PLAYER, Dec::int(amount as i64), PLAYER, p.card)
}

// ---- Quest cards: unplayable, no combat behaviour -------------------------------------------------------------------------------
listener!(LanternKey {});
listener!(SpoilsMap {});
listener!(ByrdonisEgg {});
listener!(Dowsing {});

// ---- Abundance: choose 1 of 3 upgraded Power cards from the pool; it enters the hand free this turn ------------------------------
listener!(Abundance {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let pool = cx.character_pool();
                let cards = cx.get_distinct_for_combat(pool, 3, |d| d.ctype == CardType::Power);
                for &c in cards.iter() {
                    cx.upgrade_in_combat(c);
                }
                match cx.ask_options(ids::card::ABUNDANCE, cards.as_slice(), false) {
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

// ---- Apotheosis: upgrade every other upgradable card in combat ----------------------------------------------------------------------
listener!(Apotheosis {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let cards = cx.all_combat_cards();
        for &c in cards.iter() {
            if c != p.card && cx.is_upgradable(c) {
                cx.upgrade_in_combat(c);
            }
        }
        Flow::Done
    }
});

listener!(Apparition {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::INTANGIBLE_POWER);
        apply_self(cx, ids::power::INTANGIBLE_POWER, v, p);
        Flow::Done
    }
});

// ---- Brightest Flame: +Energy, draw, lose max HP (in that order) ---------------------------------------------------------------------
listener!(BrightestFlame {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        cx.gain_energy(e);
        let n = cx.card_var(p.card, VarKind::Cards);
        cx.draw_cards_nosuspend(n, false);
        let m = cx.card_var(p.card, VarKind::MaxHp);
        cx.lose_max_hp(PLAYER, Dec::int(m as i64), true);
        Flow::Done
    }
});

listener!(ByrdSwoop {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::Single(p.target));
        cx.execute_attack(&a);
        Flow::Done
    }
});

listener!(Caltrops {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::THORNS_POWER);
        apply_self(cx, ids::power::THORNS_POWER, v, p);
        Flow::Done
    }
});

// ---- Clash: only playable while every card in hand is an Attack ----------------------------------------------------------------------
listener!(Clash {
    fn is_playable(&self, cx: &Combat, _card: CardIdx) -> bool {
        cx.player.hand.iter().all(|&c| cx.card_def(c).ctype == CardType::Attack)
    }
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::Single(p.target));
        cx.execute_attack(&a);
        Flow::Done
    }
});

// ---- Distraction: a random Skill from the pool, free this turn, into the hand --------------------------------------------------------
listener!(Distraction {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, _phase: u8) -> Flow {
        let pool = cx.character_pool();
        let cards = cx.get_distinct_for_combat(pool, 1, |d| d.ctype == CardType::Skill);
        if let Some(c) = cards.first() {
            cx.set_to_free_this_turn(c);
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

// ---- Dual Wield: pick an Attack/Power in hand, add Cards copies of it to the hand -----------------------------------------------------
listener!(DualWield {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => match cx.ask_hand(ids::card::DUAL_WIELD, 1, 1, |cx, c| matches!(cx.card_def(c).ctype, CardType::Attack | CardType::Power)) {
                Ask::Resolved(cards) => {
                    if let Some(c) = cards.first() {
                        copies(cx, p, c);
                    }
                    Flow::Done
                }
                Ask::Pending => Flow::Suspend(1),
            },
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    copies(cx, p, c);
                }
                Flow::Done
            }
        }
    }
});

fn copies(cx: &mut Combat, p: &CardPlay, selection: CardIdx) {
    let n = cx.card_var(p.card, VarKind::Cards);
    for _ in 0..n {
        if let Some(c) = cx.clone_card(selection) {
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
    }
}

// ---- Enlightenment: every card in hand costs at most 1 (this turn / until played; upgraded: for the combat) --------------------------
listener!(Enlightenment {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let upgraded = cx.cards[p.card as usize].upgrade > 0;
        let hand = cx.player.hand;
        for &c in hand.iter() {
            if upgraded {
                cx.set_cost_this_combat(c, 1, true);
            } else {
                cx.set_cost_this_turn_or_until_played(c, 1, true);
            }
        }
        Flow::Done
    }
});

// ---- Entrench: double the current Block (Unpowered) ------------------------------------------------------------------------------------
listener!(Entrench {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let b = cx.cr(PLAYER).block();
        cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::UNPOWERED.or(ValueProp::MOVE), p.card);
        Flow::Done
    }
});

listener!(Exterminate {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Repeat);
        let a = card_attack(cx, p, Targeting::AllOpponents).hits(n);
        cx.execute_attack(&a);
        Flow::Done
    }
});

// FeedingFrenzyPower (a `TemporaryStrengthPower`) lives in `powers/event_only.rs`.
listener!(FeedingFrenzy {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let v = cx.card_power_var(p.card, ids::power::STRENGTH_POWER);
        apply_self(cx, ids::power::FEEDING_FRENZY_POWER, v, p);
        Flow::Done
    }
});

listener!(HelloWorld {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        apply_self(cx, ids::power::HELLO_WORLD_POWER, 1, p);
        Flow::Done
    }
});

// ---- Maul: 2 hits; every Maul in the combat (this one included) permanently gains Increase damage -------------------------------------
listener!(Maul {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::Single(p.target)).hits(2);
        cx.execute_attack(&a);
        let inc = cx.card_named_var(p.card, crate::content::gen_cards::var_name::INCREASE);
        let all = cx.all_combat_cards();
        for &c in all.iter() {
            if cx.cards[c as usize].id == ids::card::MAUL {
                cx.add_card_damage(c, Dec::int(inc as i64));
            }
        }
        Flow::Done
    }
});

// ---- Metamorphosis: Cards random Attacks (with replacement), free this combat, shuffled into the draw pile --------------------------------
listener!(Metamorphosis {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Cards).max(0) as usize;
        let pool = cx.character_pool();
        let cards = cx.get_for_combat_where(pool, n, |d| d.ctype == CardType::Attack);
        for &c in cards.iter() {
            cx.set_to_free_this_combat(c);
            cx.add_generated_card(c, PileType::Draw, CardPilePosition::Random);
        }
        Flow::Done
    }
});

// ---- Neow's Fury: attack, then return up to Cards cards from the discard pile to the hand --------------------------------------------
listener!(NeowsFury {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => {
                let a = card_attack(cx, p, Targeting::Single(p.target));
                cx.execute_attack(&a);
                let cards = cx.card_var(p.card, VarKind::Cards);
                let room = MAX_HAND as i32 - cx.player.hand.len() as i32;
                let num = cards.min(room);
                if num <= 0 {
                    return Flow::Done;
                }
                match cx.ask_pile(ids::card::NEOWS_FURY, PileType::Discard, 0, num as u8, |_, _| true) {
                    Ask::Resolved(c) => {
                        cx.add_cards_to_pile(c.as_slice(), PileType::Hand, CardPilePosition::Bottom);
                        Flow::Done
                    }
                    Ask::Pending => Flow::Suspend(1),
                }
            }
            _ => {
                let picked = cx.choice.cards;
                cx.add_cards_to_pile(picked.as_slice(), PileType::Hand, CardPilePosition::Bottom);
                Flow::Done
            }
        }
    }
});

listener!(Outmaneuver {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let e = cx.card_var(p.card, VarKind::Energy);
        apply_self(cx, ids::power::ENERGY_NEXT_TURN_POWER, e, p);
        Flow::Done
    }
});

listener!(Peck {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.card_var(p.card, VarKind::Repeat);
        let a = card_attack(cx, p, Targeting::Single(p.target)).hits(n);
        cx.execute_attack(&a);
        Flow::Done
    }
});

listener!(Relax {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let b = cx.card_var(p.card, VarKind::Block);
        cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card);
        let n = cx.card_var(p.card, VarKind::Cards);
        apply_self(cx, ids::power::DRAW_CARDS_NEXT_TURN_POWER, n, p);
        let e = cx.card_var(p.card, VarKind::Energy);
        apply_self(cx, ids::power::ENERGY_NEXT_TURN_POWER, e, p);
        Flow::Done
    }
});

listener!(RipAndTear {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::Random).hits(2);
        cx.execute_attack(&a);
        Flow::Done
    }
});

listener!(Squash {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let a = card_attack(cx, p, Targeting::Single(p.target));
        cx.execute_attack(&a);
        let v = cx.card_power_var(p.card, ids::power::VULNERABLE_POWER);
        cx.apply_power(ids::power::VULNERABLE_POWER, p.target, Dec::int(v as i64), PLAYER, p.card);
        Flow::Done
    }
});

// ---- Stack: Block = CalcBase + CalcExtra * (cards in the discard pile) ---------------------------------------------------------------
listener!(Stack {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let n = cx.pile(PileType::Discard).len() as i32;
        let b = cx.card_var(p.card, VarKind::CalcBase) + cx.card_var(p.card, VarKind::CalcExtra) * n;
        cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card);
        Flow::Done
    }
    fn calculated_value(&self, cx: &Combat, card: CardIdx, target: Cid) -> Option<crate::dec::Dec> {
        let _ = target;
        Some(crate::engine::calc_extra_with(cx, card, cx.pile(PileType::Discard).len() as i32))
    }
});

// ---- Toric Toughness: Block now, and the same amount again at the start of each of the next Turns turns ------------------------------
// (`ToricToughnessPower.SetBlock(blockAmount)`: the power instance stores the Block that was actually gained, in `aux` as 1/10000.)
listener!(ToricToughness {
    fn on_play(&self, cx: &mut Combat, p: &CardPlay, _phase: u8) -> Flow {
        let b = cx.card_var(p.card, VarKind::Block);
        let gained = cx.gain_block(PLAYER, Dec::int(b as i64), ValueProp::MOVE, p.card);
        let turns = cx.card_named_var(p.card, crate::content::gen_cards::var_name::TURNS);
        if let Some(uid) = apply_self(cx, ids::power::TORIC_TOUGHNESS_POWER, turns, p) {
            if let Some(i) = cx.power_idx(PLAYER, uid) {
                cx.cr_mut(PLAYER).powers[i].aux = (gained * Dec::int(10_000)).trunc();
            }
        }
        Flow::Done
    }
});

// ---- Wish: pick a card from the draw pile into the hand -----------------------------------------------------------------------------
listener!(Wish {
    fn on_play(&self, cx: &mut Combat, _p: &CardPlay, phase: u8) -> Flow {
        match phase {
            0 => match cx.ask_pile(ids::card::WISH, PileType::Draw, 1, 1, |_, _| true) {
                Ask::Resolved(cards) => {
                    if let Some(c) = cards.first() {
                        cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                    }
                    Flow::Done
                }
                Ask::Pending => Flow::Suspend(1),
            },
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                }
                Flow::Done
            }
        }
    }
});
