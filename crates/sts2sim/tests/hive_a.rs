//! Act 2 "Hive" weak / normal content: smoke runs of every encounter plus the non-obvious lifecycle rules (Imbalanced
//! self-stun, Burrowed block-break stun, Hard-to-Kill damage cap, Ovicopter eggs + hatching, Slumber wake-up, card theft,
//! Parafright revive). Fidelity itself is validated by the oracle sweeps (`oracle/templates/hive_*.json`).
use sts2sim::dec::Dec;
use sts2sim::engine::STUN_NODE;
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn scenario(enc: u16, seed: u64, hp: i32) -> Scenario {
    let mut deck = vec![];
    for _ in 0..8 {
        deck.push(DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 });
    }
    for _ in 0..2 {
        deck.push(DeckCard { id: ids::card::DEFEND_IRONCLAD, upgrade: 0 });
    }
    Scenario {
        run_seed: seed,
        total_floor: 20,
        character: 0,
        ascension: 10,
        encounter: enc,
        max_hp: hp,
        hp,
        max_energy: 3,
        orb_slots: 0,
        potion_slots: 2,
        deck,
        relics: vec![RelicInit { id: ids::relic::BURNING_BLOOD, counter: 0 }],
        potions: vec![],
        rng: RngSet::from_run_seed(seed),
    }
}

/// Plays the first playable card at the first hittable enemy, else ends the turn.
fn greedy_step(cx: &mut Combat) {
    let target = cx.hittable_enemies().first().unwrap_or(NO);
    for pos in 0..cx.player.hand.len() {
        let c = cx.player.hand[pos];
        if cx.can_play(c) {
            let t = if cx.card_def(c).target == TargetType::AnyEnemy { target } else { NO };
            if cx.step(Action::PlayCard { hand_pos: pos as u8, target: t }) {
                return;
            }
        }
    }
    assert!(cx.step(Action::EndTurn));
}

fn play_out(cx: &mut Combat) {
    let mut steps = 0;
    while cx.stage != Stage::Over {
        assert_eq!(cx.stage, Stage::AwaitAction);
        greedy_step(cx);
        steps += 1;
        assert!(steps < 3000, "fight does not terminate");
    }
}

const HIVE: [u16; 14] = [
    ids::encounter::BOWLBUGS_WEAK,
    ids::encounter::EXOSKELETONS_WEAK,
    ids::encounter::THIEVING_HOPPER_WEAK,
    ids::encounter::TUNNELER_WEAK,
    ids::encounter::BOWLBUGS_NORMAL,
    ids::encounter::CHOMPERS_NORMAL,
    ids::encounter::EXOSKELETONS_NORMAL,
    ids::encounter::HUNTER_KILLER_NORMAL,
    ids::encounter::LOUSE_PROGENITOR_NORMAL,
    ids::encounter::MYTES_NORMAL,
    ids::encounter::OVICOPTER_NORMAL,
    ids::encounter::SLUMBERING_BEETLE_NORMAL,
    ids::encounter::SPINY_TOAD_NORMAL,
    ids::encounter::THE_OBSCURA_NORMAL,
];

#[test]
fn every_hive_weak_and_normal_encounter_runs_to_the_end() {
    for &enc in HIVE.iter() {
        for seed in 0..6 {
            let mut cx = Combat::new(&scenario(enc, seed, 3000));
            play_out(&mut cx);
            assert!(cx.missing.is_none(), "unported content in {}: {:?}", ids::encounter::NAMES[enc as usize], cx.missing);
        }
    }
}

fn move_id(cx: &Combat, c: Cid) -> &'static str {
    cx.move_view(c).map_or("?", |(id, _)| id)
}

#[test]
fn bowlbug_rock_knocks_itself_off_balance_when_its_headbutt_is_fully_blocked() {
    let mut cx = Combat::new(&scenario(ids::encounter::BOWLBUGS_NORMAL, 1, 3000));
    let rock = cx.enemies[0];
    assert_eq!(cx.cr(rock).monster.id, ids::monster::BOWLBUG_ROCK);
    assert!(cx.has_power(rock, ids::power::IMBALANCED_POWER));
    assert_eq!(move_id(&cx, rock), "HEADBUTT_MOVE");
    cx.cr_mut(PLAYER).block = 500;
    assert!(cx.step(Action::EndTurn));
    // the Headbutt was swallowed by block: the Rock stunned itself inside its own move
    assert!(cx.is_stunned(rock));
    assert_eq!(cx.cr(rock).monster.next_move, STUN_NODE);
    // the dizzy turn passes, then HEADBUTT again
    assert!(cx.step(Action::EndTurn));
    assert!(!cx.is_stunned(rock));
    assert_eq!(move_id(&cx, rock), "HEADBUTT_MOVE");
}

#[test]
fn tunneler_burrow_keeps_block_and_breaking_it_stuns() {
    let mut cx = Combat::new(&scenario(ids::encounter::TUNNELER_WEAK, 2, 3000));
    let t = cx.enemies[0];
    assert_eq!(move_id(&cx, t), "BITE_MOVE");
    assert!(cx.step(Action::EndTurn));
    assert_eq!(move_id(&cx, t), "BURROW_MOVE");
    assert!(cx.step(Action::EndTurn));
    // BURROW performed: Burrowed + 37 block (A10), pending BELOW
    assert!(cx.has_power(t, ids::power::BURROWED_POWER));
    assert_eq!(cx.cr(t).block, 37);
    assert_eq!(move_id(&cx, t), "BELOW_MOVE");
    // the block survives the enemy turn start (Burrowed `ShouldClearBlock`): finish the turn cycle
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.cr(t).block, 37, "Burrowed keeps the block across turns");
    // break the block with a Strike
    cx.cr_mut(t).block = 5;
    let pos = cx.player.hand.iter().position(|&c| cx.cards[c as usize].id == ids::card::STRIKE_IRONCLAD).expect("a Strike") as u8;
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: t }));
    assert!(!cx.has_power(t, ids::power::BURROWED_POWER));
    assert_eq!(cx.cr(t).block, 0);
    assert!(cx.is_stunned(t), "block broken -> stunned");
    let hp = cx.cr(PLAYER).hp;
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.cr(PLAYER).hp, hp, "the stunned Tunneler does nothing");
    assert_eq!(move_id(&cx, t), "BITE_MOVE", "stun follow-up is BITE_MOVE");
}

#[test]
fn exoskeleton_hard_to_kill_caps_every_hit_at_nine() {
    let mut cx = Combat::new(&scenario(ids::encounter::EXOSKELETONS_WEAK, 3, 3000));
    let e = cx.enemies[0];
    assert_eq!(cx.power_amount(e, ids::power::HARD_TO_KILL_POWER), 9);
    let hp = cx.cr(e).hp;
    cx.damage(&[e], Dec::int(50), ValueProp::MOVE, PLAYER, NO);
    assert_eq!(cx.cr(e).hp, hp - 9);
    // slot order decides the openers: first SKITTER, second MANDIBLES, third ENRAGE
    assert_eq!(move_id(&cx, cx.enemies[0]), "SKITTER_MOVE");
    assert_eq!(move_id(&cx, cx.enemies[1]), "MANDIBLES_MOVE");
    assert_eq!(move_id(&cx, cx.enemies[2]), "ENRAGE_MOVE");
}

#[test]
fn ovicopter_lays_three_eggs_from_the_last_free_slot_and_they_hatch() {
    let mut cx = Combat::new(&scenario(ids::encounter::OVICOPTER_NORMAL, 4, 3000));
    assert_eq!(cx.enemies.len(), 1);
    assert!(cx.step(Action::EndTurn)); // LAY_EGGS (enemy turn): eggs are added but do not act this turn
    assert_eq!(cx.enemies.len(), 4);
    // slots egg3, egg4, egg5 (indices 2..4) sort before the Ovicopter (index 5)
    let slots: Vec<u8> = cx.enemies.iter().map(|&e| cx.cr(e).slot).collect();
    assert_eq!(slots, vec![2, 3, 4, 5]);
    for &e in cx.enemies.iter().take(3) {
        assert_eq!(cx.cr(e).monster.id, ids::monster::TOUGH_EGG);
        assert!(cx.has_power(e, ids::power::MINION_POWER));
        // laid during the enemy turn: the countdown shows 2, minus one tick at the end of that turn
        assert_eq!(cx.power_amount(e, ids::power::HATCH_POWER), 1);
        assert!(cx.cr(e).max_hp >= 15 && cx.cr(e).max_hp <= 19);
    }
    assert!(cx.step(Action::EndTurn)); // the eggs perform HATCH_MOVE
    for &e in cx.enemies.iter().take(3) {
        assert!(!cx.has_power(e, ids::power::HATCH_POWER));
        assert!(cx.has_power(e, ids::power::MINION_POWER));
        assert!(cx.cr(e).max_hp >= 20 && cx.cr(e).max_hp <= 23, "hatchling HP {}", cx.cr(e).max_hp);
        assert_eq!(cx.cr(e).hp, cx.cr(e).max_hp);
        assert_eq!(move_id(&cx, e), "NIBBLE_MOVE"); // the next roll moves on to NIBBLE
    }
}

#[test]
fn slumbering_beetle_sleeps_three_enemy_turns_then_rolls_out() {
    let mut cx = Combat::new(&scenario(ids::encounter::SLUMBERING_BEETLE_NORMAL, 5, 3000));
    let beetle = cx.enemies[2];
    assert_eq!(cx.cr(beetle).monster.id, ids::monster::SLUMBERING_BEETLE);
    assert_eq!(cx.power_amount(beetle, ids::power::PLATING_POWER), 18);
    assert_eq!(cx.power_amount(beetle, ids::power::SLUMBER_POWER), 3);
    assert_eq!(move_id(&cx, beetle), "SNORE_MOVE");
    for _ in 0..3 {
        assert!(cx.step(Action::EndTurn));
    }
    assert!(!cx.has_power(beetle, ids::power::SLUMBER_POWER));
    assert!(!cx.has_power(beetle, ids::power::PLATING_POWER), "waking up strips the Plating");
    assert_eq!(move_id(&cx, beetle), "ROLL_OUT_MOVE");
}

#[test]
fn slumbering_beetle_wakes_stunned_when_hurt_three_times() {
    let mut cx = Combat::new(&scenario(ids::encounter::SLUMBERING_BEETLE_NORMAL, 6, 3000));
    let beetle = cx.enemies[2];
    cx.cr_mut(beetle).block = 0; // Plating starts the fight as block
    for _ in 0..3 {
        cx.damage(&[beetle], Dec::int(1), ValueProp::MOVE, PLAYER, NO);
    }
    assert!(!cx.has_power(beetle, ids::power::SLUMBER_POWER));
    assert!(cx.is_stunned(beetle), "the third unblocked hit stuns the beetle");
    assert!(cx.step(Action::EndTurn));
    assert!(!cx.has_power(beetle, ids::power::PLATING_POWER), "its WakeUpMove strips the Plating");
    assert_eq!(move_id(&cx, beetle), "ROLL_OUT_MOVE");
}

#[test]
fn thieving_hopper_steals_a_card_from_the_combat_piles() {
    let mut cx = Combat::new(&scenario(ids::encounter::THIEVING_HOPPER_WEAK, 7, 3000));
    let hopper = cx.enemies[0];
    assert_eq!(move_id(&cx, hopper), "THIEVERY_MOVE");
    let total = |cx: &Combat| cx.player.draw.len() + cx.player.hand.len() + cx.player.discard.len() + cx.player.exhaust.len();
    let before = total(&cx);
    assert!(cx.step(Action::EndTurn));
    assert_eq!(total(&cx), before - 1, "one card left the combat");
    assert_eq!(cx.power_amount(hopper, ids::power::SWIPE_POWER), 1);
    assert_eq!(move_id(&cx, hopper), "FLUTTER_MOVE");
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.power_amount(hopper, ids::power::FLUTTER_POWER), 5);
    assert_eq!(move_id(&cx, hopper), "HAT_TRICK_MOVE");
}

#[test]
fn thieving_hopper_flutter_halves_attacks_and_five_hits_stun_it_into_nab() {
    let mut cx = Combat::new(&scenario(ids::encounter::THIEVING_HOPPER_WEAK, 8, 3000));
    let hopper = cx.enemies[0];
    assert!(cx.step(Action::EndTurn));
    assert!(cx.step(Action::EndTurn)); // FLUTTER performed
    assert_eq!(cx.power_amount(hopper, ids::power::FLUTTER_POWER), 5);
    let hp = cx.cr(hopper).hp;
    let pos = cx.player.hand.iter().position(|&c| cx.cards[c as usize].id == ids::card::STRIKE_IRONCLAD).expect("a Strike") as u8;
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: hopper }));
    assert_eq!(hp - cx.cr(hopper).hp, 3, "6 damage halved");
    assert_eq!(cx.power_amount(hopper, ids::power::FLUTTER_POWER), 4);
    for _ in 0..4 {
        cx.damage(&[hopper], Dec::int(2), ValueProp::MOVE, PLAYER, NO);
    }
    assert!(!cx.has_power(hopper, ids::power::FLUTTER_POWER));
    assert!(cx.is_stunned(hopper));
    // the pending move was HAT_TRICK: after the stunned turn the Hopper continues with NAB
    assert!(cx.step(Action::EndTurn));
    assert_eq!(move_id(&cx, hopper), "NAB_MOVE");
}

#[test]
fn parafright_revives_and_dies_with_the_obscura() {
    let mut cx = Combat::new(&scenario(ids::encounter::THE_OBSCURA_NORMAL, 9, 3000));
    assert_eq!(cx.enemies.len(), 1);
    assert!(cx.step(Action::EndTurn)); // ILLUSION_MOVE: Parafright into slot `illusion` (turn order first)
    assert_eq!(cx.enemies.len(), 2);
    let para = cx.enemies[0];
    assert_eq!(cx.cr(para).monster.id, ids::monster::PARAFRIGHT);
    assert!(cx.has_power(para, ids::power::ILLUSION_POWER) && cx.has_power(para, ids::power::MINION_POWER));
    cx.cr_mut(para).hp = 1;
    let pos = cx.player.hand.iter().position(|&c| cx.cards[c as usize].id == ids::card::STRIKE_IRONCLAD).expect("a Strike") as u8;
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: para }));
    assert!(cx.cr(para).is_dead());
    assert!(!cx.hittable_enemies().contains(para));
    assert!(cx.step(Action::EndTurn));
    assert!(cx.cr(para).is_alive() && cx.cr(para).hp == cx.cr(para).max_hp);
    // killing the Obscura (the only primary enemy) ends the fight even though Parafright lives
    let obs = cx.enemies[1];
    cx.cr_mut(obs).hp = 1;
    cx.cr_mut(obs).block = 0;
    let pos = cx.player.hand.iter().position(|&c| cx.cards[c as usize].id == ids::card::STRIKE_IRONCLAD).expect("a Strike") as u8;
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: obs }));
    assert_eq!(cx.stage, Stage::Over);
    assert_eq!(cx.outcome, Outcome::Victory);
}
