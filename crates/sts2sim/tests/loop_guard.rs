//! Runaway-work safeguard (`ov::LOOP`, `engine/budget.rs`): a trigger chain that would never end inside one step (or inside the
//! enemy look-ahead of an observation) is cut short, the step returns with the flag set and the combat is left safe to drop.
//! Most loops are built from real content in states the game never reaches (an enemy carrying the Ironclad's Inferno); the last
//! group is one the game reaches and never ends either (Pillage + Hellraiser + Velvet Choker, E6).
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

// ---- a loop the real game reaches (E6, `docs/research/evidence.md`) ------------------------------------------------------------
// Pillage draws until it draws a non-Attack (or the hand is full); Hellraiser auto-plays every Strike-tagged card as it is drawn;
// Velvet Choker refuses every play past the 6th of the turn, auto-plays included, and a refused auto-play goes to the discard
// without being played. With only Strikes left in the draw and discard piles, Pillage played as the 6th card draws a Strike, the
// refused auto-play discards it, Pillage draws again, the empty draw pile reshuffles it back, and so on: nothing changes and nothing
// ends the loop (no damage, the hand never fills). The game itself has no cap here (`HellraiserPower` caps auto-plays only when
// every enemy has infinite HP; `CardPileCmd.DrawInternal` / `ShuffleIfNecessary` reshuffle as often as asked), so the step never
// ends in the game either. Without the Choker the same chain is a finite combo: the Strikes land until the enemies die.

/// Pillage alone in hand, two Strikes in the discard pile, nothing else to draw, Hellraiser on; with `choker_plays` the Choker has
/// already counted that many plays this turn.
fn pillage_hellraiser(choker_plays: Option<i32>) -> (Combat, Cid) {
    let deck: Vec<DeckCard> = (0..10).map(|_| DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 }).collect();
    let mut relics = vec![RelicInit { id: ids::relic::BURNING_BLOOD, ..Default::default() }];
    if choker_plays.is_some() {
        relics.push(RelicInit { id: ids::relic::VELVET_CHOKER, ..Default::default() });
    }
    let mut cx = Combat::new(&Scenario {
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
        relics,
        potions: vec![],
        rng: RngSet::from_run_seed(3),
    });
    let all: Vec<CardIdx> = cx.player.hand.iter().chain(cx.player.draw.iter()).chain(cx.player.discard.iter()).copied().collect();
    for (k, &c) in all.iter().enumerate() {
        let to = if k < 2 { PileType::Discard } else { PileType::Exhaust };
        cx.move_card(c, to, CardPilePosition::Bottom);
    }
    let p = cx.new_card(ids::card::PILLAGE, 0).unwrap();
    cx.move_card(p, PileType::Hand, CardPilePosition::Bottom);
    cx.apply_power(ids::power::HELLRAISER_POWER, PLAYER, Dec::int(1), PLAYER, NO);
    if let Some(n) = choker_plays {
        let r = cx.player.relics.iter().position(|r| r.id == ids::relic::VELVET_CHOKER).unwrap();
        cx.player.relics[r].counter = n;
    }
    cx.sync_overflow();
    assert_eq!(cx.overflow, 0);
    assert_eq!((cx.player.hand.len(), cx.player.draw.len(), cx.player.discard.len()), (1, 0, 2));
    let e = cx.enemies[0];
    (cx, e)
}

/// Enemies too big to die from a few Strikes (1,000 HP, no block); returns them.
fn big_enemies(cx: &mut Combat) -> Vec<Cid> {
    let enemies: Vec<Cid> = cx.enemies.iter().copied().collect();
    for &m in &enemies {
        cx.creatures[m as usize].max_hp = 1000;
        cx.creatures[m as usize].hp = 1000;
        cx.creatures[m as usize].block = 0;
    }
    enemies
}

fn hp_lost(cx: &Combat, enemies: &[Cid]) -> i32 {
    enemies.iter().map(|&m| 1000 - cx.cr(m).hp).sum()
}

#[test]
fn pillage_hellraiser_velvet_choker_loops_forever_and_trips() {
    // Pillage as the 6th play of the turn. It counts for the Choker only once it has resolved (`VelvetChoker.AfterCardPlayed`), so the
    // first auto-played Strike still lands (the 6th count); every later one is refused and discarded unplayed, for ever
    let (mut cx, e) = pillage_hellraiser(Some(5));
    let enemies = big_enemies(&mut cx);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.hook_depth, 0);
    assert!(cx.work > WORK_LIMIT, "the work budget, not the depth guard, ends this flat loop (work {})", cx.work);
    assert_eq!(hp_lost(&cx, &enemies), 6 + 6, "Pillage's hit and one Strike, then nothing more ever happens");
    assert_tripped_and_inert(&mut cx);
}

#[test]
fn pillage_hellraiser_without_the_choker_is_a_finite_combo() {
    // the same chain when the auto-plays land: the Strikes kill the enemies and the step ends with the fight won (no trip)
    let (mut cx, e) = pillage_hellraiser(None);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(cx.overflow, 0);
    assert_eq!(cx.stage, Stage::Over);
    assert_eq!(cx.outcome, Outcome::Victory);
    assert!(cx.work < 1000, "work {}", cx.work);
}

#[test]
fn pillage_hellraiser_choker_closing_mid_chain_starts_the_loop() {
    // the search's case: Pillage early in the turn (the 1st play here), the Choker lets auto-played Strikes through until it has
    // counted 6 plays and then closes mid-chain; if they have not won the fight by then, the refused-Strike loop follows
    let (mut cx, e) = pillage_hellraiser(Some(0));
    let enemies = big_enemies(&mut cx);
    assert!(cx.step(Action::PlayCard { hand_pos: 0, target: e }));
    assert_eq!(hp_lost(&cx, &enemies), 6 + 6 * 6, "Pillage's hit and exactly 6 Strikes landed before the Choker closed");
    assert_tripped_and_inert(&mut cx);
}
