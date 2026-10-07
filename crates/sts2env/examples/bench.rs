//! Batched random-policy throughput: N envs, masked-random actions, obs+mask written every step.
use rayon::prelude::*;
use std::time::Instant;
use sts2env::*;
use sts2sim::ids;
use sts2sim::rng::Rng;
use sts2sim::state::RngSet;
use sts2sim::{DeckCard, RelicInit, Scenario};

fn main() {
    let mut deck = vec![];
    for _ in 0..5 { deck.push(DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 }); }
    for _ in 0..4 { deck.push(DeckCard { id: ids::card::DEFEND_IRONCLAD, upgrade: 0 }); }
    deck.push(DeckCard { id: ids::card::BASH, upgrade: 0 });
    deck.push(DeckCard { id: ids::card::ASCENDERS_BANE, upgrade: 0 });
    let sc = Scenario {
        run_seed: 0, total_floor: 1, character: 0, ascension: 10, encounter: ids::encounter::NIBBITS_WEAK, max_hp: 80, hp: 80,
        max_energy: 3, orb_slots: 0, potion_slots: 2, deck, relics: vec![RelicInit { id: ids::relic::BURNING_BLOOD, counter: 0, ..Default::default() }],
        potions: vec![], rng: RngSet::from_run_seed(0),
    };
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(10_000);
    // BENCH_SCENARIOS=data/train/eval.json: the episodes cycle through a scenario list (the training mix) instead of one Nibbit fight
    let source: Box<dyn ScenarioSource> = match std::env::var("BENCH_SCENARIOS") {
        Ok(path) => {
            let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            let pool: Vec<_> = v.as_array().unwrap().iter().filter_map(|sj| sts2diff::convert::scenario_ex(sj).ok()).filter(|(sc, _)| sc.validate().is_ok()).collect();
            Box::new(PoolScenario::with_extras(pool))
        }
        Err(_) => Box::new(FixedScenario(sc)),
    };
    let mut env = BatchEnv::new(n, source, RewardConfig::default(), 2000, 1);
    let mut obs = vec![0f32; n * OBS];
    let mut mask = vec![0u8; n * ACTIONS];
    let (mut reward, mut done, mut outcome, mut illegal) = (vec![0f32; n], vec![0u8; n], vec![0i8; n], vec![0u8; n]);
    env.observe_all(&mut obs, &mut mask).unwrap();
    let mut actions = vec![0i32; n];
    let steps = 200;
    let serial = std::env::var("BENCH_SERIAL_POLICY").is_ok();
    let t = Instant::now();
    let (mut episodes, mut wins) = (0u64, 0u64);
    for step in 0..steps {
        // masked-random policy; parallel (like a GPU / vectorised policy would be) unless BENCH_SERIAL_POLICY is set
        let pick_action = |i: usize, a: &mut i32, row: &[u8]| {
            let mut pol = Rng::new(7 ^ ((step as u64) << 32) ^ i as u64);
            let k = row.iter().filter(|&&m| m != 0).count().max(1);
            let mut pick = pol.next_int(k as i32) as usize;
            for (j, &m) in row.iter().enumerate() {
                if m != 0 {
                    if pick == 0 {
                        *a = j as i32;
                        break;
                    }
                    pick -= 1;
                }
            }
        };
        if serial {
            for (i, (a, row)) in actions.iter_mut().zip(mask.chunks(ACTIONS)).enumerate() {
                pick_action(i, a, row);
            }
        } else {
            actions.par_iter_mut().zip(mask.par_chunks(ACTIONS)).enumerate().for_each(|(i, (a, row))| pick_action(i, a, row));
        }
        env.step(&actions, StepOut { obs: &mut obs, mask: &mut mask, reward: &mut reward, done: &mut done, outcome: &mut outcome, illegal: &mut illegal }).unwrap();
        for i in 0..n { if done[i] != 0 { episodes += 1; if outcome[i] == OUTCOME_WIN { wins += 1; } } }
        assert!(illegal.iter().all(|&x| x == 0));
    }
    let dt = t.elapsed().as_secs_f64();
    println!("{n} envs x {steps} steps in {dt:.2}s = {:.2}M env-steps/s (incl. python-side-equivalent random policy); {episodes} episodes, {wins} wins",
        (n * steps) as f64 / dt / 1e6);
}
