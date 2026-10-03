//! Potions that generate cards or move cards between piles.

use crate::content::gen_pools;
use crate::defs::{CardDef, VarKind};
use crate::engine::{Ask, RunResult};
use crate::hooks::*;
use crate::ids;
use crate::listener;
use crate::state::*;
use crate::types::*;

/// Decision purpose tag of a potion (card purposes are `ids::card::*`, potions set the high bit).
const fn purpose(potion: u16) -> u16 {
    0x8000 | potion
}

/// Shared body of Attack/Skill/Power/ColorlessPotion: `GetDistinctForCombat(pool.Where(filter), 3)` ->
/// `FromChooseACardScreen(canSkip: true)` -> chosen card is free this turn and added to the hand.
fn choose_a_card(cx: &mut Combat, potion: u16, phase: u8, pool: &[u16], extra: impl Fn(&CardDef) -> bool) -> Flow {
    match phase {
        0 => {
            let cards = cx.get_distinct_for_combat(pool, 3, extra);
            match cx.ask_options(purpose(potion), cards.as_slice(), true) {
                Ask::Resolved(_) => Flow::Done,
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

listener!(AttackPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, phase: u8) -> Flow {
        let pool = cx.character_pool();
        choose_a_card(cx, potion, phase, pool, |d| d.ctype == CardType::Attack)
    }
});

listener!(SkillPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, phase: u8) -> Flow {
        let pool = cx.character_pool();
        choose_a_card(cx, potion, phase, pool, |d| d.ctype == CardType::Skill)
    }
});

listener!(PowerPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, phase: u8) -> Flow {
        let pool = cx.character_pool();
        choose_a_card(cx, potion, phase, pool, |d| d.ctype == CardType::Power)
    }
});

listener!(ColorlessPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, phase: u8) -> Flow {
        choose_a_card(cx, potion, phase, &gen_pools::COLORLESS, |_| true)
    }
});

// Three colorless cards, upgraded, added to the hand.
listener!(CosmicConcoction {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Cards).max(0) as usize;
        let cards = cx.get_distinct_for_combat(&gen_pools::COLORLESS, n, |_| true);
        for &c in cards.iter() {
            cx.upgrade_in_combat(c);
            cx.add_generated_card(c, PileType::Hand, CardPilePosition::Bottom);
        }
        Flow::Done
    }
});

// One random Attack, Skill and Power (three independent distinct-draws), all free this turn.
listener!(OrobicAcid {
    fn on_use_potion(&self, cx: &mut Combat, _potion: u16, _target: Cid, _phase: u8) -> Flow {
        let pool = cx.character_pool();
        let mut list: crate::util::ArrayVec<CardIdx, 3> = crate::util::ArrayVec::new();
        for ty in [CardType::Attack, CardType::Skill, CardType::Power] {
            let got = cx.get_distinct_for_combat(pool, 1, |d| d.ctype == ty);
            for &c in got.iter() {
                list.push(c);
            }
        }
        for &c in list.iter() {
            cx.set_to_free_this_turn(c);
        }
        cx.add_generated_cards(list.as_slice(), PileType::Hand, CardPilePosition::Bottom);
        Flow::Done
    }
});

// `Shiv.CreateInHand(n)` then upgrade each.
listener!(CunningPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Cards);
        create_in_hand(cx, ids::card::SHIV, n, true);
        Flow::Done
    }
});

// `Soul.CreateInHand(n)`.
listener!(PotOfGhouls {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Cards);
        create_in_hand(cx, ids::card::SOUL, n, false);
        Flow::Done
    }
});

fn create_in_hand(cx: &mut Combat, id: u16, n: i32, upgrade: bool) {
    if n <= 0 || cx.is_over_or_ending() {
        return;
    }
    let mut cards: crate::util::ArrayVec<CardIdx, 16> = crate::util::ArrayVec::new();
    for _ in 0..n {
        if let Some(c) = cx.new_card(id, 0) {
            cards.push(c);
        }
    }
    cx.add_generated_cards(cards.as_slice(), PileType::Hand, CardPilePosition::Bottom);
    if upgrade {
        for &c in cards.iter() {
            if !cx.is_ending() {
                cx.upgrade_in_combat(c);
            }
        }
    }
}

// Hand -> top of draw pile, shuffle, draw 5.
listener!(BottledPotential {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let hand = cx.player.hand;
        cx.add_cards_to_pile(hand.as_slice(), PileType::Draw, CardPilePosition::Bottom);
        cx.shuffle_discard_into_draw();
        let n = cx.potion_var(potion, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

// Choose 1 card from the draw pile (shown sorted) and put it into the hand.
listener!(DropletOfPrecognition {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, phase: u8) -> Flow {
        match phase {
            0 => match cx.ask_pile(purpose(potion), PileType::Draw, 1, 1, |_, _| true) {
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

// Choose 1 card from the discard pile; it costs 0 this turn and returns to the hand.
listener!(LiquidMemories {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, phase: u8) -> Flow {
        match phase {
            0 => match cx.ask_pile(purpose(potion), PileType::Discard, 1, 1, |_, _| true) {
                Ask::Resolved(cards) => {
                    if let Some(c) = cards.first() {
                        cx.set_to_free_this_turn(c);
                        cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                    }
                    Flow::Done
                }
                Ask::Pending => Flow::Suspend(1),
            },
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.set_to_free_this_turn(c);
                    cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
                }
                Flow::Done
            }
        }
    }
});

// Upgrade every upgradable card in the hand.
listener!(BlessingOfTheForge {
    fn on_use_potion(&self, cx: &mut Combat, _potion: u16, _target: Cid, _phase: u8) -> Flow {
        if cx.is_ending() {
            return Flow::Done;
        }
        let hand = cx.player.hand;
        for &c in hand.iter() {
            if cx.is_upgradable(c) {
                cx.upgrade_in_combat(c);
            }
        }
        Flow::Done
    }
});

// Choose any number of hand cards to exhaust (min 0, max unbounded; manual confirm).
listener!(Ashwater {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, phase: u8) -> Flow {
        match phase {
            0 => match cx.ask_hand(purpose(potion), 0, u8::MAX, |_, _| true) {
                Ask::Resolved(cards) => {
                    exhaust_all(cx, cards.as_slice());
                    Flow::Done
                }
                Ask::Pending => Flow::Suspend(1),
            },
            _ => {
                let cards = cx.choice.cards;
                exhaust_all(cx, cards.as_slice());
                Flow::Done
            }
        }
    }
});

fn exhaust_all(cx: &mut Combat, cards: &[CardIdx]) {
    for &c in cards {
        cx.exhaust_card(c, false);
    }
}

// `CardCmd.DiscardAndDraw`; a Sly card whose auto-play asks for a decision suspends the potion until it is answered.
fn brew(cx: &mut Combat, cards: &[CardIdx]) -> Flow {
    match cx.discard_cards(cards, cards.len() as i32) {
        RunResult::Suspended => Flow::Suspend(2),
        RunResult::Finished => Flow::Done,
    }
}

// Choose any number of hand cards to discard, then draw that many.
listener!(GamblersBrew {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, phase: u8) -> Flow {
        match phase {
            0 => match cx.ask_hand(purpose(potion), 0, u8::MAX, |_, _| true) {
                Ask::Resolved(cards) => brew(cx, cards.as_slice()),
                Ask::Pending => Flow::Suspend(1),
            },
            1 => {
                let cards = cx.choice.cards;
                brew(cx, cards.as_slice())
            }
            _ => Flow::Done,
        }
    }
});

// Choose a card costing energy/stars in the hand; it is free for the rest of the combat.
listener!(TouchOfInsanity {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, phase: u8) -> Flow {
        let costs = |cx: &Combat, c: CardIdx| {
            let d = cx.card_def(c);
            (d.star_cost > 0) || (!d.x_cost && (cx.card_cost(c, false) > 0 || cx.card_cost(c, true) > 0))
        };
        match phase {
            0 => match cx.ask_hand(purpose(potion), 1, 1, costs) {
                Ask::Resolved(cards) => {
                    if let Some(c) = cards.first() {
                        cx.set_to_free_this_combat(c);
                    }
                    Flow::Done
                }
                Ask::Pending => Flow::Suspend(1),
            },
            _ => {
                if let Some(c) = cx.choice.cards.first() {
                    cx.set_to_free_this_combat(c);
                }
                Flow::Done
            }
        }
    }
});

// Exhaust the whole hand (in hand order), then draw 10.
listener!(GlowwaterPotion {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let hand = cx.player.hand;
        exhaust_all(cx, hand.as_slice());
        let n = cx.potion_var(potion, VarKind::Cards);
        cx.draw_cards(n, false);
        Flow::Done
    }
});

// Draw 7, then every non-X, costed card in the hand gets a random cost 0..=3 until played / end of turn.
listener!(SneckoOil {
    fn on_use_potion(&self, cx: &mut Combat, potion: u16, _target: Cid, _phase: u8) -> Flow {
        let n = cx.potion_var(potion, VarKind::Cards);
        cx.draw_cards(n, false);
        let hand = cx.player.hand;
        for &c in hand.iter() {
            if cx.card_def(c).x_cost {
                continue;
            }
            if cx.card_cost(c, false) >= 0 {
                let cost = cx.rng.combat_energy_costs.next_int(4);
                cx.cards[c as usize].mods.push(CostMod::new(cost as i8, false, false, EXPIRE_END_OF_TURN | EXPIRE_WHEN_PLAYED));
            }
        }
        Flow::Done
    }
});

// Every Strike in the combat piles gets +1 replay.
listener!(SoldiersStew {
    fn on_use_potion(&self, cx: &mut Combat, _potion: u16, _target: Cid, _phase: u8) -> Flow {
        let pl = cx.player.hand.iter().chain(cx.player.draw.iter()).chain(cx.player.discard.iter()).chain(cx.player.exhaust.iter()).chain(cx.player.play.iter()).copied();
        let all: crate::util::ArrayVec<CardIdx, MAX_CARDS> = {
            let mut v = crate::util::ArrayVec::new();
            for c in pl {
                v.push(c);
            }
            v
        };
        for &c in all.iter() {
            if cx.card_def(c).tags & tag::STRIKE != 0 {
                let r = &mut cx.cards[c as usize].base_replay;
                *r = r.saturating_add(1);
            }
        }
        Flow::Done
    }
});
