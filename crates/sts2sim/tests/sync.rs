//! `sync_hand` / `sync_pile`: aligning the simulated visible state with an observation of the real game.
use sts2sim::engine::{ObsCard, ObsEnemy};
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

#[test]
fn sync_powers_sets_amounts_drops_and_applies() {
    let mut cx = combat(11);
    let e = cx.enemies[0];
    cx.apply_power(ids::power::STRENGTH_POWER, e, sts2sim::dec::Dec::int(2), e, NO);
    cx.apply_power(ids::power::VULNERABLE_POWER, e, sts2sim::dec::Dec::int(3), PLAYER, NO);
    // the real enemy: Strength 5 (not 2), no Vulnerable, Weak 1 (the simulator has none)
    let n = cx.sync_powers(e, &[(ids::power::STRENGTH_POWER, 5), (ids::power::WEAK_POWER, 1)]);
    assert_eq!(n, 3);
    assert_eq!(cx.power_amount(e, ids::power::STRENGTH_POWER), 5);
    assert!(!cx.has_power(e, ids::power::VULNERABLE_POWER));
    assert_eq!(cx.power_amount(e, ids::power::WEAK_POWER), 1);
    // already equal: nothing changes
    assert_eq!(cx.sync_powers(e, &[(ids::power::STRENGTH_POWER, 5), (ids::power::WEAK_POWER, 1)]), 0);
}

fn slimes(seed: u64) -> Combat {
    let deck: Vec<DeckCard> = (0..10).map(|_| DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 }).collect();
    Combat::new(&Scenario {
        run_seed: 0,
        total_floor: 3,
        character: 0,
        ascension: 10,
        encounter: ids::encounter::SLIMES_NORMAL,
        max_hp: 80,
        hp: 80,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 3,
        deck,
        relics: vec![],
        potions: vec![],
        rng: RngSet::from_run_seed(seed),
    })
}

fn obs_of(cx: &Combat, c: Cid) -> ObsEnemy {
    let cr = cx.cr(c);
    ObsEnemy { monster: cr.monster.id, hp: cr.hp, max_hp: cr.max_hp, block: cr.block, alive: cr.is_alive() }
}

#[test]
fn sync_enemies_repairs_a_random_target_that_killed_another_enemy() {
    // the playtest: a random hit (Lightning) killed enemy A in the simulator and enemy B in the game; the game then lists A (alive) where the
    // simulator's list starts with B, and a positional sync wrote A's numbers onto B (`.enemies[0].id LEAF_SLIME_S vs TWIG_SLIME_M`)
    let mut tested = 0;
    for seed in 0..20u64 {
        let mut cx = slimes(seed);
        let before: Vec<Cid> = cx.enemies.iter().copied().collect();
        if before.len() < 2 {
            continue;
        }
        tested += 1;
        let (a, b) = (before[0], before[1]);
        let mut real: Vec<ObsEnemy> = before.iter().filter(|&&c| c != b).map(|&c| obs_of(&cx, c)).collect();
        real[0].hp -= 1; // the game's A took a scratch
        cx.kill(&[a]); // the simulator's sample killed A instead
        assert!(!cx.enemies.contains(a));
        let rep = cx.sync_enemies(&real);
        let after: Vec<Cid> = cx.enemies.iter().copied().collect();
        let want: Vec<Cid> = before.iter().copied().filter(|&c| c != b).collect();
        assert_eq!(after, want, "seed {seed}: the list follows the game's order and identities");
        assert_eq!(rep.pairs, want.iter().map(|&c| Some(c)).collect::<Vec<_>>());
        assert!(cx.cr(a).in_combat && cx.cr(a).hp == real[0].hp, "seed {seed}: A is back with the game's HP");
        assert!(!cx.cr(b).in_combat, "seed {seed}: B left the fight");
        assert_eq!((rep.revived, rep.removed, rep.missing), (1, 1, 0));
        // a second sync with the same observation changes nothing
        let rep2 = cx.sync_enemies(&real);
        assert_eq!((rep2.revived, rep2.removed, rep2.missing), (0, 0, 0));
        assert_eq!(cx.enemies.iter().copied().collect::<Vec<_>>(), want);
    }
    assert!(tested > 0, "no seed spawned two slimes");
}

#[test]
fn sync_enemies_pairs_by_identity_and_follows_the_game_order() {
    for seed in 0..20u64 {
        let mut cx = slimes(seed);
        let before: Vec<Cid> = cx.enemies.iter().copied().collect();
        if before.len() < 2 {
            continue;
        }
        let mut real: Vec<ObsEnemy> = before.iter().rev().map(|&c| obs_of(&cx, c)).collect(); // the game lists them in the other order
        real[0].block = 5;
        cx.sync_enemies(&real);
        let after: Vec<Cid> = cx.enemies.iter().copied().collect();
        assert_eq!(after, before.iter().rev().copied().collect::<Vec<_>>(), "seed {seed}");
        assert_eq!(cx.cr(after[0]).block, 5);
    }
}

#[test]
fn sync_options_takes_the_games_offer_of_generated_cards() {
    let deck: Vec<DeckCard> = (0..10).map(|_| DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 }).collect();
    let mut cx = Combat::new(&Scenario {
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
        potions: vec![ids::potion::POWER_POTION],
        rng: RngSet::from_run_seed(7),
    });
    assert!(!cx.sync_options(&[(ids::card::INFLAME, 0)]), "no prompt pending");
    assert!(cx.step(Action::UsePotion { slot: 0, target: NO }));
    assert_eq!(cx.stage, Stage::AwaitChoice);
    let want = [(ids::card::INFLAME, 0u8), (ids::card::DEMON_FORM, 1), (ids::card::BARRICADE, 0)];
    assert!(cx.sync_options(&want));
    let d = cx.decision.unwrap();
    let got: Vec<(u16, u8)> = d.cands.iter().map(|&c| (cx.cards[c as usize].id, cx.cards[c as usize].upgrade)).collect();
    assert_eq!(got, want.to_vec());
    let hand = cx.player.hand.len();
    let view = cx.decision_view(&d);
    let k = view.iter().position(|&g| g == 1).unwrap() as u8; // the display slot of Demon Form+
    assert!(cx.step(Action::Pick { idx: k }));
    if cx.stage == Stage::AwaitChoice {
        assert!(cx.step(Action::Confirm));
    }
    assert_eq!(cx.player.hand.len(), hand + 1);
    let last = *cx.player.hand.iter().last().unwrap();
    assert_eq!((cx.cards[last as usize].id, cx.cards[last as usize].upgrade), (ids::card::DEMON_FORM, 1));
}
