//! Cost and coverage of the enemy look-ahead (`Combat::lookahead_fresh`, uncached) over mid-fight states of a scenario list (random
//! play): microseconds per look-ahead, the most expensive monsters, and how many rows lose probability mass (a row sums to less than
//! 1 when the projected monster dies, the projected fight ends, or the path cap drops unlikely paths).
//!   cargo run --release -p sts2env --example lookprof -- data/train/eval.json
use std::collections::BTreeMap;
use std::time::Instant;
use sts2sim::engine::{ActionBuf, LOOK_H};
use sts2sim::rng::Rng;
use sts2sim::state::RngSet;
use sts2sim::Combat;

fn main() {
    let path = std::env::args().nth(1).expect("scenario json (list)");
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut states: Vec<Combat> = vec![];
    let mut rng = Rng::new(5);
    for (i, sj) in v.as_array().unwrap().iter().enumerate().take(600) {
        let Ok((sc, ex)) = sts2diff::convert::scenario_ex(sj) else { continue };
        if sc.validate().is_err() {
            continue;
        }
        let mut cx = Combat::try_new_with(&sc, &ex).unwrap();
        cx.reset_validated(&sc, &ex, i as u64 + 1, RngSet::from_run_seed_fast(i as u64 + 1)).unwrap();
        for step in 0..120 {
            let mut buf = ActionBuf::new();
            cx.legal_actions(&mut buf);
            if buf.is_empty() {
                break;
            }
            let a = buf[rng.next_int(buf.len() as i32) as usize];
            if !cx.step(a) {
                break;
            }
            if step % 6 == 3 {
                states.push(cx.clone());
            }
        }
    }
    let mut per: BTreeMap<u16, (u32, f64)> = BTreeMap::new();
    let (mut n, mut low) = (0u64, [0u64; LOOK_H]);
    let t = Instant::now();
    for s in &states {
        for &e in s.enemies.iter() {
            if !s.cr(e).is_alive() || !s.cr(e).in_combat {
                continue;
            }
            let t0 = Instant::now();
            let rows = s.lookahead_fresh(e);
            let x = per.entry(s.cr(e).monster.id).or_default();
            x.0 += 1;
            x.1 += t0.elapsed().as_secs_f64();
            n += 1;
            for (h, r) in rows.iter().enumerate() {
                if r.prob.iter().sum::<f32>() < 0.95 {
                    low[h] += 1;
                }
            }
        }
    }
    let dt = t.elapsed().as_secs_f64();
    println!("{} states, {n} look-aheads: {:.2} us each, {:.2} us per state", states.len(), dt / n as f64 * 1e6, dt / states.len() as f64 * 1e6);
    println!("rows summing to < 0.95, by turn: {:?} of {n}", low);
    let mut v: Vec<_> = per.into_iter().collect();
    v.sort_by(|a, b| b.1 .1.partial_cmp(&a.1 .1).unwrap());
    for (id, (k, t)) in v.iter().take(12) {
        println!("  {:<28} {:5} calls {:8.2} us each  {:5.1}% of the time", sts2sim::ids::monster::NAMES[*id as usize], k, t / *k as f64 * 1e6, 100.0 * t / dt);
    }
}
