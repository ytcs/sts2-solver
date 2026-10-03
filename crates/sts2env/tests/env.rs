//! Batch env hardening: bad inputs are errors (never panics), outcome codes, and a stress run over many envs on the worker pool.
use sts2env::*;
use sts2sim::ids;
use sts2sim::rng::Rng;
use sts2sim::state::RngSet;
use sts2sim::{DeckCard, RelicInit, Scenario, ScenarioExtras};

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

/// A fork copies the fight (same observation: the hidden state is resampled, not the visible one), plays on independently, and with
/// auto-reset off a finished fight stays finished and ignores actions.
#[test]
fn fork_copies_the_visible_state_and_frozen_slots_stay_finished() {
    let mut main = BatchEnv::new(2, Box::new(FixedScenario(scenario(11))), RewardConfig::default(), 400, 5);
    let n = 16;
    let mut sim = BatchEnv::new(n, Box::new(FixedScenario(scenario(11))), RewardConfig::default(), 400, 9);
    sim.set_autoreset(false);
    let mut obs0 = vec![0f32; 2 * OBS];
    let mut mask0 = vec![0u8; 2 * ACTIONS];
    main.observe_all(&mut obs0, &mut mask0).unwrap();
    let src: Vec<u32> = (0..n).map(|k| (k % 2) as u32).collect();
    let dst: Vec<u32> = (0..n as u32).collect();
    let seeds: Vec<u64> = (0..n as u64).map(|k| 100 + k).collect();
    sim.fork_from(&main, &src, &dst, &seeds).unwrap();
    let mut obs = vec![0f32; n * OBS];
    let mut mask = vec![0u8; n * ACTIONS];
    sim.observe_all(&mut obs, &mut mask).unwrap();
    for k in 0..n {
        let s = (k % 2) * OBS;
        assert!(obs[k * OBS..(k + 1) * OBS] == obs0[s..s + OBS], "fork {k} sees a different state");
        assert!(mask[k * ACTIONS..(k + 1) * ACTIONS] == mask0[(k % 2) * ACTIONS..(k % 2 + 1) * ACTIONS]);
    }
    assert!(sim.fork_from(&main, &src, &dst[..n - 1], &seeds).is_err());
    assert!(sim.fork_from(&main, &[5], &[0], &[1]).is_err());
    // play every fork to the end with the first legal action; finished ones stay done with a fixed outcome
    let (mut reward, mut done, mut outcome, mut illegal) = (vec![0f32; n], vec![0u8; n], vec![0i8; n], vec![0u8; n]);
    let mut seen = vec![0i8; n];
    for _ in 0..600 {
        let actions: Vec<i32> = (0..n).map(|k| mask[k * ACTIONS..(k + 1) * ACTIONS].iter().position(|&m| m != 0).unwrap_or(0) as i32).collect();
        sim.step(&actions, StepOut { obs: &mut obs, mask: &mut mask, reward: &mut reward, done: &mut done, outcome: &mut outcome, illegal: &mut illegal }).unwrap();
        for k in 0..n {
            if seen[k] != 0 {
                assert!(done[k] == 1 && outcome[k] == seen[k], "a finished fork changed");
            } else if done[k] == 1 {
                seen[k] = outcome[k];
            }
        }
    }
    assert!(seen.iter().all(|&o| o != 0), "every fork finished");
}

/// Deck enchantments of a scenario reach the combats the env starts (the first episode and every auto-reset).
#[test]
fn pool_scenario_applies_deck_enchantments() {
    use sts2sim::DeckExtra;
    let mut ex = ScenarioExtras::default();
    ex.deck = vec![DeckExtra::default(); 10];
    ex.deck[0] = DeckExtra { enchant: (ids::enchantment::SHARP + 1) as u8, enchant_amount: 3, ..Default::default() };
    let sc = scenario(10);
    let n = 8;
    let mut env = BatchEnv::new(n, Box::new(PoolScenario::with_extras(vec![(sc.clone(), ex)])), RewardConfig::default(), 50, 1);
    let mut plain = BatchEnv::new(n, Box::new(PoolScenario::new(vec![sc])), RewardConfig::default(), 50, 1);
    let (mut a, mut ma) = (vec![0f32; n * OBS], vec![0u8; n * ACTIONS]);
    let (mut b, mut mb) = (vec![0f32; n * OBS], vec![0u8; n * ACTIONS]);
    env.observe_all(&mut a, &mut ma).unwrap();
    plain.observe_all(&mut b, &mut mb).unwrap();
    // some hand shows the enchanted card's larger damage preview: the observations differ somewhere
    assert!(a != b, "the enchantment never showed up in an observation");
}
