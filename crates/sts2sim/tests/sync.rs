//! `sync_hand` / `sync_pile`: aligning the simulated visible state with an observation of the real game.
use sts2sim::engine::ObsCard;
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn combat(seed: u64) -> Combat {
    let mut deck = vec![];
    for _ in 0..5 {
        deck.push(DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 });
    }
    for _ in 0..4 {
        deck.push(DeckCard { id: ids::card::DEFEND_IRONCLAD, upgrade: 0 });
    }
    for id in [ids::card::BASH, ids::card::SHRUG_IT_OFF, ids::card::POMMEL_STRIKE, ids::card::TWIN_STRIKE] {
        deck.push(DeckCard { id, upgrade: 0 });
    }
    let sc = Scenario {
        run_seed: 0,
        total_floor: 1,
        character: 0,
        ascension: 0,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: 80,
        hp: 80,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 3,
        deck,
        relics: vec![RelicInit { id: ids::relic::BURNING_BLOOD, ..Default::default() }],
        potions: vec![],
        rng: RngSet::from_run_seed(seed),
    };
    Combat::new(&sc)
}

fn names(cx: &Combat, p: &Pile) -> Vec<(u16, u8)> {
    p.iter().map(|&c| (cx.cards[c as usize].id, cx.cards[c as usize].upgrade)).collect()
}

fn total(cx: &Combat) -> usize {
    cx.player.hand.len() + cx.player.draw.len() + cx.player.discard.len() + cx.player.exhaust.len()
}

#[test]
fn sync_hand_takes_the_observed_hand_in_order_and_conserves_cards() {
    for seed in 0..20u64 {
        let mut cx = combat(seed);
        let before = total(&cx);
        // the "real" hand: five cards drawn from the same deck by a different shuffle
        let mut other = combat(seed + 1000);
        let real: Vec<ObsCard> = names(&other, &other.player.hand.clone()).into_iter().map(|(id, upgrade)| ObsCard { id, upgrade, cost: None }).collect();
        let rep = cx.sync_hand(&real);
        assert_eq!(names(&cx, &cx.player.hand), real.iter().map(|o| (o.id, o.upgrade)).collect::<Vec<_>>(), "seed {seed}: hand differs");
        assert_eq!(total(&cx), before, "seed {seed}: cards were created or lost");
        assert_eq!(rep.created, 0, "seed {seed}");
        let _ = &mut other;
    }
}

#[test]
fn sync_hand_creates_only_cards_that_exist_nowhere() {
    let mut cx = combat(3);
    let before = total(&cx);
    let mut real: Vec<ObsCard> = names(&cx, &cx.player.hand.clone()).into_iter().map(|(id, upgrade)| ObsCard { id, upgrade, cost: None }).collect();
    real.push(ObsCard { id: ids::card::ANGER, upgrade: 0, cost: None }); // a card the deck does not have
    let rep = cx.sync_hand(&real);
    assert_eq!(rep.created, 1);
    assert_eq!(total(&cx), before + 1);
    assert_eq!(cx.player.hand.len(), real.len());
}

#[test]
fn sync_pile_follows_the_observed_discard() {
    let mut cx = combat(5);
    let before = total(&cx);
    // observation: two Strikes in the discard pile (the simulator's is empty)
    let obs = vec![ObsCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0, cost: None }; 2];
    // (the hand may already hold some of them: the draw pile or hand cannot supply more than the deck has, so ask for what exists)
    let have = names(&cx, &cx.player.draw.clone()).iter().filter(|(id, _)| *id == ids::card::STRIKE_IRONCLAD).count();
    let want = obs.len().min(have);
    let rep = cx.sync_pile(PileType::Discard, &obs[..want]);
    assert_eq!(cx.player.discard.len(), want);
    assert_eq!(rep.created, 0);
    assert_eq!(total(&cx), before);
    // asking for none again returns them to the draw pile
    cx.sync_pile(PileType::Discard, &[]);
    assert_eq!(cx.player.discard.len(), 0);
    assert_eq!(total(&cx), before);
}

#[test]
fn sync_draw_drops_a_generated_card_the_real_game_does_not_have() {
    let mut cx = combat(7);
    // a card created during the fight (random generation: Stoke, Discovery ...) sits in the simulated hand
    let g = cx.new_card(ids::card::WHIRLWIND, 0).unwrap();
    cx.player.hand.push(g);
    cx.cards[g as usize].pile = PileType::Hand as u8;
    let deck_total = total(&cx) - 1;
    // the real hand holds a different generated card instead; the real draw pile is the deck's remainder
    let mut real: Vec<ObsCard> = names(&cx, &cx.player.hand.clone()).into_iter().filter(|(id, _)| *id != ids::card::WHIRLWIND).map(|(id, upgrade)| ObsCard { id, upgrade, cost: None }).collect();
    real.push(ObsCard { id: ids::card::ANGER, upgrade: 0, cost: None });
    let rep = cx.sync_hand(&real);
    assert_eq!((rep.created, rep.returned), (1, 1));
    // the phantom went back to the draw pile with the sync; the observed draw pile does not have it
    let want: Vec<ObsCard> = names(&cx, &cx.player.draw.clone()).into_iter().filter(|(id, _)| *id != ids::card::WHIRLWIND).map(|(id, upgrade)| ObsCard { id, upgrade, cost: None }).collect();
    cx.sync_draw(&want);
    assert_eq!(total(&cx), deck_total + 1, "the phantom Whirlwind must leave the combat");
    assert!(names(&cx, &cx.player.draw.clone()).iter().all(|(id, _)| *id != ids::card::WHIRLWIND));
}

#[test]
fn full_sync_sequence_conserves_cards_against_a_different_shuffle() {
    for seed in 0..30u64 {
        let mut cx = combat(seed);
        let before = total(&cx);
        let other = combat(seed + 1000);
        // the "real" state: the same deck, another shuffle, a few cards already in the discard pile
        let obs = |cx: &Combat, p: &Pile| -> Vec<ObsCard> { names(cx, p).into_iter().map(|(id, upgrade)| ObsCard { id, upgrade, cost: None }).collect() };
        let hand = obs(&other, &other.player.hand);
        let draw = obs(&other, &other.player.draw);
        cx.sync_hand(&hand);
        cx.sync_pile(PileType::Exhaust, &[]);
        cx.sync_pile(PileType::Discard, &[]);
        cx.sync_draw(&draw);
        assert_eq!(total(&cx), before, "seed {seed}: a deck card was created or deleted");
        let mut a = names(&cx, &cx.player.draw.clone());
        let mut b: Vec<(u16, u8)> = draw.iter().map(|o| (o.id, o.upgrade)).collect();
        a.sort();
        b.sort();
        assert_eq!(a, b, "seed {seed}: draw pile multiset");
    }
}
