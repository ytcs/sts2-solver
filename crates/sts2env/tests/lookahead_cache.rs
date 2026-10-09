use std::sync::atomic::Ordering;
use sts2sim::engine::{ActionBuf, LOOK_VERIFIED, LOOK_VERIFY};
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

fn states(path: &str, n: usize, per: usize, seed: u64) -> Vec<Combat> {
    let Ok(txt) = std::fs::read_to_string(path) else { return Vec::new() };
    let v: serde_json::Value = serde_json::from_str(&txt).unwrap();
    let mut rng = Rng::new(seed);
    let mut out = Vec::new();
    for (i, sj) in v.as_array().unwrap().iter().enumerate().take(n) {
        let sj = sj.get("scenario").filter(|_| sj.get("deck").is_none()).unwrap_or(sj);
        let Ok((sc, ex)) = sts2diff::convert::scenario_ex(sj) else { continue };
        if sc.validate().is_err() {
            continue;
        }
        let mut cx = Combat::try_new_with(&sc, &ex).unwrap();
        cx.reset_validated(&sc, &ex, i as u64 + 7, RngSet::from_run_seed_fast(i as u64 + 7)).unwrap();
        for step in 0..60 * per {
            let mut buf = ActionBuf::new();
            cx.legal_actions(&mut buf);
            if buf.is_empty() {
                break;
            }
            if step % 60 == 11 {
                out.push(cx.clone());
            }
            if !cx.step(buf[rng.next_int(buf.len() as i32) as usize]) {
                break;
            }
        }
    }
    out
}

#[test]
fn relaxed_key_cache_is_exact() {
    relaxed_key_hits_equal_fresh_under_other_hp_and_block();
    search_with_every_relaxed_hit_verified();
}

fn relaxed_key_hits_equal_fresh_under_other_hp_and_block() {
    let mut rng = Rng::new(17);
    let (mut checked, mut diffs) = (0u64, 0u64);
    LOOK_VERIFY.store(true, Ordering::Relaxed);
    let v0 = LOOK_VERIFIED.load(Ordering::Relaxed);
    for path in ["/../../data/bench/mix.json", "/../../data/bench/tail.json", "/../../data/train/eval.json"] {
        for cx in states(&(env!("CARGO_MANIFEST_DIR").to_string() + path), 300, 3, 5) {
            for &e in cx.enemies.clone().iter() {
                let _ = cx.lookahead(e);
            }
            for _ in 0..3 {
                let mut p = cx.clone();
                for k in 1..p.creatures.len() {
                    let cr = &mut p.creatures[k];
                    if cr.active && !cr.is_player && cr.hp > 0 {
                        cr.hp = 1 + rng.next_int(cr.max_hp.max(1));
                        cr.block = if rng.next_int(2) == 0 { 0 } else { rng.next_int(40) };
                    }
                }
                for &e in p.enemies.clone().iter() {
                    let (a, b) = (p.lookahead(e), p.lookahead_fresh(e));
                    for h in 0..a.len() {
                        checked += 1;
                        if a[h].prob != b[h].prob || a[h].exp_damage.to_bits() != b[h].exp_damage.to_bits() {
                            diffs += 1;
                        }
                    }
                }
            }
        }
    }
    LOOK_VERIFY.store(false, Ordering::Relaxed);
    assert!(checked > 5_000, "only {checked} rows checked");
    assert_eq!(diffs, 0, "{diffs} of {checked} lookahead rows differ");
    let n = LOOK_VERIFIED.load(Ordering::Relaxed) - v0;
    assert!(n > 1_000, "only {n} relaxed hits");
}

fn search_with_every_relaxed_hit_verified() {
    use sts2env::search::*;
    use sts2sim::engine::ACTION_SPACE;
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/bench/mix.json");
    let Ok(txt) = std::fs::read_to_string(path) else { return };
    let v: serde_json::Value = serde_json::from_str(&txt).unwrap();
    let scen: Vec<_> = v.as_array().unwrap().iter().filter_map(|x| sts2diff::convert::scenario_ex(x.get("scenario").unwrap_or(x)).ok()).filter(|(sc, _)| sc.validate().is_ok()).take(24).collect();
    let jobs: Vec<(u32, u64)> = (0..scen.len()).map(|i| (i as u32, 31 + i as u64)).collect();
    let cfg = SearchCfg { m: 5, k: 8, roll_cap: 120, leaf_turns: 2, turn_cap: 30, max_steps: 60, ..SearchCfg::default() };
    LOOK_VERIFY.store(true, Ordering::Relaxed);
    let v0 = LOOK_VERIFIED.load(Ordering::Relaxed);
    let mut eng = SearchEngine::new(scen, jobs, 12, cfg, 3, false).unwrap();
    let cap = eng.shared_rows();
    let (mut obs, mut mask, mut pk, mut pu, mut vk) = (vec![0f32; cap * sts2sim::observe::OBS_SIZE], vec![0u8; cap * ACTION_SPACE], vec![0u8; cap], vec![0f32; cap], vec![0u8; cap]);
    let stride = 2 * cfg.m + 1;
    let (mut pol, mut val) = (vec![0f32; cap * stride], vec![0f32; cap]);
    let (mut np, mut nv) = eng.advance_shared(None, None, &mut obs, &mut mask, &mut pk, &mut pu, &mut vk).unwrap();
    while np + nv > 0 {
        for r in 0..np {
            let legal: Vec<usize> = (0..ACTION_SPACE).filter(|&a| mask[r * ACTION_SPACE + a] > 0).collect();
            let o = &mut pol[r * stride..(r + 1) * stride];
            for j in 0..cfg.m {
                o[j] = *legal.get(j).unwrap_or(&0) as f32;
                o[cfg.m + j] = if j < legal.len() { 1.0 / cfg.m.min(legal.len()) as f32 } else { 0.0 };
            }
            o[2 * cfg.m] = legal[((pu[r] * legal.len() as f32) as usize).min(legal.len() - 1)] as f32;
        }
        for r in 0..nv {
            val[r] = 0.1;
        }
        (np, nv) = eng.advance_shared(Some(&pol[..np * stride]), Some(&val[..nv]), &mut obs, &mut mask, &mut pk, &mut pu, &mut vk).unwrap();
    }
    LOOK_VERIFY.store(false, Ordering::Relaxed);
    assert!(eng.results().iter().all(|r| r.done));
    let n = LOOK_VERIFIED.load(Ordering::Relaxed) - v0;
    assert!(n > 1_000, "only {n} relaxed hits verified");
}
