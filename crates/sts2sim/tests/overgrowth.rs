//! Act 1a "Overgrowth" content: smoke runs of every encounter plus the non-obvious lifecycle rules
//! (Infested burst, Illusion revive, Plow stun, Tangled afflictions). Fidelity itself is validated by the oracle sweeps.
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
        total_floor: 1,
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

const OVERGROWTH: [u16; 22] = [
    ids::encounter::FUZZY_WURM_CRAWLER_WEAK,
    ids::encounter::NIBBITS_WEAK,
    ids::encounter::SHRINKER_BEETLE_WEAK,
    ids::encounter::SLIMES_WEAK,
    ids::encounter::CUBEX_CONSTRUCT_NORMAL,
    ids::encounter::FLYCONID_NORMAL,
    ids::encounter::FOGMOG_NORMAL,
    ids::encounter::INKLETS_NORMAL,
    ids::encounter::MAWLER_NORMAL,
    ids::encounter::NIBBITS_NORMAL,
    ids::encounter::OVERGROWTH_CRAWLERS,
    ids::encounter::RUBY_RAIDERS_NORMAL,
    ids::encounter::SLIMES_NORMAL,
    ids::encounter::SLITHERING_STRANGLER_NORMAL,
    ids::encounter::SNAPPING_JAXFRUIT_NORMAL,
    ids::encounter::VINE_SHAMBLER_NORMAL,
    ids::encounter::BYGONE_EFFIGY_ELITE,
    ids::encounter::BYRDONIS_ELITE,
    ids::encounter::PHROG_PARASITE_ELITE,
    ids::encounter::CEREMONIAL_BEAST_BOSS,
    ids::encounter::THE_KIN_BOSS,
    ids::encounter::VANTOM_BOSS,
];

#[test]
fn every_overgrowth_encounter_runs_to_the_end() {
    for &enc in OVERGROWTH.iter() {
        for seed in 0..6 {
            let mut cx = Combat::new(&scenario(enc, seed, 2000));
            play_out(&mut cx);
            assert!(cx.missing.is_none(), "unported content in {}: {:?}", ids::encounter::NAMES[enc as usize], cx.missing);
        }
    }
}

fn find_in_hand(cx: &Combat, id: u16) -> Option<u8> {
    cx.player.hand.iter().position(|&c| cx.cards[c as usize].id == id).map(|p| p as u8)
}

#[test]
fn phrog_death_bursts_into_four_stunned_wrigglers() {
    let mut cx = Combat::new(&scenario(ids::encounter::PHROG_PARASITE_ELITE, 3, 500));
    let phrog = cx.enemies[0];
    cx.cr_mut(phrog).hp = 1;
    let pos = find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).expect("a Strike in the opening hand");
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: phrog }));
    // combat is still on: four Wrigglers (slots wriggler1..4) replaced the Phrog, each showing the stun intent
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert_eq!(cx.enemies.len(), 4);
    for (i, &w) in cx.enemies.iter().enumerate() {
        assert_eq!(cx.cr(w).monster.id, ids::monster::WRIGGLER);
        assert_eq!(cx.cr(w).slot, 1 + i as u8);
        assert_eq!(cx.cr(w).monster.next_move, 1, "SPAWNED_MOVE");
    }
}

#[test]
fn eye_with_teeth_revives_and_dies_with_fogmog() {
    let mut cx = Combat::new(&scenario(ids::encounter::FOGMOG_NORMAL, 5, 2000));
    assert_eq!(cx.enemies.len(), 1);
    assert!(cx.step(Action::EndTurn)); // Fogmog's ILLUSION_MOVE summons the Eye in slot `illusion` (turn order: Eye first)
    assert_eq!(cx.enemies.len(), 2);
    let eye = cx.enemies[0];
    assert_eq!(cx.cr(eye).monster.id, ids::monster::EYE_WITH_TEETH);
    // kill the Eye: it stays in the enemy list, unhittable, and revives at full HP after performing REVIVE_MOVE
    cx.cr_mut(eye).hp = 1;
    let pos = find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).expect("a Strike");
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: eye }));
    assert_eq!(cx.enemies.len(), 2);
    assert!(cx.cr(eye).is_dead());
    assert!(!cx.hittable_enemies().contains(eye));
    assert!(cx.has_power(eye, ids::power::ILLUSION_POWER) && cx.has_power(eye, ids::power::MINION_POWER));
    assert!(cx.step(Action::EndTurn));
    assert!(cx.cr(eye).is_alive() && cx.cr(eye).hp == cx.cr(eye).max_hp);
    // killing the Fogmog (the only primary enemy) ends the fight even though the Eye lives
    let fog = cx.enemies[1];
    cx.cr_mut(fog).hp = 1;
    let pos = find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).expect("a Strike");
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target: fog }));
    assert_eq!(cx.stage, Stage::Over);
    assert_eq!(cx.outcome, Outcome::Victory);
}

#[test]
fn vine_shambler_tangled_makes_attacks_cost_more_until_turn_end() {
    // GRASPING_VINES is the second move: SWIPE (turn 1) -> GRASPING_VINES (turn 2)
    let mut cx = Combat::new(&scenario(ids::encounter::VINE_SHAMBLER_NORMAL, 1, 2000));
    assert!(cx.step(Action::EndTurn));
    assert!(cx.step(Action::EndTurn));
    // after the turn-2 enemy move the player is Tangled: every attack in the hand costs +1
    assert!(cx.has_power(PLAYER, ids::power::TANGLED_POWER));
    let strike = find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).expect("a Strike");
    assert_eq!(cx.card_cost(cx.player.hand[strike as usize], true), 2);
    // the power (and the affliction) vanish at the end of the player's turn
    assert!(cx.step(Action::EndTurn));
    assert!(!cx.has_power(PLAYER, ids::power::TANGLED_POWER));
    let strike = find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).expect("a Strike");
    assert_eq!(cx.card_cost(cx.player.hand[strike as usize], true), 1);
}
