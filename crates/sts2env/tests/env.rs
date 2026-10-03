//! Batch env hardening: bad inputs are errors (never panics), outcome codes, and a stress run over many envs on the worker pool.
use sts2env::*;
use sts2sim::ids;
use sts2sim::rng::Rng;
use sts2sim::state::RngSet;
use sts2sim::{DeckCard, RelicInit, Scenario};

fn scenario(n_deck: usize) -> Scenario {
    let mut deck = vec![];
    for i in 0..n_deck {
        deck.push(DeckCard { id: if i % 2 == 0 { ids::card::STRIKE_IRONCLAD } else { ids::card::DEFEND_IRONCLAD }, upgrade: 0 });
    }
    Scenario {
        run_seed: 0,
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
        rng: RngSet::from_run_seed(0),
    }
}

#[test]
fn invalid_scenario_is_an_error_not_a_panic() {
    let r = BatchEnv::try_new(4, Box::new(FixedScenario(scenario(200))), RewardConfig::default(), 100, 1);
    assert!(matches!(r, Err(EnvError::Scenario(_))));
}

#[test]
fn short_buffers_are_errors() {
    let n = 8;
    let mut env = BatchEnv::new(n, Box::new(FixedScenario(scenario(10))), RewardConfig::default(), 100, 1);
    let mut obs = vec![0f32; n * OBS];
    let mut mask = vec![0u8; n * ACTIONS];
    assert!(env.observe_all(&mut obs, &mut mask).is_ok());
    assert!(matches!(env.observe_all(&mut obs[..n * OBS - 1], &mut mask), Err(EnvError::Buffer(_))));
    assert!(matches!(env.observe_all(&mut obs, &mut mask[..n * ACTIONS - 1]), Err(EnvError::Buffer(_))));
    let (mut reward, mut done, mut outcome, mut illegal) = (vec![0f32; n], vec![0u8; n], vec![0i8; n], vec![0u8; n]);
    let actions = vec![0i32; n - 1];
    let r = env.step(&actions, StepOut { obs: &mut obs, mask: &mut mask, reward: &mut reward, done: &mut done, outcome: &mut outcome, illegal: &mut illegal });
    assert!(matches!(r, Err(EnvError::Buffer(_))));
}

#[test]
fn out_of_range_actions_are_illegal_not_fatal() {
    let n = 4;
    let mut env = BatchEnv::new(n, Box::new(FixedScenario(scenario(10))), RewardConfig::default(), 100, 1);
    let mut obs = vec![0f32; n * OBS];
    let mut mask = vec![0u8; n * ACTIONS];
    env.observe_all(&mut obs, &mut mask).unwrap();
    let (mut reward, mut done, mut outcome, mut illegal) = (vec![0f32; n], vec![0u8; n], vec![0i8; n], vec![0u8; n]);
    let actions = vec![-5, ACTIONS as i32 + 100, i32::MAX, ACTIONS as i32 - 1];
    env.step(&actions, StepOut { obs: &mut obs, mask: &mut mask, reward: &mut reward, done: &mut done, outcome: &mut outcome, illegal: &mut illegal }).unwrap();
    assert!(illegal.iter().all(|&x| x == 1));
}

/// Many envs on the worker pool with a masked random policy: no panic / stack overflow, every outcome code is one of the known
/// ones, and finished episodes restart (done -> fresh observation with a non-empty mask).
#[test]
fn stress_random_policy() {
    let n = 2048;
    let mut env = BatchEnv::new(n, Box::new(FixedScenario(scenario(11))), RewardConfig::default(), 400, 5);
    let mut obs = vec![0f32; n * OBS];
    let mut mask = vec![0u8; n * ACTIONS];
    env.observe_all(&mut obs, &mut mask).unwrap();
    let (mut reward, mut done, mut outcome, mut illegal) = (vec![0f32; n], vec![0u8; n], vec![0i8; n], vec![0u8; n]);
    let mut actions = vec![0i32; n];
    let mut r = Rng::new(3);
    let mut episodes = 0;
    for _ in 0..120 {
        for i in 0..n {
            let row = &mask[i * ACTIONS..(i + 1) * ACTIONS];
            let k = row.iter().filter(|&&m| m != 0).count();
            assert!(k > 0, "env {i} has no legal action");
            let mut pick = r.next_int(k as i32) as usize;
            for (j, &m) in row.iter().enumerate() {
                if m != 0 {
                    if pick == 0 {
                        actions[i] = j as i32;
                        break;
                    }
                    pick -= 1;
                }
            }
        }
        env.step(&actions, StepOut { obs: &mut obs, mask: &mut mask, reward: &mut reward, done: &mut done, outcome: &mut outcome, illegal: &mut illegal }).unwrap();
        assert!(illegal.iter().all(|&x| x == 0));
        for i in 0..n {
            if done[i] != 0 {
                episodes += 1;
                assert!([OUTCOME_WIN, OUTCOME_LOSS, OUTCOME_TRUNCATED, OUTCOME_UNIMPLEMENTED, OUTCOME_OVERFLOW].contains(&outcome[i]));
                assert_ne!(outcome[i], OUTCOME_UNIMPLEMENTED);
                assert_ne!(outcome[i], OUTCOME_OVERFLOW, "starter fights must not overflow any capacity");
            }
        }
    }
    assert!(episodes > n);
}
