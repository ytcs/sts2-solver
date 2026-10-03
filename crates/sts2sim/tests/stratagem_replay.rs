//! Stratagem's reshuffle prompt in a draw that is not the last statement of a card (Battle Trance): the step runs under replay
//! (`engine/replay.rs`). The agent sees the effect's partial results at the prompt; its answer re-runs the step.
use sts2sim::dec::Dec;
use sts2sim::ids;
use sts2sim::observe::OBS_SIZE;
use sts2sim::state::*;
use sts2sim::types::*;
use sts2sim::*;

fn combat(seed: u64) -> Combat {
    let mut deck = vec![DeckCard { id: ids::card::BATTLE_TRANCE, upgrade: 0 }, DeckCard { id: ids::card::STRATAGEM, upgrade: 0 }];
    deck.extend((0..5).map(|_| DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 }));
    let mut cx = Combat::new(&Scenario {
        run_seed: seed,
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
        relics: vec![],
        potions: vec![],
        rng: RngSet::from_run_seed(seed),
    });
    cx.apply_power(ids::power::STRATAGEM_POWER, PLAYER, Dec::int(1), PLAYER, NO);
    cx
}

fn obs(cx: &Combat) -> Vec<f32> {
    let mut v = vec![0.0; OBS_SIZE];
    cx.observe(&mut v);
    v
}

fn hand_pos(cx: &Combat, id: u16) -> Option<usize> {
    cx.player.hand.iter().position(|&c| cx.cards[c as usize].id == id)
}

/// A fight whose hand holds Battle Trance and two Strikes: after playing both Strikes the discard pile has two cards and the draw
/// pile two, so Battle Trance's third card comes after a reshuffle that offers a real choice.
fn setup() -> Combat {
    for seed in 0..500 {
        let mut cx = combat(seed);
        let strikes = cx.player.hand.iter().filter(|&&c| cx.cards[c as usize].id == ids::card::STRIKE_IRONCLAD).count();
        if hand_pos(&cx, ids::card::BATTLE_TRANCE).is_none() || strikes < 2 || cx.player.draw.len() != 2 {
            continue;
        }
        let e = cx.enemies[0];
        for _ in 0..2 {
            let p = hand_pos(&cx, ids::card::STRIKE_IRONCLAD).unwrap();
            assert!(cx.step(Action::PlayCard { hand_pos: p as u8, target: e }));
        }
        return cx;
    }
    panic!("no seed with the wanted opening hand");
}

#[test]
fn battle_trance_reshuffle_prompt_shows_the_partial_draw_then_replays() {
    let mut cx = setup();
    let hand0 = cx.player.hand.len();
    let energy = cx.player.energy;
    let before = obs(&cx);
    let bt = hand_pos(&cx, ids::card::BATTLE_TRANCE).unwrap();
    assert!(cx.step(Action::PlayCard { hand_pos: bt as u8, target: NO }));

    // the prompt: two cards were drawn before the draw pile ran out, the discard pile was shuffled in, a pick is asked
    assert_eq!(cx.stage, Stage::AwaitChoice);
    assert!(cx.missing.is_none() && cx.overflow == 0);
    assert!(cx.replay.is_some());
    let d = cx.decision.as_ref().expect("prompt");
    assert_eq!((d.min, d.max, d.cands.len()), (1, 1, 2));
    assert_eq!(cx.player.hand.len(), hand0 - 1 + 2, "Battle Trance left the hand, two cards are drawn");
    assert_eq!(cx.player.draw.len(), 2, "the shuffled discard pile");
    assert_eq!(cx.player.energy, energy);
    assert_ne!(obs(&cx), before, "the agent sees the intermediate state, not the state the step began in");

    // a clone of the prompt answers on its own (search copies states at prompts)
    let mut other = cx.clone();
    assert!(other.step(Action::Pick { idx: 1 }));

    assert!(cx.step(Action::Pick { idx: 0 }));
    assert_eq!(cx.stage, Stage::AwaitAction);
    assert!(cx.replay.is_none() && cx.missing.is_none() && cx.overflow == 0);
    // 2 drawn before the shuffle + 1 chosen + the third card of the draw
    assert_eq!(cx.player.hand.len(), hand0 - 1 + 2 + 1 + 1);
    assert_eq!(cx.player.energy, energy);
    assert_eq!(other.player.hand.len(), cx.player.hand.len());
    assert_eq!(other.stage, Stage::AwaitAction);
}

#[test]
fn the_prompt_only_accepts_picks_and_the_state_is_unchanged_by_illegal_actions() {
    let mut cx = setup();
    let bt = hand_pos(&cx, ids::card::BATTLE_TRANCE).unwrap();
    assert!(cx.step(Action::PlayCard { hand_pos: bt as u8, target: NO }));
    let seen = obs(&cx);
    assert!(!cx.step(Action::EndTurn));
    assert!(!cx.step(Action::PlayCard { hand_pos: 0, target: NO }));
    assert!(!cx.step(Action::Confirm), "exactly one pick is required");
    assert_eq!(obs(&cx), seen);
    assert!(cx.step(Action::Pick { idx: 1 }));
    assert_eq!(cx.stage, Stage::AwaitAction);
}
