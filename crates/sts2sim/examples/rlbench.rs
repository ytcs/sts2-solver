//! RL-realistic throughput: random policy over legal actions + a full observation written every step.
use std::time::Instant;
use sts2sim::engine::ActionBuf;
use sts2sim::ids;
use sts2sim::observe::OBS_SIZE;
use sts2sim::rng::Rng;
use sts2sim::state::*;
use sts2sim::*;

fn scenario(seed: u64) -> Scenario {
    let mut deck = vec![];
    for _ in 0..5 { deck.push(DeckCard { id: ids::card::STRIKE_IRONCLAD, upgrade: 0 }); }
    for _ in 0..4 { deck.push(DeckCard { id: ids::card::DEFEND_IRONCLAD, upgrade: 0 }); }
    deck.push(DeckCard { id: ids::card::BASH, upgrade: 0 });
    Scenario {
        run_seed: 0,
        total_floor: 1,
        character: 0,
        ascension: 0, encounter: ids::encounter::NIBBITS_WEAK, max_hp: 80, hp: 80, max_energy: 3, orb_slots: 0,
        potion_slots: 3, deck, relics: vec![RelicInit { id: ids::relic::BURNING_BLOOD, ..Default::default() }],
        potions: vec![], rng: RngSet::from_run_seed(seed),
    }
}

fn run(n: u64, seed0: u64) -> u64 {
    let mut sc = scenario(0);
    let mut obs = vec![0f32; OBS_SIZE];
    let mut buf = ActionBuf::new();
    let mut steps = 0;
    let mut pol = Rng::new(seed0);
    for i in 0..n {
        sc.rng = RngSet::from_run_seed(seed0 + i); // per-episode RNG; reset = Combat::new (no allocation)
        let mut cx = Combat::new(&sc);
        while cx.stage != Stage::Over {
            cx.observe(&mut obs);
            cx.legal_actions(&mut buf);
            let a = buf[pol.next_int(buf.len() as i32) as usize];
            cx.step(a);
            steps += 1;
            if steps > 100_000_000 { break; }
        }
    }
    steps
}

fn main() {
    let n = 20_000;
    let t = Instant::now();
    let steps = run(n, 1);
    let dt = t.elapsed().as_secs_f64();
    println!("1 thread : {n} fights, {steps} steps in {dt:.2}s = {:.2}M steps/s (obs+legal+step)", steps as f64 / dt / 1e6);
    let th = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(1);
    let t = Instant::now();
    let hs: Vec<_> = (0..th).map(|k| std::thread::spawn(move || run(n, 1 + k as u64 * 1_000_000))).collect();
    let steps: u64 = hs.into_iter().map(|h| h.join().unwrap()).sum();
    let dt = t.elapsed().as_secs_f64();
    println!("{th} threads: {steps} steps in {dt:.2}s = {:.2}M steps/s", steps as f64 / dt / 1e6);
}
