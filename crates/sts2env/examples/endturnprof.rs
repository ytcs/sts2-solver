//! Callgrind target: the cost of the turn-ending step (enemy turn + next draw) and of an observation on mid-fight states.
//!   CARGO_PROFILE_RELEASE_DEBUG=true cargo build --release -p sts2env --example endturnprof && valgrind --tool=callgrind target/release/examples/endturnprof data/train/eval.json
use sts2sim::engine::ActionBuf;
use sts2sim::observe::OBS_SIZE;
use sts2sim::rng::Rng;
use sts2sim::state::RngSet;
use sts2sim::{Action, Combat};

fn main() {
    let path = std::env::args().nth(1).expect("scenario json (list)");
    let n_scen: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(60);
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut rng = Rng::new(5);
    let mut states: Vec<Combat> = vec![];
    for (i, sj) in v.as_array().unwrap().iter().enumerate().take(n_scen) {
        let Ok((sc, ex)) = sts2diff::convert::scenario_ex(sj) else { continue };
        if sc.validate().is_err() {
            continue;
        }
        let mut cx = Combat::try_new_with(&sc, &ex).unwrap();
        cx.reset_validated(&sc, &ex, i as u64 + 1, RngSet::from_run_seed_fast(i as u64 + 1)).unwrap();
        for step in 0..60 {
            let mut buf = ActionBuf::new();
            cx.legal_actions(&mut buf);
            if buf.is_empty() {
                break;
            }
            if step % 5 == 2 {
                states.push(cx.clone());
            }
            if !cx.step(buf[rng.next_int(buf.len() as i32) as usize]) {
                break;
            }
        }
    }
    let mut out = vec![0f32; OBS_SIZE];
    let mut n = 0;
    for s in &states {
        let mut c = s.clone();
        c.determinize(3);
        let mut buf = ActionBuf::new();
        c.legal_actions(&mut buf);
        if buf.iter().any(|a| matches!(a, Action::EndTurn)) && c.step(Action::EndTurn) {
            c.observe_ex(&mut out, None);
            n += 1;
        }
    }
    println!("{} states, {} end-turn steps + observations", states.len(), n);
}
