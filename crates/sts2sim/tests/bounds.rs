//! The unwinnable-fight prover (`sts2sim::bounds`).
use sts2sim::bounds::*;
use sts2sim::ids;
use sts2sim::state::RngSet;
use sts2sim::{DeckCard, RelicInit, Scenario, ScenarioExtras};

fn ironclad(deck: &[(u16, u8)], encounter: u16, hp: i32) -> Scenario {
    Scenario {
        run_seed: 3,
        total_floor: 1,
        character: 0,
        ascension: 10,
        encounter,
        max_hp: 80,
        hp,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 2,
        deck: deck.iter().map(|&(id, upgrade)| DeckCard { id, upgrade }).collect(),
        relics: vec![RelicInit { id: ids::relic::BURNING_BLOOD, ..Default::default() }],
        potions: vec![],
        rng: RngSet::from_run_seed(3),
    }
}

fn starter() -> Vec<(u16, u8)> {
    let mut d = vec![];
    for _ in 0..5 {
        d.push((ids::card::STRIKE_IRONCLAD, 0));
    }
    for _ in 0..4 {
        d.push((ids::card::DEFEND_IRONCLAD, 0));
    }
    d.push((ids::card::BASH, 0));
    d.push((ids::card::ASCENDERS_BANE, 0));
    d
}

fn class(id: u16, up: u8) -> CardClass {
    classify_card(0, 10, 3, id, up)
}

#[test]
fn basic_cards_are_pure_with_the_numbers_on_the_card() {
    let CardClass::Pure(p) = class(ids::card::STRIKE_IRONCLAD, 0) else { panic!("Strike") };
    assert_eq!((p.cost, p.damage, p.block), (1, 6, 0));
    let CardClass::Pure(p) = class(ids::card::STRIKE_IRONCLAD, 1) else { panic!("Strike+") };
    assert_eq!(p.damage, 9);
    let CardClass::Pure(p) = class(ids::card::DEFEND_IRONCLAD, 0) else { panic!("Defend") };
    assert_eq!((p.cost, p.damage, p.block), (1, 0, 5));
    let CardClass::Pure(p) = class(ids::card::BASH, 0) else { panic!("Bash") };
    assert_eq!((p.cost, p.damage, p.vuln), (2, 8, true));
    assert!(p.damage_vuln > p.damage);
    assert_eq!(class(ids::card::ASCENDERS_BANE, 0), CardClass::Junk);
}

#[test]
fn cards_with_other_effects_are_impure() {
    assert_eq!(class(ids::card::BLOODLETTING, 0), CardClass::Impure, "gains energy");
    assert_eq!(class(ids::card::ANGER, 0), CardClass::Impure, "adds a card");
    assert_eq!(class(ids::card::DEMON_FORM, 0), CardClass::Impure, "power");
    assert_eq!(class(ids::card::INFLAME, 0), CardClass::Impure, "strength");
    assert_eq!(class(ids::card::RAMPAGE, 0), CardClass::Impure, "grows with every play");
    assert_eq!(class(ids::card::BODY_SLAM, 0), CardClass::Impure, "damage depends on block");
}

#[test]
fn the_starting_deck_cannot_beat_a_strong_boss_but_a_weak_fight_is_not_ruled_out() {
    let ex = ScenarioExtras::default();
    let boss = ironclad(&starter(), ids::encounter::THE_INSATIABLE_BOSS, 80);
    let weak = ironclad(&starter(), ids::encounter::NIBBITS_WEAK, 80);
    let proof = provably_unwinnable(&boss, &ex);
    assert!(proof.is_some(), "the starter deck against the Insatiable");
    assert!(provably_unwinnable(&weak, &ex).is_none(), "a weak fight must not be ruled out");
}

#[test]
fn anything_the_analysis_does_not_cover_is_not_proven() {
    let ex = ScenarioExtras::default();
    let mut s = ironclad(&starter(), ids::encounter::THE_INSATIABLE_BOSS, 80);
    s.potions = vec![ids::potion::FIRE_POTION];
    assert!(provably_unwinnable(&s, &ex).is_none(), "potions");
    let mut s = ironclad(&starter(), ids::encounter::THE_INSATIABLE_BOSS, 80);
    s.relics.push(RelicInit { id: ids::relic::VAJRA, ..Default::default() });
    assert!(provably_unwinnable(&s, &ex).is_none(), "an unknown relic");
    let mut d = starter();
    d.push((ids::card::DEMON_FORM, 0));
    assert!(provably_unwinnable(&ironclad(&d, ids::encounter::THE_INSATIABLE_BOSS, 80), &ex).is_none(), "an impure card");
}

/// Prints how many cards / relics the prover can vouch for (run with `--nocapture`); not an assertion about the numbers.
#[test]
fn coverage_report() {
    for (ch, name) in [(0u8, "Ironclad"), (1, "Silent")] {
        let (mut pure, mut junk, mut imp) = (0, 0, 0);
        let pool: Vec<u16> = match ch {
            0 => sts2sim::content::gen_pools::IRONCLAD.to_vec(),
            _ => sts2sim::content::gen_pools::SILENT.to_vec(),
        };
        let mut pure_names = vec![];
        for &id in pool.iter() {
            match classify_card(ch, 10, 3, id, 0) {
                CardClass::Pure(_) => {
                    pure += 1;
                    pure_names.push(ids::card::NAMES[id as usize]);
                }
                CardClass::Junk => junk += 1,
                CardClass::Impure => imp += 1,
            }
        }
        println!("{name}: {pure} pure, {junk} junk, {imp} impure of {}: {:?}", pool.len(), pure_names);
    }
    let neutral = (0..ids::relic::COUNT as u16).filter(|&r| relic_is_neutral(r)).count();
    println!("relics: {neutral} neutral of {}", ids::relic::COUNT);
}

#[test]
fn monster_hook_report() {
    use sts2sim::hooks::hookbit;
    let names = ["after_damage_received", "after_damage_given", "before_damage_received", "after_current_hp_changed", "after_power_amount_changed", "should_die", "before_death", "after_death", "after_side_turn_start", "before_side_turn_start", "after_side_turn_end", "after_block_broken", "modify_damage_additive", "modify_damage_multiplicative"];
    let bits = [hookbit::after_damage_received, hookbit::after_damage_given, hookbit::before_damage_received, hookbit::after_current_hp_changed, hookbit::after_power_amount_changed, hookbit::should_die, hookbit::before_death, hookbit::after_death, hookbit::after_side_turn_start, hookbit::before_side_turn_start, hookbit::after_side_turn_end, hookbit::after_block_broken, hookbit::modify_damage_additive, hookbit::modify_damage_multiplicative];
    let mut empty = 0;
    let mut with: Vec<String> = vec![];
    for id in 0..ids::monster::COUNT as u16 {
        let m = sts2sim::content::monster_mask(id);
        if m == sts2sim::hooks::Mask::EMPTY {
            empty += 1;
        } else {
            let hs: Vec<&str> = bits.iter().zip(names.iter()).filter(|(b, _)| m.has(**b)).map(|(_, n)| *n).collect();
            with.push(format!("{}:{:?}", ids::monster::NAMES[id as usize], hs));
        }
    }
    println!("monsters with an empty hook mask: {empty} of {}; others: {:?}", ids::monster::COUNT, with);
    let mut pw = vec![];
    for id in 0..ids::power::COUNT as u16 {
        let m = sts2sim::content::power_mask(id);
        if m.has(hookbit::after_damage_received) || m.has(hookbit::after_current_hp_changed) || m.has(hookbit::before_damage_received) {
            pw.push(ids::power::NAMES[id as usize]);
        }
    }
    println!("powers reacting to damage: {pw:?}");
}
