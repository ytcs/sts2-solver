//! Behaviour of the Ironclad cards at pool positions [0,45) that are not covered by the differential sweeps in a
//! hand-constructed way: auto-play (nested decisions), Fatal/max HP, calculated damage.
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

/// Puts the cards on top of the draw pile (first = top).
fn set_draw_top(cx: &mut Combat, cards: &[(u16, u8)]) -> Vec<u8> {
    let mut out = vec![];
    // inserted at the top one after another: insert the last listed first so that the first listed ends up on top
    for &(id, up) in cards.iter().rev() {
        let c = cx.new_card(id, up).unwrap();
        cx.move_card(c, PileType::Draw, CardPilePosition::Top);
        out.push(c);
    }
    out.reverse();
    out
}

fn ids_of(cx: &Combat, pile: PileType) -> Vec<u16> {
    cx.pile(pile).iter().map(|&c| cx.cards[c as usize].id).collect()
}

#[test]
fn havoc_autoplays_top_card_and_exhausts_it() {
    let mut cx = base();
    let e = cx.enemies[0];
    set_hand(&mut cx, &[(ids::card::HAVOC, 0)]);
    set_draw_top(&mut cx, &[(ids::card::STRIKE_IRONCLAD, 0)]);
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.cr(e).hp, hp - 6);
    assert!(ids_of(&cx, PileType::Exhaust).contains(&ids::card::STRIKE_IRONCLAD));
    assert!(ids_of(&cx, PileType::Discard).contains(&ids::card::HAVOC));
    assert!(cx.player.play.is_empty());
    assert_eq!(cx.stage, Stage::AwaitAction);
}

#[test]
fn havoc_on_unplayable_card_exhausts_without_playing() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::HAVOC, 0)]);
    set_draw_top(&mut cx, &[(ids::card::ASCENDERS_BANE, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(ids_of(&cx, PileType::Exhaust), vec![ids::card::ASCENDERS_BANE]);
}

#[test]
fn havoc_nested_decision_resumes_outer_card() {
    let mut cx = base();
    // Havoc plays Armaments from the draw pile: the upgrade choice is raised from inside the nested play.
    set_hand(&mut cx, &[(ids::card::HAVOC, 0), (ids::card::STRIKE_IRONCLAD, 0), (ids::card::DEFEND_IRONCLAD, 0)]);
    set_draw_top(&mut cx, &[(ids::card::ARMAMENTS, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.stage, Stage::AwaitChoice);
    assert!(cx.decision.is_some());
    // pick the Defend (hand is [Strike, Defend] now)
    assert!(cx.step(Action::Pick { idx: 1 }));
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert!(cx.player.play.is_empty());
    assert_eq!(cx.cr(PLAYER).block, 5);
    let defend = cx.player.hand[1];
    assert_eq!(cx.cards[defend as usize].upgrade, 1);
    assert!(ids_of(&cx, PileType::Exhaust).contains(&ids::card::ARMAMENTS));
    assert!(ids_of(&cx, PileType::Discard).contains(&ids::card::HAVOC));
    assert!(cx.missing.is_none());
}

#[test]
fn cascade_plays_x_cards_with_nested_decision_in_the_middle() {
    let mut cx = base();
    let e = cx.enemies[0];
    set_hand(&mut cx, &[(ids::card::CASCADE, 0), (ids::card::STRIKE_IRONCLAD, 0), (ids::card::DEFEND_IRONCLAD, 0)]);
    set_draw_top(&mut cx, &[(ids::card::STRIKE_IRONCLAD, 0), (ids::card::ARMAMENTS, 0), (ids::card::STRIKE_IRONCLAD, 0)]);
    let hp = cx.cr(e).hp;
    // X = 3 energy: three cards are pulled into the Play pile, then played in order.
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.stage, Stage::AwaitChoice);
    assert_eq!(cx.cr(e).hp, hp - 6); // the first Strike already hit
    assert!(cx.step(Action::Pick { idx: 0 }));
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert_eq!(cx.cr(e).hp, hp - 12); // the third card (Strike) resumed after the decision
    assert!(cx.player.play.is_empty());
    assert_eq!(cx.player.energy, 0);
    // un-forced exhaust: the auto-played cards go to the discard pile
    assert!(ids_of(&cx, PileType::Exhaust).is_empty());
}

#[test]
fn feed_gains_max_hp_only_on_kill() {
    let mut cx = base();
    let e = cx.enemies[0];
    set_hand(&mut cx, &[(ids::card::FEED, 0), (ids::card::FEED, 0)]);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.cr(PLAYER).max_hp, 80);
    cx.cr_mut(e).hp = 5;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    // the Nibbits fight may continue (two enemies) or be won; either way max HP rose by 3 on the kill
    assert!(cx.cr(PLAYER).max_hp >= 83);
}

#[test]
fn body_slam_uses_block() {
    let mut cx = base();
    let e = cx.enemies[0];
    set_hand(&mut cx, &[(ids::card::BODY_SLAM, 0)]);
    cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(2), PLAYER, NO);
    cx.cr_mut(PLAYER).block = 7;
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.cr(e).hp, hp - (7 + 2));
}

#[test]
fn nested_cascades_finish_inner_first() {
    let mut cx = base();
    // Corruption: Skills exhaust. Outer Cascade (X = 1) auto-plays Cascade+ (X = 0 + 1) which auto-plays Armaments;
    // the Armaments decision is raised two plays deep. Result piles are decided innermost-first.
    cx.apply_power(ids::power::CORRUPTION_POWER, PLAYER, Dec::int(1), PLAYER, NO);
    set_hand(&mut cx, &[(ids::card::CASCADE, 0), (ids::card::STRIKE_IRONCLAD, 0), (ids::card::DEFEND_IRONCLAD, 0)]);
    set_draw_top(&mut cx, &[(ids::card::CASCADE, 1), (ids::card::ARMAMENTS, 0)]);
    cx.player.energy = 1;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.stage, Stage::AwaitChoice);
    assert!(cx.step(Action::Pick { idx: 0 }));
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert!(cx.player.play.is_empty());
    assert!(cx.play_stack.is_empty() && cx.play_ctx.is_none());
    assert_eq!(ids_of(&cx, PileType::Exhaust), vec![ids::card::ARMAMENTS, ids::card::CASCADE, ids::card::CASCADE]);
    let ups: Vec<u8> = cx.pile(PileType::Exhaust).iter().map(|&c| cx.cards[c as usize].upgrade).collect();
    assert_eq!(ups, vec![0, 1, 0]);
}
