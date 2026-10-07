//! Runaway-work safeguard (`ov::LOOP`, `engine/budget.rs`): a trigger chain that would never end inside one step (or inside the
//! enemy look-ahead of an observation) is cut short, the step returns with the flag set and the combat is left safe to drop.
//! The loops are built from real content in states the game never reaches (an enemy carrying the Ironclad's Inferno).
use sts2sim::dec::Dec;
use sts2sim::engine::{Action, ActionBuf};
use sts2sim::ids;
use sts2sim::observe::OBS_SIZE;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn combat(deck: Vec<DeckCard>) -> Combat {
    Combat::new(&Scenario {
        run_seed: 3,
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
        relics: vec![RelicInit { id: ids::relic::BURNING_BLOOD, ..Default::default() }],
        potions: vec![],
        rng: RngSet::from_run_seed(3),
    })
}

fn starter() -> Vec<DeckCard> {
    let mut d: Vec<DeckCard> = (0..5).map(|_| DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 }).collect();
    d.extend((0..4).map(|_| DeckCard { id: ids::card::DEFEND_IRONCLAD, upgrade: 0 }));
    d.push(DeckCard { id: ids::card::BASH, upgrade: 0 });
    d
}

/// An enemy with Inferno (damages every enemy whenever its owner loses HP on its own turn) and Poison: the first poison tick of the
/// enemy turn damages the enemy, whose Inferno damages it again, and so on: unbounded hook recursion (huge HP, it never dies).
fn arm_inferno_loop(cx: &mut Combat) {
    let e = cx.enemies[0];
    cx.creatures[e as usize].max_hp = 1 << 24;
    cx.creatures[e as usize].hp = 1 << 24;
    cx.apply_power(ids::power::INFERNO_POWER, e, Dec::int(1), e, NO);
    cx.apply_power(ids::power::POISON_POWER, e, Dec::int(3), PLAYER, NO);
    cx.sync_overflow();
    assert_eq!(cx.overflow, 0);
}

/// What a dropped combat may still see: the env / search read the flag, write one observation, maybe clone it; none of it may panic
/// or loop, and the combat takes no further action.
fn assert_tripped_and_inert(cx: &mut Combat) {
    assert!(cx.overflow & ov::LOOP != 0, "overflow = {:#x}", cx.overflow);
    assert_eq!(cx.stage, Stage::Over);
    let mut buf = ActionBuf::new();
    cx.legal_actions(&mut buf);
    assert_eq!(buf.len(), 0, "a tripped combat offers no actions");
    let mut obs = vec![0f32; OBS_SIZE];
    cx.observe(&mut obs);
    let mut copy = cx.clone();
    assert!(!copy.step(Action::EndTurn), "a tripped combat refuses further steps");
    assert!(copy.overflow & ov::LOOP != 0);
}

#[test]
fn hook_recursion_trips_the_depth_guard() {
    let mut cx = combat(starter());
    arm_inferno_loop(&mut cx);
    assert!(cx.step(Action::EndTurn));
    assert_eq!(cx.hook_depth, 0, "every pass entered was left again on the way out");
    assert!(cx.work < 10 * HOOK_DEPTH_LIMIT as u32, "the trip ends the step promptly (work {})", cx.work);
    assert_tripped_and_inert(&mut cx);
}

#[test]
fn replayed_steps_trip_too() {
    // a Stratagem in the deck: every step runs under replay (`engine/replay.rs`)
    let mut deck = starter();
    deck.push(DeckCard { id: ids::card::STRATAGEM, upgrade: 0 });
    let mut cx = combat(deck);
    assert!(cx.strat_possible);
    arm_inferno_loop(&mut cx);
    assert!(cx.step(Action::EndTurn));
    assert!(cx.replay.is_none());
    assert_tripped_and_inert(&mut cx);
}

#[test]
fn lookahead_projection_of_a_loop_terminates() {
    // the observation projects the enemy turn on a copy (`look_turn`): the copy trips, the real combat is untouched
    let mut cx = combat(starter());
    arm_inferno_loop(&mut cx);
    let before = cx.clone();
    let mut obs = vec![0f32; OBS_SIZE];
    cx.observe(&mut obs);
    cx.sync_overflow();
    assert_eq!(cx.overflow, 0, "the projection's trip stays in the projection");
    assert_eq!(cx.stage, before.stage);
    assert_eq!(cx.work_limit, WORK_LIMIT);
}

#[test]
fn auto_ended_turns_trip_the_turn_guard() {
    // Mayhem auto-plays the top card at every turn start; every card is Void Form, which ends the turn: turn after turn recurses
    // inside the one EndTurn step (finite here only because the deck of 60 runs out; far beyond any real step)
    let deck: Vec<DeckCard> = (0..60).map(|_| DeckCard { id: ids::card::VOID_FORM, upgrade: 0 }).collect();
    let mut cx = combat(deck);
    cx.creatures[PLAYER as usize].max_hp = 1 << 24;
    cx.creatures[PLAYER as usize].hp = 1 << 24;
    cx.apply_power(ids::power::MAYHEM_POWER, PLAYER, Dec::int(1), PLAYER, NO);
    cx.apply_power(ids::power::MIND_ROT_POWER, PLAYER, Dec::int(5), NO, NO); // no hand draw: one card per turn
    cx.sync_overflow();
    assert_eq!(cx.overflow, 0);
    let turn = cx.player.turn_number;
    assert!(cx.step(Action::EndTurn));
    assert!(cx.player.turn_number - turn <= TURN_LIMIT as i32 + 1, "turns {} -> {}", turn, cx.player.turn_number);
    assert_tripped_and_inert(&mut cx);
}

#[test]
fn work_budget_cuts_a_step_short() {
    // the work budget itself, on an ordinary end of turn: lowered to a few units, the step stops with the flag
    let mut cx = combat(starter());
    let mut normal = cx.clone();
    assert!(normal.step(Action::EndTurn));
    assert_eq!(normal.overflow, 0);
    assert!(normal.work > 3 && normal.work <= WORK_LIMIT, "work {}", normal.work);
    cx.work_limit = 3;
    assert!(cx.step(Action::EndTurn));
    assert_tripped_and_inert(&mut cx);
}

#[test]
fn ordinary_steps_reset_the_budget() {
    // the counters are per step: a long fight never accumulates toward the limits
    let mut cx = combat(starter());
    let mut steps = 0;
    while cx.stage != Stage::Over && steps < 400 {
        let mut buf = ActionBuf::new();
        cx.legal_actions(&mut buf);
        let a = buf.iter().copied().find(|a| matches!(a, Action::PlayCard { .. })).unwrap_or(buf[0]);
        assert!(cx.step(a));
        assert!(cx.work <= 1000 && cx.step_turns <= 2 && cx.hook_depth == 0, "work {} turns {}", cx.work, cx.step_turns);
        steps += 1;
    }
    assert_eq!(cx.overflow, 0);
}
