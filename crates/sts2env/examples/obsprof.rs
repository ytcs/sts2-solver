//! Where does `Combat::observe_ex` spend its time? Plays scenarios with a random policy, keeps mid-fight states, times the observation per section
//! (build with `--features sts2sim/obs_prof`; single threaded).   cargo run --release -p sts2env --features sts2sim/obs_prof --example obsprof -- scenarios.json
use std::time::Instant;
use sts2sim::engine::ActionBuf;
use sts2sim::observe::OBS_SIZE;
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
    println!("{} states", states.len());
    let mut out = vec![0f32; OBS_SIZE];
    let reps = 20;
    let t = Instant::now();
    for _ in 0..reps {
        for s in &states {
            s.observe_ex(&mut out, None);
        }
    }
    let dt = t.elapsed().as_secs_f64();
    let n = (reps * states.len()) as f64;
    println!("observe_ex: {:.2} us per call", dt / n * 1e6);
    #[cfg(feature = "obs_prof")]
    unsafe {
        let p = sts2sim::observe::OBS_PROF;
        let tot: u64 = p.iter().sum();
        for (name, k) in [("hand cards", 1), ("piles", 2), ("enemies (intents)", 3), ("lookahead", 4), ("player+relics", 5)] {
            println!("  {name:<20} {:8.0} cycles/call  {:5.1}% of the profiled sections", p[k] as f64 / n, 100.0 * p[k] as f64 / tot as f64);
        }
        println!("  total profiled {:.0} cycles/call", tot as f64 / n);
    }
    let t = Instant::now();
    for _ in 0..reps {
        for s in &states {
            let mut b = ActionBuf::new();
            s.legal_actions(&mut b);
        }
    }
    println!("legal_actions: {:.2} us per call", t.elapsed().as_secs_f64() / n * 1e6);
    let t = Instant::now();
    for _ in 0..reps {
        for s in &states {
            let mut c = s.clone();
            c.determinize(7);
        }
    }
    println!("clone + determinize: {:.2} us per call", t.elapsed().as_secs_f64() / n * 1e6);
}
