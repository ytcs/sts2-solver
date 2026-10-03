//! Act 3 "Glory" weak + normal encounters: smoke runs of every encounter plus the non-obvious lifecycle rules (Axebot
//! respawns, Fabricator bots, Scroll starter moves, Lost/Forgotten stat restore, Galvanic afflictions).
//! Fidelity itself is validated by the oracle sweeps (`oracle/templates/glory_a_*.json`).
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
        relics: vec![RelicInit { id: ids::relic::BURNING_BLOOD, counter: 0, ..Default::default() }],
        potions: vec![],
        rng: RngSet::from_run_seed(seed),
    }
}

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
        assert!(steps < 4000, "fight does not terminate");
    }
}

const GLORY: [u16; 12] = [
    ids::encounter::DEVOTED_SCULPTOR_WEAK,
    ids::encounter::SCROLLS_OF_BITING_WEAK,
    ids::encounter::TURRET_OPERATOR_WEAK,
    ids::encounter::AXEBOTS_NORMAL,
    ids::encounter::CONSTRUCT_MENAGERIE_NORMAL,
    ids::encounter::FABRICATOR_NORMAL,
    ids::encounter::FROG_KNIGHT_NORMAL,
    ids::encounter::GLOBE_HEAD_NORMAL,
    ids::encounter::OWL_MAGISTRATE_NORMAL,
    ids::encounter::SCROLLS_OF_BITING_NORMAL,
    ids::encounter::SLIMED_BERSERKER_NORMAL,
    ids::encounter::THE_LOST_AND_FORGOTTEN_NORMAL,
];

#[test]
fn every_glory_encounter_runs_to_the_end() {
    for &enc in GLORY.iter() {
        for seed in 0..6 {
            let mut cx = Combat::new(&scenario(enc, seed, 300));
            play_out(&mut cx);
            assert!(cx.missing.is_none(), "unported content in {}: {:?}", ids::encounter::NAMES[enc as usize], cx.missing);
        }
    }
}

fn find_in_hand(cx: &Combat, id: u16) -> Option<u8> {
    cx.player.hand.iter().position(|&c| cx.cards[c as usize].id == id).map(|p| p as u8)
}

fn move_id(cx: &Combat, c: Cid) -> &'static str {
    cx.move_view(c).map(|(id, _)| id).unwrap_or("UNSET")
}

fn kill_with_strike(cx: &mut Combat, target: Cid) {
    cx.cr_mut(target).hp = 1;
    let pos = find_in_hand(cx, ids::card::STRIKE_IRONCLAD).expect("a Strike in hand");
    assert!(cx.step(Action::PlayCard { hand_pos: pos, target }));
}

#[test]
fn axebot_respawns_twice_with_more_hp_and_a_boot_up_move() {
    let mut cx = Combat::new(&scenario(ids::encounter::AXEBOTS_NORMAL, 4, 5000));
    let first = cx.enemies[0];
    assert_eq!(cx.cr(first).power_amount(ids::power::STOCK_POWER), 2);
    assert_eq!(move_id(&cx, first), "HAMMER_UPPERCUT_MOVE");
    assert!((76..=86).contains(&cx.cr(first).max_hp));
    kill_with_strike(&mut cx, first);
    // replaced (same slot) by a respawn: +10 max HP, Stock 1, BOOT_UP rolled immediately (killed on the player's turn)
    assert_eq!(cx.stage, Stage::AwaitAction);
    let second = cx.enemies.last().unwrap();
    assert_ne!(second, first);
    assert_eq!(cx.cr(second).monster.id, ids::monster::AXEBOT);
    assert!((86..=96).contains(&cx.cr(second).max_hp), "hp {}", cx.cr(second).max_hp);
    assert_eq!(cx.cr(second).power_amount(ids::power::STOCK_POWER), 1);
    assert_eq!(move_id(&cx, second), "BOOT_UP_MOVE");
    // second death: Stock 0 -> no Stock power on the last Axebot (+20 max HP)
    if find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).is_none() {
        assert!(cx.step(Action::EndTurn));
    }
    kill_with_strike(&mut cx, second);
    let third = cx.enemies.last().unwrap();
    assert!((96..=106).contains(&cx.cr(third).max_hp), "hp {}", cx.cr(third).max_hp);
    assert!(!cx.has_power(third, ids::power::STOCK_POWER));
    assert_eq!(move_id(&cx, third), "BOOT_UP_MOVE");
    // killing the last one ends the combat
    if find_in_hand(&cx, ids::card::STRIKE_IRONCLAD).is_none() {
        assert!(cx.step(Action::EndTurn));
    }
    kill_with_strike(&mut cx, third);
    assert_eq!(cx.stage, Stage::Over);
    assert_eq!(cx.outcome, Outcome::Victory);
}

#[test]
fn scroll_starter_moves_rotate_with_the_encounter_draw() {
    let cx = Combat::new(&scenario(ids::encounter::SCROLLS_OF_BITING_NORMAL, 9, 5000));
    assert_eq!(cx.enemies.len(), 4);
    let mut idx = [0i32; 4];
    for (i, &e) in cx.enemies.iter().enumerate() {
        idx[i] = cx.cr(e).monster.vars[0];
        let expect = match idx[i] % 3 {
            0 => "CHOMP",
            1 => "CHEW",
            _ => "MORE_TEETH",
        };
        assert_eq!(move_id(&cx, e), expect);
        assert_eq!(cx.cr(e).power_amount(ids::power::PAPER_CUTS_POWER), 2);
    }
    assert_eq!(idx[1], (idx[0] + 1) % 3);
    assert_eq!(idx[2], (idx[0] + 2) % 3);
    assert_eq!(idx[3], 2);
}

#[test]
fn fabricator_builds_one_defensive_and_one_aggressive_bot_into_free_slots() {
    let mut cx = Combat::new(&scenario(ids::encounter::FABRICATOR_NORMAL, 2, 5000));
    let fab = cx.enemies[0];
    assert_eq!(cx.cr(fab).slot, 2);
    // first roll draws nothing but the branch's RAND (1 draw): FABRICATE (two bots) or FABRICATING_STRIKE (one bot)
    let strike = move_id(&cx, fab) == "FABRICATING_STRIKE_MOVE";
    assert!(cx.step(Action::EndTurn));
    let bots = cx.enemies.len() - 1;
    assert_eq!(bots, if strike { 1 } else { 2 });
    for &e in cx.enemies.iter() {
        if e == fab {
            continue;
        }
        assert!(cx.has_power(e, ids::power::MINION_POWER));
        assert_eq!(cx.cr(e).power(ids::power::MINION_POWER).unwrap().applier, fab);
    }
    // bots occupy bot1 / bot2 first (slots 0, 1): enemy order is slot order
    assert_eq!(cx.cr(cx.enemies[0]).slot, 0);
}

#[test]
fn the_lost_gives_the_stolen_strength_back_when_it_dies() {
    let mut cx = Combat::new(&scenario(ids::encounter::THE_LOST_AND_FORGOTTEN_NORMAL, 6, 5000));
    let lost = cx.enemies[0];
    assert_eq!(cx.cr(lost).monster.id, ids::monster::THE_LOST);
    assert!(cx.has_power(lost, ids::power::POSSESS_STRENGTH_POWER));
    assert_eq!(move_id(&cx, lost), "DEBILITATING_SMOG");
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.cr(PLAYER).power_amount(ids::power::STRENGTH_POWER), -2);
    assert_eq!(cx.cr(PLAYER).power_amount(ids::power::DEXTERITY_POWER), -2);
    kill_with_strike(&mut cx, lost);
    // Strength is restored (the stolen amount is returned), the Forgotten's stolen Dexterity is not
    assert_eq!(cx.cr(PLAYER).power_amount(ids::power::STRENGTH_POWER), 0);
    assert_eq!(cx.cr(PLAYER).power_amount(ids::power::DEXTERITY_POWER), -2);
}

#[test]
fn living_shield_switches_to_smash_when_alone() {
    let mut cx = Combat::new(&scenario(ids::encounter::TURRET_OPERATOR_WEAK, 1, 5000));
    let shield = cx.enemies[0];
    let turret = cx.enemies[1];
    assert_eq!(cx.cr(shield).monster.id, ids::monster::LIVING_SHIELD);
    assert_eq!(move_id(&cx, shield), "SHIELD_SLAM_MOVE");
    // Rampart: the Turret Operator starts every player turn with 25 block
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.cr(turret).block, 25);
    cx.cr_mut(turret).block = 0;
    kill_with_strike(&mut cx, turret);
    assert!(cx.step(Action::EndTurn));
    assert_eq!(move_id(&cx, shield), "SMASH_MOVE");
}

#[test]
fn killing_a_minion_is_not_fatal_for_feed() {
    let mut cx = Combat::new(&scenario(ids::encounter::FABRICATOR_NORMAL, 2, 5000));
    let fab = cx.enemies[0];
    assert!(cx.all_powers_trigger_fatal(fab));
    assert!(cx.step(Action::EndTurn));
    let bot = cx.enemies.iter().copied().find(|&e| e != fab).unwrap();
    assert!(!cx.all_powers_trigger_fatal(bot));
}
