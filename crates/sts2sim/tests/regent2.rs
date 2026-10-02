//! More Regent rules: history-driven hit counts, Black Hole, Void Form, forced selections.
use sts2sim::dec::Dec;
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn base() -> Combat {
    let deck: Vec<DeckCard> = (0..10).map(|_| DeckCard { id: ids::card::STRIKE_REGENT, upgrade: 0 }).collect();
    Combat::new(&Scenario {
        run_seed: 0,
        total_floor: 1,
        character: 4,
        ascension: 0,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: 75,
        hp: 75,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 3,
        deck,
        relics: vec![RelicInit { id: ids::relic::DIVINE_RIGHT, counter: 0 }],
        potions: vec![],
        rng: RngSet::from_run_seed(42),
    })
}

fn set_hand(cx: &mut Combat, cards: &[(u16, u8)]) -> Vec<u8> {
    let old = cx.player.hand;
    for &c in old.iter() {
        cx.move_card(c, PileType::Discard, CardPilePosition::Bottom);
    }
    cards
        .iter()
        .map(|&(id, up)| {
            let c = cx.new_card(id, up).unwrap();
            cx.move_card(c, PileType::Hand, CardPilePosition::Bottom);
            c
        })
        .collect()
}

#[test]
fn radiate_counts_stars_gained_this_turn_including_divine_right() {
    let mut cx = base();
    let e = cx.enemies[0];
    let hp = cx.cr(e).hp;
    set_hand(&mut cx, &[(ids::card::RADIATE, 0)]);
    // DivineRight's +3 happened in turn 1 (same round / side / turn): 3 hits of 3.
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.cr(e).hp, hp - 3 * 3);
}

#[test]
fn black_hole_triggers_on_every_star_gain() {
    let mut cx = base();
    let e = cx.enemies[0];
    cx.apply_power(ids::power::BLACK_HOLE_POWER, PLAYER, Dec::int(3), PLAYER, NO);
    let hp = cx.cr(e).hp;
    cx.gain_stars(2);
    assert_eq!(cx.cr(e).hp, hp - 3, "unpowered damage to every hittable enemy");
}

#[test]
fn void_form_ends_the_turn_and_makes_the_first_cards_free_next_turn() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::VOID_FORM, 1)]);
    cx.player.energy = 3;
    let round = cx.round;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert!(cx.round > round || cx.stage == Stage::Over, "Void Form ends the turn once the play has finished");
    let hand = cx.player.hand;
    for &c in hand.iter().take(2) {
        assert_eq!(cx.card_cost(c, true), 0);
    }
}

#[test]
fn forced_draw_pile_choice_keeps_pile_order() {
    let mut cx = base();
    let hand = cx.player.hand;
    for &c in hand.iter() {
        cx.move_card(c, PileType::Discard, CardPilePosition::Bottom);
    }
    let draw = cx.player.draw;
    for &c in draw.iter() {
        cx.move_card(c, PileType::Discard, CardPilePosition::Bottom);
    }
    let a = cx.new_card(ids::card::DEVASTATE, 0).unwrap(); // Uncommon
    let b = cx.new_card(ids::card::DEFEND_REGENT, 0).unwrap(); // Basic
    cx.move_card(a, PileType::Draw, CardPilePosition::Bottom);
    cx.move_card(b, PileType::Draw, CardPilePosition::Bottom);
    match cx.ask_pile(0, PileType::Draw, 2, 2, |_, _| true) {
        sts2sim::engine::Ask::Resolved(cards) => assert_eq!(cards.as_slice(), &[a, b]),
        _ => panic!("forced choice must resolve"),
    }
}
