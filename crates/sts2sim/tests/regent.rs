//! Regent mechanics: stars, star costs, Forge / Sovereign Blade (hand-constructed situations).
use sts2sim::dec::Dec;
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::engine::ActionBuf;
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
fn divine_right_gives_three_stars_at_combat_start() {
    let cx = base();
    assert_eq!(cx.player.stars, 3);
}

#[test]
fn star_cost_is_paid_and_gated() {
    let mut cx = base();
    let e = cx.enemies[0];
    set_hand(&mut cx, &[(ids::card::FALLING_STAR, 0)]); // 0 energy, 2 stars
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.player.stars, 1);
    set_hand(&mut cx, &[(ids::card::FALLING_STAR, 0)]);
    let mut buf = ActionBuf::new();
    cx.legal_actions(&mut buf);
    assert!(!buf.iter().any(|a| matches!(a, Action::PlayCard { .. })), "1 star cannot pay a 2-star card");
}

#[test]
fn star_x_card_spends_everything() {
    let mut cx = base();
    set_hand(&mut cx, &[(ids::card::STARDUST, 0)]);
    let e = cx.enemies[0];
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.player.stars, 0);
    assert_eq!(cx.cr(e).hp, hp - 3 * 5, "3 stars = 3 hits of 5 on the only enemy");
}

#[test]
fn temporary_star_cost_expires() {
    let mut cx = base();
    let c = set_hand(&mut cx, &[(ids::card::FALLING_STAR, 0)])[0];
    cx.set_star_cost_this_turn(c, 0);
    assert_eq!(cx.card_star_cost(c), 0);
    cx.end_of_turn_cleanup();
    assert_eq!(cx.card_star_cost(c), 2);
    // a temporary 0 never gives a card without a star cost one
    let s = set_hand(&mut cx, &[(ids::card::STRIKE_REGENT, 0)])[0];
    cx.set_star_cost_this_combat(s, 0);
    assert_eq!(cx.card_current_star_cost(s), -1);
}

#[test]
fn forge_creates_one_blade_and_grows_all_blades_including_exhausted() {
    let mut cx = base();
    cx.forge(5);
    let blades = |cx: &Combat| -> Vec<u8> { cx.player_combat_cards().iter().copied().filter(|&c| cx.cards[c as usize].id == ids::card::SOVEREIGN_BLADE).collect() };
    assert_eq!(blades(&cx).len(), 1);
    let b = blades(&cx)[0];
    assert_eq!(cx.card_base_damage(b), 15);
    cx.exhaust_card(b, false);
    // an exhausted blade does not count: a new one is created, both grow
    cx.forge(3);
    let all = blades(&cx);
    assert_eq!(all.len(), 2);
    assert_eq!(cx.card_base_damage(b), 18);
    let other = all.into_iter().find(|&c| c != b).unwrap();
    assert_eq!(cx.card_base_damage(other), 13);
}

#[test]
fn sovereign_blade_hits_all_enemies_under_seeking_edge() {
    let mut cx = base();
    cx.apply_power(ids::power::SEEKING_EDGE_POWER, PLAYER, Dec::int(1), PLAYER, NO);
    cx.forge(0);
    let b = cx.player_combat_cards().iter().copied().find(|&c| cx.cards[c as usize].id == ids::card::SOVEREIGN_BLADE).unwrap();
    assert_eq!(cx.card_target_type(b), TargetType::AllEnemies);
}

#[test]
fn shining_strike_returns_to_the_top_of_the_draw_pile() {
    let mut cx = base();
    let e = cx.enemies[0];
    let c = set_hand(&mut cx, &[(ids::card::SHINING_STRIKE, 0)])[0];
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.player.draw.first(), Some(c));
    assert_eq!(cx.player.stars, 3 + 2);
}
