//! Rules of the Ironclad cards Infernal Blade .. Whirlwind that are easy to get wrong (hand-constructed situations).
//! The broad validation is the differential sweep against the real game (`tools/diff_sweep.py`).
use sts2sim::dec::Dec;
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn base() -> Combat {
    let deck: Vec<DeckCard> = (0..10).map(|_| DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 }).collect();
    Combat::new(&Scenario {
        run_seed: 0,
        total_floor: 1,
        character: 0,
        ascension: 10,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: 80,
        hp: 80,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 2,
        deck,
        relics: vec![],
        potions: vec![],
        rng: RngSet::from_run_seed(7),
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
fn stomp_gets_cheaper_with_every_attack_this_turn() {
    let mut cx = base();
    let h = set_hand(&mut cx, &[(ids::card::STRIKE_IRONCLAD, 0), (ids::card::STRIKE_IRONCLAD, 0), (ids::card::STOMP, 0)]);
    assert_eq!(cx.card_cost(h[2], true), 3);
    let e = cx.enemies[0];
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.card_cost(h[2], true), 2);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.card_cost(h[2], true), 1);
}

#[test]
fn rampage_grows_permanently() {
    let mut cx = base();
    let h = set_hand(&mut cx, &[(ids::card::RAMPAGE, 0)]);
    let e = cx.enemies[0];
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.cr(e).hp, hp - 10);
    assert_eq!(cx.card_var(h[0], sts2sim::defs::VarKind::Damage), 15);
}

#[test]
fn one_two_punch_replays_the_next_attack_once() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::ONE_TWO_PUNCH, 0), (ids::card::STRIKE_IRONCLAD, 0), (ids::card::STRIKE_IRONCLAD, 0)]);
    let e = cx.enemies[0];
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.cr(e).hp, hp - 12); // 6 twice
    assert!(!cx.has_power(PLAYER, ids::power::ONE_TWO_PUNCH_POWER));
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.cr(e).hp, hp - 18);
}

#[test]
fn unmovable_doubles_only_the_first_card_block() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::UNMOVABLE, 0), (ids::card::DEFEND_IRONCLAD, 0), (ids::card::DEFEND_IRONCLAD, 0)]);
    cx.player.energy = 10;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.cr(PLAYER).block, 10);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.cr(PLAYER).block, 15);
}

#[test]
fn second_wind_exhausts_non_attacks_for_block_each() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::SECOND_WIND, 0), (ids::card::DEFEND_IRONCLAD, 0), (ids::card::STRIKE_IRONCLAD, 0), (ids::card::DEFEND_IRONCLAD, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.cr(PLAYER).block, 10);
    assert_eq!(cx.player.exhaust.len(), 2); // the two Defends (Second Wind itself is discarded)
}

#[test]
fn primal_force_turns_attacks_into_giant_rocks() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::PRIMAL_FORCE, 1), (ids::card::STRIKE_IRONCLAD, 0), (ids::card::DEFEND_IRONCLAD, 0), (ids::card::BASH, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    let hand: Vec<(u16, u8)> = cx.player.hand.iter().map(|&c| (cx.cards[c as usize].id, cx.cards[c as usize].upgrade)).collect();
    assert_eq!(hand, vec![(ids::card::GIANT_ROCK, 1), (ids::card::DEFEND_IRONCLAD, 0), (ids::card::GIANT_ROCK, 1)]);
}

#[test]
fn thrash_absorbs_the_exhausted_attacks_modified_damage() {
    let mut cx = base();
    let h = set_hand(&mut cx, &[(ids::card::THRASH, 0), (ids::card::STRIKE_IRONCLAD, 0)]);
    cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(2), PLAYER, NO);
    let e = cx.enemies[0];
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    // the Strike (6 + 2 Strength) is exhausted and added to Thrash's damage
    assert_eq!(cx.card_var(h[0], sts2sim::defs::VarKind::Damage), 4 + 8);
    assert_eq!(cx.player.exhaust.len(), 1);
}
