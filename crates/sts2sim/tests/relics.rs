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

#[test]
fn gambling_chip_suspends_the_turn_start_and_resumes_it() {
    let mut cx = with_relics(vec![relic(ids::relic::GAMBLING_CHIP)], 80);
    // Combat::new stopped inside the first turn start: a hand decision (discard any number) is pending.
    assert_eq!(cx.stage, Stage::AwaitChoice);
    assert_eq!(cx.player.phase, Phase::Start);
    let d = cx.decision.as_ref().expect("decision");
    assert_eq!((d.min, d.max, d.cands.len()), (0, u8::MAX, 5));
    let first_two = [d.cands[0], d.cands[1]];
    assert!(cx.step(Action::Pick { idx: 0 }));
    assert!(cx.step(Action::Pick { idx: 1 }));
    assert!(cx.step(Action::Confirm));
    // the turn start continued: play phase, two cards discarded and two drawn
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert_eq!(cx.player.phase, Phase::Play);
    assert_eq!(cx.player.hand.len(), 5);
    assert_eq!(cx.player.discard.len(), 2);
    for c in first_two {
        assert!(cx.player.discard.contains(c));
    }
    assert!(cx.turn_cont == 0 && cx.hook_ctx.is_none());
}

#[test]
fn gambling_chip_with_an_empty_selection_just_continues() {
    let mut cx = with_relics(vec![relic(ids::relic::GAMBLING_CHIP)], 80);
    assert!(cx.step(Action::Confirm));
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert_eq!(cx.player.hand.len(), 5);
    assert!(cx.player.discard.is_empty());
}

fn play_first(cx: &mut Combat, id: u16) -> bool {
    let e = cx.enemies[0];
    let hand = cx.player.hand;
    let Some(pos) = hand.iter().position(|&c| cx.cards[c as usize].id == id) else { return false };
    let c = hand[pos];
    let target = if cx.card_def(c).target == TargetType::AnyEnemy { e } else { NO };
    cx.step(Action::PlayCard { hand_pos: pos as u8, target })
}

#[test]
fn pen_nib_doubles_the_tenth_attack() {
    let mut r = relic(ids::relic::PEN_NIB);
    r.counter = 9; // AttacksPlayed % 10 == 9: the next attack is the 10th
    let mut cx = with_relics(vec![r], 80);
    let e = cx.enemies[0];
    let hp = cx.cr(e).hp;
    assert!(play_first(&mut cx, ids::card::STRIKE_IRONCLAD));
    assert_eq!(hp - cx.cr(e).hp, 12, "6 doubled");
    assert_eq!(cx.player.relics[0].counter, 0);
    assert_eq!(cx.player.relics[0].aux, 0, "AttackToDouble cleared after the card was played");
    let hp = cx.cr(e).hp;
    assert!(play_first(&mut cx, ids::card::STRIKE_IRONCLAD));
    assert_eq!(hp - cx.cr(e).hp, 6);
}

#[test]
fn throwing_axe_replays_only_the_first_card() {
    let mut cx = with_relics(vec![relic(ids::relic::THROWING_AXE)], 80);
    let e = cx.enemies[0];
    let hp = cx.cr(e).hp;
    assert!(play_first(&mut cx, ids::card::STRIKE_IRONCLAD));
    assert_eq!(hp - cx.cr(e).hp, 12, "played twice");
    let hp = cx.cr(e).hp;
    assert!(play_first(&mut cx, ids::card::STRIKE_IRONCLAD));
    assert_eq!(hp - cx.cr(e).hp, 6);
}

#[test]
fn sturdy_clamp_keeps_up_to_ten_block() {
    let mut cx = with_relics(vec![relic(ids::relic::STURDY_CLAMP)], 80);
    cx.cr_mut(PLAYER).block = 25;
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.cr(PLAYER).block, 10);
}

/// Every relic class is registered (the orb relics landed with the Defect/orb engine).
#[test]
fn every_relic_is_registered_except_the_known_unported() {
    const UNPORTED: &[&str] = &[];
    let mut missing = vec![];
    for (i, name) in ids::relic::NAMES.iter().enumerate() {
        if !content::relic_implemented(i as u16) {
            missing.push(*name);
        }
    }
    let mut want: Vec<&str> = UNPORTED.to_vec();
    want.sort();
    missing.sort();
    assert_eq!(missing, want);
}

#[test]
fn combat_state_stays_small() {
    // design.md: `Clone` of a fight is a memcpy of ~14 KB; relic state / hook plumbing must not blow it up.
    let n = std::mem::size_of::<Combat>();
    assert!(n < 24 * 1024, "size_of::<Combat>() = {n}");
}
