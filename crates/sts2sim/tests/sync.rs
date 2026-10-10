use sts2sim::engine::{ActionBuf, ObsCard, ObsEnemy};
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
        let mut other = combat(seed + 1000);
        let real: Vec<ObsCard> = names(&other, &other.player.hand.clone()).into_iter().map(|(id, upgrade)| ObsCard { id, upgrade, ..Default::default() }).collect();
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
    let mut real: Vec<ObsCard> = names(&cx, &cx.player.hand.clone()).into_iter().map(|(id, upgrade)| ObsCard { id, upgrade, ..Default::default() }).collect();
    real.push(ObsCard { id: ids::card::ANGER, upgrade: 0, ..Default::default() });
    let rep = cx.sync_hand(&real);
    assert_eq!(rep.created, 1);
    assert_eq!(total(&cx), before + 1);
    assert_eq!(cx.player.hand.len(), real.len());
}

#[test]
fn sync_hand_pairs_cards_by_enchantment() {
    for seed in 0..10u64 {
        let mut cx = combat(seed);
        let defend = |cx: &Combat, p: &Pile| p.iter().copied().find(|&c| cx.cards[c as usize].id == ids::card::DEFEND_IRONCLAD);
        let spiral = defend(&cx, &cx.player.hand).or_else(|| defend(&cx, &cx.player.draw)).unwrap();
        cx.enchant_unchecked(spiral, ids::enchantment::SPIRAL, 1);
        let before = total(&cx);
        let plain = ObsCard { id: ids::card::DEFEND_IRONCLAD, upgrade: 0, ..Default::default() };
        let rep = cx.sync_hand(&[plain, ObsCard { enchant: ids::enchantment::SPIRAL as u8 + 1, enchant_amount: 1, ..plain }]);
        let hand = cx.player.hand.as_slice().to_vec();
        assert_eq!(hand.len(), 2);
        assert_eq!(cx.cards[hand[0] as usize].enchant, 0, "seed {seed}: the plain Defend took the enchanted one");
        assert_eq!(hand[1], spiral, "seed {seed}");
        assert_eq!((rep.created, total(&cx)), (0, before), "seed {seed}");
        let strike = ObsCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0, enchant: ids::enchantment::SPIRAL as u8 + 1, enchant_amount: 1, ..Default::default() };
        let rep = cx.sync_hand(&[strike]);
        let c = cx.player.hand.as_slice()[0] as usize;
        assert_eq!((rep.created, total(&cx), cx.cards[c].id, cx.cards[c].enchant), (0, before, strike.id, strike.enchant), "seed {seed}: an enchantment the simulator lacks goes on an existing copy");
    }
}

#[test]
fn sync_pile_follows_the_observed_discard() {
    let mut cx = combat(5);
    let before = total(&cx);
    let obs = vec![ObsCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0, ..Default::default() }; 2];
    let have = names(&cx, &cx.player.draw.clone()).iter().filter(|(id, _)| *id == ids::card::STRIKE_IRONCLAD).count();
    let want = obs.len().min(have);
    let rep = cx.sync_pile(PileType::Discard, &obs[..want]);
    assert_eq!(cx.player.discard.len(), want);
    assert_eq!(rep.created, 0);
    assert_eq!(total(&cx), before);
    cx.sync_pile(PileType::Discard, &[]);
    assert_eq!(cx.player.discard.len(), 0);
    assert_eq!(total(&cx), before);
}

#[test]
fn sync_draw_drops_a_generated_card_the_real_game_does_not_have() {
    let mut cx = combat(7);
    let g = cx.new_card(ids::card::WHIRLWIND, 0).unwrap();
    cx.player.hand.push(g);
    cx.cards[g as usize].pile = PileType::Hand as u8;
    let deck_total = total(&cx) - 1;
    let mut real: Vec<ObsCard> = names(&cx, &cx.player.hand.clone()).into_iter().filter(|(id, _)| *id != ids::card::WHIRLWIND).map(|(id, upgrade)| ObsCard { id, upgrade, ..Default::default() }).collect();
    real.push(ObsCard { id: ids::card::ANGER, upgrade: 0, ..Default::default() });
    let rep = cx.sync_hand(&real);
    assert_eq!((rep.created, rep.returned), (1, 1));
    let want: Vec<ObsCard> = names(&cx, &cx.player.draw.clone()).into_iter().filter(|(id, _)| *id != ids::card::WHIRLWIND).map(|(id, upgrade)| ObsCard { id, upgrade, ..Default::default() }).collect();
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
        let obs = |cx: &Combat, p: &Pile| -> Vec<ObsCard> { names(cx, p).into_iter().map(|(id, upgrade)| ObsCard { id, upgrade, ..Default::default() }).collect() };
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
    let n = cx.sync_powers(e, &[(ids::power::STRENGTH_POWER, 5), (ids::power::WEAK_POWER, 1)]);
    assert_eq!(n, 3);
    assert_eq!(cx.power_amount(e, ids::power::STRENGTH_POWER), 5);
    assert!(!cx.has_power(e, ids::power::VULNERABLE_POWER));
    assert_eq!(cx.power_amount(e, ids::power::WEAK_POWER), 1);
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
        real[0].hp -= 1;
        cx.kill(&[a]);
        assert!(!cx.enemies.contains(a));
        let rep = cx.sync_enemies(&real);
        let after: Vec<Cid> = cx.enemies.iter().copied().collect();
        let want: Vec<Cid> = before.iter().copied().filter(|&c| c != b).collect();
        assert_eq!(after, want, "seed {seed}: the list follows the game's order and identities");
        assert_eq!(rep.pairs, want.iter().map(|&c| Some(c)).collect::<Vec<_>>());
        assert!(cx.cr(a).in_combat && cx.cr(a).hp == real[0].hp, "seed {seed}: A is back with the game's HP");
        assert!(!cx.cr(b).in_combat, "seed {seed}: B left the fight");
        assert_eq!((rep.revived, rep.removed, rep.missing), (1, 1, 0));
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
        let mut real: Vec<ObsEnemy> = before.iter().rev().map(|&c| obs_of(&cx, c)).collect();
        real[0].block = 5;
        cx.sync_enemies(&real);
        let after: Vec<Cid> = cx.enemies.iter().copied().collect();
        assert_eq!(after, before.iter().rev().copied().collect::<Vec<_>>(), "seed {seed}");
        assert_eq!(cx.cr(after[0]).block, 5);
        let gone = after[0];
        cx.kill(&[gone]);
        real[0].hp = 0;
        real[0].alive = false;
        let rep = cx.sync_enemies(&real);
        assert_eq!(cx.enemies.iter().copied().collect::<Vec<_>>(), after, "seed {seed}");
        assert!(!cx.cr(gone).in_combat && cx.cr(gone).hp == 0 && rep.revived == 0, "seed {seed}");
    }
}

#[test]
fn sync_enemies_keeps_identical_monsters_in_their_slots() {
    for seed in 0..20u64 {
        let deck: Vec<DeckCard> = (0..10).map(|_| DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 }).collect();
        let mut cx = Combat::new(&Scenario {
            run_seed: 0,
            total_floor: 15,
            character: 0,
            ascension: 10,
            encounter: ids::encounter::PHANTASMAL_GARDENERS_ELITE,
            max_hp: 80,
            hp: 80,
            max_energy: 3,
            orb_slots: 0,
            potion_slots: 3,
            deck,
            relics: vec![],
            potions: vec![],
            rng: RngSet::from_run_seed(seed),
        });
        let before: Vec<Cid> = cx.enemies.iter().copied().collect();
        assert_eq!(before.iter().map(|&c| cx.cr(c).slot).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
        let mut real: Vec<ObsEnemy> = before.iter().map(|&c| obs_of(&cx, c)).collect();
        let rolled: Vec<i32> = real.iter().rev().map(|o| o.hp).collect();
        for (o, hp) in real.iter_mut().zip(rolled) {
            (o.hp, o.max_hp) = (hp, hp);
        }
        for _ in 0..2 {
            let rep = cx.sync_enemies(&real);
            assert_eq!(rep.pairs, before.iter().map(|&c| Some(c)).collect::<Vec<_>>(), "seed {seed}: the game's HP roll moved a slot");
            for (k, &c) in cx.enemies.iter().enumerate() {
                assert_eq!((cx.cr(c).slot as usize, cx.cr(c).hp), (k, real[k].hp), "seed {seed}");
            }
        }
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
    let k = view.iter().position(|&g| g == 1).unwrap() as u8;
    assert!(cx.step(Action::Pick { idx: k }));
    if cx.stage == Stage::AwaitChoice {
        assert!(cx.step(Action::Confirm));
    }
    assert_eq!(cx.player.hand.len(), hand + 1);
    let last = *cx.player.hand.iter().last().unwrap();
    assert_eq!((cx.cards[last as usize].id, cx.cards[last as usize].upgrade), (ids::card::DEMON_FORM, 1));
}

fn belt(cx: &Combat) -> Vec<(usize, u16)> {
    cx.player.potions.iter().enumerate().filter_map(|(i, p)| p.map(|p| (i, p.id))).collect()
}

fn usable(cx: &Combat) -> Vec<u8> {
    let mut buf = ActionBuf::new();
    cx.legal_actions(&mut buf);
    let mut out: Vec<u8> = buf.iter().filter_map(|a| if let Action::UsePotion { slot, .. } = a { Some(*slot) } else { None }).collect();
    out.dedup();
    out
}

#[test]
fn sync_potions_takes_the_games_belt_by_slot() {
    let mut cx = combat(1);
    let game = [(0usize, ids::potion::BLOCK_POTION), (2, ids::potion::FIRE_POTION)];
    assert_eq!(cx.sync_potions(3, &game), 2);
    assert_eq!(belt(&cx), game.to_vec());
    assert_eq!(usable(&cx), vec![0, 2]);
    assert_eq!(cx.sync_potions(3, &game), 0);
    assert!(cx.step(Action::UsePotion { slot: 0, target: NO }));
    assert_eq!(belt(&cx), vec![game[1]], "using a potion leaves its slot empty");
    assert_eq!(cx.sync_potions(4, &game[1..]), 1);
    assert_eq!(cx.player.potion_slots, 4);
    assert_eq!(cx.sync_potions(3, &[]), 2);
    assert!(belt(&cx).is_empty() && usable(&cx).is_empty());
}

#[test]
fn sync_potions_replaces_the_simulators_entropic_brew_roll() {
    for seed in 0..10u64 {
        let mut cx = combat(seed);
        cx.sync_potions(3, &[(1, ids::potion::ENTROPIC_BREW)]);
        assert!(cx.step(Action::UsePotion { slot: 1, target: NO }), "seed {seed}");
        assert_eq!(belt(&cx).len(), 3, "seed {seed}: Entropic Brew fills every slot");
        let game = [(0usize, ids::potion::FIRE_POTION), (1, ids::potion::BLOCK_POTION), (2, ids::potion::FIRE_POTION)];
        cx.sync_potions(3, &game);
        assert_eq!(belt(&cx), game.to_vec(), "seed {seed}");
        let hp = cx.cr(cx.enemies.as_slice()[0]).hp;
        assert!(cx.step(Action::UsePotion { slot: 2, target: cx.enemies.as_slice()[0] }), "seed {seed}");
        assert!(cx.cr(cx.enemies.as_slice()[0]).hp < hp, "seed {seed}: slot 2 throws the synced Fire Potion");
    }
}
