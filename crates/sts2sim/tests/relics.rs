//! Relic behaviour in hand-constructed situations (the differential sweeps cover the rest).
use sts2sim::dec::Dec;
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn with_relics(relics: Vec<RelicInit>, hp: i32) -> Combat {
    let mut deck: Vec<DeckCard> = (0..8).map(|_| DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 }).collect();
    deck.extend((0..4).map(|_| DeckCard { id: ids::card::DEFEND_IRONCLAD, upgrade: 0 }));
    Combat::new(&Scenario {
        run_seed: 0,
        total_floor: 1,
        character: 0,
        ascension: 10,
        encounter: ids::encounter::NIBBITS_WEAK,
        max_hp: 80,
        hp,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 2,
        deck,
        relics,
        potions: vec![],
        rng: RngSet::from_run_seed(7),
    })
}

fn relic(id: u16) -> RelicInit {
    RelicInit { id, ..Default::default() }
}

#[test]
fn a_power_applied_mid_turn_reads_zero_amount_on_turn_start() {
    let mut cx = with_relics(vec![], 80);
    cx.apply_power(ids::power::STRENGTH_POWER, PLAYER, Dec::int(2), PLAYER, NO);
    assert_eq!(cx.cr(PLAYER).powers[0].amount_on_turn_start, 0);
}

#[test]
fn lizard_tail_prevents_the_first_death_and_heals_half() {
    let mut cx = with_relics(vec![relic(ids::relic::LIZARD_TAIL)], 80);
    cx.cr_mut(PLAYER).hp = 5;
    // lethal hit
    cx.damage(&[PLAYER], Dec::int(40), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED), NO, NO);
    assert_eq!(cx.cr(PLAYER).hp, 40, "healed to 50% of max HP");
    assert!(cx.player.relics[0].flag(0), "WasUsed");
    assert!(cx.in_progress && !cx.pending_loss);
    // the second lethal hit kills
    cx.damage(&[PLAYER], Dec::int(100), ValueProp::UNBLOCKABLE.or(ValueProp::UNPOWERED), NO, NO);
    assert!(cx.cr(PLAYER).is_dead());
    assert!(cx.pending_loss);
}

#[test]
fn ice_cream_keeps_energy_between_turns_and_bread_trades_it() {
    // Ice Cream: after turn 1 energy is added to instead of reset.
    let mut cx = with_relics(vec![relic(ids::relic::ICE_CREAM)], 80);
    assert_eq!(cx.player.energy, 3);
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.player.energy, 6);
    // Bread: turn 1 has 2 energy less, later turns 1 more.
    let mut cx = with_relics(vec![relic(ids::relic::BREAD)], 80);
    assert_eq!(cx.player.energy, 1);
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.player.energy, 4);
}

#[test]
fn fiddle_blocks_draws_during_the_players_turn_but_not_the_hand_draw() {
    let mut cx = with_relics(vec![relic(ids::relic::FIDDLE)], 80);
    assert_eq!(cx.player.hand.len(), 7, "5 + Fiddle's 2");
    let before = cx.player.hand.len();
    cx.draw_cards(1, false);
    assert_eq!(cx.player.hand.len(), before);
}

#[test]
fn nunchaku_counts_attacks_and_gives_energy_on_the_tenth() {
    let mut r = relic(ids::relic::NUNCHAKU);
    r.counter = 9;
    let mut cx = with_relics(vec![r], 80);
    let e = cx.enemies[0];
    let hand = cx.player.hand;
    let strike = hand.iter().copied().find(|&c| cx.cards[c as usize].id == ids::card::STRIKE_IRONCLAD).unwrap();
    let pos = hand.iter().position(|&c| c == strike).unwrap();
    let energy = cx.player.energy;
    assert!(cx.step(Action::PlayCard { hand_pos: pos as u8, target: e }));
    assert_eq!(cx.player.relics[0].counter, 10);
    assert_eq!(cx.player.energy, energy - 1 + 1);
}

#[test]
fn relic_props_roundtrip_through_slots() {
    let l = content::relic_listener(ids::relic::EMBER_TEA);
    let defs = l.meta_props();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].name, "CombatsLeft");
    assert_eq!(l.meta_initial().0, 5);
}
