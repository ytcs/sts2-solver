//! Behaviour of ported Silent cards, second half of the pool (hand-constructed situations).
use sts2sim::dec::Dec;
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn base() -> Combat {
    let deck: Vec<DeckCard> = (0..10).map(|_| DeckCard { id: ids::card::STRIKE_SILENT, upgrade: 0 }).collect();
    Combat::new(&Scenario {
        run_seed: 0,
        total_floor: 1,
        character: 1,
        ascension: 0,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: 70,
        hp: 70,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 3,
        deck,
        relics: vec![],
        potions: vec![],
        rng: RngSet::from_run_seed(42),
    })
}

/// Empties the hand (to discard) and puts the given cards in it, returning their arena indices.
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
fn phantom_blades_boosts_only_the_first_shiv_each_turn() {
    let mut cx = base();
    let e = cx.enemies[0];
    cx.apply_power(ids::power::PHANTOM_BLADES_POWER, PLAYER, Dec::int(9), PLAYER, NO);
    let shivs = cx.create_shivs_in_hand(2);
    // Shivs created while the power is active are retained.
    assert!(shivs.iter().all(|&s| cx.card_keywords(s) & kw::RETAIN != 0));
    let hand = cx.player.hand;
    let pos: Vec<usize> = shivs.iter().map(|s| hand.position(*s).unwrap()).collect();
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: pos[0] as u8, target: e }));
    assert_eq!(cx.cr(e).hp, hp - (4 + 9));
    let p2 = cx.player.hand.position(shivs[1]).unwrap();
    assert!(cx.step(Action::PlayCard { hand_pos: p2 as u8, target: e }));
    assert_eq!(cx.cr(e).hp, hp - (4 + 9) - 4);
}

#[test]
fn memento_mori_counts_cards_discarded_this_turn() {
    let mut cx = base();
    let e = cx.enemies[0];
    let h = set_hand(&mut cx, &[(ids::card::MEMENTO_MORI, 0), (ids::card::STRIKE_SILENT, 0), (ids::card::STRIKE_SILENT, 0)]);
    cx.discard_cards(&[h[1], h[2]], 0);
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.cr(e).hp, hp - (9 + 4 * 2));
}

#[test]
fn murder_counts_every_card_drawn_this_combat() {
    let mut cx = base();
    let e = cx.enemies[0];
    let drawn = cx.hist.drawn_combat; // the opening hand
    assert_eq!(drawn, 5);
    set_hand(&mut cx, &[(ids::card::MURDER, 0)]);
    cx.draw_cards(2, false);
    let n = cx.hist.drawn_combat;
    assert_eq!(n, 7);
    let hp = cx.cr(e).hp;
    // Murder costs 3 and is the first card of the (re-built) hand.
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.cr(e).hp, hp - (1 + n));
}

#[test]
fn shadowmeld_doubles_block_per_stack() {
    let mut cx = base();
    cx.apply_power(ids::power::SHADOWMELD_POWER, PLAYER, Dec::int(2), PLAYER, NO);
    set_hand(&mut cx, &[(ids::card::DEFEND_SILENT, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.cr(PLAYER).block, 5 * 4);
}

#[test]
fn tools_of_the_trade_draws_extra_then_asks_for_a_discard() {
    let mut cx = base();
    cx.apply_power(ids::power::TOOLS_OF_THE_TRADE_POWER, PLAYER, Dec::ONE, PLAYER, NO);
    assert!(cx.step(Action::EndTurn));
    // Next turn: 5 + 1 cards drawn, then one must be discarded (the choice is pending).
    assert_eq!(cx.stage, Stage::AwaitChoice);
    assert_eq!(cx.player.hand.len(), 6);
    assert!(cx.step(Action::Pick { idx: 0 }));
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert_eq!(cx.player.hand.len(), 5);
    assert_eq!(cx.player.discard.len(), 1); // the reshuffle emptied the discard; only the chosen card is in it
}

#[test]
fn wraith_form_grants_intangible_and_drains_dexterity_each_turn() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::WRAITH_FORM, 0)]);
    cx.player.energy = 3;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.power_amount(PLAYER, ids::power::INTANGIBLE_POWER), 2);
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.power_amount(PLAYER, ids::power::DEXTERITY_POWER), -1);
}
