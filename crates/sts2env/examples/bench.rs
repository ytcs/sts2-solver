//! Batched random-policy throughput: N envs, masked-random actions, obs+mask written every step.
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
        max_energy: 3, orb_slots: 0, potion_slots: 2, deck, relics: vec![RelicInit { id: ids::relic::BURNING_BLOOD, counter: 0 }],
        potions: vec![], rng: RngSet::from_run_seed(0),
    };
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(10_000);
    let mut env = BatchEnv::new(n, Box::new(FixedScenario(sc)), RewardConfig::default(), 2000, 1);
    let mut obs = vec![0f32; n * OBS];
    let mut mask = vec![0u8; n * ACTIONS];
    let (mut reward, mut done, mut outcome, mut illegal) = (vec![0f32; n], vec![0u8; n], vec![0i8; n], vec![0u8; n]);
    env.observe_all(&mut obs, &mut mask);
    let mut pol = Rng::new(7);
    let mut actions = vec![0i32; n];
    let steps = 200;
    let t = Instant::now();
    let (mut episodes, mut wins) = (0u64, 0u64);
    for _ in 0..steps {
        for i in 0..n {
            let row = &mask[i * ACTIONS..(i + 1) * ACTIONS];
            let k = row.iter().filter(|&&m| m != 0).count().max(1);
            let mut pick = pol.next_int(k as i32) as usize;
            let mut a = 0;
            for (j, &m) in row.iter().enumerate() { if m != 0 { if pick == 0 { a = j; break; } pick -= 1; } }
            actions[i] = a as i32;
        }
        env.step(&actions, StepOut { obs: &mut obs, mask: &mut mask, reward: &mut reward, done: &mut done, outcome: &mut outcome, illegal: &mut illegal });
        for i in 0..n { if done[i] != 0 { episodes += 1; if outcome[i] == OUTCOME_WIN { wins += 1; } } }
        assert!(illegal.iter().all(|&x| x == 0));
    }
    let dt = t.elapsed().as_secs_f64();
    println!("{n} envs x {steps} steps in {dt:.2}s = {:.2}M env-steps/s (incl. python-side-equivalent random policy); {episodes} episodes, {wins} wins",
        (n * steps) as f64 / dt / 1e6);
}
