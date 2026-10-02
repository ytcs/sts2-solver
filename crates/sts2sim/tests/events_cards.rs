//! Event-only encounters and the EventCardPool cards: rules that are easy to get wrong.
use sts2sim::dec::Dec;
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn combat(encounter: u16, deck: &[(u16, u8)]) -> Combat {
    let deck: Vec<DeckCard> = deck.iter().map(|&(id, upgrade)| DeckCard { id, upgrade }).collect();
    Combat::new(&Scenario {
        run_seed: 0,
        total_floor: 1,
        character: 0,
        ascension: 10,
        encounter,
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

fn strikes() -> Vec<(u16, u8)> {
    (0..10).map(|_| (ids::card::STRIKE_IRONCLAD, 0)).collect()
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
fn battleworn_dummy_escapes_after_its_third_turn_and_the_combat_is_won() {
    let mut cx = combat(ids::encounter::BATTLEWORN_DUMMY_EVENT_V1_ENCOUNTER, &strikes());
    let dummy = cx.enemies[0];
    assert_eq!(cx.cr(dummy).max_hp, 75);
    assert_eq!(cx.power_amount(dummy, ids::power::BATTLEWORN_DUMMY_TIME_LIMIT_POWER), 3);
    for expected in [2, 1] {
        assert!(cx.step(Action::EndTurn));
        assert_eq!(cx.power_amount(dummy, ids::power::BATTLEWORN_DUMMY_TIME_LIMIT_POWER), expected);
    }
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.stage, Stage::Over);
    assert_eq!(cx.outcome, Outcome::Victory, "the escaped dummy leaves no enemy: combat ends in a win");
}

#[test]
fn mysterious_knight_spawns_with_strength_and_plating() {
    let cx = combat(ids::encounter::MYSTERIOUS_KNIGHT_EVENT_ENCOUNTER, &strikes());
    let k = cx.enemies[0];
    assert_eq!(cx.power_amount(k, ids::power::STRENGTH_POWER), 6);
    assert_eq!(cx.power_amount(k, ids::power::PLATING_POWER), 6);
    assert_eq!(cx.cr(k).max_hp, 108, "A8+: FlailKnight HP");
}

#[test]
fn maul_buffs_every_maul_in_the_combat_including_itself() {
    let mut cx = combat(ids::encounter::BATTLEWORN_DUMMY_EVENT_V3_ENCOUNTER, &strikes());
    let hand = set_hand(&mut cx, &[(ids::card::MAUL, 0)]);
    let other = cx.new_card(ids::card::MAUL, 0).unwrap();
    cx.move_card(other, PileType::Discard, CardPilePosition::Bottom);
    let e = cx.enemies[0];
    let hp = cx.cr(e).hp;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.cr(e).hp, hp - 10, "5 x 2 hits");
    assert_eq!(cx.card_damage_dec(hand[0]), Dec::int(7));
    assert_eq!(cx.card_damage_dec(other), Dec::int(7), "every Maul gains Increase, wherever it is");
}

#[test]
fn toric_toughness_repeats_the_block_that_was_actually_gained() {
    let mut cx = combat(ids::encounter::BATTLEWORN_DUMMY_EVENT_V3_ENCOUNTER, &strikes());
    cx.apply_power(ids::power::FRAIL_POWER, PLAYER, Dec::int(2), PLAYER, NO);
    set_hand(&mut cx, &[(ids::card::TORIC_TOUGHNESS, 0)]);
    cx.player.energy = 3;
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert_eq!(cx.cr(PLAYER).block, 3, "5 x 0.75 truncated");
    assert!(cx.step(Action::EndTurn));
    // new turn: Block cleared, then the power gives 3.75 again (Unpowered, no Frail) and counts down
    assert_eq!(cx.cr(PLAYER).block, 3);
    assert_eq!(cx.power_amount(PLAYER, ids::power::TORIC_TOUGHNESS_POWER), 1);
}

#[test]
fn mad_science_takes_its_type_and_target_from_the_saved_props() {
    let mut cx = combat(ids::encounter::BATTLEWORN_DUMMY_EVENT_V3_ENCOUNTER, &strikes());
    let c = cx.new_card(ids::card::MAD_SCIENCE, 0).unwrap();
    // props are stored as [rider, type] (sorted JSON keys)
    cx.cards[c as usize].counter = [7, 3];
    assert_eq!(cx.card_def(c).ctype, CardType::Power);
    assert_eq!(cx.card_def(c).target, TargetType::Self_);
    cx.cards[c as usize].counter = [1, 1];
    assert_eq!(cx.card_def(c).ctype, CardType::Attack);
    assert_eq!(cx.card_def(c).target, TargetType::AnyEnemy);
    cx.cards[c as usize].counter = [0, 2];
    assert_eq!(cx.card_def(c).ctype, CardType::Skill);
    // a single prop (no rider) lands in counter[0]
    cx.cards[c as usize].counter = [2, 0];
    assert_eq!(cx.card_def(c).ctype, CardType::Skill);
}

#[test]
fn clash_is_only_playable_with_attacks_in_hand() {
    let mut cx = combat(ids::encounter::BATTLEWORN_DUMMY_EVENT_V3_ENCOUNTER, &strikes());
    set_hand(&mut cx, &[(ids::card::CLASH, 0), (ids::card::STRIKE_IRONCLAD, 0), (ids::card::DEFEND_IRONCLAD, 0)]);
    let mut buf = sts2sim::engine::ActionBuf::new();
    cx.legal_actions(&mut buf);
    assert!(!buf.iter().any(|a| matches!(a, Action::PlayCard { hand_pos: 0, .. })), "a Skill in hand blocks Clash");
}
