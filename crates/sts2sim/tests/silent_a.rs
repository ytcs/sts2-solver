//! Silent slice (first half of the pool): Sly auto-play, nested decisions, Burst, Poison, Anticipate, Afterimage, Shiv/Fan.
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
        ascension: 10,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: 70,
        hp: 70,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 2,
        deck,
        relics: vec![],
        potions: vec![],
        rng: RngSet::from_run_seed(42),
    })
}

/// Replaces the hand by the given cards (the old hand goes to the discard pile).
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

fn hand_ids(cx: &Combat) -> Vec<u16> {
    cx.player.hand.iter().map(|&c| cx.cards[c as usize].id).collect()
}

#[test]
fn survivor_discarding_a_sly_card_auto_plays_it_for_free() {
    let mut cx = base();
    let e = cx.enemies[0];
    set_hand(&mut cx, &[(ids::card::SURVIVOR, 0), (ids::card::FLICK_FLACK, 0)]);
    let (hp, energy) = (cx.cr(e).hp, cx.player.energy);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    // the only candidate is auto-selected (forced choice), discarded, then auto-played: 7 damage, no energy spent
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert_eq!(cx.cr(e).hp, hp - 7);
    assert_eq!(cx.player.energy, energy - 1);
    assert_eq!(cx.cr(PLAYER).block, 8);
}

#[test]
fn sly_auto_play_with_its_own_decision_resumes_the_outer_card() {
    let mut cx = base();
    // Hand Trick makes Acrobatics Sly this turn; Survivor then discards it: Acrobatics auto-plays (draw 3) and asks what to
    // discard while Survivor is still on the stack.
    set_hand(&mut cx, &[(ids::card::HAND_TRICK, 0), (ids::card::SURVIVOR, 0), (ids::card::ACROBATICS, 0), (ids::card::DEFEND_SILENT, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.stage, Stage::AwaitChoice);
    assert_eq!(cx.decision.as_ref().unwrap().cands.len(), 3);
    assert!(cx.step(Action::Pick { idx: 1 })); // Acrobatics
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO })); // Survivor
    assert_eq!(cx.stage, Stage::AwaitChoice);
    assert_eq!(cx.decision.as_ref().unwrap().cands.len(), 2);
    let energy = cx.player.energy;
    assert!(cx.step(Action::Pick { idx: 0 })); // discard Acrobatics -> auto-play -> its own discard prompt
    assert_eq!(cx.stage, Stage::AwaitChoice);
    assert_eq!(cx.decision.as_ref().unwrap().cands.len(), 4); // Defend + 3 drawn
    assert_eq!(cx.play_outer.len(), 1); // Survivor parked behind the nested play
    assert_eq!(cx.player.energy, energy); // auto-play is free
    assert!(cx.step(Action::Pick { idx: 0 }));
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert!(cx.play_ctx.is_none() && cx.play_outer.is_empty());
    assert_eq!(cx.cr(PLAYER).block, 7 + 8);
    assert_eq!(cx.player.hand.len(), 3);
}

#[test]
fn burst_plays_the_next_skill_twice() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::BURST, 0), (ids::card::DEFEND_SILENT, 0), (ids::card::DEFEND_SILENT, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.power_amount(PLAYER, ids::power::BURST_POWER), 1);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.cr(PLAYER).block, 10);
    assert!(!cx.has_power(PLAYER, ids::power::BURST_POWER));
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.cr(PLAYER).block, 15);
}

#[test]
fn poison_ticks_current_amount_and_accelerant_repeats_it() {
    let mut cx = base();
    let e = cx.enemies[0];
    cx.apply_power(ids::power::POISON_POWER, e, Dec::int(5), PLAYER, NO);
    cx.apply_power(ids::power::ACCELERANT_POWER, PLAYER, Dec::int(2), PLAYER, NO);
    let hp = cx.cr(e).hp;
    set_hand(&mut cx, &[]);
    assert!(cx.step(Action::EndTurn));
    // enemy turn start: min(5, 1 + 2) = 3 ticks of 5, 4, 3
    assert_eq!(cx.cr(e).hp, hp - 12);
    assert_eq!(cx.power_amount(e, ids::power::POISON_POWER), 2);
}

#[test]
fn anticipate_dexterity_is_temporary() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::ANTICIPATE, 0), (ids::card::DEFEND_SILENT, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.power_amount(PLAYER, ids::power::DEXTERITY_POWER), 2);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.cr(PLAYER).block, 7);
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.power_amount(PLAYER, ids::power::DEXTERITY_POWER), 0);
    assert!(!cx.has_power(PLAYER, ids::power::ANTICIPATE_POWER));
}

#[test]
fn afterimage_blocks_per_card_but_not_for_its_own_play() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::AFTERIMAGE, 0), (ids::card::DEFEND_SILENT, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.cr(PLAYER).block, 0);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.cr(PLAYER).block, 5 + 1);
}

#[test]
fn shiv_targets_all_enemies_with_fan_of_knives() {
    let mut cx = base();
    let c = set_hand(&mut cx, &[(ids::card::SHIV, 0)])[0];
    assert_eq!(cx.card_target_type(c), TargetType::AnyEnemy);
    cx.apply_power(ids::power::FAN_OF_KNIVES_POWER, PLAYER, Dec::ONE, PLAYER, NO);
    assert_eq!(cx.card_target_type(c), TargetType::AllEnemies);
    let e = cx.enemies[0];
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.cr(e).hp, hp - 4);
}

#[test]
fn calculated_gamble_discards_the_hand_then_draws_that_many() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::CALCULATED_GAMBLE, 0), (ids::card::DEFEND_SILENT, 0), (ids::card::DEFEND_SILENT, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.player.hand.len(), 2);
    assert_eq!(cx.stage, Stage::AwaitAction);
}
