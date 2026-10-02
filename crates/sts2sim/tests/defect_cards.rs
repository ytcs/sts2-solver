//! Defect card / power rules that are easy to get subtly wrong (the oracle sweeps are the ground truth; these pin the
//! rules down so a regression fails fast without the game).
use sts2sim::defs::VarKind;
use sts2sim::engine::VALID_ORBS;
use sts2sim::dec::Dec;
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn scenario(seed: u64) -> Scenario {
    let mut deck = vec![];
    for _ in 0..5 {
        deck.push(DeckCard { id: ids::card::STRIKE_DEFECT, upgrade: 0 });
    }
    for _ in 0..5 {
        deck.push(DeckCard { id: ids::card::DEFEND_DEFECT, upgrade: 0 });
    }
    Scenario {
        run_seed: seed,
        total_floor: 1,
        character: 2,
        ascension: 10,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: 75,
        hp: 75,
        max_energy: 3,
        orb_slots: 3,
        potion_slots: 2,
        deck,
        relics: vec![],
        potions: vec![],
        rng: RngSet::from_run_seed(seed),
    }
}

/// Puts a fresh card into `pile` (bottom) and returns it.
fn put(cx: &mut Combat, id: u16, upgrade: u8, pile: PileType) -> CardIdx {
    let c = cx.new_card(id, upgrade).unwrap();
    assert!(cx.move_card(c, pile, CardPilePosition::Bottom));
    c
}

fn hand_pos(cx: &Combat, c: CardIdx) -> u8 {
    cx.player.hand.position(c).unwrap() as u8
}

fn play(cx: &mut Combat, c: CardIdx, target: Cid) {
    let pos = hand_pos(cx, c);
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target }), "card not playable");
}

fn enemy_hp(cx: &Combat) -> i32 {
    cx.cr(cx.enemies[0]).hp
}

#[test]
fn claw_growth_reaches_every_claw_in_every_combat_pile() {
    let mut cx = Combat::new(&scenario(1));
    let e = cx.enemies[0];
    cx.player.energy = 10;
    let played = put(&mut cx, ids::card::CLAW, 0, PileType::Hand);
    let in_discard = put(&mut cx, ids::card::CLAW, 0, PileType::Discard);
    let in_draw = put(&mut cx, ids::card::CLAW, 1, PileType::Draw);
    let in_exhaust = put(&mut cx, ids::card::CLAW, 0, PileType::Exhaust);
    let hp = enemy_hp(&cx);
    play(&mut cx, played, e);
    assert_eq!(hp - enemy_hp(&cx), 3); // the playing Claw deals its own (un-grown) damage
    for c in [played, in_discard, in_exhaust] {
        assert_eq!(cx.card_var(c, VarKind::Damage), 3 + 2, "Increase = 2");
    }
    assert_eq!(cx.card_var(in_draw, VarKind::Damage), 4 + 2, "upgraded Claw: Damage +1 and Increase +1 only for ITS play");
    // the next Claw play hits for the grown value
    let c2 = in_discard;
    assert!(cx.move_card(c2, PileType::Hand, CardPilePosition::Bottom));
    let hp = enemy_hp(&cx);
    play(&mut cx, c2, e);
    assert_eq!(hp - enemy_hp(&cx), 5);
}

#[test]
fn genetic_algorithm_block_grows_per_instance() {
    let mut cx = Combat::new(&scenario(2));
    cx.player.energy = 10;
    let ga = put(&mut cx, ids::card::GENETIC_ALGORITHM, 0, PileType::Hand);
    assert_eq!(cx.card_var(ga, VarKind::Block), 1);
    let before = cx.cr(PLAYER).block;
    play(&mut cx, ga, NO);
    assert_eq!(cx.cr(PLAYER).block, before + 1);
    assert_eq!(cx.card_var(ga, VarKind::Block), 1 + 3, "+Increase (3) after the play");
}

#[test]
fn echo_form_plays_only_the_first_card_twice() {
    let mut cx = Combat::new(&scenario(3));
    let e = cx.enemies[0];
    cx.player.energy = 10;
    cx.apply_power(ids::power::ECHO_FORM_POWER, PLAYER, Dec::int(1), PLAYER, NO);
    let a = put(&mut cx, ids::card::STRIKE_DEFECT, 0, PileType::Hand);
    let b = put(&mut cx, ids::card::STRIKE_DEFECT, 0, PileType::Hand);
    let hp = enemy_hp(&cx);
    play(&mut cx, a, e);
    let after_first = enemy_hp(&cx);
    assert_eq!(hp - after_first, 12, "two plays of a 6-damage Strike");
    play(&mut cx, b, e);
    assert_eq!(after_first - enemy_hp(&cx), 6, "the second card of the turn is played once");
}

#[test]
fn feral_returns_the_first_zero_cost_attacks_to_the_top_of_the_hand() {
    let mut cx = Combat::new(&scenario(4));
    let e = cx.enemies[0];
    cx.apply_power(ids::power::FERAL_POWER, PLAYER, Dec::int(1), PLAYER, NO);
    let beam = put(&mut cx, ids::card::BEAM_CELL, 0, PileType::Hand);
    let beam2 = put(&mut cx, ids::card::BEAM_CELL, 0, PileType::Hand);
    play(&mut cx, beam, e);
    assert_eq!(cx.player.hand[0], beam, "a free Attack goes back to the top of the hand");
    assert_eq!(cx.card_pile_type(beam), PileType::Hand);
    play(&mut cx, beam2, e);
    assert_eq!(cx.card_pile_type(beam2), PileType::Discard, "Feral (1) is used up for this turn");
}

#[test]
fn chaos_draws_the_game_orb_order_from_the_combat_orbs_stream() {
    let mut cx = Combat::new(&scenario(5));
    cx.player.energy = 10;
    let mut probe = cx.rng.combat_orbs;
    let expect = VALID_ORBS[probe.next_int_range(0, 5) as usize];
    let chaos = put(&mut cx, ids::card::CHAOS, 0, PileType::Hand);
    play(&mut cx, chaos, NO);
    assert_eq!(cx.player.orbs.first().map(|o| o.kind), Some(expect));
    assert_eq!(cx.rng.combat_orbs.counter, probe.counter);
}

#[test]
fn thunder_fires_for_every_lightning_evoke_dualcast_evokes_the_same_orb_twice() {
    let mut cx = Combat::new(&scenario(6));
    let e = cx.enemies[0];
    cx.player.energy = 10;
    cx.apply_power(ids::power::THUNDER_POWER, PLAYER, Dec::int(8), PLAYER, NO);
    cx.channel_orb(ids::orb::LIGHTNING_ORB);
    let dual = put(&mut cx, ids::card::DUALCAST, 0, PileType::Hand);
    let hp = cx.cr(e).hp;
    let block = cx.cr(e).block;
    assert_eq!(block, 0);
    play(&mut cx, dual, NO);
    // two evokes (8 each) + two Thunder procs (8 each) = 32, the orb is gone afterwards
    assert_eq!(hp - cx.cr(e).hp, 32);
    assert!(cx.player.orbs.is_empty());
}

#[test]
fn loop_triggers_the_front_orb_passive_at_turn_start() {
    let mut cx = Combat::new(&scenario(7));
    cx.apply_power(ids::power::LOOP_POWER, PLAYER, Dec::int(1), PLAYER, NO);
    cx.channel_orb(ids::orb::DARK_ORB);
    cx.channel_orb(ids::orb::FROST_ORB);
    assert!(cx.step(Action::EndTurn)); // end-of-turn passives: Dark +6 (12), Frost +2 block ...
    // ... and Loop adds one more Dark trigger at the start of the next turn (front orb)
    let dark = cx.player.orbs[0];
    assert_eq!(dark.kind, ids::orb::DARK_ORB);
    assert_eq!(cx.orb_evoke_val(&dark).trunc(), 6 + 6 + 6);
}

#[test]
fn uproar_auto_plays_a_random_attack_from_the_draw_pile() {
    let mut cx = Combat::new(&scenario(8));
    let e = cx.enemies[0];
    cx.player.energy = 10;
    // a draw pile with exactly one Attack: Beam Cell (Vulnerable 1)
    cx.player.draw.clear();
    let beam = cx.new_card(ids::card::BEAM_CELL, 0).unwrap();
    cx.player.draw.push(beam);
    cx.cards[beam as usize].pile = PileType::Draw as u8;
    let uproar = put(&mut cx, ids::card::UPROAR, 0, PileType::Hand);
    let hp = enemy_hp(&cx);
    play(&mut cx, uproar, e);
    assert_eq!(cx.card_pile_type(beam), PileType::Discard, "the auto-played card ends in the discard pile");
    assert!(cx.has_power(e, ids::power::VULNERABLE_POWER));
    assert!(hp - enemy_hp(&cx) >= 6 + 3, "Uproar 2x6 (Vulnerable arrives after) and Beam Cell");
}

#[test]
fn hyperbeam_focus_loss_is_temporary_and_keeps_orb_values_floored() {
    let mut cx = Combat::new(&scenario(9));
    cx.player.energy = 10;
    cx.channel_orb(ids::orb::FROST_ORB);
    let hb = put(&mut cx, ids::card::HYPERBEAM, 0, PileType::Hand);
    let o = cx.player.orbs[0];
    assert_eq!(cx.orb_passive_val(&o).trunc(), 2);
    play(&mut cx, hb, NO);
    assert_eq!(cx.power_amount(PLAYER, ids::power::FOCUS_POWER), -3);
    let o = cx.player.orbs[0];
    assert_eq!(cx.orb_passive_val(&o).trunc(), 0, "2 - 3 floors at 0");
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.power_amount(PLAYER, ids::power::FOCUS_POWER), 0, "the Focus loss ends with the turn");
    assert!(!cx.has_power(PLAYER, ids::power::HYPERBEAM_FOCUS_DOWN_POWER));
}

#[test]
fn feral_ignores_auto_played_attacks_that_have_an_energy_cost() {
    // Uproar auto-plays a 1-cost Strike: EnergySpent is 0 but EnergyValue is 1, so Feral must not return it to hand
    // and must not count it among the free attacks of the turn.
    let mut cx = Combat::new(&scenario(10));
    let e = cx.enemies[0];
    cx.player.energy = 10;
    cx.apply_power(ids::power::FERAL_POWER, PLAYER, Dec::int(1), PLAYER, NO);
    cx.player.draw.clear();
    let strike = cx.new_card(ids::card::STRIKE_DEFECT, 0).unwrap();
    cx.player.draw.push(strike);
    cx.cards[strike as usize].pile = PileType::Draw as u8;
    let uproar = put(&mut cx, ids::card::UPROAR, 0, PileType::Hand);
    play(&mut cx, uproar, e);
    assert_eq!(cx.card_pile_type(strike), PileType::Discard);
    let aux = cx.cr(PLAYER).power(ids::power::FERAL_POWER).unwrap().aux;
    assert_eq!(aux, 0, "the auto-played 1-cost Strike did not use Feral");
    assert_eq!(cx.hist.zero_cost_attacks_started, 0);
}

#[test]
fn every_defect_pool_card_and_the_starter_relic_are_registered() {
    use sts2sim::content;
    for &id in sts2sim::content::gen_pools::DEFECT.iter() {
        assert!(content::card_implemented(id), "{} has no Rust implementation", ids::card::NAMES[id as usize]);
    }
    assert!(content::relic_implemented(ids::relic::CRACKED_CORE));
    assert!(content::power_implemented(ids::power::FOCUS_POWER));
}
