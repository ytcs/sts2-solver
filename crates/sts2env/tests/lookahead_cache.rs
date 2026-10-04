//! The lookahead cache must never change an observation: over many mid-fight states of the training mix, cached == fresh for every enemy,
//! including states reached after the cache was filled by other (similar) states.
use sts2sim::engine::ActionBuf;
use sts2sim::rng::Rng;
use sts2sim::state::RngSet;
use sts2sim::Combat;

#[test]
fn cached_lookahead_equals_fresh() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/train/eval.json");
    let Ok(txt) = std::fs::read_to_string(path) else { return };
    let v: serde_json::Value = serde_json::from_str(&txt).unwrap();
    let mut rng = Rng::new(9);
    let (mut checked, mut diffs) = (0u64, 0u64);
    for (i, sj) in v.as_array().unwrap().iter().enumerate().take(500) {
        let Ok((sc, ex)) = sts2diff::convert::scenario_ex(sj) else { continue };
        if sc.validate().is_err() {
            continue;
        }
        let mut cx = Combat::try_new_with(&sc, &ex).unwrap();
        cx.reset_validated(&sc, &ex, i as u64 + 1, RngSet::from_run_seed_fast(i as u64 + 1)).unwrap();
        for _ in 0..80 {
            let mut buf = ActionBuf::new();
            cx.legal_actions(&mut buf);
            if buf.is_empty() {
                break;
            }
            for &e in cx.enemies.clone().iter() {
                let (a, b) = (cx.lookahead(e), cx.lookahead_fresh(e));
                for h in 0..a.len() {
                    checked += 1;
                    if a[h].prob != b[h].prob || a[h].exp_damage != b[h].exp_damage {
                        diffs += 1;
                    }
                }
            }
            if !cx.step(buf[rng.next_int(buf.len() as i32) as usize]) {
                break;
            }
        }
    }
    assert!(checked > 10_000, "only {checked} rows checked");
    assert_eq!(diffs, 0, "{diffs} of {checked} lookahead rows differ");
}
