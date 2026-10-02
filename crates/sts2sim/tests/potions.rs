//! Potion rules that the differential sweeps do not pin down by themselves (hand-constructed situations).
use sts2sim::dec::Dec;
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn with_potions(potions: &[u16]) -> Combat {
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
        potions: potions.to_vec(),
        rng: RngSet::from_run_seed(7),
    })
}

fn strength(cx: &Combat) -> i32 {
    cx.power_amount(PLAYER, ids::power::STRENGTH_POWER)
}

#[test]
fn flex_potion_is_temporary_and_stacks() {
    let mut cx = with_potions(&[ids::potion::FLEX_POTION, ids::potion::FLEX_POTION]);
    assert!(cx.step(Action::UsePotion { slot: 0, target: NO }));
    assert_eq!(strength(&cx), 5);
    // second potion: the temporary power's amount grows and the inner Strength follows (delta != amount path)
    assert!(cx.step(Action::UsePotion { slot: 1, target: NO }));
    assert_eq!(strength(&cx), 10);
    assert_eq!(cx.power_amount(PLAYER, ids::power::FLEX_POTION_POWER), 10);
    assert!(cx.step(Action::EndTurn));
    assert_eq!(strength(&cx), 0, "Strength is taken back at the end of the player's turn");
    assert_eq!(cx.power_amount(PLAYER, ids::power::FLEX_POTION_POWER), 0);
}

#[test]
fn speed_potion_gives_and_takes_dexterity() {
    let mut cx = with_potions(&[ids::potion::SPEED_POTION]);
    assert!(cx.step(Action::UsePotion { slot: 0, target: NO }));
    assert_eq!(cx.power_amount(PLAYER, ids::power::DEXTERITY_POWER), 5);
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.power_amount(PLAYER, ids::power::DEXTERITY_POWER), 0);
}

#[test]
fn shackling_potion_is_negative_strength_until_the_enemy_turn_ends() {
    let mut cx = with_potions(&[ids::potion::SHACKLING_POTION]);
    let e = cx.enemies[0];
    assert!(cx.step(Action::UsePotion { slot: 0, target: NO }));
    assert_eq!(cx.power_amount(e, ids::power::STRENGTH_POWER), -7);
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.power_amount(e, ids::power::STRENGTH_POWER), 0);
}

#[test]
fn fairy_in_a_bottle_prevents_death_once() {
    let mut cx = with_potions(&[ids::potion::FAIRY_IN_A_BOTTLE]);
    cx.cr_mut(PLAYER).hp = 5;
    // Automatic: never a legal manual action.
    assert!(!cx.step(Action::UsePotion { slot: 0, target: NO }));
    cx.damage(&[PLAYER], Dec::int(50), ValueProp::UNPOWERED, NO, NO);
    assert_eq!(cx.cr(PLAYER).hp, 24, "heals max(30% of 80, 1)");
    assert!(cx.player.potions[0].is_none(), "the potion is consumed");
    assert_ne!(cx.outcome, Outcome::Defeat);
    // without the potion the next lethal hit kills
    cx.damage(&[PLAYER], Dec::int(50), ValueProp::UNPOWERED, NO, NO);
    assert!(cx.cr(PLAYER).is_dead());
}

#[test]
fn blood_potion_heals_a_fraction_of_max_hp() {
    let mut cx = with_potions(&[ids::potion::BLOOD_POTION]);
    cx.cr_mut(PLAYER).hp = 10;
    assert!(cx.step(Action::UsePotion { slot: 0, target: NO }));
    assert_eq!(cx.cr(PLAYER).hp, 26); // 20% of 80
}

#[test]
fn fortifier_doubles_block() {
    let mut cx = with_potions(&[ids::potion::FORTIFIER]);
    cx.cr_mut(PLAYER).block = 7;
    assert!(cx.step(Action::UsePotion { slot: 0, target: NO }));
    assert_eq!(cx.cr(PLAYER).block, 21);
}

#[test]
fn random_in_combat_potions_exclude_restricted_ones_but_entropic_brew_does_not() {
    let mut cx = with_potions(&[]);
    let mut seen_out = std::collections::HashSet::new();
    for _ in 0..4000 {
        let p = cx.create_random_potion(true).unwrap();
        assert!(sts2sim::content::potion_def(p).can_be_generated_in_combat);
        assert_ne!(p, ids::potion::FAIRY_IN_A_BOTTLE);
        let q = cx.create_random_potion(false).unwrap();
        seen_out.insert(q);
    }
    assert!(seen_out.contains(&ids::potion::FAIRY_IN_A_BOTTLE) || seen_out.contains(&ids::potion::FRUIT_JUICE));
}

#[test]
fn entropic_brew_fills_the_belt_from_the_potion_generation_stream() {
    let mut cx = with_potions(&[ids::potion::ENTROPIC_BREW]);
    let before = cx.rng.combat_potion_generation.state();
    assert!(cx.step(Action::UsePotion { slot: 0, target: NO }));
    assert_ne!(cx.rng.combat_potion_generation.state(), before);
    // both slots were free once the brew left its slot: two potions generated (2 draws each)
    assert!(cx.player.potions[0].is_some() && cx.player.potions[1].is_some());
    assert_eq!(cx.rng.combat_potion_generation.counter, 4);
}

#[test]
fn duplicator_plays_the_next_card_twice() {
    let mut cx = with_potions(&[ids::potion::DUPLICATOR]);
    let e = cx.enemies[0];
    assert!(cx.step(Action::UsePotion { slot: 0, target: NO }));
    // find a Strike in hand
    let pos = cx.player.hand.iter().position(|&c| cx.cards[c as usize].id == ids::card::STRIKE_IRONCLAD).unwrap();
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: pos as u8, target: e }));
    assert_eq!(cx.cr(e).hp, hp - 2 * 6, "Strike deals 6 twice (no block on turn 1)");
    assert_eq!(cx.power_amount(PLAYER, ids::power::DUPLICATION_POWER), 0);
}
