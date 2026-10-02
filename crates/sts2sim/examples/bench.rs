//! Throughput smoke test: greedy-policy fights, single thread and rayon-free multi-thread (std threads).
use std::time::Instant;
use sts2sim::ids;
use sts2sim::state::*;
use sts2sim::types::*;
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
        potion_slots: 3, deck, relics: vec![RelicInit { id: ids::relic::BURNING_BLOOD, counter: 0 }],
        potions: vec![], rng: RngSet::from_run_seed(seed),
    }
}

fn play_out(mut cx: Combat) -> (u64, Outcome) {
    let mut steps = 0u64;
    while cx.stage != Stage::Over {
        let e = cx.enemies.first().unwrap_or(NO);
        let mut done = false;
        for pos in 0..cx.player.hand.len() {
            let c = cx.player.hand[pos];
            if cx.can_play(c) {
                let t = if cx.card_def(c).target == TargetType::AnyEnemy { e } else { NO };
                cx.step(Action::PlayCard { hand_pos: pos as u8, target: t });
                done = true;
                break;
            }
        }
        if !done { cx.step(Action::EndTurn); }
        steps += 1;
    }
    (steps, cx.outcome)
}

fn main() {
    println!("size_of::<Combat>() = {} bytes", std::mem::size_of::<Combat>());
    let n: u64 = std::env::var("BENCH_N").ok().and_then(|s| s.parse().ok()).unwrap_or(200_000);
    let sc = scenario(0);
    let t = Instant::now();
    let mut steps = 0;
    for i in 0..n {
        let mut s = sc.clone();
        s.rng = RngSet::from_run_seed(i);
        let (st, _) = play_out(Combat::new(&s));
        steps += st;
    }
    let dt = t.elapsed().as_secs_f64();
    println!("1 thread: {n} fights in {dt:.2}s = {:.0} fights/s, {:.2}M steps/s", n as f64 / dt, steps as f64 / dt / 1e6);
    if std::env::var("BENCH_SINGLE").is_ok() { return; }
    let threads = std::thread::available_parallelism().map(|x| x.get()).unwrap_or(1);
    let t = Instant::now();
    let hs: Vec<_> = (0..threads).map(|k| {
        let sc = sc.clone();
        std::thread::spawn(move || {
            let mut steps = 0u64;
            for i in 0..n { let mut s = sc.clone(); s.rng = RngSet::from_run_seed(i + k as u64 * n); steps += play_out(Combat::new(&s)).0; }
            steps
        })
    }).collect();
    let steps: u64 = hs.into_iter().map(|h| h.join().unwrap()).sum();
    let dt = t.elapsed().as_secs_f64();
    println!("{threads} threads: {} fights in {dt:.2}s = {:.0} fights/s, {:.2}M steps/s", n * threads as u64, (n * threads as u64) as f64 / dt, steps as f64 / dt / 1e6);
}
